use bytemuck::{Pod, Zeroable};
use math::types::Matrix4;

#[repr(C)]
#[derive(Debug, Clone, Copy, Zeroable, Pod)]
pub struct CameraMatrices {
    pub view: Matrix4,
    pub proj: Matrix4,
}
