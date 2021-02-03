use {
    crate::{
        assets::{AssetKey, Assets, Prefab},
        broker::EventBroker,
        clocks::{ClockIndex, Clocks},
        config::{AssetSource, Config},
    },
    bumpalo::Bump,
    cfg_if::cfg_if,
    eyre::Report,
    flume::{bounded, Receiver, Sender},
    futures::future::TryFutureExt as _,
    goods::{Asset, AssetDefaultFormat},
    hecs::{Entity, World},
    std::{
        cell::Cell,
        future::Future,
        pin::Pin,
        rc::Rc,
        task::{Context, Poll},
        time::Duration,
    },
    type_map::TypeMap,
    winit::{
        event::Event,
        event_loop::{ControlFlow, EventLoop, EventLoopWindowTarget},
        window::{Window, WindowBuilder},
    },
};

pub use winit::event::{
    AxisId, DeviceEvent, DeviceId, ElementState, KeyboardInput, ModifiersState,
    MouseButton, MouseScrollDelta, Touch, TouchPhase,
};

pub type InputEvents = EventBroker<Event<'static, ()>>;

pub struct SystemContext<'a> {
    pub input: &'a mut InputEvents,
    pub world: &'a mut World,
    pub resources: &'a mut TypeMap,
    pub bump: &'a Bump,
    pub clocks: ClockIndex,
}

