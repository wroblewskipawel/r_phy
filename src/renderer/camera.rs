pub mod first_person;

use std::{cell::RefCell, rc::Rc};

use bytemuck::{Pod, Zeroable};

use crate::{
    input::InputHandler,
    math::types::{Matrix4, Vector3},
};

use self::first_person::FirstPersonCamera;

pub const UP: Vector3 = Vector3::z();

#[repr(C)]
#[derive(Debug, Clone, Copy, Zeroable, Pod)]
pub struct CameraMatrices {
    pub view: Matrix4,
    pub proj: Matrix4,
}

pub trait Camera {
    fn get_matrices(&self) -> CameraMatrices;
    fn update(&mut self, elapsed_time: f32);
    fn set_active(&mut self, active: bool);
}

pub enum CameraType {
    FirstPerson,
}

impl CameraType {
    pub fn create(
        self,
        proj: Matrix4,
        input_handler: &mut InputHandler,
    ) -> Rc<RefCell<dyn Camera>> {
        match self {
            Self::FirstPerson => {
                let camera = Rc::new(RefCell::new(FirstPersonCamera::new(proj)));
                FirstPersonCamera::register_callbacks(camera.clone(), input_handler);
                camera
            }
        }
    }
}
