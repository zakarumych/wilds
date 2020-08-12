use {
    nalgebra as na,
    ultraviolet::{Bivec3, Isometry3, Rotor3},
};

pub fn rotor_to_quaternion(r: Rotor3) -> na::Quaternion<f32> {
    na::Quaternion::from_parts(
        r.s,
        na::Vector3::new(-r.bv.yz, r.bv.xz, -r.bv.xy),
    )
}

pub fn quaternion_to_rotor(q: na::Quaternion<f32>) -> Rotor3 {
    let imag = q.imag();
    Rotor3 {
        s: q.scalar(),
        bv: Bivec3 {
            xy: -imag.z,
            xz: imag.y,
            yz: -imag.x,
        },
    }
}

pub fn iso_to_nalgebra(iso: &Isometry3) -> na::Isometry3<f32> {
    let r = rotor_to_quaternion(iso.rotation);
    let t: [f32; 3] = iso.translation.into();

    na::Isometry3::from_parts(
        na::Translation::from(na::Vector3::from(t)),
        na::Unit::new_normalize(r),
    )
}

pub fn iso_from_nalgebra(iso: &na::Isometry3<f32>) -> Isometry3 {
    let r = quaternion_to_rotor(iso.rotation.into_inner());
    let t: [f32; 3] = iso.translation.vector.into();
    Isometry3 {
        translation: t.into(),
        rotation: r,
    }
}

// fn sim_to_nalgebra(sim: &Similarity3) -> na::Similarity3<f32> {
//     let r = rotor_to_quaternion(sim.rotation);
//     let t: [f32; 3] = sim.translation.into();

//     na::Similarity3::from_parts(
//         na::Translation::from(na::Vector3::from(t)),
//         na::Unit::new_normalize(r),
//         sim.scale,
//     )
// }

// fn sim_from_nalgebra(sim: &na::Similarity3<f32>) -> Similarity3 {
//     let r = quaternion_to_rotor(sim.isometry.rotation.into_inner());
//     let t: [f32; 3] = sim.isometry.translation.vector.into();
//     Similarity3 {
//         translation: t.into(),
//         rotation: r,
//         scale: sim.scaling(),
//     }
// }

// pub fn decompose_transform3(
//     tr: &na::Transform3<f32>,
// ) -> (na::Isometry3<f32>, na::Vector3<f32>) {
//     let m = tr.matrix();
//     let m: na::Matrix3x4<f32> = m.remove_row(3);
//     let t: na::Vector3<f32> = m.column(3).into_owned();

//     let m = m.remove_column(3);
//     // let s = na::Vector3::new(
//     //     m.column(0).norm(),
//     //     m.column(1).norm(),
//     //     m.column(2).norm(),
//     // );

//     // let r = na::Rotation3::from_matrix(&m);

//     // let r = match r.axis_angle() {
//     //     Some((axis, angle)) => {
//     //         na::UnitQuaternion::from_axis_angle(&axis, angle)
//     //     }
//     //     None => na::UnitQuaternion::identity(),
//     // };

//     // let iso = na::Isometry3::from_parts(na::Translation3::from(t), r);

//     let svd = m.svd(true, true);
//     let w = svd.u.as_ref().unwrap();
//     let v_t = svd.v_t.as_ref().unwrap();
//     let v = v_t.adjoint();
//     let s = svd.singular_values;

//     // let p: na::Matrix3<f32> = v * na::Matrix3::from_diagonal(&s) * v_t;
//     let u = w * v_t;

//     let r = match na::Rotation3::from_matrix_unchecked(u).axis_angle() {
//         Some((axis, angle)) => {
//             na::UnitQuaternion::from_axis_angle(&axis, angle)
//         }
//         None => na::UnitQuaternion::identity(),
//     };

//     // // eprintln!("Rotation: {}, Scale: {}", r, s);
//     // eprintln!(
//     //     "Orig: {}, Reconstruct: {}",
//     //     tr.matrix(),
//     //     na::Matrix4::new_translation(&t)
//     //         * r.to_homogeneous()
//     //         * na::Matrix4::new_nonuniform_scaling(&s)
//     // );
//     let iso = na::Isometry3::from_parts(na::Translation3::from(t), r);
//     (iso, s)
// }
