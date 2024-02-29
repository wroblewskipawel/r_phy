use glob::glob;
use std::{
    error::Error,
    fs::create_dir_all,
    path::Path,
    process::{Command, Output},
};

const SHADER_SOURCE_EXTENSIONS: &'static [&str] = &["frag", "vert"];
const SHADER_SOURCE_DIRECTORY: &str = &"shaders/src/";
const SHADER_OUTPUT_DIRECTORY: &str = &"shaders/spv/";

fn main() -> Result<(), Box<dyn Error>> {
    for extension in SHADER_SOURCE_EXTENSIONS {
        let pattern = format!("{}/**/*.{}", SHADER_SOURCE_DIRECTORY, extension);
        for entry in glob(&pattern)? {
            match entry {
                Ok(path) => {
                    let shader_name = path
                        .file_stem()
                        .and_then(|stem| stem.to_str())
                        .ok_or("Shader name is not valid UTF-8 Unicode!")?;
                    create_dir_all(Path::new(&format!(
                        "{}/{}",
                        SHADER_OUTPUT_DIRECTORY, shader_name
                    )))?;
                    let output_filename = format!(
                        "{}/{}/{}.spv",
                        SHADER_OUTPUT_DIRECTORY, shader_name, extension
                    );
                    let source_filename = path
                        .to_str()
                        .ok_or("Shader source path is not valid UTF-8 Unicode string!")?;
                    let Output { status, .. } = Command::new("glslc")
                        .args(&[source_filename, "-o", &output_filename])
                        .output()?;
                    if !status.success() {
                        Err(format!(
                            "Failed to compile shader source at {}",
                            source_filename
                        ))?;
                    }
                }
                _ => (),
            }
        }
    }
    Ok(())
}
