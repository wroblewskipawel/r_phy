pub mod first_person;

use std::{cell::RefCell, rc::Rc};

use bytemuck::{Pod, Zeroable};
use input::InputHandler;
use math::types::{Matrix4, Vector3};

pub const UP: Vector3 = Vector3::z();

#[repr(C)]
#[derive(Debug, Clone, Copy, Zeroable, Pod)]
pub struct CameraMatrices {
    pub view: Matrix4,
    pub proj: Matrix4,
}

pub trait Camera: 'static {
    fn get_position(&self) -> Vector3;
    fn get_matrices(&self) -> CameraMatrices;
    fn update(&mut self, elapsed_time: f32);
    fn set_active(&mut self, active: bool);
}

pub trait CameraBuilder: 'static {
    type Camera: Camera;
    fn build(self, input_handler: &mut InputHandler) -> Rc<RefCell<Self::Camera>>;
}

pub struct CameraNone;

impl Camera for CameraNone {
    fn get_position(&self) -> Vector3 {
        unimplemented!()
    }

    fn get_matrices(&self) -> CameraMatrices {
        unimplemented!()
    }

    fn update(&mut self, _elapsed_time: f32) {
        unimplemented!()
    }

    fn set_active(&mut self, _active: bool) {
        unimplemented!()
    }
}

impl CameraBuilder for CameraNone {
    type Camera = CameraNone;
    fn build(self, _input_handler: &mut InputHandler) -> Rc<RefCell<Self::Camera>> {
        panic!("Camera Type not provided!")
    }
}
