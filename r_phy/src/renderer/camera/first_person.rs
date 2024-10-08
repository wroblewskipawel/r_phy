use std::{
    cell::RefCell,
    f32::consts::{FRAC_PI_2, PI},
    rc::Rc,
};

use winit::{dpi::PhysicalPosition, keyboard::KeyCode};

use crate::{
    input::InputHandler,
    math::types::{Matrix4, Vector3},
    renderer::camera::UP,
};

use super::{Camera, CameraBuilder, CameraMatrices};

impl Camera for FirstPersonCamera {
    fn get_position(&self) -> Vector3 {
        self.position
    }

    fn get_matrices(&self) -> CameraMatrices {
        self.into()
    }

    fn update(&mut self, elapsed_time: f32) {
        const MOVEMENT_SPEED: f32 = 4.0;
        if self.active {
            if self.move_direction.length_square() > 0.0 {
                self.position =
                    self.position + elapsed_time * MOVEMENT_SPEED * self.move_direction.norm();
            }
            self.forward = Vector3::from_euler(self.euler.x, self.euler.y, self.euler.z);
            self.right = self.forward.cross(UP).norm();
            self.move_direction = Vector3::zero();
        }
    }

    fn set_active(&mut self, active: bool) {
        self.active = active;
    }
}

pub struct FirstPersonCameraBuilder {
    proj: Matrix4,
}

impl FirstPersonCameraBuilder {
    pub fn new(proj: Matrix4) -> Self {
        Self { proj }
    }
}

impl CameraBuilder for FirstPersonCameraBuilder {
    type Camera = FirstPersonCamera;

    fn build(self, input_handler: &mut InputHandler) -> Rc<RefCell<Self::Camera>> {
        let camera = Rc::new(RefCell::new(FirstPersonCamera::new(self.proj)));
        FirstPersonCamera::register_callbacks(camera.clone(), input_handler);
        camera
    }
}

impl From<&FirstPersonCamera> for CameraMatrices {
    fn from(value: &FirstPersonCamera) -> Self {
        CameraMatrices {
            proj: value.proj,
            view: Matrix4::look_at(value.position, value.position + value.forward, UP),
        }
    }
}

pub struct FirstPersonCamera {
    proj: Matrix4,
    position: Vector3,
    forward: Vector3,
    right: Vector3,
    euler: Vector3,
    move_direction: Vector3,
    active: bool,
}

impl FirstPersonCamera {
    pub fn new(proj: Matrix4) -> Self {
        Self {
            proj,
            position: Vector3::zero(),
            forward: Vector3::x(),
            right: -Vector3::y(),
            euler: Vector3::zero(),
            move_direction: Vector3::zero(),
            active: false,
        }
    }

    pub fn register_callbacks(camera: Rc<RefCell<Self>>, input_handler: &mut InputHandler) {
        let shared_camera = camera.clone();
        input_handler.register_cursor_callback(Box::new(move |position| {
            let mut camera = shared_camera.borrow_mut();
            if camera.active {
                let PhysicalPosition { x, y } = position;
                const MOUSE_SENSITIVITY: f32 = 0.5;
                let delta_x = x - 400.0;
                let delta_y = y - 300.0;
                let delta_yaw = (delta_x / 400.0) as f32 * MOUSE_SENSITIVITY;
                let delta_pitch = (delta_y / 300.0) as f32 * MOUSE_SENSITIVITY;
                camera.euler.y =
                    (camera.euler.y + delta_pitch).clamp(-FRAC_PI_2 + 1e-4, FRAC_PI_2 - 1e-4);
                camera.euler.x = ((camera.euler.x - delta_yaw) / (2.0 * PI)).fract() * (2.0 * PI);
                camera.forward =
                    Vector3::from_euler(camera.euler.x, camera.euler.y, camera.euler.z);
                camera.right = camera.forward.cross(UP).norm();
            }
        }));
        let shared_camera = camera.clone();
        input_handler.register_key_pressed_callback(
            KeyCode::KeyW,
            Box::new(move |()| {
                let mut camera = shared_camera.borrow_mut();
                if camera.active {
                    camera.move_direction = camera.move_direction + camera.forward;
                }
            }),
        );
        let shared_camera = camera.clone();
        input_handler.register_key_pressed_callback(
            KeyCode::KeyS,
            Box::new(move |()| {
                let mut camera = shared_camera.borrow_mut();
                if camera.active {
                    camera.move_direction = camera.move_direction - camera.forward;
                }
            }),
        );
        let shared_camera = camera.clone();
        input_handler.register_key_pressed_callback(
            KeyCode::KeyD,
            Box::new(move |()| {
                let mut camera = shared_camera.borrow_mut();
                if camera.active {
                    camera.move_direction = camera.move_direction + camera.right;
                }
            }),
        );
        let shared_camera = camera.clone();
        input_handler.register_key_pressed_callback(
            KeyCode::KeyA,
            Box::new(move |()| {
                let mut camera = shared_camera.borrow_mut();
                if camera.active {
                    camera.move_direction = camera.move_direction - camera.right;
                }
            }),
        );
    }
}
