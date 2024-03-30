use winit::{
    self,
    dpi::PhysicalPosition,
    event::{ElementState, Event, StartCause, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::KeyCode,
    window::{Window, WindowBuilder},
};

use std::{cell::RefCell, error::Error, rc::Rc, time::Instant};

use crate::{
    input::InputHandler,
    math::{transform::Transform, types::Matrix4},
    renderer::{
        camera::{Camera, CameraType},
        model::{Material, MaterialHandle, Mesh, MeshHandle, Model},
        Renderer, RendererBackend,
    },
};

#[derive(Debug, Clone, Copy)]
struct DrawCommand {
    model: Model,
    transform: Matrix4,
}

pub struct Object {
    model: Model,
    transform: Transform,
    update: Box<dyn Fn(f32, Transform) -> Transform>,
}

impl Object {
    pub fn new(
        model: Model,
        transform: Transform,
        update: Box<dyn Fn(f32, Transform) -> Transform>,
    ) -> Self {
        Self {
            model,
            transform,
            update,
        }
    }

    fn update(&mut self, elapsed_time: f32) -> DrawCommand {
        self.transform = (self.update)(elapsed_time, self.transform).into();
        DrawCommand {
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

pub struct LoopBuilder {
    window_builder: Option<WindowBuilder>,
    renderer_backend: Option<RendererBackend>,
    camera: Option<(CameraType, Matrix4)>,
    meshes: Vec<Mesh>,
    materials: Vec<Material>,
}

impl LoopBuilder {
    pub fn new() -> Self {
        Self {
            window_builder: None,
            renderer_backend: None,
            camera: None,
            meshes: vec![],
            materials: vec![],
        }
    }

    pub fn with_window(self, window_builder: WindowBuilder) -> Self {
        Self {
            window_builder: Some(window_builder),
            ..self
        }
    }

    pub fn with_renderer(self, renderer_backend: RendererBackend) -> Self {
        Self {
            renderer_backend: Some(renderer_backend),
            ..self
        }
    }

    pub fn with_camera(self, camera_type: CameraType, proj: Matrix4) -> Self {
        Self {
            camera: Some((camera_type, proj)),
            ..self
        }
    }

    pub fn with_meshes(self, meshes: Vec<Mesh>) -> Self {
        Self { meshes, ..self }
    }

    pub fn with_materials(self, materials: Vec<Material>) -> Self {
        Self { materials, ..self }
    }

    pub fn build(self) -> Result<(Loop, Vec<MeshHandle>, Vec<MaterialHandle>), Box<dyn Error>> {
        let Self {
            window_builder,
            renderer_backend,
            camera,
            meshes,
            materials,
        } = self;
        let mut input_handler = InputHandler::new();
        let event_loop = EventLoop::new()?;
        let window = Rc::new(
            window_builder
                .ok_or("Window configuration not provided for Loop!")?
                .build(&event_loop)?,
        );
        let mut renderer = renderer_backend
            .ok_or("Renderer backend not selected for Loop!")?
            .create(&window)?;
        let mesh_handles = renderer.load_meshes(&meshes)?;
        let material_handles = renderer.load_materials(&materials)?;
        let camera = camera
            .ok_or("Camera not selected for Loop!")
            .and_then(|(camera_type, proj)| Ok(camera_type.create(proj, &mut input_handler)))?;
        Ok((
            Loop {
                event_loop,
                window,
                renderer,
                input_handler,
                camera,
                objects: vec![],
            },
            mesh_handles,
            material_handles,
        ))
    }
}

pub struct Loop {
    event_loop: EventLoop<()>,
    window: Rc<Window>,
    renderer: Box<dyn Renderer>,
    input_handler: InputHandler,
    camera: Rc<RefCell<dyn Camera>>,
    objects: Vec<Object>,
}

impl Loop {
    pub fn with_objects(self, objects: Vec<Object>) -> Self {
        Self { objects, ..self }
    }

    pub fn run(self) -> Result<(), Box<dyn Error>> {
        let Self {
            window,
            event_loop,
            mut renderer,
            mut input_handler,
            camera,
            mut objects,
        } = self;
        let cursor_state = Rc::new(RefCell::new(CursorState::new()));
        let shared_window = window.clone();
        let shared_camera = camera.clone();
        let shared_cursor_state = cursor_state.clone();
        input_handler.register_key_state_callback(
            KeyCode::KeyG,
            Box::new(move |state| {
                if let ElementState::Pressed = state {
                    let _ = shared_cursor_state.borrow_mut().switch(&shared_window);
                    match *shared_cursor_state.borrow() {
                        CursorState::Free => shared_camera.borrow_mut().set_active(false),
                        CursorState::Locked => shared_camera.borrow_mut().set_active(true),
                    }
                }
            }),
        );
        let mut draw_commands = Vec::with_capacity(objects.len());
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
                    draw_commands = objects
                        .iter_mut()
                        .map(|object| object.update(elapsed_time))
                        .collect();
                    if let CursorState::Locked = *cursor_state.borrow() {
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
                    let _ = renderer.begin_frame(&*camera.borrow());
                    for DrawCommand { model, transform } in &draw_commands {
                        let _ = renderer.draw(*model, transform);
                    }
                    let _ = renderer.end_frame();
                }
                _ => (),
            }
        })?;
        Ok(())
    }
}