pub trait System {
    fn run(&mut self, ctx: SystemContext<'_>);
}

impl<F> System for F
where
    F: FnMut(SystemContext<'_>),
{
    fn run(&mut self, ctx: SystemContext<'_>) {
        self(ctx)
    }
}

/// Root data structure for the game engine.
pub struct Engine {
    pub world: World,
    pub resources: TypeMap,
    pub assets: Assets,
    pub input: InputEvents,
    schedule: Vec<Box<dyn System>>,
    fixed_schedule: Vec<Box<dyn System>>,
    shared: Rc<Shared>,
    recv_make_prefabs: Receiver<MakePrefab>,
    send_make_prefabs: Sender<MakePrefab>,
    clocks: Clocks,
    fixed_step_delta: Duration,
}

impl Engine {
    /// Loads asset and enqueue it for spawning.
    /// Retuns `Entity` that will be supplied to `spawn` method after asset is
    /// loaded.
    /// If asset loading fails that `Entity` will be despawned.
    pub fn load_prefab<P>(&self, key: AssetKey, info: P::Info) -> Entity
    where
        P: Prefab + AssetDefaultFormat<AssetKey> + Clone,
    {
        self.load_prefab_with_format(key, info, P::DefaultFormat::default())
    }

    /// Loads asset and enqueue it for spawning.
    /// Retuns `Entity` that will be supplied to `spawn` method after asset is
    /// loaded.
    /// If asset loading fails that `Entity` will be despawned.
    pub fn load_prefab_with_format<P, F>(
        &self,
        key: AssetKey,
        info: P::Info,
        format: F,
    ) -> Entity
    where
        P: Prefab + Asset + Clone,
        F: goods::Format<P, AssetKey>,
    {
        tracing::info!("Loading prefab '{}'", key);

        let handle = self.assets.load_with_format(key.clone(), format);
        self.make_prefab(key, info, handle.map_err(Report::from))
    }

    pub fn make_prefab<P, F>(
        &self,
        key: AssetKey,
        info: P::Info,
        prefab: F,
    ) -> Entity
    where
        P: Prefab + Send + 'static,
        F: Future<Output = Result<P, Report>> + Send + 'static,
    {
        let entity = self.world.reserve_entity();
        let send_make_prefabs = self.send_make_prefabs.clone();

        smol::spawn(async move {
            let prefab = prefab.await;

            tracing::error!("Prefab loaded");

            let loaded = match prefab {
                Ok(prefab) => MakePrefab::spawn(key, prefab, info, entity),
                Err(err) => MakePrefab::Error(key, err, entity),
            };
            let _ = send_make_prefabs.send(loaded);
        })
        .detach();

        entity
    }

    fn build_prefabs(&mut self) {
        for loaded in self.recv_make_prefabs.try_iter() {
            match loaded {
                MakePrefab::Spawn(key, build) => {
                    tracing::info!("Prefab '{}' loaded", key);
                    build(&mut self.world, &mut self.resources);
                }
                MakePrefab::Error(key, err, entity) => {
                    tracing::error!("Failed to load prefab '{}': {}", key, err);
                    let _ = self.world.despawn(entity);
                }
            }
        }
    }

    pub fn build_window(
        &mut self,
        builder: WindowBuilder,
    ) -> Result<Window, Report> {
        let elwt = self.shared.event_loop_ptr.get();
        if elwt.is_null() {
            unreachable!()
        }

        let elwt = unsafe {
            // This block can be executed only within winit's event loop
            // callback. when closure provided to `Engine::run` is
            // polled, or on initial call to that closure.
            // Because it takes `&mut self` and `Engine is not `Send` it cannot
            // be sent to another thread.
            // This function is not async so it is not possible that reference
            // we creating here will be preserved over yielding.
            &*elwt
        };

        let window = builder.build(elwt)?;
        Ok(window)
    }

    pub fn advance(&mut self, bump: &Bump) {
        self.build_prefabs();

        let clocks = self.clocks.step();

        for system in &mut self.schedule {
            system.run(SystemContext {
                world: &mut self.world,
                resources: &mut self.resources,
                input: &mut self.input,
                clocks,
                bump,
            });
        }

        for clocks in self.clocks.fixed_steps(self.fixed_step_delta) {
            for system in &mut self.fixed_schedule {
                system.run(SystemContext {
                    world: &mut self.world,
                    resources: &mut self.resources,
                    input: &mut self.input,
                    clocks,
                    bump,
                });
            }
        }

        self.input.clear();
    }

    /// Adds a system to this engine.
    pub fn add_system<S>(&mut self, system: S) -> &mut Self
    where
        S: System + 'static,
    {
        self.schedule.push(Box::new(system));
        self
    }

    /// Adds a system to this engine.
    pub fn add_fixed_step_system<S>(&mut self, system: S) -> &mut Self
    where
        S: System + 'static,
    {
        self.fixed_schedule.push(Box::new(system));
        self
    }

    /// Asynchronously wait for next event.
    pub async fn next(&mut self) -> Event<'static, ()> {
        self.shared.waiting_for_event.set(true);
        // let event = self.events.recv_async().await;

        let event = loop {
            if let Some(event) = self.shared.next_event.take() {
                break event;
            }
            smol::future::yield_now().await;
        };

        self.input.add(event.clone());
        self.shared.waiting_for_event.set(false);
        event
    }

    /// Runs an instance of an engine.
    /// This function neven returns on success.
    /// Instead it calls provided closure with create engine instance
    /// and drive it to completion.
    /// Along with polling winit's event-loop for window events.
    pub fn run<F, A>(closure: F) -> Result<(), Report>
    where
        F: FnOnce(Self) -> A,
        A: Future<Output = Result<(), Report>> + 'static,
    {
        let config = smol::block_on(Self::load_config())?;

        let registry = config
            .sources
            .iter()
            .fold(goods::RegistryBuilder::<AssetKey>::new(), |builder, source| match source {
                AssetSource::FileSystem { path } => {
                    cfg_if! {
                        if #[cfg(target_arch = "wasm32")] {
                            tracing::error!("FileSystem asset source with path '{}' ignored on WASM target", path.display());
                            Ok(builder)
                        } else {
                            let path = match std::env::current_dir() {
                                Ok(cd) => { cd.join(path) }
                                Err(err) => {
                                    tracing::error!("Failed to fetch current dir: {}", err);
                                    path.clone()
                                }
                            };
                            builder.with(goods::FileSource::new(path))
                        }
                    }
                }
            });

        let registry = registry.with(goods::DataUrlSource);

        let assets = Assets::new(registry.build(), goods::Smol);

        let shared = Rc::new(Shared {
            event_loop_ptr: Cell::new(std::ptr::null()),
            next_event: Cell::new(None),
            waiting_for_event: Cell::new(false),
        });

        let (send_make_prefabs, recv_make_prefabs) = bounded(512);

        let engine = Engine {
            assets,
            schedule: Vec::new(),
            fixed_schedule: Vec::new(),
            world: World::new(),
            resources: TypeMap::new(),
            input: EventBroker::new(),
            shared: shared.clone(),
            recv_make_prefabs,
            send_make_prefabs,
            fixed_step_delta: Duration::from_millis(10),
            clocks: Clocks::new(),
        };

        let event_loop = EventLoop::new();

        shared.event_loop_ptr.set(&*event_loop);
        let mut app_opt = Some(Box::pin(closure(engine)));
        shared.event_loop_ptr.set(std::ptr::null());

        // Here goes magic
        event_loop.run(move |event, el, flow| {
            tracing::trace!("Event {:#?}", event);

            if let Some(app) = &mut app_opt {
                // Set event. Excluding an event bound to a lifetime.
                let old = match event.to_static() {
                    Some(event) => {
                        shared.next_event.replace(Some(event))
                    }
                    None => {
                        shared.next_event.take()
                    }
                };

                assert!(old.is_none(), "Control flow must not return to event loop until event is consumed by the application");

                // Set pointer. We ensure it is always valid while
                // non-null.
                shared.event_loop_ptr.set(el);

                // Poll closure only once.
                if let Poll::Ready(result) =
                    smol::block_on(AppEventWaitFuture {
                        app: app.as_mut(),
                        ready: &shared.waiting_for_event,
                    })
                {
                    // No place where we could return this error.
                    // log and panic are only options.
                    if let Err(err) = result {
                        tracing::error!("Error: {}", err);
                    }

                    // Exit when closure resolves.
                    *flow = ControlFlow::Exit;
                    app_opt = None;
                } else {
                    // *flow = ControlFlow::Wait;
                    *flow = ControlFlow::Poll;
                }

                // Unset event loop before it is invalidated.
                shared.event_loop_ptr.set(std::ptr::null());
            } else {
                *flow = ControlFlow::Exit;
            }
        })
    }

