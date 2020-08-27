use {
    super::GltfLoadingError,
    crate::renderer::{Material, Texture},
};

pub fn load_gltf_material(
    material: gltf::Material,
    textures: &[Texture],
) -> Result<Material, GltfLoadingError> {
    let pbr = material.pbr_metallic_roughness();

    Ok(Material {
        albedo: match pbr.base_color_texture() {
            Some(info) => match textures.get(info.texture().index()) {
                Some(texture) => Some(texture.clone()),
                None => {
                    return Err(GltfLoadingError::MissingTexture);
                }
            },
            None => None,
        },
        albedo_factor: {
            let [r, g, b, a] = pbr.base_color_factor();
            [r.into(), g.into(), b.into(), a.into()]
        },

        metallic_roughness: match pbr.metallic_roughness_texture() {
            Some(info) => match textures.get(info.texture().index()) {
                Some(texture) => Some(texture.clone()),
                None => {
                    return Err(GltfLoadingError::MissingTexture);
                }
            },
            None => None,
        },
        metallic_factor: pbr.metallic_factor().into(),
        roughness_factor: pbr.roughness_factor().into(),

        emissive: match material.emissive_texture() {
            Some(info) => match textures.get(info.texture().index()) {
                Some(texture) => Some(texture.clone()),
                None => {
                    return Err(GltfLoadingError::MissingTexture);
                }
            },
            None => None,
        },
        emissive_factor: {
            let [r, g, b] = material.emissive_factor();
            [r.into(), g.into(), b.into()]
        },

        normal: match material.normal_texture() {
            Some(info) => match textures.get(info.texture().index()) {
                Some(texture) => Some(texture.clone()),
                None => {
                    return Err(GltfLoadingError::MissingTexture);
                }
            },
            None => None,
        },
        normal_factor: material
            .normal_texture()
            .map(|info| info.scale())
            .unwrap_or(0.0)
            .into(),
    })
}
