use {
    eyre::{bail, Report},
    std::{
        env,
        fs::read_dir,
        path::{Path, PathBuf},
        process::Command,
        time::SystemTime,
    },
};

fn main() -> Result<(), Report> {
    // let mut commands = Vec::with_capacity(SHADERS_TO_COMPILE.len());
    let mut commands = Vec::new();

    // for shader in SHADERS_TO_COMPILE.iter().copied() {
    for path in all_shaders()? {
        let target = shader_target(&path);

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

fn all_shaders() -> Result<Vec<PathBuf>, Report> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("renderer")
        .join("pass");

    let commons = root.join("common");
    let mut force_before = commons.metadata()?.modified()?;

    for e in read_dir(&commons)? {
        let e = e?;
        let ft = e.file_type()?;
        let p = e.path();

        if ft.is_file() {
            force_before =
                std::cmp::max(force_before, p.metadata()?.modified()?);
        } else {
            panic!("Common shaders dir should contain only files");
        }
    }

    let mut result = Vec::new();
    find_shaders(&root, force_before, &mut result)?;

    Ok(result)
}

fn find_shaders(
    root: &Path,
    force_before: SystemTime,
    paths: &mut Vec<PathBuf>,
) -> Result<SystemTime, Report> {
    const SHADER_EXTENSIONS: [&'static str; 7] =
        ["vert", "frag", "comp", "rgen", "rmiss", "rchit", "rahit"];

    let mut force_before_next = force_before;

    for e in read_dir(&root)? {
        let e = e?;
        let ft = e.file_type()?;
        let p = e.path();

        if ft.is_dir() {
            force_before_next = std::cmp::max(
                force_before_next,
                find_shaders(&p, force_before, paths)?,
            );
        } else if ft.is_file() {
            force_before_next =
                std::cmp::max(force_before_next, p.metadata()?.modified()?);
        }
    }

    for e in read_dir(&root)? {
        let e = e?;
        let ft = e.file_type()?;
        let p = e.path();
        if ft.is_file() {
            if let Some(ext) = p.extension() {
                if SHADER_EXTENSIONS.iter().any(|e| **e == *ext) {
                    let target = shader_target(&p);

                    if target.exists() {
                        let target_modified = target.metadata()?.modified()?;
                        if target_modified < force_before_next {
                            paths.push(p);
                        }
                    } else {
                        paths.push(p);
                    }
                }
            }
        }
    }

    Ok(force_before_next)
}

fn shader_target(path: &Path) -> PathBuf {
    if let Some(ext) = path.extension() {
        path.with_extension(format!("{}.spv", ext.to_str().unwrap()))
    } else {
        path.with_extension("spv")
    }
}
