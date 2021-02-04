use {
    nalgebra as na,
    wilds::{
        engine::{Engine, SystemContext},
        light::{DirectionalLight, SkyLight},
    },
};

pub fn spawn_sun(engine: &mut Engine) {
    let sunlight =
        (na::Vector3::new(255.0, 207.0, 72.0) / 255.0).map(|c| c / (1.1 - c));

    let skyradiance =
        (na::Vector3::new(117.0, 187.0, 253.0) / 255.0).map(|c| c / (1.2 - c));

    engine.world.spawn((
        DirectionalLight {
            direction: na::Vector3::new(-30.0, -25.0, -5.0),
            radiance: sunlight.into(),
        },
        SkyLight {
            radiance: skyradiance.into(),
        },
    ));

    engine.add_system(move |ctx: SystemContext<'_>| {
        let elapsed = ctx.clocks.step - ctx.clocks.start;
        let d = elapsed.as_secs_f32() / 10.0;
        let mut query = ctx.world.query::<&mut DirectionalLight>();

        for (_, dirlight) in query.iter() {
            dirlight.direction =
                na::Vector3::new(d.sin() * 30.0, d.cos() * 25.0, d.cos() * 5.0);
        }

        let mut query = ctx.world.query::<&mut SkyLight>();

        for (_, skylight) in query.iter() {
            skylight.radiance = (skyradiance * (1.1 - d.cos()) / 2.1).into();
        }
    });
}
