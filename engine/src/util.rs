use nalgebra as na;

pub fn decompose_transform3(
    m: na::MatrixSlice4<f32>,
) -> (na::Isometry3<f32>, na::Vector3<f32>) {
    let t: na::Vector3<f32> = m.column(3).xyz().into_owned();

    let mut r = m.remove_column(3).remove_row(3);
    let mut s = na::Vector3::new(
        r.column(0).norm(),
        r.column(1).norm(),
        r.column(2).norm(),
    );
    let sign = r.determinant().signum();
    s *= sign;
    r *= sign;

    r *= na::Matrix3::from_diagonal(&s.apply_into(|c: f32| 1.0 / c));

    let r = na::Rotation3::from_matrix(&r);

    let r = match r.axis_angle() {
        Some((axis, angle)) => {
            na::UnitQuaternion::from_axis_angle(&axis, angle)
        }
        None => na::UnitQuaternion::identity(),
    };

    let iso = na::Isometry3::from_parts(na::Translation3::from(t), r);
    (iso, s)
}
