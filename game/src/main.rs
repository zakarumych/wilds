use {
    bumpalo::Bump,
    color_eyre::Report,
    hecs::World,
    ultraviolet::Mat4,
    wilds::{
        Camera, Clocks, DirectionalLight, Engine, /* Event, */ Gltf,
        GltfFormat, GltfNode, Renderer, /* WindowEvent, */
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
                width: 640,
                height: 480,
            }),
        )?;

        let mut bump = Bump::new();
        let mut renderer = Renderer::new(&window)?;
        let mut clocks = Clocks::new();

        engine.world.spawn((
            Camera::Perspective {
                vertical_fov: std::f32::consts::PI / 2.0,
                aspect_ratio: 640.0 / 480.0,
                z_near: 0.1,
                z_far: 1000.0,
            },
            Mat4::identity(),
        ));

        let mut city_opt = Some(engine.assets.load_with_format(
            "desert-city.gltf".to_owned(),
            GltfFormat {
                raster: false,
                blas: true,
            },
        ));
        window.request_redraw();

        loop {
            if let Some(city) = &mut city_opt {
                if let Some(city) = city.get() {
                    load_gltf_scene(city, &mut engine.world);
                    city_opt = None;
                }
            }

            let clock = clocks.step();

            // Main game loop
            match engine.next().await {
                Event::WindowEvent {
                    window_id,
                    event: WindowEvent::CloseRequested,
                } if window_id == window.id() => {
                    break;
                }
                Event::RedrawRequested(window_id) => {
                    tracing::trace!("Request redraw");
                    renderer.draw(&mut engine.world, &clock, &bump)?;
                }
                Event::MainEventsCleared => {
                    window.request_redraw();
                }
                _ => {}
            }

            bump.reset();
            engine.assets.process(&mut renderer.device);
        }

        Ok(())
    })
}

pub fn load_gltf_scene(gltf: &Gltf, world: &mut World) {
    let scene = gltf.scene.unwrap();

    for &node in &*gltf.scenes[scene].nodes {
        let node = &gltf.nodes[node];
        load_gltf_node(gltf, node, Mat4::identity(), world);
    }
}

fn load_gltf_node(
    gltf: &Gltf,
    node: &GltfNode,
    transform: Mat4,
    world: &mut World,
) {
    let transform = transform * node.transform;

    if let Some(mesh) = node.mesh {
        for mesh in &*gltf.meshes[mesh] {
            world.spawn((mesh.clone(), transform));
        }
    }

    for &child in &*node.children {
        load_gltf_node(gltf, &gltf.nodes[child], transform, world);
    }
}
