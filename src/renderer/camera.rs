use bytemuck::{Pod, Zeroable};

use crate::math::types::Matrix4;

#[repr(C)]
#[derive(Debug, Clone, Copy, Zeroable, Pod)]
pub struct Camera {
    pub view: Matrix4,
    pub proj: Matrix4,
}
