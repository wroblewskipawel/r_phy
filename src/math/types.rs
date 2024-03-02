mod matrix;
mod quat;
mod vector;

pub use matrix::{Matrix2, Matrix3, Matrix4};
pub use quat::Quat;
pub use vector::{Vector2, Vector3, Vector4};

const EPS: f32 = 1e-6;
