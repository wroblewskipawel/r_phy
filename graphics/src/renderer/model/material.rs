use core::slice;
use std::{
    any::TypeId, collections::HashMap, error::Error, marker::PhantomData, ops::Deref, path::PathBuf,
};

use bytemuck::AnyBitPattern;

use math::types::{Vector3, Vector4};
use type_list::{Cons, Nil, TypedNil};

#[allow(dead_code)]
pub const fn has_data<T: Material>() -> bool {
    T::NUM_IMAGES != 0 || size_of::<T::Uniform>() != 0
}

pub trait Material: 'static {
    const NUM_IMAGES: usize;
    type Uniform: Clone + Copy + AnyBitPattern;

    fn images(&self) -> Option<impl Iterator<Item = &Image>>;
    fn uniform(&self) -> Option<&Self::Uniform>;
}

#[derive(Debug, Clone)]
pub enum Image {
    Buffer(Vec<u8>),
    File(PathBuf),
}

#[derive(Debug)]
pub struct MaterialHandle<M: Material> {
    index: u32,
    _phantom: PhantomData<M>,
}

impl<M: Material> Clone for MaterialHandle<M> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<M: Material> Copy for MaterialHandle<M> {}

impl<M: Material> MaterialHandle<M> {
    pub fn new(index: u32) -> Self {
        Self {
            index,
            _phantom: PhantomData,
        }
    }

    pub fn index(&self) -> u32 {
        self.index
    }
}

pub struct UnlitMaterialBuilder {
    albedo: Option<Image>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct EmptyMaterial {}

impl Material for EmptyMaterial {
    const NUM_IMAGES: usize = 0;
    type Uniform = ();

    fn images(&self) -> Option<impl Iterator<Item = &Image>> {
        Option::<slice::Iter<Image>>::None
    }

    fn uniform(&self) -> Option<&Self::Uniform> {
        None
    }
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
    type Uniform = ();

    fn images(&self) -> Option<impl Iterator<Item = &Image>> {
        Some([&self.albedo].into_iter())
    }
    fn uniform(&self) -> Option<&Self::Uniform> {
        None
    }
}

#[derive(Debug, Clone)]
pub enum PbrMaps {
    Albedo,
    Normal,
    MetallicRoughness,
    Occlusion,
    Emissive,
}

#[repr(C, align(16))]
#[derive(Debug, Clone, Copy, Default, AnyBitPattern)]
pub struct PbrFactors {
    pub base_color: Vector4,
    pub emissive: Vector3,
    _padding: f32,
    pub metallic: f32,
    pub roughness: f32,
    pub occlusion: f32,
}

#[derive(Debug, Clone)]
pub struct PbrImages {
    images: [Image; 5],
}

#[derive(Debug, Clone)]
pub struct PbrMaterial {
    images: PbrImages,
    factors: PbrFactors,
}

impl PbrMaterial {
    pub fn builder() -> PbrMaterialBuilder {
        PbrMaterialBuilder {
            images: Default::default(),
            factors: Default::default(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct PbrMaterialBuilder {
    images: [Option<Image>; 5],
    factors: PbrFactors,
}

impl PbrMaterialBuilder {
    pub fn build(self) -> Result<PbrMaterial, Box<dyn Error>> {
        let Self {
            images: [albedo, normal, metallic_roughness, occlusion, emissive],
            factors,
        } = self;
        Ok(PbrMaterial {
            images: PbrImages {
                images: [
                    albedo.ok_or("Albedo texture not provided!")?,
                    normal.ok_or("Normal texture not provided!")?,
                    metallic_roughness.ok_or("Metallic-roughness texture not provided!")?,
                    occlusion.ok_or("Occlusion texture not provided!")?,
                    emissive.ok_or("Emissive texture not provided!")?,
                ],
            },
            factors,
        })
    }

    pub fn with_image(mut self, image: Image, map: PbrMaps) -> Self {
        self.images[map as usize] = Some(image);
        self
    }

    pub fn with_base_color(mut self, base_color: Vector4) -> Self {
        self.factors.base_color = base_color;
        self
    }

    pub fn with_metallic(mut self, metallic: f32) -> Self {
        self.factors.metallic = metallic;
        self
    }

    pub fn with_roughness(mut self, roughness: f32) -> Self {
        self.factors.roughness = roughness;
        self
    }

    pub fn with_occlusion(mut self, occlusion: f32) -> Self {
        self.factors.occlusion = occlusion;
        self
    }

    pub fn with_emissive(mut self, emissive: Vector3) -> Self {
        self.factors.emissive = emissive;
        self
    }
}

impl Material for PbrMaterial {
    const NUM_IMAGES: usize = 5;
    type Uniform = PbrFactors;

    fn images(&self) -> Option<impl Iterator<Item = &Image>> {
        Some(self.images.images.as_slice().into_iter())
    }

    fn uniform(&self) -> Option<&Self::Uniform> {
        Some(&self.factors)
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

impl MaterialTypeList for Nil {
    const LEN: usize = 0;
    type Item = EmptyMaterial;
    type Next = Self;
}

impl<T: 'static> MaterialTypeList for TypedNil<T> {
    const LEN: usize = 0;
    type Item = EmptyMaterial;
    type Next = Self;
}

impl MaterialCollection for Nil {
    fn get(&self) -> &[Self::Item] {
        unreachable!()
    }

    fn next(&self) -> &Self::Next {
        unreachable!()
    }
}

impl<M: Material, N: MaterialTypeList> MaterialTypeList for Cons<Vec<M>, N> {
    const LEN: usize = Self::Next::LEN + 1;
    type Item = M;
    type Next = N;
}

impl<M: Material, N: MaterialTypeList> MaterialCollection for Cons<Vec<M>, N> {
    fn get(&self) -> &[Self::Item] {
        &self.head
    }

    fn next(&self) -> &Self::Next {
        &self.tail
    }
}

pub struct Materials<N: MaterialTypeList> {
    list: N,
    pub shaders: HashMap<TypeId, PathBuf>,
}

impl Default for Materials<Nil> {
    fn default() -> Self {
        Self::new()
    }
}

impl Materials<Nil> {
    pub fn new() -> Self {
        Self {
            list: Nil {},
            shaders: HashMap::new(),
        }
    }
}

impl<N: MaterialTypeList> Materials<N> {
    pub fn push<M: Material>(
        mut self,
        materials: Vec<M>,
        shader_path: PathBuf,
    ) -> Materials<Cons<Vec<M>, N>> {
        self.shaders.insert(TypeId::of::<M>(), shader_path);
        Materials {
            list: Cons {
                head: materials,
                tail: self.list,
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
