mod pawn;
mod player;

use {
    self::pawn::*,
    bumpalo::Bump,
    color_eyre::Report,
    goods::RonFormat,
    hecs::World,
    std::{alloc::System, cmp::max, time::Duration},
    ultraviolet::{Isometry3, Mat4, Rotor3, Vec3},
    wilds::{
        alloc::Region,
        assets::{
            Gltf, GltfFormat, GltfNode, Prefab, TerrainAsset, TerrainFormat,
        },
        camera::{
            following::{FollowingCamera, FollowingCameraSystem},
            free::{FreeCamera, FreeCameraSystem},
            Camera,
        },
        clocks::Clocks,
        engine::Engine,
        fps_counter::FpsCounter,
        light::{DirectionalLight, PointLight, SkyLight},
        physics::Physics,
        renderer::{RenderConstants, Renderable, Renderer},
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

        let sunlight = Vec3::new(255.0, 207.0, 72.0)
            .map(|c| c / 255.0)
            .map(|c| c / (1.3 - c))
            .map(|c| c * 5.0);
        let skylight = Vec3::new(117.0, 187.0, 253.0)
            .map(|c| c / 255.0)
            .map(|c| c / (1.3 - c))
            .map(|c| c * 5.0);

        engine.world.spawn((
            DirectionalLight {
                direction: Vec3::new(-30.0, -30.0, -30.0),
                radiance: sunlight.into(),
            },
            SkyLight {
                radiance: skylight.into(),
            },
        ));

        engine.world.spawn((
            PointLight {
                radiance: [10.0, 10.0, 10.0],
            },
            Isometry3::new(Vec3::new(0.0, 0.0, 0.0), Rotor3::identity()),
        ));

        engine.add_system(|ctx: wilds::engine::SystemContext<'_>| {
            let mut query =
                ctx.world.query::<&mut Isometry3>().with::<PointLight>();

            for (_, iso) in query.iter() {
                iso.translation.x =
                    (ctx.clocks.step - ctx.clocks.start).as_secs_f32().sin();
                iso.translation.y = 5.0
                    + 3.0
                        * (ctx.clocks.step - ctx.clocks.start)
                            .as_secs_f32()
                            .cos();
            }
        });

        let mut gltf_opt = Some(engine.assets.load_with_format(
            "sponza/Sponza.gltf".to_owned(),
            GltfFormat {
                raster: false,
                blas: true,
            },
        ));

        // let _terrain = TerrainAsset::load(
        //     &engine,
        //     "terrain/0001.png".to_owned(),
        //     TerrainFormat {
        //         raster: false,
        //         blas: true,
        //         factor: 3.0,
        //     },
        //     Isometry3::identity(),
        // );

        // let pawn = PawnAsset::load(
        //     &engine,
        //     "pawn.ron".to_owned(),
        //     RonFormat,
        //     Isometry3::new(Vec3::new(0.0, 5.0, 0.0), Rotor3::identity()),
        // );

        // let pawn2 = PawnAsset::load(
        //     &engine,
        //     "pawn.ron".to_owned(),
        //     RonFormat,
        //     Isometry3::new(Vec3::new(1.0, 10.0, 1.0), Rotor3::identity()),
        // );

        // engine.add_system(player::Player::new(&window, pawn));

        engine.world.spawn((
            Camera::Perspective {
                vertical_fov: std::f32::consts::PI / 3.0,
                aspect_ratio: aspect,
                z_near: 0.1,
                z_far: 1000.0,
            },
            Isometry3::identity(),
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

        let mut reg = Region::new();

        loop {
            if let Some(gltf) = &mut gltf_opt {
                if let Some(gltf) = gltf.get() {
                    tracing::info!("Scene loaded");
                    load_gltf_scene(
                        gltf,
                        &mut engine.world,
                        Isometry3::identity(),
                        1.0,
                    );

                    gltf_opt = None;
                }
            }

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

                        let stats = reg.change_and_reset();
                        tracing::info!(
                            "Alloc {} ({} - {})",
                            stats.bytes_allocated as isize
                                - stats.bytes_deallocated as isize,
                            stats.bytes_allocated,
                            stats.bytes_deallocated
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

pub fn load_gltf_scene(
    gltf: &Gltf,
    world: &mut World,
    iso: Isometry3,
    scale: f32,
) {
    let scene = gltf.scene.unwrap();

    for &node in &*gltf.scenes[scene].nodes {
        let node = &gltf.nodes[node];
        load_gltf_node(gltf, node, iso, Mat4::from_scale(scale), world);
    }
}

fn load_gltf_node(
    gltf: &Gltf,
    node: &GltfNode,
    iso: Isometry3,
    transform: Mat4,
    world: &mut World,
) {
    let transform = transform * node.transform;

    if let Some(mesh) = &node.mesh {
        for (mesh, material) in Iterator::zip(
            gltf.meshes[mesh.primitives.clone()].iter(),
            mesh.materials.iter(),
        ) {
            world.spawn((
                Renderable {
                    mesh: mesh.clone(),
                    material: material.clone(),
                    transform: Some(transform),
                },
                iso,
            ));
        }
    }

    for &child in &*node.children {
        load_gltf_node(gltf, &gltf.nodes[child], iso, transform, world);
    }
}
