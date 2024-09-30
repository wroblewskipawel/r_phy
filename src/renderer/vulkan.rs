mod core;
mod debug;
mod device;
mod surface;

use self::device::resources::{
    MaterialPackList, MaterialPackListBuilder, MaterialPackListPartial, MeshPackList,
    MeshPackListBuilder, MeshPackListPartial,
};
use crate::{
    core::{Cons, Contains, Marker, Nil},
    math::types::Matrix4,
};
use core::Context;

use super::{
    camera::Camera,
    model::{Drawable, Material, Mesh, Vertex},
    shader::{ShaderHandle, ShaderType},
    Renderer, RendererBuilder, RendererContext, RendererContextBuilder,
};
use ash::vk;
use device::{
    frame::{Frame, FrameContext},
    memory::{AllocatorCreate, DefaultAllocator, StaticAllocator, StaticAllocatorConfig},
    pipeline::{GraphicsPipelineListBuilder, GraphicsPipelinePackList},
    renderer::deferred::DeferredRenderer,
};
use std::{cell::RefCell, error::Error, marker::PhantomData, rc::Rc};
use winit::window::Window;

pub use device::renderer::deferred::DeferredShader;

#[derive(Debug, Clone, Copy)]
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
    resources: PhantomData<(M, V, S)>,
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
            resources: PhantomData,
            config: None,
        }
    }
}

