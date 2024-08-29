use std::{
    any::TypeId, collections::HashMap, error::Error, marker::PhantomData, ops::Deref, path::PathBuf,
};

use crate::core::{Contains, Here, Marker, There};

pub trait Material: 'static {
    const NUM_IMAGES: usize;
    fn images(&self) -> impl Iterator<Item = &Image>;
}

#[derive(Debug, Clone)]
pub enum Image {
    Buffer(Vec<u8>),
    File(PathBuf),
}

#[derive(Debug)]
pub struct MaterialHandle<M: Material>(pub u64, pub PhantomData<M>);

impl<M: Material> Clone for MaterialHandle<M> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<M: Material> Copy for MaterialHandle<M> {}

pub struct UnlitMaterialBuilder {
    albedo: Option<Image>,
}

#[derive(Debug, Clone)]
pub struct UnlitMaterial {
    pub albedo: Image,
}

impl UnlitMaterialBuilder {
    pub fn build(self) -> Result<UnlitMaterial, Box<dyn Error>> {
        Ok(UnlitMaterial {
            albedo: self.albedo.ok_or("Albedo texture not provided!")?,
        })
    }

    pub fn with_albedo(self, image: Image) -> Self {
        Self {
            albedo: Some(image),
        }
    }
}

impl UnlitMaterial {
    pub fn builder() -> UnlitMaterialBuilder {
        UnlitMaterialBuilder { albedo: None }
    }
}

impl Material for UnlitMaterial {
    const NUM_IMAGES: usize = 1;
    fn images(&self) -> impl Iterator<Item = &Image> {
        [&self.albedo].into_iter()
    }
}

#[derive(Debug, Clone)]
pub struct PbrMaterial {
    albedo: Image,
    normal: Image,
    metallic_roughness: Image,
    occlusion: Image,
    emissive: Image,
}

impl PbrMaterial {
    pub fn builder() -> PbrMaterialBuilder {
        PbrMaterialBuilder {
            albedo: None,
            normal: None,
            metallic_roughness: None,
            occlusion: None,
            emissive: None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct PbrMaterialBuilder {
    pub albedo: Option<Image>,
    pub normal: Option<Image>,
    pub metallic_roughness: Option<Image>,
    pub occlusion: Option<Image>,
    pub emissive: Option<Image>,
}

impl PbrMaterialBuilder {
    pub fn build(self) -> Result<PbrMaterial, Box<dyn Error>> {
        Ok(PbrMaterial {
            albedo: self.albedo.ok_or("Albedo texture not provided!")?,
            normal: self.normal.ok_or("Normal texture not provided!")?,
            metallic_roughness: self
                .metallic_roughness
                .ok_or("MetallicRougness texture not provided!")?,
            occlusion: self.occlusion.ok_or("Occlusion texture not provided!")?,
            emissive: self.emissive.ok_or("Emissive texture not provided!")?,
        })
    }

    pub fn with_albedo(self, image: Image) -> Self {
        Self {
            albedo: Some(image),
            ..self
        }
    }

    pub fn with_normal(self, image: Image) -> Self {
        Self {
            normal: Some(image),
            ..self
        }
    }

    pub fn with_metallic_roughness(self, image: Image) -> Self {
        Self {
            metallic_roughness: Some(image),
            ..self
        }
    }

    pub fn with_emissive(self, image: Image) -> Self {
        Self {
            emissive: Some(image),
            ..self
        }
    }

    pub fn with_occlusion(self, image: Image) -> Self {
        Self {
            occlusion: Some(image),
            ..self
        }
    }
}

impl Material for PbrMaterial {
    const NUM_IMAGES: usize = 5;

    fn images(&self) -> impl Iterator<Item = &Image> {
        [
            &self.albedo,
            &self.normal,
            &self.metallic_roughness,
            &self.occlusion,
            &self.emissive,
        ]
        .into_iter()
    }
}

pub trait MaterialTypeList: 'static {
    const LEN: usize;
    type Item: Material;
    type Next: MaterialTypeList;
}

pub trait MaterialCollection: MaterialTypeList {
    fn get(&self) -> &[Self::Item];
    fn next(&self) -> &Self::Next;
}

pub struct MaterialTypeTerminator {}

impl Material for MaterialTypeTerminator {
    const NUM_IMAGES: usize = 0;
    fn images(&self) -> impl Iterator<Item = &Image> {
        [].into_iter()
    }
}

impl MaterialTypeList for MaterialTypeTerminator {
    const LEN: usize = 0;
    type Item = Self;
    type Next = Self;
}

impl MaterialCollection for MaterialTypeTerminator {
    fn get(&self) -> &[Self::Item] {
        unreachable!()
    }

    fn next(&self) -> &Self::Next {
        unreachable!()
    }
}

impl<M: Material, N: MaterialTypeList> Contains<Vec<M>, Here> for MaterialTypeNode<M, N> {
    fn get(&self) -> &Vec<M> {
        &self.materials
    }

    fn get_mut(&mut self) -> &mut Vec<M> {
        &mut self.materials
    }
}

impl<S: Material, M: Material, T: Marker, N: MaterialTypeList + Contains<Vec<M>, T>>
    Contains<Vec<M>, There<T>> for MaterialTypeNode<S, N>
{
    fn get(&self) -> &Vec<M> {
        self.next.get()
    }

    fn get_mut(&mut self) -> &mut Vec<M> {
        self.next.get_mut()
    }
}

// TODO: Resolve temporary `pub` workaround
pub struct MaterialTypeNode<M: Material, N: MaterialTypeList> {
    pub materials: Vec<M>,
    pub next: N,
}

impl<M: Material, N: MaterialTypeList> MaterialTypeList for MaterialTypeNode<M, N> {
    const LEN: usize = Self::Next::LEN + 1;
    type Item = M;
    type Next = N;
}

impl<M: Material, N: MaterialTypeList> MaterialCollection for MaterialTypeNode<M, N> {
    fn get(&self) -> &[Self::Item] {
        &self.materials
    }

    fn next(&self) -> &Self::Next {
        &self.next
    }
}

pub struct Materials<N: MaterialTypeList> {
    list: N,
    pub shaders: HashMap<TypeId, PathBuf>,
}

impl Default for Materials<MaterialTypeTerminator> {
    fn default() -> Self {
        Self::new()
    }
}

impl Materials<MaterialTypeTerminator> {
    pub fn new() -> Self {
        Self {
            list: MaterialTypeTerminator {},
            shaders: HashMap::new(),
        }
    }
}

impl<N: MaterialTypeList> Materials<N> {
    pub fn push<M: Material>(
        mut self,
        materials: Vec<M>,
        shader_path: PathBuf,
    ) -> Materials<MaterialTypeNode<M, N>> {
        self.shaders.insert(TypeId::of::<M>(), shader_path);
        Materials {
            list: MaterialTypeNode {
                materials,
                next: self.list,
            },
            shaders: self.shaders,
        }
    }
}

impl<N: MaterialTypeList> Deref for Materials<N> {
    type Target = N;

    fn deref(&self) -> &Self::Target {
        &self.list
    }
}
