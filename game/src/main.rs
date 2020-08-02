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
        Camera, Clocks, DirectionalLight, Engine, FpsCounter, FreeCamera,
        FreeCameraSystem, Gltf, GltfFormat, GltfNode, Material, Mesh, Prefab,
        Renderable, Renderer, Terrain, TerrainAsset, TerrainFormat,
    },
    winit::{
        dpi::PhysicalSize,
        event::{Event, WindowEvent},
        window::{Window, WindowBuilder},
    },
};

fn main() -> Result<(), Report> {
    Engine::run(|mut engine| async move {
        engine.add_system(
            FreeCameraSystem::new()
                .with_factor(0.3, 0.3)
                .with_speed(5.0),
        );

        let window = engine.build_window(
            WindowBuilder::new().with_inner_size(PhysicalSize {
                width: 1920,
                height: 1080,
            }),
        )?;

        let mut bump = Bump::new();
        let mut renderer = Renderer::new(&window)?;
        let mut clocks = Clocks::new();

        engine.world.spawn((
            Camera::Perspective {
                vertical_fov: std::f32::consts::PI / 3.0,
                aspect_ratio: 1920.0 / 1080.0,
                z_near: 0.1,
                z_far: 1000.0,
            },
            Isometry3::identity(),
            FreeCamera,
        ));

        engine.world.spawn((DirectionalLight {
            direction: -Vec3::unit_y(),
            radiance: [8.5, 6.9, 2.4],
        },));

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

                        eprintln!(
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
