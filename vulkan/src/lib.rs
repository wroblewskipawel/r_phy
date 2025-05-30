pub mod context;

use ash::vk;
use context::device::memory::DefaultAllocator;
use context::device::renderer::deferred::DeferredRenderer;
use context::device::resources::{
    MaterialPackList, MaterialPackListBuilder, MaterialPackListPartial, MeshPackList,
    MeshPackListBuilder, MeshPackListPartial,
};
use context::device::Device;
use context::Context;
use math::types::Matrix4;
use type_kit::{Cons, Contains, Create, Destroy, DestroyResult, DropGuard, Marker, Nil};

use context::device::{
    frame::{Frame, FrameContext},
    memory::{AllocatorCreate, StaticAllocator, StaticAllocatorConfig},
    pipeline::{GraphicsPipelineListBuilder, GraphicsPipelinePackList},
};
use graphics::renderer::{
    camera::Camera, ContextBuilder, Renderer, RendererBuilder, RendererContext,
};
use graphics::{
    model::{Drawable, Material, MaterialHandle, Mesh, MeshHandle, Vertex},
    shader::{ShaderHandle, ShaderType},
};
use std::convert::Infallible;
use std::{cell::RefCell, error::Error, marker::PhantomData, rc::Rc};
use winit::window::Window;

#[derive(Debug, Clone, Copy)]
pub struct VulkanRendererConfig {
    pub page_size: vk::DeviceSize,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct VulkanRendererConfigBuilder {
    page_size: Option<vk::DeviceSize>,
}

impl VulkanRendererConfig {
    pub fn builder() -> VulkanRendererConfigBuilder {
        VulkanRendererConfigBuilder::default()
    }
}

impl VulkanRendererConfigBuilder {
    pub fn build(self) -> Result<VulkanRendererConfig, Box<dyn Error>> {
        let config = VulkanRendererConfig {
            page_size: self.page_size.ok_or("Page size not provided")?,
        };
        Ok(config)
    }

