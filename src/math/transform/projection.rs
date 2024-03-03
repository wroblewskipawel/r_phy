use crate::math::types::{Matrix4, Vector3, Vector4};

impl Matrix4 {
    #[inline]
    pub fn perspective(fov_y_rad: f32, aspect_ratio: f32, z_near: f32, z_far: f32) -> Matrix4 {
        let x_scale = (fov_y_rad * 0.5).tan().recip();
        let y_scale = -x_scale / aspect_ratio;
        let z_scale = 0.5 * (z_near + z_far) / (z_near - z_far) - 0.5;
        let l_z = (z_near * z_far) / (z_near - z_far);
        Matrix4 {
            i: Vector4::new(x_scale, 0.0, 0.0, 0.0),
            j: Vector4::new(0.0, y_scale, 0.0, 0.0),
            k: Vector4::new(0.0, 0.0, z_scale, -1.0),
            l: Vector4::new(0.0, 0.0, l_z, 0.0),
        }
    }

    #[inline]
    pub fn orthographic(min: Vector3, max: Vector3) -> Matrix4 {
        let b = max - min;
        let mut t = -(max + min);
        t.x /= b.x;
        t.y /= b.y;
        t.z = 0.5 * t.z / b.z + 0.5;
        let mut s = Vector3::new(2.0, 2.0, -2.0);
        s.x /= b.x;
        s.y = -s.y / b.y;
        s.z = 0.5 * s.z / b.z;
        Matrix4 {
            i: Vector4::new(s.x, 0.0, 0.0, 0.0),
            j: Vector4::new(0.0, s.y, 0.0, 0.0),
            k: Vector4::new(0.0, 0.0, s.z, 0.0),
            l: Vector4::point(t),
        }
    }
}
