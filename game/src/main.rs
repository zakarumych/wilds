mod config;
mod construct;
mod pawn;
mod player;
mod sky;
mod terrain;

use {
    bumpalo::Bump,
    color_eyre::Report,
    goods_ron::RonFormat,
    nalgebra as na,
    std::{
        f32::consts::{PI, TAU},
        time::Duration,
    },
    tracing_subscriber::layer::SubscriberExt as _,
    wilds::{
        assets::{Gltf, GltfFormat},
        camera::{
            following::{FollowingCamera, FollowingCameraSystem},
            free::{FreeCamera, FreeCameraSystem},
        },
        clocks::Clocks,
        engine::Engine,
        fps_counter::FpsCounter,
        physics::{
            dynamics::RigidBodyBuilder,
            geometry::{ColliderBuilder, Cone, SharedShape},
            Physics, PhysicsData,
        },
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

        terrain::spawn_terrain(&mut engine);

        engine.add_system(sky::SkySystem {
            angle: 0.0,
            velocity: 0.0333,
        });

        engine.load_prefab_with_format::<sky::Sky, _>(
            "mars.sky.ron".into(),
            RonFormat,
        );

        // engine.add_system(
        //     FollowingCameraSystem::new()
        //         .with_factor(0.01, 0.01 * aspect)
        //         .with_speed(50.0),
        // );

        engine.add_system(
            FreeCameraSystem::new()
                .with_factor(0.003, 0.003)
                .with_speed(3.0),
        );

        engine.add_system(pawn::PawnSystem);

        window.request_redraw();

        let mut fps_counter = FpsCounter::new(Duration::from_secs(5));
        let mut ticker = Duration::from_secs(0);

        engine
            .resources
            .get_or_default::<wilds::physics::Constants>()
            .gravity
            .y = -3.6848;

        // spawn_farms(&mut engine);
        // spawn_pawns(&mut engine);

        let camera = engine.world.spawn((
            game.camera.into_camera(aspect),
            // Camera::Matrix(na::Projective3::identity()),
            Global3::from_iso(na::Isometry {
                translation: na::Vector3::new(0.0, 0.0, 0.0).into(),
                rotation: na::UnitQuaternion::identity(),
            }),
            // FollowingCamera { follows: pawn },
            FreeCamera,
        ));

        loop {
            // Main game loop

            let event = engine.next().await;

            match event {
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
                        ticker += Duration::from_secs(1).max(clock.delta);

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
                            virtual_keycode: Some(key),
                            state: ElementState::Released,
                            ..
                        }),
                    ..
                } => match key {
                    VirtualKeyCode::F => {
                        let filter_enabled = &mut engine
                            .resources
                            .get_or_else(RenderConstants::new)
                            .filter_enabled;

                        *filter_enabled = !*filter_enabled;
                    }
                    VirtualKeyCode::P => {
                        spawn_pawns(&mut engine);
                    }
                    _ => {}
                },
                _ => {}
            }

            bump.reset();
            engine.assets.process(&mut *renderer);
        }

        Ok(())
    })
}

fn spawn_farms(engine: &mut Engine) {
    let collider = ColliderBuilder::cylinder(4.0, 5.0)
        .rotation(na::Vector3::new(PI, 0.0, 0.0))
        .translation(0.0, 0.0, 0.0)
        .build();

    for i in 0..1 {
        for j in 0..1 {
            let x = i as f32 * (15.0 + rand::random::<f32>() * 3.0);
            let y = 2.5;
            let z = j as f32 * (15.0 + rand::random::<f32>() * 3.0);

            let farm = engine.load_prefab_with_format::<Gltf, _>(
                "constructs/farm-dome.gltf".into(),
                GltfFormat::for_raytracing(),
            );

            let sets = engine.resources.get_or_else(PhysicsData::new);
            let body = sets.bodies.insert(
                RigidBodyBuilder::new_static().translation(x, y, z).build(),
            );
            let collider =
                sets.colliders
                    .insert(collider.clone(), body, &mut sets.bodies);
            engine
                .world
                .insert(
                    farm,
                    (
                        Global3::from_iso(na::Isometry3 {
                            rotation:
                                na::geometry::UnitQuaternion::from_euler_angles(
                                    0.0,
                                    rand::random::<f32>() * TAU,
                                    0.0,
                                ),
                            translation: na::Vector3::new(x, y, z).into(),
                        }),
                        body,
                        collider,
                    ),
                )
                .unwrap();
        }
    }
}

fn spawn_pawns(engine: &mut Engine) {
    for i in 0..10 {
        for j in 0..10 {
            let x = (i as f32) - 4.5;
            let y = 5.0;
            let z = (j as f32) - 4.5;

            let pawn = engine.load_prefab_with_format::<pawn::Pawn, _>(
                "pawns/simple.ron".into(),
                goods_ron::RonFormat,
            );

            engine
                .world
                .insert(
                    pawn,
                    (Global3::from_iso(na::Isometry3 {
                        rotation: na::geometry::UnitQuaternion::identity(),
                        translation: na::Vector3::new(x, y, z).into(),
                    }),),
                )
                .unwrap();
        }
    }
}
