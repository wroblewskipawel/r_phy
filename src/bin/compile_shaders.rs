use glob::glob;
use std::{
    error::Error,
    fs::create_dir_all,
    path::Path,
    process::{Command, Output},
};

const SHADER_SOURCE_EXTENSIONS: &[&str] = &["frag", "vert"];
const SHADER_SOURCE_DIRECTORY: &str = "shaders/src/";
const SHADER_TARGET_DIRECTORY: &str = "shaders/spv/";

fn to_str(path: &Path) -> Result<&str, Box<dyn Error>> {
    Ok(path
        .to_str()
        .ok_or("Path is not valid UTF-8 Unicode string!")?)
}

fn main() -> Result<(), Box<dyn Error>> {
    for extension in SHADER_SOURCE_EXTENSIONS {
        let pattern = format!("{}/**/*.{}", SHADER_SOURCE_DIRECTORY, extension);
        for source_path in (glob(&pattern)?).flatten() {
            let target_path = Path::new(SHADER_TARGET_DIRECTORY).join(
                source_path
                    .strip_prefix(SHADER_SOURCE_DIRECTORY)?
                    .with_file_name(format!("{}.spv", extension)),
            );
            create_dir_all(target_path.parent().unwrap())?;
            let source_filename = to_str(&source_path)?;
            let target_filename = to_str(&target_path)?;
            let Output { status, stderr, .. } = Command::new("glslc")
                .args([source_filename, "-o", target_filename])
                .output()?;
            let stderr = String::from_utf8(stderr)?;
            if !status.success() {
                Err(format!(
                    "Failed to compile shader source at {}\n\t with error: {}",
                    source_filename, stderr,
                ))?;
            }
        }
    }
    Ok(())
}
