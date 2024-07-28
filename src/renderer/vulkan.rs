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
use crate::math::types::Matrix4;
use core::Context;

use super::{
    camera::Camera,
    model::{
        Drawable, Material, MaterialHandle, MaterialTypeNode, MaterialTypeTerminator, Materials,
        Mesh, MeshHandle, MeshNode, MeshTerminator, Meshes, Vertex,
    },
    Renderer, RendererBuilder,
};
use device::frame::Frame;
use std::{any::TypeId, collections::HashMap, error::Error, path::PathBuf};
use winit::window::Window;

pub struct VulkanRendererBuilder<M: MaterialPackListBuilder, V: MeshPackListBuilder> {
    materials: Materials<M>,
    meshes: Meshes<V>,
}

impl Default for VulkanRendererBuilder<MaterialTypeTerminator, MeshTerminator> {
    fn default() -> Self {
        Self::new()
    }
}

impl VulkanRendererBuilder<MaterialTypeTerminator, MeshTerminator> {
    pub fn new() -> Self {
        Self {
            materials: Materials::new(),
            meshes: Meshes::new(),
        }
    }
}

impl<M: MaterialPackListBuilder, V: MeshPackListBuilder> VulkanRendererBuilder<M, V> {
    pub fn with_materials<N: Material>(
        self,
        materials: Vec<N>,
        shader_path: PathBuf,
    ) -> VulkanRendererBuilder<MaterialTypeNode<N, M>, V> {
        VulkanRendererBuilder {
            materials: self.materials.push(materials, shader_path),
            meshes: self.meshes,
        }
    }

    pub fn with_meshes<N: Vertex>(
        self,
        meshes: Vec<Mesh<N>>,
    ) -> VulkanRendererBuilder<M, MeshNode<N, V>> {
        VulkanRendererBuilder {
            materials: self.materials,
            meshes: self.meshes.push(meshes),
        }
    }
}

impl<M: MaterialPackListBuilder, V: MeshPackListBuilder> RendererBuilder
    for VulkanRendererBuilder<M, V>
{
    type Renderer = VulkanRenderer<M::Pack, V::Pack>;

    fn build(self, window: &Window) -> Result<Self::Renderer, Box<dyn Error>> {
        let renderer = VulkanRenderer::new(
            window,
            &*self.materials,
            &*self.meshes,
            &self.materials.shaders,
        )?;
        Ok(renderer)
    }
}

pub struct VulkanRenderer<M: MaterialPackList, V: MeshPackList> {
    materials: MaterialPacks<M>,
    meshes: MeshPacks<V>,
    renderer: DeferredRenderer<M>,
    context: Context,
}

// TODO: Error handling should be improved - currently when shader source files are missing,
// execution ends with panic! while dropping HostMappedMemory of UniforBuffer structure
// while error message indicating true cause of the issue is never presented to the user
// TODO: User should be able to load custom shareds,
// while also some preset of preconfigured one should be available
// API for user-defined shaders should be based on PipelineLayoutBuilder type-list
impl<M: MaterialPackList, V: MeshPackList> VulkanRenderer<M, V> {
    pub fn new(
        window: &Window,
        materials: &impl MaterialPackListBuilder<Pack = M>,
        meshes: &impl MeshPackListBuilder<Pack = V>,
        shaders: &HashMap<TypeId, PathBuf>,
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

impl<M: MaterialPackList, V: MeshPackList> Drop for VulkanRenderer<M, V> {
    fn drop(&mut self) {
        let _ = self.context.wait_idle();
        self.context.destroy_materials(&mut self.materials);
        self.context.destroy_meshes(&mut self.meshes);
        self.context.destroy_deferred_renderer(&mut self.renderer);
    }
}

impl<M: MaterialPackList, V: MeshPackList> Renderer for VulkanRenderer<M, V> {
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

    fn draw<D: Drawable>(
        &mut self,
        drawable: &D,
        transform: &Matrix4,
    ) -> Result<(), Box<dyn Error>> {
        self.renderer.draw(
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
}
