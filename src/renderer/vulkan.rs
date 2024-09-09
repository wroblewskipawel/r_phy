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
    core::{Cons, Contains, Marker, Nil},
    math::types::Matrix4,
};
use core::Context;

use super::{
    camera::Camera,
    model::{Drawable, Material, MaterialHandle, Mesh, MeshHandle, Vertex},
    shader::{ShaderHandle, ShaderType},
    Renderer, RendererBuilder,
};
use ash::vk;
use device::{
    frame::Frame,
    pipeline::{GraphicsPipelineListBuilder, GraphicsPipelinePackList},
    renderer::deferred::DeferredShader,
};
use std::{error::Error, marker::PhantomData};
use winit::window::Window;

#[derive(Debug)]
pub struct VulkanRendererConfig {
    pub page_size: vk::DeviceSize,
}

impl VulkanRendererConfig {
    pub fn builder() -> VulkanRendererConfigBuilder {
        VulkanRendererConfigBuilder::default()
    }
}

#[derive(Debug, Default)]
pub struct VulkanRendererConfigBuilder {
    page_size: Option<vk::DeviceSize>,
}

impl VulkanRendererConfigBuilder {
    pub fn build(self) -> Result<VulkanRendererConfig, Box<dyn Error>> {
        let config = VulkanRendererConfig {
            page_size: self.page_size.ok_or("Page size not provided")?,
        };
        Ok(config)
    }

    pub fn with_page_size(mut self, size: vk::DeviceSize) -> Self {
        self.page_size = Some(size);
        self
    }
}

pub struct VulkanRendererBuilder<
    M: MaterialPackListBuilder,
    V: MeshPackListBuilder,
    S: GraphicsPipelineListBuilder,
> {
    materials: M,
    meshes: V,
    shaders: S,
    config: Option<VulkanRendererConfig>,
}

impl Default for VulkanRendererBuilder<Nil, Nil, Nil> {
    fn default() -> Self {
        Self::new()
    }
}

impl VulkanRendererBuilder<Nil, Nil, Nil> {
    pub fn new() -> Self {
        Self {
            materials: Nil {},
            meshes: Nil {},
            shaders: Nil {},
            config: None,
        }
    }
}

impl<M: MaterialPackListBuilder, V: MeshPackListBuilder, S: GraphicsPipelineListBuilder>
    VulkanRendererBuilder<M, V, S>
{
    pub fn with_material_type<N: Material>(self) -> VulkanRendererBuilder<Cons<Vec<N>, M>, V, S> {
        let Self {
            materials,
            meshes,
            shaders,
            config: configuration,
        } = self;
        VulkanRendererBuilder {
            materials: Cons {
                head: vec![],
                tail: materials,
            },
            meshes,
            shaders,
            config: configuration,
        }
    }

    pub fn with_vertex_type<N: Vertex>(self) -> VulkanRendererBuilder<M, Cons<Vec<Mesh<N>>, V>, S> {
        let Self {
            materials,
            meshes,
            shaders,
            config: configuration,
        } = self;
        VulkanRendererBuilder {
            meshes: Cons {
                head: vec![],
                tail: meshes,
            },
            materials,
            shaders,
            config: configuration,
        }
    }

    pub fn with_shader_type<N: ShaderType, T: Marker, O: Marker>(
        self,
        _shader_type: PhantomData<N>,
    ) -> VulkanRendererBuilder<M, V, Cons<Vec<DeferredShader<N>>, S>>
    where
        M: Contains<Vec<N::Material>, T>,
        V: Contains<Vec<Mesh<N::Vertex>>, O>,
    {
        let Self {
            materials,
            meshes,
            shaders,
            config: configuration,
        } = self;
        VulkanRendererBuilder {
            shaders: Cons {
                head: vec![],
                tail: shaders,
            },
            materials,
            meshes,
            config: configuration,
        }
    }

    pub fn with_config(mut self, config: VulkanRendererConfig) -> Self {
        self.config = Some(config);
        self
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
        S: Contains<Vec<DeferredShader<N>>, T>,
    {
        self.shaders
            .get_mut()
            .extend(shaders.into_iter().map(Into::<DeferredShader<N>>::into));
        self
    }
}

impl<M: MaterialPackListBuilder, V: MeshPackListBuilder, S: GraphicsPipelineListBuilder>
    RendererBuilder for VulkanRendererBuilder<M, V, S>
{
    type Renderer = VulkanRenderer<M::Pack, V::Pack, S::Pack>;

    fn build(self, window: &Window) -> Result<Self::Renderer, Box<dyn Error>> {
        let renderer = VulkanRenderer::new(
            window,
            &self.materials,
            &self.meshes,
            &self.shaders,
            &self.config.ok_or("Configuration not provided")?,
        )?;
        Ok(renderer)
    }
}

pub struct VulkanRenderer<M: MaterialPackList, V: MeshPackList, S: GraphicsPipelinePackList> {
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
impl<M: MaterialPackList, V: MeshPackList, S: GraphicsPipelinePackList> VulkanRenderer<M, V, S> {
    pub fn new(
        window: &Window,
        materials: &impl MaterialPackListBuilder<Pack = M>,
        meshes: &impl MeshPackListBuilder<Pack = V>,
        shaders: &impl GraphicsPipelineListBuilder<Pack = S>,
        config: &VulkanRendererConfig,
    ) -> Result<Self, Box<dyn Error>> {
        let mut context = Context::build(window, config)?;
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

impl<M: MaterialPackList, V: MeshPackList, S: GraphicsPipelinePackList> Drop
    for VulkanRenderer<M, V, S>
{
    fn drop(&mut self) {
        let _ = self.context.wait_idle();
        self.context.destroy_materials(&mut self.materials);
        self.context.destroy_meshes(&mut self.meshes);
        self.context.destroy_deferred_renderer(&mut self.renderer);
    }
}

impl<M: MaterialPackList, V: MeshPackList, S: GraphicsPipelinePackList> Renderer
    for VulkanRenderer<M, V, S>
{
    type Shaders = S;
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
