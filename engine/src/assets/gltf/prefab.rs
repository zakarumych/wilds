use {
    super::GltfAsset,
    crate::{
        assets::Prefab,
        renderer::Renderable,
        resources::Resources,
        scene::{Global3, Local3},
    },
    gltf::Node,
    hecs::{Entity, World},
    nalgebra as na,
};

pub struct GltfScene {
    nodes: Box<[Entity]>,
}

pub struct Gltf;

impl Prefab for Gltf {
    type Asset = GltfAsset;

    fn spawn(
        asset: GltfAsset,
        world: &mut World,
        _resources: &mut Resources,
        entity: Entity,
    ) {
        if !world.contains(entity) {
            tracing::warn!("Prefab loaded but entity is already despawned");
            return;
        }

        let scene = match asset.gltf.default_scene() {
            Some(scene) => scene,
            None => asset.gltf.scenes().next().unwrap(),
        };

        match scene.nodes().len() {
            0 => {
                tracing::warn!("Gltf asset with 0 nodes loaded");
                world.despawn(entity).unwrap()
            }
            1 if node_transform_identity(&scene.nodes().next().unwrap()) => {
                tracing::info!("Gltf asset with single node at origin");

                let node = scene.nodes().next().unwrap();

                match node.mesh().and_then(|m| asset.renderables.get(m.index()))
                {
                    Some(renderables) => match renderables.len() {
                        1 => {
                            world
                                .insert(entity, (renderables[0].clone(),))
                                .unwrap();
                        }
                        _ => {
                            world.spawn_batch(renderables.iter().cloned().map(
                                |r| {
                                    (
                                        r,
                                        Local3::identity(entity),
                                        Global3::identity(),
                                    )
                                },
                            ));
                        }
                    },
                    None => {}
                };

                spawn_children(
                    entity,
                    na::Vector3::new(1.0, 1.0, 1.0),
                    &node,
                    &asset,
                    world,
                );
            }
            _ => {
                tracing::info!("Gltf asset loaded");
                let nodes = scene
                    .nodes()
                    .map(|node| {
                        spawn_node(
                            entity,
                            na::Vector3::new(1.0, 1.0, 1.0),
                            node,
                            &asset,
                            world,
                        )
                    })
                    .collect();

                world.insert(entity, (GltfScene { nodes },)).unwrap();
            }
        }
    }
}

fn spawn_node(
    parent: Entity,
    parent_scale: na::Vector3<f32>,
    node: Node<'_>,
    asset: &GltfAsset,
    world: &mut World,
) -> Entity {
    let (iso, scale) = node_transform(&node);

    let entity =
        match node.mesh().and_then(|m| asset.renderables.get(m.index())) {
            Some(renderables) => match renderables.len() {
                0 => spawn_empty(parent, iso, world),
                1 => {
                    let mut renderable = renderables[0].clone();
                    renderable.transform =
                        Some(na::Matrix4::new_nonuniform_scaling(&scale));
                    spawn_renderable(parent, iso, renderable, world)
                }
                _ => {
                    let entity = spawn_empty(parent, iso, world);
                    world.spawn_batch(renderables.iter().cloned().map(|r| {
                        (r, Global3::identity(), Local3::identity(entity))
                    }));
                    entity
                }
            },
            None => spawn_empty(parent, iso, world),
        };

    spawn_children(
        entity,
        parent_scale.component_mul(&scale),
        &node,
        asset,
        world,
    );
    entity
}

fn spawn_children(
    entity: Entity,
    scale: na::Vector3<f32>,
    node: &Node<'_>,
    asset: &GltfAsset,
    world: &mut World,
) {
    for child in node.children() {
        spawn_node(entity, scale, child, asset, world);
    }
}

fn spawn_empty(
    parent: Entity,
    iso: na::Isometry3<f32>,
    world: &mut World,
) -> Entity {
    let local = Local3 { iso, parent };
    world.spawn((local, Global3::identity()))
}

fn spawn_renderable(
    parent: Entity,
    iso: na::Isometry3<f32>,
    renderable: Renderable,
    world: &mut World,
) -> Entity {
    let local = Local3 { iso, parent };
    world.spawn((local, Global3::identity(), renderable))
}

fn node_transform(node: &Node) -> (na::Isometry3<f32>, na::Vector3<f32>) {
    let (t, r, s) = node.transform().decomposed();
    let [tx, ty, tz] = t;
    let [rx, ry, rz, rw] = r;
    (
        na::Isometry3 {
            rotation: na::Unit::new_normalize(na::Quaternion::new(
                rw, rx, ry, rz,
            )),
            translation: na::Translation3::new(tx, ty, tz),
        },
        s.into(),
    )
}

fn node_transform_identity(node: &Node) -> bool {
    let (t, r, s) = node.transform().decomposed();

    let [x, y, z] = s;
    if (x - 1.0).abs() > std::f32::EPSILON
        || (y - 1.0).abs() > std::f32::EPSILON
        || (z - 1.0).abs() > std::f32::EPSILON
    {
        return false;
    }

    let [x, y, z] = t;
    if x.abs() > std::f32::EPSILON
        || y.abs() > std::f32::EPSILON
        || z.abs() > std::f32::EPSILON
    {
        return false;
    }
    let [x, y, z, w] = r;
    x.abs() <= std::f32::EPSILON
        && y.abs() <= std::f32::EPSILON
        && z.abs() <= std::f32::EPSILON
        && (w - 1.0).abs() <= std::f32::EPSILON
}
