mod core;
mod debug;
mod device;
mod surface;

use self::device::{
    renderer::deferred::DeferredRenderer,
    resources::{
        MaterialPackList, MaterialPackListBuilder, MaterialPackListPartial, MeshPackList,
        MeshPackListBuilder, MeshPackListPartial,
    },
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
    frame::Frame,
    memory::{Allocator, HostCoherent, HostVisibleMemory, StaticStackAllocator},
    pipeline::{GraphicsPipelineListBuilder, GraphicsPipelinePackList},
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
    // TODO: Temporary RefCell to allow for shareable mutable reference
    context: Rc<RefCell<Context>>,
    config: VulkanRendererConfig,
    _phantom: PhantomData<(M, V, S)>,
}

pub struct VulkanResourcePack<
    A: Allocator,
    M: MaterialPackList<A>,
    V: MeshPackList<A>,
    S: GraphicsPipelinePackList,
> {
    materials: M,
    meshes: V,
    renderer: DeferredRenderer<A, S>,
    allocator: A,
}

impl<A: Allocator, M: MaterialPackList<A>, V: MeshPackList<A>, S: GraphicsPipelinePackList>
    VulkanResourcePack<A, M, V, S>
where
    A::Allocation<HostCoherent>: HostVisibleMemory,
{
    fn load(
        context: &mut Context,
        config: &VulkanRendererConfig,
        materials: &impl MaterialPackListBuilder<Pack<A> = M>,
        meshes: &impl MeshPackListBuilder<Pack<A> = V>,
        renderer: &impl GraphicsPipelineListBuilder<Pack = S>,
    ) -> Result<Self, Box<dyn Error>> {
        let mut allocator = A::new(&context, &config);
        let materials = materials
            .prepare(&context)?
            .allocate(&context, &mut allocator)?;
        let meshes = meshes
            .prepare(&context)?
            .allocate(&context, &mut allocator)?;
        let renderer = context.create_deferred_renderer(&mut allocator, renderer)?;
        Ok(Self {
            materials,
            meshes,
            renderer,
            allocator,
        })
    }

    fn destroy(&mut self, context: &Context) {
        context.destroy_materials(&mut self.materials, &mut self.allocator);
        context.destroy_meshes(&mut self.meshes, &mut self.allocator);
        context.destroy_deferred_renderer(&mut self.renderer, &mut self.allocator);
        self.allocator.destroy(&context)
    }
}

pub struct VulkanRendererContext<
    A: Allocator,
    M: MaterialPackList<A>,
    V: MeshPackList<A>,
    S: GraphicsPipelinePackList,
> where
    A::Allocation<HostCoherent>: HostVisibleMemory,
{
    context: Rc<RefCell<Context>>,
    resources: VulkanResourcePack<A, M, V, S>,
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
        let context = Rc::new(RefCell::new(Context::build(window)?));
        Ok(Self {
            context,
            config,
            _phantom: PhantomData,
        })
    }
}

impl<A: Allocator, M: MaterialPackList<A>, V: MeshPackList<A>, S: GraphicsPipelinePackList> Drop
    for VulkanRendererContext<A, M, V, S>
where
    A::Allocation<HostCoherent>: HostVisibleMemory,
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
        StaticStackAllocator,
        M::Pack<StaticStackAllocator>,
        V::Pack<StaticStackAllocator>,
        S::Pack,
    >;

    fn load_context(&mut self, builder: Self::Builder) -> Result<Self::Context, Box<dyn Error>> {
        let mut context = self.context.borrow_mut();
        let resources = VulkanResourcePack::load(
            &mut context,
            &self.config,
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

impl<A: Allocator, M: MaterialPackList<A>, V: MeshPackList<A>, S: GraphicsPipelinePackList>
    RendererContext for VulkanRendererContext<A, M, V, S>
where
    A::Allocation<HostCoherent>: HostVisibleMemory,
{
    type Shaders = S;
    type Materials = M;
    type Meshes = V;

    fn begin_frame<C: Camera>(&mut self, camera: &C) -> Result<(), Box<dyn Error>> {
        let context = self.context.borrow();
        let camera_matrices = camera.get_matrices();
        self.resources
            .renderer
            .begin_frame(&context, &camera_matrices)?;
        Ok(())
    }

    fn end_frame(&mut self) -> Result<(), Box<dyn Error>> {
        let context = self.context.borrow();
        self.resources.renderer.end_frame(&context)?;
        Ok(())
    }

    fn draw<T: ShaderType, D: Drawable<Material = T::Material, Vertex = T::Vertex>>(
        &mut self,
        shader: ShaderHandle<T>,
        drawable: &D,
        transform: &Matrix4,
    ) -> Result<(), Box<dyn Error>> {
        self.resources.renderer.draw(
            shader,
            drawable,
            transform,
            &self.resources.materials,
            &self.resources.meshes,
        );
        Ok(())
    }
}