impl<M: MaterialPackListBuilder, V: MeshPackListBuilder, S: GraphicsPipelineListBuilder>
    VulkanRendererBuilder<M, V, S>
{
    pub fn with_material_type<N: Material>(self) -> VulkanRendererBuilder<Cons<Vec<N>, M>, V, S> {
        let Self { config, .. } = self;
        VulkanRendererBuilder {
            resources: PhantomData,
            config,
        }
    }

    pub fn with_vertex_type<N: Vertex>(self) -> VulkanRendererBuilder<M, Cons<Vec<Mesh<N>>, V>, S> {
        let Self { config, .. } = self;
        VulkanRendererBuilder {
            resources: PhantomData,
            config,
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
        let Self { config, .. } = self;
        VulkanRendererBuilder {
            resources: PhantomData,
            config,
        }
    }

    pub fn with_config(mut self, config: VulkanRendererConfig) -> Self {
        self.config = Some(config);
        self
    }
}

impl<
        M: MaterialPackListBuilder + Default,
        V: MeshPackListBuilder + Default,
        S: GraphicsPipelineListBuilder + Default,
    > RendererBuilder for VulkanRendererBuilder<M, V, S>
{
    type Renderer = VulkanRenderer<M, V, S>;

    fn build(self, window: &Window) -> Result<Self::Renderer, Box<dyn Error>> {
        let renderer =
            VulkanRenderer::new(window, self.config.ok_or("Configuration not provided")?)?;
        Ok(renderer)
    }
}

pub struct VulkanRenderer<
    M: MaterialPackListBuilder,
    V: MeshPackListBuilder,
    S: GraphicsPipelineListBuilder,
> {
    context: Rc<RefCell<Context>>,
    renderer: Rc<RefCell<DeferredRenderer<DefaultAllocator>>>,
    _config: VulkanRendererConfig,
    _phantom: PhantomData<(M, V, S)>,
}

impl<M: MaterialPackListBuilder, V: MeshPackListBuilder, S: GraphicsPipelineListBuilder> Drop
    for VulkanRenderer<M, V, S>
{
    fn drop(&mut self) {
        let context = self.context.borrow();
        let _ = context.wait_idle();
        let mut renderer = self.renderer.borrow_mut();
        context.destroy_deferred_renderer(&mut renderer, &mut DefaultAllocator {});
    }
}

pub struct VulkanResourcePack<
    R: Frame,
    M: MaterialPackList<StaticAllocator>,
    V: MeshPackList<StaticAllocator>,
    S: GraphicsPipelinePackList,
> {
    materials: M,
    meshes: V,
    renderer_context: R::Context<S>,
    allocator: StaticAllocator,
}

impl<
        R: Frame,
        M: MaterialPackList<StaticAllocator>,
        V: MeshPackList<StaticAllocator>,
        S: GraphicsPipelinePackList,
    > VulkanResourcePack<R, M, V, S>
{
    fn load(
        context: &mut Context,
        renderer: &R,
        materials: &impl MaterialPackListBuilder<Pack<StaticAllocator> = M>,
        meshes: &impl MeshPackListBuilder<Pack<StaticAllocator> = V>,
        pipelines: &impl GraphicsPipelineListBuilder<Pack = S>,
    ) -> Result<Self, Box<dyn Error>> {
        let mut config = StaticAllocatorConfig::create(&context);
        let meshes = meshes.prepare(&context)?;
        meshes
            .get_memory_requirements()
            .into_iter()
            .for_each(|req| config.add_allocation(req));
        let materials = materials.prepare(&context)?;
        materials
            .get_memory_requirements()
            .into_iter()
            .for_each(|req| config.add_allocation(req));
        let mut allocator = StaticAllocator::create(&context, &config)?;
        let materials = materials.allocate(&context, &mut allocator)?;
        let meshes = meshes.allocate(&context, &mut allocator)?;
        let renderer_context = renderer.load_context(&context, pipelines)?;
        Ok(Self {
            materials,
            meshes,
            renderer_context,
            allocator,
        })
    }

    fn destroy(&mut self, context: &Context) {
        context.destroy_materials(&mut self.materials, &mut self.allocator);
        context.destroy_meshes(&mut self.meshes, &mut self.allocator);
        R::destroy_context(&mut self.renderer_context, &context);
        self.allocator.destroy(&context)
    }
}

pub struct VulkanRendererContext<
    R: Frame,
    M: MaterialPackList<StaticAllocator>,
    V: MeshPackList<StaticAllocator>,
    S: GraphicsPipelinePackList,
> {
    context: Rc<RefCell<Context>>,
    resources: VulkanResourcePack<R, M, V, S>,
}

// TODO: Error handling should be improved - currently when shader source files are missing,
// execution ends with panic! while dropping HostMappedMemory of UniforBuffer structure
// while error message indicating true cause of the issue is never presented to the user
// TODO: User should be able to load custom shareds,
// while also some preset of preconfigured one should be available
// API for user-defined shaders should be based on PipelineLayoutBuilder type-list
impl<M: MaterialPackListBuilder, V: MeshPackListBuilder, S: GraphicsPipelineListBuilder>
    VulkanRenderer<M, V, S>
{
    pub fn new(window: &Window, config: VulkanRendererConfig) -> Result<Self, Box<dyn Error>> {
        let context = Context::build(window)?;
        let renderer = context.create_deferred_renderer(&mut DefaultAllocator {})?;
        Ok(Self {
            context: Rc::new(RefCell::new(context)),
            renderer: Rc::new(RefCell::new(renderer)),
            _config: config,
            _phantom: PhantomData,
        })
    }
}

impl<
        R: Frame,
        M: MaterialPackList<StaticAllocator>,
        V: MeshPackList<StaticAllocator>,
        S: GraphicsPipelinePackList,
    > Drop for VulkanRendererContext<R, M, V, S>
{
    fn drop(&mut self) {
        let context = self.context.borrow();
        let _ = self.context.borrow().wait_idle();
        self.resources.destroy(&context);
    }
}

impl<
        M: MaterialPackListBuilder + Default,
        V: MeshPackListBuilder + Default,
        S: GraphicsPipelineListBuilder + Default,
    > Renderer for VulkanRenderer<M, V, S>
{
    type Builder = RendererContextBuilder<S, M, V>;
    type Context = VulkanRendererContext<
        Rc<RefCell<DeferredRenderer<DefaultAllocator>>>,
        M::Pack<StaticAllocator>,
        V::Pack<StaticAllocator>,
        S::Pack,
    >;

    fn load_context(&mut self, builder: Self::Builder) -> Result<Self::Context, Box<dyn Error>> {
        let mut context = self.context.borrow_mut();
        let resources = VulkanResourcePack::load(
            &mut context,
            &self.renderer,
            &builder.materials,
            &builder.meshes,
            &builder.shaders,
        )?;
        Ok(VulkanRendererContext {
            context: self.context.clone(),
            resources,
        })
    }
}

impl<
        R: Frame,
        M: MaterialPackList<StaticAllocator>,
        V: MeshPackList<StaticAllocator>,
        S: GraphicsPipelinePackList,
    > RendererContext for VulkanRendererContext<R, M, V, S>
{
    type Shaders = S;
    type Materials = M;
    type Meshes = V;

    fn begin_frame<C: Camera>(&mut self, camera: &C) -> Result<(), Box<dyn Error>> {
        let context = self.context.borrow();
        let camera_matrices = camera.get_matrices();
        self.resources
            .renderer_context
            .begin_frame(&context, &camera_matrices)?;
        Ok(())
    }

    fn end_frame(&mut self) -> Result<(), Box<dyn Error>> {
        let context = self.context.borrow();
        self.resources.renderer_context.end_frame(&context)?;
        Ok(())
    }

    fn draw<T: ShaderType, D: Drawable<Material = T::Material, Vertex = T::Vertex>>(
        &mut self,
        shader: ShaderHandle<T>,
        drawable: &D,
        transform: &Matrix4,
    ) -> Result<(), Box<dyn Error>> {
        self.resources.renderer_context.draw(
            shader,
            drawable,
            transform,
            &self.resources.materials,
            &self.resources.meshes,
        );
        Ok(())
    }
}
