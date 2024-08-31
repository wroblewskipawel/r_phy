mod type_list;

pub use type_list::*;

use winit::{
    dpi::PhysicalPosition,
    event::{ElementState, Event, StartCause, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::KeyCode,
    window::{Window, WindowBuilder},
};

use std::{cell::RefCell, error::Error, marker::PhantomData, rc::Rc, time::Instant};

use crate::{
    input::InputHandler,
    math::{transform::Transform, types::Matrix4},
    renderer::{
        camera::{Camera, CameraBuilder, CameraNone},
        model::{
            Drawable, DrawableType, EmptyMaterial, Material, MaterialHandle, MeshHandle, Vertex,
            VertexNone,
        },
        shader::{ShaderHandle, ShaderType},
        Renderer, RendererBuilder, RendererNone,
    },
};

#[derive(Clone, Copy)]
struct DrawCommand<S: ShaderType, D: Drawable<Material = S::Material, Vertex = S::Vertex>> {
    shader: ShaderHandle<S>,
    model: D,
    transform: Matrix4,
}

pub struct Object<D: Drawable + Clone + Copy> {
    model: D,
    transform: Transform,
    update: Box<dyn Fn(f32, Transform) -> Transform>,
}

impl<D: Drawable + Clone + Copy> Object<D> {
    pub fn new(
        model: D,
        transform: Transform,
        update: Box<dyn Fn(f32, Transform) -> Transform>,
    ) -> Self {
        Self {
            model,
            transform,
            update,
        }
    }

    fn update<S: ShaderType<Vertex = D::Vertex, Material = D::Material>>(
        &mut self,
        shader: ShaderHandle<S>,
        elapsed_time: f32,
    ) -> DrawCommand<S, D> {
        self.transform = (self.update)(elapsed_time, self.transform);
        DrawCommand {
            shader,
            model: self.model,
            transform: self.transform.into(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum CursorState {
    Locked,
    Free,
}

impl CursorState {
    pub fn new() -> Self {
        Self::Free
    }
    pub fn switch(&mut self, window: &Window) -> Result<(), Box<dyn Error>> {
        *self = match self {
            Self::Free => {
                let window_extent = window.inner_size();
                window.set_cursor_grab(winit::window::CursorGrabMode::Confined)?;
                window.set_cursor_position(PhysicalPosition {
                    x: window_extent.width / 2,
                    y: window_extent.height / 2,
                })?;
                window.set_cursor_visible(false);
                Self::Locked
            }
            Self::Locked => {
                window.set_cursor_grab(winit::window::CursorGrabMode::None)?;
                window.set_cursor_visible(true);
                Self::Free
            }
        };
        Ok(())
    }
}

pub struct LoopBuilder<R: RendererBuilder, C: CameraBuilder> {
    camera: Option<C>,
    renderer: Option<R>,
    window: Option<WindowBuilder>,
}

impl Default for LoopBuilder<RendererNone, CameraNone> {
    fn default() -> Self {
        Self::new()
    }
}

impl LoopBuilder<RendererNone, CameraNone> {
    pub fn new() -> Self {
        Self {
            camera: None,
            window: None,
            renderer: None,
        }
    }
}

impl<R: RendererBuilder, C: CameraBuilder> LoopBuilder<R, C> {
    pub fn with_window(self, window: WindowBuilder) -> Self {
        Self {
            window: Some(window),
            ..self
        }
    }

    pub fn with_renderer<N: RendererBuilder>(self, renderer: N) -> LoopBuilder<N, C> {
        let Self { window, camera, .. } = self;
        LoopBuilder {
            renderer: Some(renderer),
            window,
            camera,
        }
    }

    pub fn with_camera<N: CameraBuilder>(self, camera: N) -> LoopBuilder<R, N> {
        let Self {
            window, renderer, ..
        } = self;
        LoopBuilder {
            camera: Some(camera),
            window,
            renderer,
        }
    }

    pub fn build(self) -> Result<Loop<R::Renderer, C::Camera>, Box<dyn Error>> {
        let Self {
            window,
            renderer,
            camera,
        } = self;
        let mut input_handler = InputHandler::new();
        let event_loop = EventLoop::new()?;
        let window = Rc::new(
            window
                .ok_or("Window configuration not provided for Loop!")?
                .build(&event_loop)?,
        );
        let renderer = renderer
            .ok_or("Renderer backend not selected for Loop!")?
            .build(&window)?;
        let camera = camera
            .ok_or("Camera not selected for Loop!")?
            .build(&mut input_handler);
        Ok(Loop {
            event_loop,
            window,
            renderer,
            input_handler,
            camera,
        })
    }
}

pub trait DrawableTypeList: 'static {
    const LEN: usize;
    type Drawable: Drawable + Clone + Copy;
    type Next: DrawableTypeList;
}

#[derive(Debug, Clone, Copy)]
pub struct DrawableTerminator {}

impl DrawableType for DrawableTerminator {
    type Vertex = VertexNone;
    type Material = EmptyMaterial;
}

impl Drawable for DrawableTerminator {
    fn material(&self) -> MaterialHandle<Self::Material> {
        unreachable!()
    }

    fn mesh(&self) -> MeshHandle<Self::Vertex> {
        unreachable!()
    }
}

impl DrawableTypeList for DrawableTerminator {
    const LEN: usize = 0;
    type Drawable = Self;
    type Next = Self;
}

pub struct DrawableObjectNode<
    S: ShaderType,
    D: Drawable<Material = S::Material, Vertex = S::Vertex> + Clone + Copy,
    N: DrawableTypeList,
> {
    shader: ShaderHandle<S>,
    objects: Vec<Object<D>>,
    next: N,
}

impl<
        S: ShaderType,
        D: Drawable<Material = S::Material, Vertex = S::Vertex> + Clone + Copy,
        N: DrawableTypeList,
    > DrawableTypeList for DrawableObjectNode<S, D, N>
{
    const LEN: usize = N::LEN + 1;
    type Drawable = D;
    type Next = N;
}

pub trait DrawCommandCollection: DrawableTypeList {
    fn draw<R: Renderer>(self, renderer: &mut R);
}

impl DrawCommandCollection for DrawableTerminator {
    fn draw<R: Renderer>(self, _renderer: &mut R) {}
}

pub struct DrawCommandNode<
    S: ShaderType,
    D: Drawable<Material = S::Material, Vertex = S::Vertex>,
    N: DrawCommandCollection,
> {
    draw: Vec<DrawCommand<S, D>>,
    next: N,
}

impl<
        S: ShaderType,
        D: Drawable<Vertex = S::Vertex, Material = S::Material> + Clone + Copy,
        N: DrawCommandCollection,
    > DrawableTypeList for DrawCommandNode<S, D, N>
{
    const LEN: usize = N::LEN + 1;
    type Drawable = D;
    type Next = N;
}

impl<
        S: ShaderType,
        D: Drawable<Vertex = S::Vertex, Material = S::Material> + Clone + Copy,
        N: DrawCommandCollection,
    > DrawCommandCollection for DrawCommandNode<S, D, N>
{
    fn draw<R: Renderer>(self, renderer: &mut R) {
        for DrawCommand {
            shader,
            model,
            transform,
        } in self.draw
        {
            let _ = renderer.draw(shader, &model, &transform);
        }
        self.next.draw(renderer);
    }
}

pub trait DrawableCollection: DrawableTypeList {
    type DrawCommands: DrawCommandCollection;
    fn update(&mut self, elapsed_time: f32) -> Self::DrawCommands;
}

impl DrawableCollection for DrawableTerminator {
    type DrawCommands = Self;
    fn update(&mut self, _elapsed_time: f32) -> Self::DrawCommands {
        Self {}
    }
}

impl<
        S: ShaderType,
        D: Drawable<Vertex = S::Vertex, Material = S::Material> + Clone + Copy,
        N: DrawableCollection,
    > DrawableCollection for DrawableObjectNode<S, D, N>
{
    type DrawCommands = DrawCommandNode<S, D, N::DrawCommands>;
    fn update(&mut self, elapsed_time: f32) -> Self::DrawCommands {
        let draw = self
            .objects
            .iter_mut()
            .map(|object| object.update(self.shader, elapsed_time))
            .collect();
        DrawCommandNode {
            draw,
            next: self.next.update(elapsed_time),
        }
    }
}

pub struct Loop<R: Renderer, C: Camera> {
    renderer: R,
    window: Rc<Window>,
    event_loop: EventLoop<()>,
    input_handler: InputHandler,
    camera: Rc<RefCell<C>>,
}

pub trait LoopTypes {
    type Renderer: Renderer;
    type Camera: Camera;
}

impl<R: Renderer, C: Camera> LoopTypes for Loop<R, C> {
    type Renderer = R;
    type Camera = C;
}

pub struct Scene<D: DrawableCollection, L: LoopTypes> {
    objects: D,
    _loop: PhantomData<L>,
}

impl<D: DrawableCollection, L: LoopTypes> Scene<D, L> {
    pub fn with_objects<
        S: ShaderType,
        T: Drawable<Vertex = S::Vertex, Material = S::Material> + Clone + Copy,
    >(
        self,
        shader: ShaderHandle<S>,
        objects: Vec<Object<T>>,
    ) -> Scene<DrawableObjectNode<S, T, D>, L> {
        Scene {
            objects: DrawableObjectNode {
                shader,
                objects,
                next: self.objects,
            },
            _loop: PhantomData,
        }
    }
}

impl<R: Renderer, C: Camera> Loop<R, C> {
    pub fn scene(&self) -> Scene<DrawableTerminator, Self> {
        Scene {
            objects: DrawableTerminator {},
            _loop: PhantomData,
        }
    }

    pub fn get_mesh_handles<V: Vertex>(&self) -> Option<Vec<MeshHandle<V>>> {
        self.renderer.get_mesh_handles()
    }

    pub fn get_material_handles<M: Material>(&self) -> Option<Vec<MaterialHandle<M>>> {
        self.renderer.get_material_handles()
    }

    pub fn get_shader_handles<S: ShaderType>(&self) -> Option<Vec<ShaderHandle<S>>> {
        self.renderer.get_shader_handles()
    }

    pub fn run<D: DrawableCollection>(
        self,
        mut scene: Scene<D, Self>,
    ) -> Result<(), Box<dyn Error>> {
        let Self {
            window,
            event_loop,
            mut renderer,
            mut input_handler,
            camera,
        } = self;
        let cursor_state = Rc::new(RefCell::new(CursorState::new()));
        let shared_cursor_state = cursor_state.clone();
        let shared_window = window.clone();
        let shared_camera = camera.clone();
        input_handler.register_key_state_callback(
            KeyCode::KeyG,
            Box::new(move |state| {
                if let ElementState::Pressed = state {
                    let _ = shared_cursor_state.borrow_mut().switch(&shared_window);
                    match *(*shared_cursor_state).borrow() {
                        CursorState::Free => shared_camera.borrow_mut().set_active(false),
                        CursorState::Locked => shared_camera.borrow_mut().set_active(true),
                    }
                }
            }),
        );
        let mut draw_commands = None;
        let mut previous_frame_time = Instant::now();
        event_loop.set_control_flow(ControlFlow::Poll);
        event_loop.run(|event, elwt| {
            input_handler.handle_event(event.clone());
            match event {
                Event::NewEvents(StartCause::Poll) => {
                    let current_frame_time = Instant::now();
                    let elapsed_time = (current_frame_time - previous_frame_time).as_secs_f32();
                    previous_frame_time = current_frame_time;

                    camera.borrow_mut().update(elapsed_time);
                    draw_commands = Some(scene.objects.update(elapsed_time));
                    if let CursorState::Locked = *(*cursor_state).borrow() {
                        let window_extent = window.inner_size();
                        let _ = window.set_cursor_position(PhysicalPosition {
                            x: window_extent.width / 2,
                            y: window_extent.height / 2,
                        });
                    }
                }
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                } => {
                    elwt.exit();
                }
                Event::AboutToWait => {
                    let camera: &C = &(*camera).borrow();
                    let _ = renderer.begin_frame(camera);
                    if let Some(draw_commands) = draw_commands.take() {
                        draw_commands.draw(&mut renderer);
                    }
                    let _ = renderer.end_frame();
                }
                _ => (),
            }
        })?;
        Ok(())
    }
}
