use std::{
    marker::PhantomData,
    path::{Path, PathBuf},
};

use crate::{
    core::{Contains, Here, Marker, There},
    renderer::model::{EmptyMaterial, Material, Vertex, VertexNone},
};

pub trait ShaderType: 'static {
    type Vertex: Vertex;
    type Material: Material;

    fn source(&self) -> &Path;
}

pub struct Shader<V: Vertex, M: Material> {
    source: PathBuf,
    _phantom: PhantomData<(V, M)>,
}

impl<V: Vertex, M: Material> Shader<V, M> {
    pub fn marker() -> PhantomData<Self> {
        PhantomData
    }

    pub fn new(source_path: &str) -> Self {
        Self {
            source: PathBuf::from(source_path),
            _phantom: PhantomData,
        }
    }
}

impl<V: Vertex, M: Material> ShaderType for Shader<V, M> {
    type Vertex = V;
    type Material = M;

    fn source(&self) -> &Path {
        &self.source
    }
}

pub trait ShaderTypeList: 'static {
    const LEN: usize;
    type Item: ShaderType;
    type Next: ShaderTypeList;

    fn shaders(&self) -> &[Self::Item];

    fn next(&self) -> &Self::Next;
}

pub struct ShaderTypeTerminator {}

impl ShaderType for ShaderTypeTerminator {
    type Vertex = VertexNone;
    type Material = EmptyMaterial;

    fn source(&self) -> &Path {
        unreachable!()
    }
}

impl ShaderTypeList for ShaderTypeTerminator {
    const LEN: usize = 0;
    type Item = Self;
    type Next = Self;

    fn shaders(&self) -> &[Self::Item] {
        unreachable!()
    }

    fn next(&self) -> &Self::Next {
        unreachable!()
    }
}

pub struct ShaderTypeNode<S: ShaderType, N: ShaderTypeList> {
    pub shader_sources: Vec<S>,
    pub next: N,
}

impl<S: ShaderType, N: ShaderTypeList> Contains<Vec<S>, Here> for ShaderTypeNode<S, N> {
    fn get(&self) -> &Vec<S> {
        &self.shader_sources
    }

    fn get_mut(&mut self) -> &mut Vec<S> {
        &mut self.shader_sources
    }
}

impl<O: ShaderType, S: ShaderType, T: Marker, N: ShaderTypeList + Contains<Vec<S>, T>>
    Contains<Vec<S>, There<T>> for ShaderTypeNode<O, N>
{
    fn get(&self) -> &Vec<S> {
        self.next.get()
    }

    fn get_mut(&mut self) -> &mut Vec<S> {
        self.next.get_mut()
    }
}

impl<S: ShaderType, N: ShaderTypeList> ShaderTypeList for ShaderTypeNode<S, N> {
    const LEN: usize = N::LEN + 1;
    type Item = S;
    type Next = N;

    fn shaders(&self) -> &[Self::Item] {
        &self.shader_sources
    }

    fn next(&self) -> &Self::Next {
        &self.next
    }
}

#[derive(Debug)]
pub struct ShaderHandle<S: ShaderType> {
    pub index: usize,
    pub _phantom: PhantomData<S>,
}

impl<S: ShaderType> Clone for ShaderHandle<S> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<S: ShaderType> Copy for ShaderHandle<S> {}
