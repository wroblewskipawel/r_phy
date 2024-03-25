use colored::{self, Colorize};
use std::{
    error::Error,
    f32::consts::{FRAC_PI_2, PI},
    result::Result,
    time::Instant,
};
use winit::{
    dpi::{LogicalPosition, PhysicalPosition, PhysicalSize},
    event::{ElementState, Event, KeyEvent, StartCause, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowBuilder, WindowButtons},
};

use r_phy::{
    math::{
        transform::Transform,
        types::{Matrix4, Vector3},
    },
    physics::shape,
    renderer::{camera::Camera, RendererBackend},
};

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
                window.set_cursor_grab(winit::window::CursorGrabMode::Confined)?;
                window.set_cursor_position(LogicalPosition { x: 400, y: 300 })?;
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

fn main() -> Result<(), Box<dyn Error>> {
    let event_loop = EventLoop::new()?;
    let window = WindowBuilder::new()
        .with_inner_size(PhysicalSize {
            width: 800,
            height: 600,
        })
        .with_resizable(false)
        .with_enabled_buttons(WindowButtons::CLOSE | WindowButtons::MINIMIZE)
        .with_title("r_phy")
        .with_transparent(false)
        .build(&event_loop)?;
    let mut renderer = RendererBackend::Vulkan.create(&window)?;
    let meshes = renderer.load_meshes(&[
        shape::Sphere::new(0.75).into(),
        shape::Cube::new(1.0).into(),
        shape::Box::new(3.0, 1.0, 1.0).into(),
    ])?;
    let proj = Matrix4::perspective(std::f32::consts::FRAC_PI_3, 600.0 / 800.0, 1e-4, 1e4);
    let mut model_rotation = 0.0;

    let camera_up = Vector3::z();
    let mut camera_position = Vector3::new(-10.0, 0.0, 0.0);
    let mut camera_euler = Vector3::zero();
    let mut camera_forward = Vector3::from_euler(camera_euler.x, camera_euler.y, camera_euler.z);
    let mut camera_right = camera_forward.cross(camera_up).norm();
    let mut cursor_state = CursorState::new();

    let mut key_table = vec![false; 194].into_boxed_slice();

    let get_camera_delta_position =
        |key_table: &[bool], camera_forward: Vector3, camera_right: Vector3| {
            const MOVEMENT_SPEED: f32 = 4.0;
            let mut camera_delta = Vector3::zero();
            if key_table[KeyCode::KeyW as usize] {
                camera_delta = camera_delta + camera_forward;
            }
            if key_table[KeyCode::KeyS as usize] {
                camera_delta = camera_delta - camera_forward;
            }
            if key_table[KeyCode::KeyD as usize] {
                camera_delta = camera_delta + camera_right;
            }
            if key_table[KeyCode::KeyA as usize] {
                camera_delta = camera_delta - camera_right;
            }
            if camera_delta.length_square() > 0.0 {
                MOVEMENT_SPEED * camera_delta.norm()
            } else {
                Vector3::zero()
            }
        };

    let mut elapsed_time = 0.0;
    let mut previous_frame_time = Instant::now();
    event_loop.set_control_flow(ControlFlow::Poll);
    event_loop.run(move |event, elwt| match event {
        Event::NewEvents(StartCause::Poll) => {
            let current_frame_time = Instant::now();
            elapsed_time = (current_frame_time - previous_frame_time).as_secs_f32();
            previous_frame_time = current_frame_time;
            if let CursorState::Locked = cursor_state {
                let _ = window.set_cursor_position(PhysicalPosition::new(400, 300));
            }
        }
        Event::WindowEvent { event, .. } => match event {
            WindowEvent::KeyboardInput { event, .. } => match event {
                KeyEvent {
                    physical_key: PhysicalKey::Code(key),
                    state: ElementState::Pressed,
                    repeat: false,
                    ..
                } => {
                    match key {
                        KeyCode::KeyQ => elwt.exit(),
                        KeyCode::KeyG => {
                            cursor_state.switch(&window).unwrap();
                        }
                        _ => (),
                    };
                    if (key as usize) < key_table.len() {
                        key_table[key as usize] = true;
                    }
                }
                KeyEvent {
                    physical_key: PhysicalKey::Code(key),
                    state: ElementState::Released,
                    repeat: false,
                    ..
                } if (key as usize) < key_table.len() => {
                    key_table[key as usize] = false;
                }
                _ => (),
            },
            WindowEvent::CursorMoved {
                position: PhysicalPosition { x, y },
                ..
            } if x != 0.0 || y != 0.0 => {
                const MOUSE_SENSITIVITY: f32 = 0.5;
                if let CursorState::Locked = cursor_state {
                    let delta_x = x - 400.0;
                    let delta_y = y - 300.0;
                    let delta_yaw = (delta_x / 400.0) as f32 * MOUSE_SENSITIVITY;
                    let delta_pitch = (delta_y / 300.0) as f32 * MOUSE_SENSITIVITY;
                    println!(
                        "Cursor delta movement: {}: {} {}: {}",
                        "X".red(),
                        delta_x,
                        "Y".green(),
                        delta_y,
                    );
                    camera_euler.y =
                        (camera_euler.y + delta_pitch).clamp(-FRAC_PI_2 + 1e-4, FRAC_PI_2 - 1e-4);
                    camera_euler.x =
                        ((camera_euler.x - delta_yaw) / (2.0 * PI)).fract() * (2.0 * PI);
                    camera_forward =
                        Vector3::from_euler(camera_euler.x, camera_euler.y, camera_euler.z);
                    camera_right = camera_forward.cross(camera_up).norm();
                }
            }
            WindowEvent::CloseRequested => {
                elwt.exit();
            }
            _ => (),
        },
        Event::AboutToWait => {
            model_rotation += elapsed_time * (2.0 * std::f32::consts::PI);
            camera_position = camera_position
                + elapsed_time
                    * get_camera_delta_position(&key_table, camera_forward, camera_right);
            let view =
                Matrix4::look_at(camera_position, camera_position + camera_forward, camera_up);
            let camera = Camera { view, proj };
            let _ = renderer.begin_frame(&camera);
            let _ = renderer.draw(
                meshes[0],
                &(Transform::identity()
                    .rotate(Vector3::z(), model_rotation)
                    .translate(Vector3::new(0.0, 3.0, 0.0))
                    .rotate(Vector3::z(), model_rotation / 10.0)
                    .translate(Vector3::new(0.0, 0.0, 1.0 + (model_rotation / 5.0).sin())))
                .rotate(
                    Vector3::new(0.0, 1.0, 2.0).norm(),
                    std::f32::consts::FRAC_PI_2,
                )
                .into(),
            );
            let _ = renderer.draw(
                meshes[2],
                &(Transform::identity()
                    .rotate(Vector3::y(), -model_rotation / 2.0)
                    .rotate(Vector3::z(), -model_rotation / 3.0)
                    .translate(Vector3::new(0.0, 4.0, 0.0))
                    .rotate(Vector3::z(), -model_rotation / 6.0)
                    .translate(Vector3::new(0.0, 0.0, 1.0 + (model_rotation / 2.0).cos())))
                .rotate(
                    Vector3::new(1.0, -1.0, 0.0).norm(),
                    std::f32::consts::FRAC_PI_2,
                )
                .into(),
            );
            let _ = renderer.draw(
                meshes[1],
                &(<Transform as Into<Matrix4>>::into(
                    Transform::identity().rotate(Vector3::z(), model_rotation / 3.0),
                ) * Matrix4::scale(2.0)),
            );
            let _ = renderer.end_frame();
        }
        _ => (),
    })?;
    Ok(())
}
