use {
    crate::{
        assets::{append_key, AssetKey, Assets, Handle, ImageAsset},
        renderer::{Context, Material, Texture},
    },
    illume::{OutOfMemory, Sampler, SamplerInfo},
    ordered_float::OrderedFloat,
};

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(untagged)]
pub enum TextureInfo {
    Image(AssetKey),
    ImageWithSampler {
        image: AssetKey,

        #[serde(flatten)]
        sampler: SamplerInfo,
    },
}

#[derive(Clone, Debug)]
pub struct TextureRepr {
    image: Handle<ImageAsset>,
    sampler: SamplerInfo,
}

#[derive(Clone, Debug)]
pub struct TexturePrebuild {
    image: Handle<ImageAsset>,
    sampler: Sampler,
}

impl TextureInfo {
    fn load(self, prefix: Option<&AssetKey>, assets: &Assets) -> TextureRepr {
        let with_prefix = |key: AssetKey| match prefix {
            Some(prefix) => append_key(prefix, &*key),
            None => key,
        };

        match self {
            TextureInfo::Image(image) => TextureRepr {
                image: assets.load(with_prefix(image)),
                sampler: SamplerInfo::default(),
            },
            TextureInfo::ImageWithSampler { image, sampler } => TextureRepr {
                image: assets.load(with_prefix(image)),
                sampler,
            },
        }
    }
}

impl TextureRepr {
    fn prebuild(
        self,
        ctx: &mut Context,
    ) -> Result<TexturePrebuild, OutOfMemory> {
        Ok(TexturePrebuild {
            image: self.image,
            sampler: ctx.create_sampler(self.sampler)?,
        })
    }
}

impl TexturePrebuild {
    async fn finish(self) -> Result<Texture, goods::Error> {
        Ok(Texture {
            image: self.image.await?.image,
            sampler: self.sampler,
        })
    }
}

#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct MaterialInfo {
    #[serde(default)]
    pub albedo: Option<TextureInfo>,

    #[serde(default = "defaults::albedo_factor")]
    pub albedo_factor: [OrderedFloat<f32>; 4],

    #[serde(default)]
    pub metallic_roughness: Option<TextureInfo>,

    #[serde(default = "defaults::metallic_factor")]
    pub metallic_factor: OrderedFloat<f32>,

    #[serde(default = "defaults::roughness_factor")]
    pub roughness_factor: OrderedFloat<f32>,

    #[serde(default)]
    pub emissive: Option<TextureInfo>,

    #[serde(default = "defaults::emissive_factor")]
    pub emissive_factor: [OrderedFloat<f32>; 3],

    #[serde(default)]
    pub normal: Option<TextureInfo>,

    #[serde(default = "defaults::normal_factor")]
    pub normal_factor: OrderedFloat<f32>,
}

mod defaults {
    use ordered_float::OrderedFloat;

    pub const fn albedo_factor() -> [OrderedFloat<f32>; 4] {
        [OrderedFloat(1.0); 4]
    }

    pub const fn metallic_factor() -> OrderedFloat<f32> {
        OrderedFloat(1.0)
    }

    pub const fn roughness_factor() -> OrderedFloat<f32> {
        OrderedFloat(1.0)
    }

    pub const fn emissive_factor() -> [OrderedFloat<f32>; 3] {
        [OrderedFloat(0.0); 3]
    }

    pub const fn normal_factor() -> OrderedFloat<f32> {
        OrderedFloat(1.0)
    }
}

impl MaterialInfo {
    pub fn load(
        self,
        prefix: Option<&AssetKey>,
        assets: &Assets,
    ) -> MaterialRepr {
        MaterialRepr {
            albedo: self.albedo.map(|info| info.load(prefix, assets)),
            albedo_factor: self.albedo_factor,
            metallic_roughness: self
                .metallic_roughness
                .map(|info| info.load(prefix, assets)),
            metallic_factor: self.metallic_factor,
            roughness_factor: self.roughness_factor,
            emissive: self.emissive.map(|info| info.load(prefix, assets)),
            emissive_factor: self.emissive_factor,
            normal: self.normal.map(|info| info.load(prefix, assets)),
            normal_factor: self.normal_factor,
        }
    }
}

#[derive(Clone, Debug)]
pub struct MaterialRepr {
    pub albedo: Option<TextureRepr>,
    pub albedo_factor: [OrderedFloat<f32>; 4],
    pub metallic_roughness: Option<TextureRepr>,
    pub metallic_factor: OrderedFloat<f32>,
    pub roughness_factor: OrderedFloat<f32>,
    pub emissive: Option<TextureRepr>,
    pub emissive_factor: [OrderedFloat<f32>; 3],
    pub normal: Option<TextureRepr>,
    pub normal_factor: OrderedFloat<f32>,
}

impl MaterialRepr {
    pub fn prebuild(
        self,
        ctx: &mut Context,
    ) -> Result<MaterialPrebuild, OutOfMemory> {
        Ok(MaterialPrebuild {
            albedo: self
                .albedo
                .map(|albedo| albedo.prebuild(ctx))
                .transpose()?,
            albedo_factor: self.albedo_factor,
            metallic_roughness: self
                .metallic_roughness
                .map(|metallic_roughness| metallic_roughness.prebuild(ctx))
                .transpose()?,
            metallic_factor: self.metallic_factor,
            roughness_factor: self.roughness_factor,
            emissive: self
                .emissive
                .map(|emissive| emissive.prebuild(ctx))
                .transpose()?,
            emissive_factor: self.emissive_factor,
            normal: self
                .normal
                .map(|normal| normal.prebuild(ctx))
                .transpose()?,
            normal_factor: self.normal_factor,
        })
    }
}

#[derive(Clone, Debug)]
pub struct MaterialPrebuild {
    pub albedo: Option<TexturePrebuild>,
    pub albedo_factor: [OrderedFloat<f32>; 4],
    pub metallic_roughness: Option<TexturePrebuild>,
    pub metallic_factor: OrderedFloat<f32>,
    pub roughness_factor: OrderedFloat<f32>,
    pub emissive: Option<TexturePrebuild>,
    pub emissive_factor: [OrderedFloat<f32>; 3],
    pub normal: Option<TexturePrebuild>,
    pub normal_factor: OrderedFloat<f32>,
}

impl MaterialPrebuild {
    pub async fn finish(self) -> Result<Material, goods::Error> {
        Ok(Material {
            albedo: match self.albedo {
                Some(albedo) => Some(albedo.finish().await?),
                None => None,
            },
            albedo_factor: self.albedo_factor,
            metallic_roughness: match self.metallic_roughness {
                Some(metallic_roughness) => {
                    Some(metallic_roughness.finish().await?)
                }
                None => None,
            },
            metallic_factor: self.metallic_factor,
            roughness_factor: self.roughness_factor,
            emissive: match self.emissive {
                Some(emissive) => Some(emissive.finish().await?),
                None => None,
            },
            emissive_factor: self.emissive_factor,
            normal: match self.normal {
                Some(normal) => Some(normal.finish().await?),
                None => None,
            },
            normal_factor: self.normal_factor,
        })
    }
}
