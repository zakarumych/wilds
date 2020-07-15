use {
    bumpalo::Bump,
    color_eyre::Report,
    hecs::World,
    std::{cmp::max, collections::VecDeque, time::Duration},
    ultraviolet::{Mat4, Vec3},
    wilds::{
        Camera, Clocks, DirectionalLight, Engine, Gltf, GltfFormat, GltfNode,
        Material, Mesh, Renderer,
    },
    winit::{
        dpi::PhysicalSize,
        event::{Event, WindowEvent},
        window::{Window, WindowBuilder},
    },
};

fn main() -> Result<(), Report> {
    Engine::run(|mut engine| async move {
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
            Mat4::from_translation(Vec3::new(0.0, 10.0, 30.0)),
        ));

        engine.world.spawn((DirectionalLight {
            direction: -Vec3::unit_y(),
            radiance: [8.5, 6.9, 2.4],
        },));

        let mut city_opt = Some(engine.assets.load_with_format(
            "thor_and_the_midgard_serpent/scene.gltf".to_owned(),
            GltfFormat {
                raster: false,
                blas: true,
            },
        ));
        window.request_redraw();

        let mut frame_times = VecDeque::new();
        let mut frame_times_total = Duration::new(0, 0);
        let mut ticker = Duration::new(0, 0);

        loop {
            if let Some(city) = &mut city_opt {
                if let Some(city) = city.get() {
                    tracing::info!("Scene loaded");
                    load_gltf_scene(
                        city,
                        &mut engine.world,
                        Mat4::from_scale(0.01),
                    );

                    city_opt = None;
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
                Event::RedrawRequested(window_id) => {
                    let clock = clocks.step();
                    frame_times.push_back(clock.delta);
                    frame_times_total += clock.delta;

                    while frame_times_total.as_secs() > 5 {
                        match frame_times.pop_front() {
                            Some(delta) => frame_times_total -= delta,
                            None => break,
                        }
                    }

                    if ticker < clock.delta {
                        ticker += max(Duration::from_secs(1), clock.delta);

                        tracing::info!(
                            "FPS: {}",
                            (frame_times.len() as f32
                                / frame_times_total.as_secs_f32())
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
            engine.assets.process(&mut renderer);
        }

        Ok(())
    })
}

pub fn load_gltf_scene(gltf: &Gltf, world: &mut World, transform: Mat4) {
    let scene = gltf.scene.unwrap();

    for &node in &*gltf.scenes[scene].nodes {
        let node = &gltf.nodes[node];
        load_gltf_node(gltf, node, transform, world);
    }
}

fn load_gltf_node(
    gltf: &Gltf,
    node: &GltfNode,
    transform: Mat4,
    world: &mut World,
) {
    let transform = transform * node.transform;

    if let Some(mesh) = &node.mesh {
        for (mesh, material) in Iterator::zip(
            gltf.meshes[mesh.primitives.clone()].iter(),
            mesh.materials.iter(),
        ) {
            world.spawn((mesh.clone(), material.clone(), transform));
        }
    }

    for &child in &*node.children {
        load_gltf_node(gltf, &gltf.nodes[child], transform, world);
    }
}
