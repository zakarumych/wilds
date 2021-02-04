mod config;
mod pawn;
mod player;
mod sun;
mod terrain;

use {
    bumpalo::Bump,
    color_eyre::Report,
    std::{cmp::max, time::Duration},
    tracing_subscriber::layer::SubscriberExt as _,
    wilds::{
        camera::{
            following::FollowingCameraSystem,
            free::{FreeCamera, FreeCameraSystem},
        },
        clocks::Clocks,
        engine::Engine,
        fps_counter::FpsCounter,
        physics::Physics,
        renderer::{Extent2d, RenderConstants, Renderer},
        scene::{Global3, SceneSystem},
    },
    winit::{
        dpi::PhysicalSize,
        event::{
            DeviceEvent, ElementState, Event, KeyboardInput, VirtualKeyCode,
            WindowEvent,
        },
        window::WindowBuilder,
    },
};

const WINDOW_EXTENT: Extent2d = Extent2d {
    width: 1280,
    height: 720,
};

fn main() -> Result<(), Report> {
    color_eyre::install()?;

    tracing::subscriber::set_global_default(
        tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .pretty()
            .finish()
            .with(tracing_error::ErrorLayer::default()),
    )?;

    tracing::info!("App started");

    let config_file = std::fs::File::open("cfg.ron")?;
    let config::Config { engine, game } = ron::de::from_reader(config_file)?;

    Engine::run(engine, move |mut engine| async move {
        engine.add_system(Physics::new());
        engine.add_system(SceneSystem);

        let window = engine.build_window(
            WindowBuilder::new().with_inner_size(PhysicalSize {
                width: WINDOW_EXTENT.width,
                height: WINDOW_EXTENT.height,
            }),
        )?;

        let aspect = WINDOW_EXTENT.aspect_ratio();

        let mut bump = Bump::with_capacity(1024 * 1024);
        let mut renderer = Renderer::new(&window)?;
        let mut clocks = Clocks::new();

        sun::spawn_sun(&mut engine);
        terrain::spawn_terrain(&mut engine);

        let camera = engine.world.spawn((
            game.camera.into_camera(aspect),
            // Camera::Matrix(na::Projective3::identity()),
            Global3::identity(),
            // FollowingCamera { follows: pawn },
            FreeCamera,
        ));

        engine.add_system(
            FollowingCameraSystem::new()
                .with_factor(0.01, 0.01 * aspect)
                .with_speed(50.0),
        );

        engine.add_system(
            FreeCameraSystem::new()
                .with_factor(0.003, 0.003)
                .with_speed(3.0),
        );

        window.request_redraw();

        let mut fps_counter = FpsCounter::new(Duration::from_secs(5));
        let mut ticker = Duration::from_secs(0);

        loop {
            // Main game loop
            match engine.next().await {
                Event::WindowEvent {
                    window_id,
                    event: WindowEvent::CloseRequested,
                } if window_id == window.id() => {
                    break;
                }
                Event::WindowEvent {
                    window_id,
                    event: WindowEvent::Resized(size),
                } if window_id == window.id() => {
                    let aspect = size.width as f32 / size.height as f32;
                    *engine.world.get_mut(camera).unwrap() =
                        game.camera.into_camera(aspect);
                }
                Event::MainEventsCleared => {
                    engine.advance(&bump);
                    window.request_redraw();
                }
                Event::RedrawRequested(_) => {
                    let clock = clocks.step();
                    fps_counter.add_sample(clock.delta);

                    if ticker < clock.delta {
                        ticker += max(Duration::from_secs(1), clock.delta);

                        tracing::info!(
                            "FPS: {}",
                            1.0 / fps_counter.average().as_secs_f32()
                        );
                    }
                    ticker -= clock.delta;

                    tracing::trace!("Request redraw");
                    renderer.draw(
                        &mut engine.world,
                        &mut engine.resources,
                        &clock,
                        &bump,
                    )?;
                }
                Event::DeviceEvent {
                    event:
                        DeviceEvent::Key(KeyboardInput {
                            virtual_keycode: Some(VirtualKeyCode::F),
                            state: ElementState::Released,
                            ..
                        }),
                    ..
                } => {
                    let filter_enabled = &mut engine
                        .resources
                        .get_or_else(RenderConstants::new)
                        .filter_enabled;

                    *filter_enabled = !*filter_enabled;
                }
                _ => {}
            }

            bump.reset();
            engine.assets.process(&mut *renderer);
        }

        Ok(())
    })
}