    async fn load_config() -> Result<Config, Report> {
        tracing::info!("Running at {}", std::env::current_dir()?.display());

        // Now load config.
        let config = Config::load_default().await?;
        tracing::info!("Config loaded: {:?}", config);
        Ok(config)
    }
}

/// Future that polls main application future
/// until it finishes or awaits in `Engine::next` function.
#[must_use = "futures/streams/sinks do nothing unless you `.await` or poll them"]
struct AppEventWaitFuture<'a, A> {
    app: Pin<&'a mut A>,
    ready: &'a Cell<bool>,
}

impl<'a, A> Future for AppEventWaitFuture<'a, A>
where
    A: Future<Output = Result<(), Report>>,
{
    type Output = Poll<Result<(), Report>>;

    fn poll(
        self: Pin<&mut Self>,
        ctx: &mut Context,
    ) -> Poll<Poll<Result<(), Report>>> {
        let this = self.get_mut();
        let poll = Future::poll(this.app.as_mut(), ctx);
        match poll {
            Poll::Ready(result) => Poll::Ready(Poll::Ready(result)),
            Poll::Pending if this.ready.get() => Poll::Ready(Poll::Pending),
            Poll::Pending => Poll::Pending,
        }
    }
}

struct Shared {
    event_loop_ptr: Cell<*const EventLoopWindowTarget<()>>,
    next_event: Cell<Option<Event<'static, ()>>>,
    waiting_for_event: Cell<bool>,
}

enum MakePrefab {
    Spawn(AssetKey, Box<dyn FnOnce(&mut World, &mut TypeMap) + Send>),
    Error(AssetKey, Report, Entity),
}

impl MakePrefab {
    fn spawn<P>(key: AssetKey, prefab: P, info: P::Info, entity: Entity) -> Self
    where
        P: Prefab + Send + 'static,
    {
        MakePrefab::Spawn(
            key,
            Box::new(move |world, resources| {
                prefab.spawn(info, world, resources, entity)
            }),
        )
    }
}
