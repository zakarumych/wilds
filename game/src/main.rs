// mod pawn;
mod player;

use {
    bumpalo::Bump,
    color_eyre::Report,
    hecs::{Entity, EntityBuilder, World},
    nalgebra as na,
    std::{alloc::System, cmp::max, time::Duration},
    tracing_subscriber::layer::SubscriberExt as _,
    wilds::{
        animate::Pose,
        assets::{
            GltfAsset, GltfFormat, Prefab, RonFormat, TerrainAsset,
            TerrainFormat,
        },
        camera::{
            following::{FollowingCamera, FollowingCameraSystem},
            free::{FreeCamera, FreeCameraSystem},
            Camera,
        },
        clocks::Clocks,
        engine::{Engine, SystemContext},
        fps_counter::FpsCounter,
        light::{DirectionalLight, PointLight, SkyLight},
        physics::{Constants, Physics},
        renderer::{
            BufferUsage, Extent2d, IndexType, Material, Mesh, Normal3d,
            PoseMesh, Position3d, PositionNormalTangent3dUV, RenderConstants,
            Renderable, Renderer, Skin, Tangent3d, VertexType as _, UV,
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

    Engine::run(|mut engine| async move {
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

        let sunlight = (na::Vector3::new(255.0, 207.0, 72.0) / 255.0)
            .map(|c| c / (1.1 - c));

        let skyradiance = (na::Vector3::new(117.0, 187.0, 253.0) / 255.0)
            .map(|c| c / (1.2 - c));

        engine.world.spawn((
            DirectionalLight {
                direction: na::Vector3::new(-30.0, -25.0, -5.0),
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
                    (skyradiance * (1.1 - d.cos()) / 2.1).into();
            }
        });

        // engine.world.spawn((
        //     PointLight {
        //         radiance: [10.0, 10.0, 10.0],
        //     },
        //     na::Isometry3 {
        //         translation: na::Translation3::new(0.0, 0.0, 0.0),
        //         rotation: na::UnitQuaternion::identity(),
        //     },
        // ));

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
        //                 * (ctx.clocks.step - ctx.clocks.start)
        //                     .as_secs_f32()
        //                     .cos();
        //     }
        // });

        // let scene = engine.load_prefab_with_format::<GltfAsset, _>(
        //     "sponza/glTF/Sponza.gltf".into(),
        //     Global3::from_scale(1.0),
        //     GltfFormat::for_raytracing(),
        // );

        let _terrain = engine.load_prefab_with_format(
            "terrain/island.ron".into(),
            Global3::from_scale(1.0),
            TerrainFormat {
                raster: false,
                blas: true,
            },
        );

        // let pawn = engine.load_prefab_with_format::<PawnAsset, _>(
        //     "pawn.ron".into(),
        //     na::Isometry3::translation(0.0, 5.0, 0.0),
        //     RonFormat,
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

        let cube_mesh = Mesh::from_generator(
            &genmesh::generators::Cube::new(),
            BufferUsage::DEVICE_ADDRESS
                | BufferUsage::ACCELERATION_STRUCTURE_BUILD_INPUT
                | BufferUsage::STORAGE,
            &mut renderer,
            IndexType::U32,
            |vertex| PositionNormalTangent3dUV {
                position: Position3d(vertex.pos.into()),
                normal: Normal3d(vertex.normal.into()),
                tangent: Tangent3d([1.0; 4]),
                uv: UV([0.0; 2]),
            },
        )?;

        // let mut entity = engine.world.spawn((
        //     Renderable {
        //         mesh: cube_mesh.clone(),
        //         material: Material::color([0.7, 0.5, 0.3, 1.0]),
        //     },
        //     Global3::from_scale(0.1),
        // ));

        // for i in 1..10 {
        //     entity = engine.world.spawn((
        //         Renderable {
        //             mesh: cube_mesh.clone(),
        //             material: Material::color([0.7, 0.5, 0.3, 1.0]),
        //         },
        //         Global3::identity(),
        //         Local3::from_translation(
        //             entity,
        //             na::Translation3::new(0.0, 3.0, 0.0),
        //         ),
        //     ));
        // }

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
