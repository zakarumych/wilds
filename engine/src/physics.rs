// use {
//     crate::{clocks::ClockIndex, util::rotor_to_quaternion},
//     hecs::{Entity, World},
//     ncollide3d::{
//         bounding_volume::AABB, broad_phase::DBVTBroadPhase,
// shape::ShapeHandle,     },
//     ultraviolet::{Isometry3, Mat4},
// };

// pub struct Physics {
//     broad_phase: DBVTBroadPhase<f32, AABB<f32>, Entity>,
// }

// struct Physical {}

// impl Physics {
//     pub fn run(&mut self, world: &mut World, clocks: ClockIndex) {
//         let new_shapes = world
//             .query::<(&Isometry3, &ShapeHandle<f32>)>()
//             .without::<Physical>();

//         for (entity, (iso, shape)) in new_shapes.iter() {
//             self.broad_phase.create_proxy()
//         }
//     }
// }