    pub fn with_page_size(mut self, size: usize) -> Self {
        self.page_size = Some(size as vk::DeviceSize);
        self
    }
}

#[derive(Debug)]
pub struct VulkanRendererBuilder<R: Destroy>
where
    Rc<RefCell<DropGuard<R>>>: Frame,
{
    config: Option<VulkanRendererConfig>,
    _phantom: PhantomData<R>,
}

impl<R: Destroy> VulkanRendererBuilder<R>
where
    Rc<RefCell<DropGuard<R>>>: Frame,
{
    pub fn new() -> Self {
        Self {
            config: None,
            _phantom: PhantomData,
        }
    }

    pub fn with_config(mut self, config: VulkanRendererConfig) -> Self {
        self.config = Some(config);
        self
    }

    pub fn with_renderer_type<N: Destroy>(self) -> VulkanRendererBuilder<N>
    where
        Rc<RefCell<DropGuard<N>>>: Frame,
    {
        VulkanRendererBuilder {
            config: self.config,
            _phantom: PhantomData,
        }
    }
}

impl<R: Destroy> RendererBuilder for VulkanRendererBuilder<R>
where
    Rc<RefCell<DropGuard<R>>>: Frame,
{
    type Renderer = VulkanRenderer;

    fn build(self, window: &Window) -> Result<Self::Renderer, Box<dyn Error>> {
        let renderer =
            VulkanRenderer::new(window, self.config.ok_or("Configuration not provided")?)?;
        Ok(renderer)
    }
}

pub struct VulkanRenderer {
    context: Rc<RefCell<Context>>,
    renderer: Rc<RefCell<DropGuard<DeferredRenderer<DefaultAllocator>>>>,
    _config: VulkanRendererConfig,
}

impl Drop for VulkanRenderer {
    fn drop(&mut self) {
        let context = self.context.borrow();
        let _ = context.wait_idle();
        let mut renderer = self.renderer.borrow_mut();
        let _ = renderer.destroy((&*context, &mut DefaultAllocator {}));
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
}

impl<
        R: Frame,
        M: MaterialPackList<StaticAllocator>,
        V: MeshPackList<StaticAllocator>,
        S: GraphicsPipelinePackList,
    > Destroy for VulkanResourcePack<R, M, V, S>
{
    type Context<'a> = &'a Context;
    type DestroyError = Infallible;

    fn destroy<'a>(&mut self, context: Self::Context<'a>) -> DestroyResult<Self> {
        let device: &Device = &*context;
        let cell_allocator = RefCell::new(&mut self.allocator);
        let destroy_context = (device, &cell_allocator);
        let _ = self.materials.destroy(destroy_context);
        let _ = self.meshes.destroy(destroy_context);
        let _ = self.renderer_context.destroy(context);
        self.allocator.destroy(context);
        Ok(())
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

impl VulkanRenderer {
    pub fn new(window: &Window, config: VulkanRendererConfig) -> Result<Self, Box<dyn Error>> {
        let context = Context::build(window)?;
        let renderer = DeferredRenderer::create((), (&context, &mut DefaultAllocator {}))?;
        Ok(Self {
            context: Rc::new(RefCell::new(context)),
            renderer: Rc::new(RefCell::new(DropGuard::new(renderer))),
            _config: config,
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
        let _ = self.resources.destroy(&*context);
    }
}

impl Renderer for VulkanRenderer {}

#[derive(Debug)]
pub struct VulkanContextBuilder<
    R: Frame,
    S: GraphicsPipelineListBuilder,
    M: MaterialPackListBuilder,
    V: MeshPackListBuilder,
> {
    shaders: S,
    materials: M,
    meshes: V,
    _phantom: PhantomData<R>,
}

impl<S: GraphicsPipelineListBuilder, M: MaterialPackListBuilder, V: MeshPackListBuilder>
    ContextBuilder
    for VulkanContextBuilder<Rc<RefCell<DropGuard<DeferredRenderer<DefaultAllocator>>>>, S, M, V>
{
    type Renderer = VulkanRenderer;
    type Context = VulkanRendererContext<
        Rc<RefCell<DropGuard<DeferredRenderer<DefaultAllocator>>>>,
        M::Pack<StaticAllocator>,
        V::Pack<StaticAllocator>,
        S::Pack,
    >;

    fn build(self, renderer: &Self::Renderer) -> Result<Self::Context, Box<dyn Error>> {
        let mut context = renderer.context.borrow_mut();
        let resources = VulkanResourcePack::load(
            &mut context,
            &renderer.renderer,
            &self.materials,
            &self.meshes,
            &self.shaders,
        )?;
        Ok(VulkanRendererContext {
            context: renderer.context.clone(),
            resources,
        })
    }
}

impl Default
    for VulkanContextBuilder<
        Rc<RefCell<DropGuard<DeferredRenderer<DefaultAllocator>>>>,
        Nil,
        Nil,
        Nil,
    >
{
    fn default() -> Self {
        Self::new()
    }
}

impl
    VulkanContextBuilder<Rc<RefCell<DropGuard<DeferredRenderer<DefaultAllocator>>>>, Nil, Nil, Nil>
{
    pub fn new() -> Self {
        VulkanContextBuilder {
            shaders: Nil::new(),
            materials: Nil::new(),
            meshes: Nil::new(),
            _phantom: PhantomData,
        }
    }
}

fn push_and_get_index<V>(vec: &mut Vec<V>, value: V) -> u32 {
    let index = vec.len();
    vec.push(value);
    index.try_into().unwrap()
}

impl<
        R: Frame,
        S: GraphicsPipelineListBuilder,
        M: MaterialPackListBuilder,
        V: MeshPackListBuilder,
    > VulkanContextBuilder<R, S, M, V>
{
    pub fn with_material_type<N: Material>(self) -> VulkanContextBuilder<R, S, Cons<Vec<N>, M>, V> {
        VulkanContextBuilder {
            materials: Cons {
                head: vec![],
                tail: self.materials,
            },
            meshes: self.meshes,
            shaders: self.shaders,
            _phantom: PhantomData,
        }
    }

    pub fn with_mesh_type<N: Vertex>(self) -> VulkanContextBuilder<R, S, M, Cons<Vec<Mesh<N>>, V>> {
        VulkanContextBuilder {
            meshes: Cons {
                head: vec![],
                tail: self.meshes,
            },
            materials: self.materials,
            shaders: self.shaders,
            _phantom: PhantomData,
        }
    }

    pub fn with_shader_type<N: ShaderType + Into<R::Shader<N>>>(
        self,
    ) -> VulkanContextBuilder<R, Cons<Vec<R::Shader<N>>, S>, M, V> {
        VulkanContextBuilder {
            shaders: Cons {
                head: vec![],
                tail: self.shaders,
            },
            materials: self.materials,
            meshes: self.meshes,
            _phantom: PhantomData,
        }
    }

    pub fn add_material<N: Material, T: Marker>(&mut self, material: N) -> MaterialHandle<N>
    where
        M: Contains<Vec<N>, T>,
    {
        MaterialHandle::new(push_and_get_index(self.materials.get_mut(), material))
    }

    pub fn add_mesh<N: Vertex, T: Marker>(&mut self, mesh: Mesh<N>) -> MeshHandle<N>
    where
        V: Contains<Vec<Mesh<N>>, T>,
    {
        MeshHandle::new(push_and_get_index(self.meshes.get_mut(), mesh))
    }

    pub fn add_shader<N: ShaderType + Into<R::Shader<N>>, T: Marker>(
        &mut self,
        shader: N,
    ) -> ShaderHandle<N>
    where
        S: Contains<Vec<R::Shader<N>>, T>,
    {
        ShaderHandle::new(push_and_get_index(self.shaders.get_mut(), shader.into()))
    }
}

impl<
        R: Frame,
        M: MaterialPackList<StaticAllocator> + 'static,
        V: MeshPackList<StaticAllocator> + 'static,
        S: GraphicsPipelinePackList + 'static,
    > RendererContext for VulkanRendererContext<R, M, V, S>
{
    type Renderer = VulkanRenderer;
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
