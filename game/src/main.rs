mod pawn;

use {
    self::pawn::*,
    bumpalo::Bump,
    color_eyre::Report,
    goods::RonFormat,
    hecs::World,
    std::{cmp::max, collections::VecDeque, task::Poll, time::Duration},
    ultraviolet::{Isometry3, Mat3, Mat4, Rotor3, Vec3},
    wilds::{
        assets::{
            Gltf, GltfFormat, GltfNode, Prefab, Terrain, TerrainAsset,
            TerrainFormat,
        },
        camera::{
            following::{FollowingCamera, FollowingCameraSystem},
            Camera,
        },
        clocks::Clocks,
        engine::Engine,
        fps_counter::FpsCounter,
        light::{DirectionalLight, SkyLight},
        physics::Physics,
        renderer::{Material, Mesh, Renderable, Renderer},
    },
    winit::{
        dpi::PhysicalSize,
        event::{Event, WindowEvent},
        window::{Window, WindowBuilder},
    },
};

fn main() -> Result<(), Report> {
    tracing_subscriber::fmt::init();
    tracing::info!("App started");

    Engine::run(|mut engine| async move {
        engine.add_system(
            FollowingCameraSystem::new()
                .with_factor(0.3, 0.3)
                .with_speed(50.0),
        );

        engine.add_system(Physics::new());

        let window = engine.build_window(
            WindowBuilder::new().with_inner_size(PhysicalSize {
                width: 640,
                height: 480,
            }),
        )?;

        let mut bump = Bump::new();
        let mut renderer = Renderer::new(&window)?;
        let mut clocks = Clocks::new();

        let sunlight = Vec3::new(255.0, 207.0, 72.0)
            .map(|c| c / 255.0)
            .map(|c| c / (1.3 - c));
        let skylight = Vec3::new(117.0, 187.0, 253.0)
            .map(|c| c / 255.0)
            .map(|c| c / (1.3 - c));

        dbg!(sunlight, skylight);

        engine.world.spawn((
            DirectionalLight {
                direction: Vec3::new(-3.0, -3.0, -3.0),
                radiance: sunlight.into(),
            },
            SkyLight {
                radiance: skylight.into(),
            },
        ));

        // let mut city_opt = Some(engine.assets.load_with_format(
        //     "thor_and_the_midgard_serpent/scene.gltf".to_owned(),
        //     GltfFormat {
        //         raster: false,
        //         blas: true,
        //     },
        // ));

        let terrain = TerrainAsset::load(
            &engine,
            "terrain/0001.png".to_owned(),
            TerrainFormat {
                raster: false,
                blas: true,
                factor: 3.0,
            },
            Isometry3::identity(),
        );

        let pawn = PawnAsset::load(
            &engine,
            "pawn.ron".to_owned(),
            RonFormat,
            Isometry3::new(Vec3::new(32.0, 5.0, 32.0), Rotor3::identity()),
        );

        engine.world.spawn((
            Camera::Perspective {
                vertical_fov: std::f32::consts::PI / 3.0,
                aspect_ratio: 640.0 / 480.0,
                z_near: 0.1,
                z_far: 1000.0,
            },
            // Isometry3::new(Vec3::new(32.0, 5.0, 35.0), Rotor3::identity()),
            Isometry3::identity(),
            FollowingCamera { follows: pawn },
        ));

        window.request_redraw();

        let mut fps_counter = FpsCounter::new(Duration::from_secs(5));
        let mut ticker = Duration::from_secs(0);

        loop {
            // if let Some(city) = &mut city_opt {
            //     if let Some(city) = city.get() {
            //         tracing::info!("Scene loaded");
            //         load_gltf_scene(
            //             city,
            //             &mut engine.world,
            //             Isometry3::identity(),
            //             0.01,
            //         );

            //         city_opt = None;
            //     }
            // }

            // Main game loop
            match engine.next().await {
                Event::WindowEvent {
                    window_id,
                    event: WindowEvent::CloseRequested,
                } if window_id == window.id() => {
                    break;
                }
                Event::RedrawRequested(window_id) => {
                    let clock = clocks.step();
                    engine.advance(clock);

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
                    renderer.draw(&mut engine.world, &clock, &bump)?;
                }
                Event::MainEventsCleared => {
                    window.request_redraw();
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
