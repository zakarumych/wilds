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

    Ok(())
}

const SHADERS_TO_COMPILE: &'static [&'static str] = &[
    "rt_prepass/primary.rchit",
    "rt_prepass/primary.rgen",
    "rt_prepass/primary.rmiss",
    "rt_prepass/diffuse.rchit",
    "rt_prepass/diffuse.rmiss",
    "rt_prepass/shadow.rmiss",
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
