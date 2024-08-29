mod core;
mod debug;
mod device;
mod surface;

use self::device::{
    renderer::deferred::DeferredRenderer,
    resources::{
        MaterialPackList, MaterialPackListBuilder, MaterialPacks, MeshPackList,
        MeshPackListBuilder, MeshPacks,
    },
};
use crate::{
    core::{Contains, Marker},
    math::types::Matrix4,
};
use core::Context;

use super::{
    camera::Camera,
    model::{
        Drawable, Material, MaterialHandle, MaterialTypeNode, MaterialTypeTerminator, Mesh,
        MeshHandle, MeshNode, MeshTerminator, Vertex,
    },
    shader::{ShaderHandle, ShaderType, ShaderTypeList, ShaderTypeNode, ShaderTypeTerminator},
    Renderer, RendererBuilder,
};
use device::frame::Frame;
use std::{error::Error, marker::PhantomData};
use winit::window::Window;

pub struct VulkanRendererBuilder<
    M: MaterialPackListBuilder,
    V: MeshPackListBuilder,
    S: ShaderTypeList,
> {
    materials: M,
    meshes: V,
    shaders: S,
}

impl Default
    for VulkanRendererBuilder<MaterialTypeTerminator, MeshTerminator, ShaderTypeTerminator>
{
    fn default() -> Self {
        Self::new()
    }
}

impl VulkanRendererBuilder<MaterialTypeTerminator, MeshTerminator, ShaderTypeTerminator> {
    pub fn new() -> Self {
        Self {
            materials: MaterialTypeTerminator {},
            meshes: MeshTerminator {},
            shaders: ShaderTypeTerminator {},
        }
    }
}

impl<M: MaterialPackListBuilder, V: MeshPackListBuilder, S: ShaderTypeList>
    VulkanRendererBuilder<M, V, S>
{
    pub fn with_material_type<N: Material>(
        self,
    ) -> VulkanRendererBuilder<MaterialTypeNode<N, M>, V, S> {
        let Self {
            materials,
            meshes,
            shaders,
        } = self;
        VulkanRendererBuilder {
            materials: MaterialTypeNode {
                materials: vec![],
                next: materials,
            },
            meshes,
            shaders,
        }
    }

    pub fn with_vertex_type<N: Vertex>(self) -> VulkanRendererBuilder<M, MeshNode<N, V>, S> {
        let Self {
            materials,
            meshes,
            shaders,
        } = self;
        VulkanRendererBuilder {
            meshes: MeshNode {
                meshes: vec![],
                next: meshes,
            },
            materials,
            shaders,
        }
    }

    pub fn with_shader_type<N: ShaderType, T: Marker, O: Marker>(
        self,
        _shader_type: PhantomData<N>,
    ) -> VulkanRendererBuilder<M, V, ShaderTypeNode<N, S>>
    where
        M: Contains<Vec<N::Material>, T>,
        V: Contains<Vec<Mesh<N::Vertex>>, O>,
    {
        let Self {
            materials,
            meshes,
            shaders,
        } = self;
        VulkanRendererBuilder {
            shaders: ShaderTypeNode {
                shader_sources: vec![],
                next: shaders,
            },
            materials,
            meshes,
        }
    }

    pub fn with_meshes<N: Vertex, T: Marker>(mut self, meshes: Vec<Mesh<N>>) -> Self
    where
        V: Contains<Vec<Mesh<N>>, T>,
    {
        self.meshes.get_mut().extend(meshes);
        self
    }

    pub fn with_materials<N: Material, T: Marker>(mut self, materials: Vec<N>) -> Self
    where
        M: Contains<Vec<N>, T>,
    {
        self.materials.get_mut().extend(materials);
        self
    }

    pub fn with_shaders<N: ShaderType, T: Marker>(mut self, shaders: Vec<N>) -> Self
    where
        S: Contains<Vec<N>, T>,
    {
        self.shaders.get_mut().extend(shaders);
        self
    }
}

impl<M: MaterialPackListBuilder, V: MeshPackListBuilder, S: ShaderTypeList> RendererBuilder
    for VulkanRendererBuilder<M, V, S>
{
    type Renderer = VulkanRenderer<M::Pack, V::Pack, S>;

    fn build(self, window: &Window) -> Result<Self::Renderer, Box<dyn Error>> {
        let renderer = VulkanRenderer::new(window, &self.materials, &self.meshes, &self.shaders)?;
        Ok(renderer)
    }
}

pub struct VulkanRenderer<M: MaterialPackList, V: MeshPackList, S: ShaderTypeList> {
    materials: MaterialPacks<M>,
    meshes: MeshPacks<V>,
    renderer: DeferredRenderer<S>,
    context: Context,
}

// TODO: Error handling should be improved - currently when shader source files are missing,
// execution ends with panic! while dropping HostMappedMemory of UniforBuffer structure
// while error message indicating true cause of the issue is never presented to the user
// TODO: User should be able to load custom shareds,
// while also some preset of preconfigured one should be available
// API for user-defined shaders should be based on PipelineLayoutBuilder type-list
impl<M: MaterialPackList, V: MeshPackList, S: ShaderTypeList> VulkanRenderer<M, V, S> {
    pub fn new(
        window: &Window,
        materials: &impl MaterialPackListBuilder<Pack = M>,
        meshes: &impl MeshPackListBuilder<Pack = V>,
        shaders: &S,
    ) -> Result<Self, Box<dyn Error>> {
        let context = Context::build(window)?;
        let renderer = context.create_deferred_renderer(shaders)?;
        let materials = context.load_materials(materials)?;
        let meshes = context.load_meshes(meshes)?;
        Ok(Self {
            materials,
            meshes,
            renderer,
            context,
        })
    }
}

impl<M: MaterialPackList, V: MeshPackList, S: ShaderTypeList> Drop for VulkanRenderer<M, V, S> {
    fn drop(&mut self) {
        let _ = self.context.wait_idle();
        self.context.destroy_materials(&mut self.materials);
        self.context.destroy_meshes(&mut self.meshes);
        self.context.destroy_deferred_renderer(&mut self.renderer);
    }
}

impl<M: MaterialPackList, V: MeshPackList, S: ShaderTypeList> Renderer for VulkanRenderer<M, V, S> {
    type Materials = M;
    type Meshes = V;

    fn begin_frame<C: Camera>(&mut self, camera: &C) -> Result<(), Box<dyn Error>> {
        let camera_matrices = camera.get_matrices();
        self.renderer.begin_frame(&self.context, &camera_matrices)?;
        Ok(())
    }

    fn end_frame(&mut self) -> Result<(), Box<dyn Error>> {
        self.renderer.end_frame(&self.context)?;
        Ok(())
    }

    fn draw<T: ShaderType, D: Drawable<Material = T::Material, Vertex = T::Vertex>>(
        &mut self,
        shader: ShaderHandle<T>,
        drawable: &D,
        transform: &Matrix4,
    ) -> Result<(), Box<dyn Error>> {
        self.renderer.draw(
            shader,
            drawable,
            transform,
            &self.materials.packs,
            &self.meshes.packs,
        );
        Ok(())
    }

    fn get_mesh_handles<T: Vertex>(&self) -> Option<Vec<MeshHandle<T>>> {
        Some(self.meshes.packs.try_get()?.get_handles())
    }

    fn get_material_handles<T: Material>(&self) -> Option<Vec<MaterialHandle<T>>> {
        Some(self.materials.packs.try_get()?.get_handles())
    }

    fn get_shader_handles<T: ShaderType>(&self) -> Option<Vec<ShaderHandle<T>>> {
        self.renderer.get_shader_handles()
    }
}
