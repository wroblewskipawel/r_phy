mod core;
mod debug;
mod device;
mod surface;

use self::device::{
    frame::{FrameData, FramePool},
    framebuffer::presets::AttachmentsGBuffer,
    render_pass::DeferedRenderPass,
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
use device::{renderer::deferred::GBuffer, swapchain::VulkanSwapchain};
use std::{any::TypeId, collections::HashMap, error::Error, path::PathBuf};
use winit::window::Window;

pub struct VulkanRendererBuilder<M: MaterialPackListBuilder, V: MeshPackListBuilder> {
    materials: Materials<M>,
    meshes: Meshes<V>,
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
    current_frame: Option<FrameData<DeferredRenderer<M>>>,
    materials: MaterialPacks<M>,
    meshes: MeshPacks<V>,
    frames: FramePool,
    g_buffer: GBuffer,
    renderer: DeferredRenderer<M>,
    swapchain: VulkanSwapchain<AttachmentsGBuffer>,
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
        // TODO: Here GBuffer creation is moved out of the DeferredRenderer for simplicity sake,
        //       nonetheless it should be moved back (DeferredRendere should own all of its required resources)
        let g_buffer = context.create_g_buffer()?;
        let swapchain = context.create_swapchain::<AttachmentsGBuffer>(
            (&context).into(),
            (&context).into(),
            |swapchain_image, extent| {
                context.build_framebuffer::<DeferedRenderPass<AttachmentsGBuffer>>(
                    g_buffer.get_framebuffer_builder(swapchain_image),
                    extent,
                )
            },
        )?;
        let renderer = context.create_deferred_renderer(&swapchain, shaders)?;
        // TODO: Why frame pool is not typed with DeferredRenderer<M> (FramePool<DeferredRenderer<M>>)?
        let frames = context.create_frame_pool::<DeferredRenderer<M>>(&swapchain)?;
        let materials = context.load_materials(materials)?;
        let meshes = context.load_meshes(meshes)?;
        Ok(Self {
            current_frame: None,
            materials,
            meshes,
            frames,
            g_buffer,
            renderer,
            swapchain,
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
        self.context.destroy_g_buffer(&mut self.g_buffer);
        self.context.destory_frame_pool(&mut self.frames);
        self.context.destroy_swapchain(&mut self.swapchain);
    }
}

impl<M: MaterialPackList, V: MeshPackList> Renderer for VulkanRenderer<M, V> {
    type Materials = M;
    type Meshes = V;

    fn begin_frame<C: Camera>(&mut self, camera: &C) -> Result<(), Box<dyn Error>> {
        let camera_matrices = camera.get_matrices();
        let frame = self.context.next_frame(
            &mut self.frames,
            &self.renderer,
            &self.swapchain,
            camera_matrices,
        )?;
        self.current_frame.replace(frame);
        Ok(())
    }

    fn end_frame(&mut self) -> Result<(), Box<dyn Error>> {
        let frame = self.current_frame.take().ok_or("current_frame is None!")?;
        self.context
            .end_frame(&self.renderer, frame, &self.swapchain)?;
        Ok(())
    }

    fn draw<D: Drawable>(
        &mut self,
        drawable: &D,
        transform: &Matrix4,
    ) -> Result<(), Box<dyn Error>> {
        let material = drawable.material();
        let mesh = drawable.mesh();
        let frame = self.current_frame.take().ok_or("current_frame is None!")?;
        let frame = self.context.draw_mesh(
            &self.renderer,
            frame,
            transform,
            mesh.into(),
            material.into(),
            &self.meshes.packs,
            &self.materials.packs,
        );
        self.current_frame.replace(frame);
        Ok(())
    }

    fn get_mesh_handles<T: Vertex>(&self) -> Option<Vec<MeshHandle<T>>> {
        Some(self.meshes.packs.try_get()?.get_handles())
    }

    fn get_material_handles<T: Material>(&self) -> Option<Vec<MaterialHandle<T>>> {
        Some(self.materials.packs.try_get()?.get_handles())
    }
}
