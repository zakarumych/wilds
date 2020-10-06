use {
    eyre::{bail, Report},
    std::{env, path::Path, process::Command},
};

fn main() -> Result<(), Report> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("renderer")
        .join("pass");

    let mut commands = Vec::with_capacity(SHADERS_TO_COMPILE.len());

    for shader in SHADERS_TO_COMPILE.iter().copied() {
        let path = root.join(shader);
        let ext = path.extension().unwrap();
        let spv = format!("{}.spv", ext.to_str().unwrap());
        let target = path.with_extension(spv);

        // if target.exists() {
        //     if target.metadata()?.modified()? >= path.metadata()?.modified()?
        // {         // Skip unchanged
        //         continue;
        //     }
        // }

        commands.push(
            Command::new("glslangValidator")
                .arg("--target-env")
                .arg("vulkan1.2")
                .arg("-V")
                .arg(&path)
                .arg("-o")
                .arg(&target)
                .spawn()?,
        );
    }

    for command in commands {
        let output = command.wait_with_output()?;

        if !output.status.success() {
            bail!(
                "Failed to compile shader. Status: {}\n{}",
                output.status,
                std::str::from_utf8(&output.stderr)?
            );
        }
    }

    // Pre-build blue-noise
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("blue_noise");
    let output = root.join(format!("RGBAF32_256x256x128"));

    if !output.exists() {
        let mut raw_blue_noise_bytes =
            Vec::with_capacity(4 * 4 * 256 * 256 * 128);
        let path_256_256 = root.join("256_256");

        for i in 0..128 {
            let path = path_256_256.join(format!("HDR_RGBA_{:04}.png", i));

            tracing::debug!("Reading blue-noise from {}", path.display());

            let noise_file = std::fs::read(path)?;
            let noise_image = image::load_from_memory(&noise_file)?;
            let noise_image = noise_image.as_rgba16().ok_or_else(|| {
                eyre::eyre!("Noise image expected to be 16 bit RGBA")
            })?;

            for &image::Rgba([r, g, b, a]) in noise_image.pixels() {
                raw_blue_noise_bytes
                    .extend_from_slice(&(r as f32 / 65535.0).to_ne_bytes());
                raw_blue_noise_bytes
                    .extend_from_slice(&(g as f32 / 65535.0).to_ne_bytes());
                raw_blue_noise_bytes
                    .extend_from_slice(&(b as f32 / 65535.0).to_ne_bytes());
                raw_blue_noise_bytes
                    .extend_from_slice(&(a as f32 / 65535.0).to_ne_bytes());
            }
        }

        std::fs::write(output, &raw_blue_noise_bytes)?;
    }

    Ok(())
}

const SHADERS_TO_COMPILE: &'static [&'static str] = &[
    "common/shadow.rmiss",
    "rt_prepass/viewport.rgen",
    "rt_prepass/primary.rchit",
    "rt_prepass/primary.rmiss",
    "rt_prepass/diffuse.rchit",
    "rt_prepass/diffuse.rmiss",
    "ray_probe/primary.rchit",
    "ray_probe/probes.rgen",
    "ray_probe/primary.rmiss",
    "combine/combine.vert",
    "combine/combine.frag",
    "gauss_filter/gauss_filter.vert",
    "gauss_filter/gauss_filter.frag",
    "atrous/atrous.vert",
    "atrous/atrous0h.frag",
    "atrous/atrous1h.frag",
    "atrous/atrous2h.frag",
    "atrous/atrous0v.frag",
    "atrous/atrous1v.frag",
    "atrous/atrous2v.frag",
    "pose/pose.comp",
];
