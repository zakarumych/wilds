use {
    super::GltfAsset,
    crate::{
        assets::Prefab,
        renderer::Renderable,
        scene::{Global3, Local3},
    },
    gltf::Node,
    hecs::{Entity, World},
    nalgebra as na,
};

pub struct GltfScene {
    nodes: Box<[Entity]>,
}

impl Prefab for GltfAsset {
    type Info = Global3;

    fn spawn(self, root: Global3, world: &mut World, entity: Entity) {
        if !world.contains(entity) {
            tracing::warn!("Prefab loaded but entity is already despawned");
            return;
        }

        let scene = match self.gltf.default_scene() {
            Some(scene) => scene,
            None => self.gltf.scenes().next().unwrap(),
        };

        match scene.nodes().len() {
            0 => {
                tracing::warn!("Gltf asset with 0 nodes loaded");
                world.despawn(entity).unwrap()
            }
            1 if node_transform_identity(&scene.nodes().next().unwrap()) => {
                tracing::info!("Gltf asset with single node at origin");

                let node = scene.nodes().next().unwrap();

                let (iso, scale) = node_transform(&node);
                let global = root.append_iso_scale(&iso, &scale);

                match node.mesh().and_then(|m| self.renderables.get(m.index()))
                {
                    Some(renderables) => match renderables.len() {
                        1 => {
                            world
                                .insert(
                                    entity,
                                    (global, renderables[0].clone()),
                                )
                                .unwrap();
                        }
                        _ => {
                            world.insert_one(entity, global).unwrap();
                            world.spawn_batch(
                                renderables
                                    .iter()
                                    .cloned()
                                    .map(|r| (r, Local3::identity(entity))),
                            );
                        }
                    },
                    None => world.insert_one(entity, global).unwrap(),
                };

                spawn_children(entity, &node, &self, world);
            }
            _ => {
                tracing::info!("Gltf asset loaded");
                let nodes = scene
                    .nodes()
                    .map(|node| {
                        spawn_node(Base::Root(&root), node, &self, world)
                    })
                    .collect();

                world.insert(entity, (GltfScene { nodes }, root)).unwrap();
            }
        }
    }
}

enum Base<'a> {
    Parent(Entity),
    Root(&'a Global3),
}

fn spawn_node(
    base: Base<'_>,
    node: Node<'_>,
    asset: &GltfAsset,
    world: &mut World,
) -> Entity {
    let entity = match node
        .mesh()
        .and_then(|m| asset.renderables.get(m.index()))
    {
        Some(renderables) => match renderables.len() {
            0 => spawn_empty(base, &node, world),
            1 => spawn_renderable(base, &node, renderables[0].clone(), world),
            _ => {
                let entity = spawn_empty(base, &node, world);
                world.spawn_batch(renderables.iter().cloned().map(|r| {
                    (r, Global3::identity(), Local3::identity(entity))
                }));
                entity
            }
        },
        None => spawn_empty(base, &node, world),
    };

    spawn_children(entity, &node, asset, world);
    entity
}

fn spawn_children(
    entity: Entity,
    node: &Node<'_>,
    asset: &GltfAsset,
    world: &mut World,
) {
    for child in node.children() {
        spawn_node(Base::Parent(entity), child, asset, world);
    }
}

fn spawn_empty(base: Base<'_>, node: &Node<'_>, world: &mut World) -> Entity {
    match base {
        Base::Parent(parent) => {
            let (iso, scale) = node_transform(&node);
            let local = Local3 { iso, scale, parent };
            world.spawn((local, Global3::identity()))
        }
        Base::Root(root) => {
            let (iso, scale) = node_transform(&node);
            let global = root.append_iso_scale(&iso, &scale);
            world.spawn((global,))
        }
    }
}

fn spawn_renderable(
    base: Base<'_>,
    node: &Node<'_>,
    renderable: Renderable,
    world: &mut World,
) -> Entity {
    match base {
        Base::Parent(parent) => {
            let (iso, scale) = node_transform(&node);
            let local = Local3 { iso, scale, parent };
            world.spawn((local, Global3::identity(), renderable))
        }
        Base::Root(root) => {
            let (iso, scale) = node_transform(&node);
            let global = root.append_iso_scale(&iso, &scale);
            world.spawn((global, renderable))
        }
    }
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
