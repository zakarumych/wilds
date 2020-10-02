mod pawn;
mod player;

use {
    self::pawn::*,
    bumpalo::Bump,
    color_eyre::Report,
    goods::RonFormat,
    hecs::{Entity, EntityBuilder, World},
    nalgebra as na,
    std::{alloc::System, cmp::max, time::Duration},
    wilds::{
        animate::Pose,
        assets::{GltfAsset, GltfFormat, Prefab, TerrainAsset, TerrainFormat},
        camera::{
            following::{FollowingCamera, FollowingCameraSystem},
            free::{FreeCamera, FreeCameraSystem},
            Camera,
        },
        clocks::Clocks,
        engine::{Engine, SystemContext},
        fps_counter::FpsCounter,
        light::{DirectionalLight, PointLight, SkyLight},
        physics::Physics,
        renderer::{
            PoseMesh, RenderConstants, Renderable, Renderer, Skin,
            VertexType as _,
        },
        scene::{Global3, Local3, SceneSystem},
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

fn main() -> Result<(), Report> {
    tracing_subscriber::fmt::init();
    tracing::info!("App started");

    Engine::run(|mut engine| async move {
        engine
            .resources
            .insert(wilds::physics::Constants { time_factor: 0.1 });

        // engine.add_system(Physics::new());
        engine.add_system(SceneSystem);

        let window = engine.build_window(
            WindowBuilder::new().with_inner_size(PhysicalSize {
                width: 640,
                height: 480,
            }),
        )?;

        let aspect = 640.0 / 480.0;

        let mut bump = Bump::with_capacity(1024 * 1024);
        let mut renderer = Renderer::new(&window)?;
        let mut clocks = Clocks::new();

        let sunlight = na::Vector3::new(255.0, 207.0, 72.0)
            .map(|c| c / 255.0)
            .map(|c| c / (1.3 - c));
        // .map(|c| c * 5.0);

        let skyradiance = na::Vector3::new(117.0, 187.0, 253.0)
            .map(|c| c / 255.0)
            .map(|c| c / (1.3 - c));
        // .map(|c| c * 5.0);

        engine.world.spawn((
            DirectionalLight {
                direction: na::Vector3::new(-30.0, -30.0, -30.0),
                radiance: sunlight.into(),
            },
            SkyLight {
                radiance: skyradiance.into(),
            },
        ));

        engine.add_system(move |ctx: SystemContext<'_>| {
            let elapsed = ctx.clocks.step - ctx.clocks.start;
            let d = elapsed.as_secs_f32() / 10.0;
            let mut query = ctx.world.query::<&mut DirectionalLight>();

            for (_, dirlight) in query.iter() {
                dirlight.direction = na::Vector3::new(
                    d.sin() * 30.0,
                    d.cos() * 25.0,
                    d.cos() * 5.0,
                );
            }

            let mut query = ctx.world.query::<&mut SkyLight>();

            for (_, skylight) in query.iter() {
                skylight.radiance =
                    (skyradiance * (1.2 - d.cos()) / 2.2).into();
            }
        });

        engine.world.spawn((
            PointLight {
                radiance: [10.0, 10.0, 10.0],
            },
            na::Isometry3 {
                translation: na::Translation3::new(0.0, 0.0, 0.0),
                rotation: na::UnitQuaternion::identity(),
            },
        ));

        // engine.add_system(|ctx: wilds::engine::SystemContext<'_>| {
        //     let mut query = ctx
        //         .world
        //         .query::<&mut na::Isometry3<f32>>()
        //         .with::<PointLight>();

        //     for (_, iso) in query.iter() {
        //         iso.translation.x =
        //             (ctx.clocks.step - ctx.clocks.start).as_secs_f32().sin();
        //         iso.translation.y = 5.0
        //             + 3.0
        //                 * (ctx.clocks.step - ctx.clocks.start) .as_secs_f32()
        //                   .cos();
        //     }
        // });

        let scene = engine.load_prefab_with_format::<GltfAsset, _>(
            "sponza2/sponza2.gltf".into(),
            Global3::from_scale(1.0),
            GltfFormat::for_raytracing(),
        );

        // let _terrain = TerrainAsset::load(
        //     &engine,
        //     "terrain/0001.png".into(),
        //     TerrainFormat {
        //         raster: false,
        //         blas: true,
        //         factor: 3.0,
        //     },
        //     na::Isometry3::identity(),
        // );

        // let pawn = PawnAsset::load(
        //     &engine,
        //     "pawn.ron".into(),
        //     RonFormat,
        //     na::Isometry3::translation(0.0, 5.0, 0.0),
        // );

        // let pawn2 = PawnAsset::load(
        //     &engine,
        //     "pawn.ron".into(),
        //     RonFormat,
        //     na::Isometry3::translation(1.0, 10.0, 1.0),
        // );

        // engine.add_system(player::Player::new(&window, pawn));

        engine.world.spawn((
            Camera::Perspective(na::Perspective3::new(
                aspect,
                std::f32::consts::PI / 3.0,
                0.1,
                1000.0,
            )),
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

        // engine.add_system(|context: SystemContext<'_>| {
        //     for (_, pose) in context.world.query::<&mut Pose>().iter() {
        //         if let [_, mid, ..] = &mut *pose.matrices {
        //             *mid = na::UnitQuaternion::from_euler_angles(
        //                 1.0 * context.clocks.delta.as_secs_f32(),
        //                 1.0 * context.clocks.delta.as_secs_f32(),
        //                 1.0 * context.clocks.delta.as_secs_f32(),
        //             )
        //             .into_matrix()
        //             .into_homogeneous()
        //                 * *mid;
        //         }
        //     }
        // });

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
                Event::MainEventsCleared => {
                    engine.advance(&bump);
                    window.request_redraw();

                    // tracing::info!("Advance:\n{:#?}",
                    // reg.change_and_reset());
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

                        // let stats = reg.change_and_reset();
                        // tracing::info!(
                        //     "Alloc {} ({} - {})",
                        //     stats.bytes_allocated as isize
                        //         - stats.bytes_deallocated as isize,
                        //     stats.bytes_allocated,
                        //     stats.bytes_deallocated
                        // );
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
                        .entry::<RenderConstants>()
                        .or_insert_with(RenderConstants::new)
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
