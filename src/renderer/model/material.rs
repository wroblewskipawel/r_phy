use std::{error::Error, path::Path};

#[derive(Debug, Clone, Copy)]
pub struct MaterialHandle(pub u64);

pub struct MaterialBuilder {
    albedo: Option<&'static Path>,
}

pub struct Material {
    pub albedo: &'static Path,
}

impl MaterialBuilder {
    pub fn build(self) -> Result<Material, Box<dyn Error>> {
        Ok(Material {
            albedo: self.albedo.ok_or("Albedo texture not provided!")?,
        })
    }

    pub fn with_albedo(self, file_path: &'static Path) -> Self {
        Self {
            albedo: Some(file_path),
        }
    }
}

impl Material {
    pub fn builder() -> MaterialBuilder {
        MaterialBuilder { albedo: None }
    }
}
