use {
    hecs::{Entity, World},
    nalgebra as na,
    palette::Srgb,
    std::f32::consts::{PI, TAU},
    wilds::{
        assets::{Asset, AssetKey, Prefab, SimpleAsset},
        engine::{Engine, System, SystemContext},
        light::{DirectionalLight, SkyLight},
        resources::Resources,
    },
};

#[derive(Clone, Copy, Debug, serde::Deserialize)]
pub struct SkyAsset {
    #[serde(with = "serde_color")]
    sky_color: Srgb,
    sky_luminocity: f32,
    #[serde(with = "serde_color")]
    sun_color: Srgb,
    sun_luminocity: f32,
}

impl SimpleAsset for SkyAsset {}

pub struct Sky;

impl Prefab for Sky {
    type Asset = SkyAsset;

    fn spawn(
        asset: Self::Asset,
        world: &mut World,
        _resources: &mut Resources,
        entity: Entity,
    ) {
        let (r, g, b) = asset.sky_color.into_linear().into_components();
        let sky = [
            asset.sky_luminocity * r / 2.2,
            asset.sky_luminocity * g / 2.2,
            asset.sky_luminocity * b / 2.2,
        ];
        let (r, g, b) = asset.sun_color.into_linear().into_components();
        let sun = [
            asset.sun_luminocity * r,
            asset.sun_luminocity * g,
            asset.sun_luminocity * b,
        ];

        let _ = world.insert(
            entity,
            (
                Sky,
                DirectionalLight {
                    direction: na::Vector3::new(0.0, 1.0, 0.0),
                    radiance: sun,
                },
                SkyLight { radiance: sky },
            ),
        );
    }
}

pub struct SkySystem {
    pub angle: f32,
    pub velocity: f32,
}

impl System for SkySystem {
    fn name(&self) -> &str {
        "Sky"
    }

    fn run(&mut self, ctx: SystemContext<'_>) {
        let old_angle = self.angle;
        let mut new_angle =
            old_angle + self.velocity * ctx.clocks.delta.as_secs_f32() * TAU;

        if new_angle > PI {
            new_angle -= TAU;
        }

        // new_angle = 1.0;

        let mut sky_query = ctx
            .world
            .query::<(&mut DirectionalLight, &mut SkyLight)>()
            .with::<Sky>();

        for (_, (dl, sl)) in sky_query.iter() {
            dl.direction =
                na::Vector3::new(0.0, -new_angle.cos(), new_angle.sin()) * 10.0;

            let m = (new_angle.cos() + 1.2) / (old_angle.cos() + 1.2);

            sl.radiance[0] *= m;
            sl.radiance[1] *= m;
            sl.radiance[2] *= m;
        }

        self.angle = new_angle;
    }
}

mod serde_color {
    use {palette::Srgb, serde::de::*, std::fmt};

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Srgb, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ColorVisitor;

        impl<'de> Visitor<'de> for ColorVisitor {
            type Value = Srgb;

            fn expecting(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt.write_str("Color HEX code or array of 3 elements")
            }

            fn visit_str<E>(self, s: &str) -> Result<Srgb, E>
            where
                E: Error,
            {
                if s.is_ascii() && s.len() == 7 && "#" == &s[0..1] {
                    let r = u8::from_str_radix(&s[1..3], 16);
                    let g = u8::from_str_radix(&s[3..5], 16);
                    let b = u8::from_str_radix(&s[5..7], 16);

                    match (r, g, b) {
                        (Ok(r), Ok(g), Ok(b)) => {
                            return Ok(Srgb::new(
                                r as f32 / 255.0,
                                g as f32 / 255.0,
                                b as f32 / 255.0,
                            ));
                        }
                        _ => {}
                    }
                }

                Err(E::invalid_value(Unexpected::Str(s), &self))
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Srgb, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let r = seq
                    .next_element()?
                    .ok_or_else(|| Error::invalid_length(3, &self))?;
                let g = seq
                    .next_element()?
                    .ok_or_else(|| Error::invalid_length(3, &self))?;
                let b = seq
                    .next_element()?
                    .ok_or_else(|| Error::invalid_length(3, &self))?;

                Ok(Srgb::new(r, g, b))
            }
        }
        deserializer.deserialize_any(ColorVisitor)
    }
}
