use super::EPS;
use bytemuck::{Pod, Zeroable};
use std::ops::{Add, Div, Index, IndexMut, Mul, Neg, Sub};

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, Zeroable, Pod)]
pub struct Vector2 {
    pub x: f32,
    pub y: f32,
}

impl Neg for Vector2 {
    type Output = Self;
    #[inline]
    fn neg(self) -> Self::Output {
        Self {
            x: -self.x,
            y: -self.y,
        }
    }
}

impl Add for Vector2 {
    type Output = Self;
    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl Sub for Vector2 {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl Mul<Vector2> for f32 {
    type Output = Vector2;
    #[inline]
    fn mul(self, rhs: Vector2) -> Self::Output {
        Vector2 {
            x: self * rhs.x,
            y: self * rhs.y,
        }
    }
}

impl Div<f32> for Vector2 {
    type Output = Self;
    #[inline]
    fn div(self, rhs: f32) -> Self::Output {
        rhs.recip() * self
    }
}

impl Mul<Vector2> for Vector2 {
    type Output = f32;
    #[inline]
    fn mul(self, rhs: Vector2) -> Self::Output {
        self.x * rhs.x + self.y * rhs.y
    }
}

impl Index<usize> for Vector2 {
    type Output = f32;
    #[inline]
    fn index(&self, index: usize) -> &Self::Output {
        debug_assert!(index < 2, "Invalid index {} for Vector2!", index);
        unsafe {
            (&self.x as *const f32)
                .offset(index as _)
                .as_ref()
                .unwrap_unchecked()
        }
    }
}

impl IndexMut<usize> for Vector2 {
    #[inline]
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        debug_assert!(index < 2, "Invalid index {} for Vector2!", index);
        unsafe {
            (&mut self.x as *mut f32)
                .offset(index as _)
                .as_mut()
                .unwrap_unchecked()
        }
    }
}

impl Vector2 {
    #[inline]
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    #[inline]
    pub fn x() -> Self {
        Self { x: 1.0, y: 0.0 }
    }

    #[inline]
    pub fn y() -> Self {
        Self { x: 0.0, y: 1.0 }
    }

    #[inline]
    pub fn length_square(self) -> f32 {
        self * self
    }

    #[inline]
    pub fn length(self) -> f32 {
        (self * self).sqrt()
    }

    #[inline]
    pub fn norm(self) -> Self {
        self / self.length()
    }

    #[inline]
    pub fn is_valid(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }

    #[inline]
    pub fn approx_equal(self, rhs: Self) -> bool {
        (self.x - rhs.x).abs() < EPS && (self.y - rhs.y).abs() < EPS
    }
}

#[cfg(test)]
mod test_vector_3 {
    use super::Vector3;
    #[test]
    fn cross() {
        assert!(Vector3::x().cross(Vector3::y()).approx_equal(Vector3::z()));
        assert!(Vector3::y().cross(Vector3::x()).approx_equal(-Vector3::z()));
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, Zeroable, Pod)]
pub struct Vector3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Neg for Vector3 {
    type Output = Self;
    #[inline]
    fn neg(self) -> Self::Output {
        Self {
            x: -self.x,
            y: -self.y,
            z: -self.z,
        }
    }
}

impl Add for Vector3 {
    type Output = Self;
    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
            z: self.z + rhs.z,
        }
    }
}

impl Sub for Vector3 {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
            z: self.z - rhs.z,
        }
    }
}

impl Mul<Vector3> for f32 {
    type Output = Vector3;
    #[inline]
    fn mul(self, rhs: Vector3) -> Self::Output {
        Vector3 {
            x: self * rhs.x,
            y: self * rhs.y,
            z: self * rhs.z,
        }
    }
}

impl Div<f32> for Vector3 {
    type Output = Self;
    #[inline]
    fn div(self, rhs: f32) -> Self::Output {
        rhs.recip() * self
    }
}

impl Mul<Vector3> for Vector3 {
    type Output = f32;
    #[inline]
    fn mul(self, rhs: Vector3) -> Self::Output {
        self.x * rhs.x + self.y * rhs.y + self.z * rhs.z
    }
}

impl Index<usize> for Vector3 {
    type Output = f32;
    #[inline]
    fn index(&self, index: usize) -> &Self::Output {
        debug_assert!(index < 3, "Invalid index {} for Vector3!", index);
        unsafe {
            (&self.x as *const f32)
                .offset(index as _)
                .as_ref()
                .unwrap_unchecked()
        }
    }
}

impl IndexMut<usize> for Vector3 {
    #[inline]
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        debug_assert!(index < 3, "Invalid index {} for Vector3!", index);
        unsafe {
            (&mut self.x as *mut f32)
                .offset(index as _)
                .as_mut()
                .unwrap_unchecked()
        }
    }
}

impl From<Vector2> for Vector3 {
    #[inline]
    fn from(value: Vector2) -> Self {
        Self {
            x: value.x,
            y: value.y,
            z: 0.0,
        }
    }
}

impl Vector3 {
    #[inline]
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    #[inline]
    pub fn x() -> Self {
        Self {
            x: 1.0,
            y: 0.0,
            z: 0.0,
        }
    }

    #[inline]
    pub fn y() -> Self {
        Self {
            x: 0.0,
            y: 1.0,
            z: 0.0,
        }
    }

    #[inline]
    pub fn z() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 1.0,
        }
    }

    #[inline]
    pub fn length_square(self) -> f32 {
        self * self
    }

    #[inline]
    pub fn length(self) -> f32 {
        (self * self).sqrt()
    }

    #[inline]
    pub fn norm(self) -> Self {
        self / self.length()
    }

    #[inline]
    pub fn cross(self, rhs: Self) -> Self {
        Self {
            x: self.y * rhs.z - self.z * rhs.y,
            y: self.z * rhs.x - self.x * rhs.z,
            z: self.x * rhs.y - self.y * rhs.x,
        }
    }

    #[inline]
    pub fn is_valid(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }

    #[inline]
    pub fn approx_equal(self, rhs: Self) -> bool {
        (self.x - rhs.x).abs() < EPS && (self.y - rhs.y).abs() < EPS && (self.z - rhs.z).abs() < EPS
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, Zeroable, Pod)]
pub struct Vector4 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Neg for Vector4 {
    type Output = Self;
    #[inline]
    fn neg(self) -> Self::Output {
        Self {
            x: -self.x,
            y: -self.y,
            z: -self.z,
            w: -self.w,
        }
    }
}

impl Add for Vector4 {
    type Output = Self;
    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
            z: self.z + rhs.z,
            w: self.w + rhs.w,
        }
    }
}

impl Sub for Vector4 {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
            z: self.z - rhs.z,
            w: self.w - rhs.w,
        }
    }
}

impl Mul<Vector4> for f32 {
    type Output = Vector4;
    #[inline]
    fn mul(self, rhs: Vector4) -> Self::Output {
        Vector4 {
            x: self * rhs.x,
            y: self * rhs.y,
            z: self * rhs.z,
            w: self * rhs.w,
        }
    }
}

impl Div<f32> for Vector4 {
    type Output = Self;
    #[inline]
    fn div(self, rhs: f32) -> Self::Output {
        rhs.recip() * self
    }
}

impl Mul<Vector4> for Vector4 {
    type Output = f32;
    #[inline]
    fn mul(self, rhs: Vector4) -> Self::Output {
        self.x * rhs.x + self.y * rhs.y + self.z * rhs.z + self.w * rhs.w
    }
}

impl Index<usize> for Vector4 {
    type Output = f32;
    #[inline]
    fn index(&self, index: usize) -> &Self::Output {
        debug_assert!(index < 4, "Invalid index {} for Vector4!", index);
        unsafe {
            (&self.x as *const f32)
                .offset(index as _)
                .as_ref()
                .unwrap_unchecked()
        }
    }
}

impl IndexMut<usize> for Vector4 {
    #[inline]
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        debug_assert!(index < 4, "Invalid index {} for Vector4!", index);
        unsafe {
            (&mut self.x as *mut f32)
                .offset(index as _)
                .as_mut()
                .unwrap_unchecked()
        }
    }
}

impl From<Vector3> for Vector4 {
    #[inline]
    fn from(value: Vector3) -> Self {
        Self {
            x: value.x,
            y: value.y,
            z: value.z,
            w: 0.0,
        }
    }
}

impl Vector4 {
    #[inline]
    pub fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self { x, y, z, w }
    }

    #[inline]
    pub fn point(p: Vector3) -> Self {
        Self {
            x: p.x,
            y: p.y,
            z: p.z,
            w: 1.0,
        }
    }

    #[inline]
    pub fn vector(v: Vector3) -> Self {
        Self {
            x: v.x,
            y: v.y,
            z: v.z,
            w: 0.0,
        }
    }

    #[inline]
    pub fn x() -> Self {
        Self {
            x: 1.0,
            y: 0.0,
            z: 0.0,
            w: 0.0,
        }
    }

    #[inline]
    pub fn y() -> Self {
        Self {
            x: 0.0,
            y: 1.0,
            z: 0.0,
            w: 0.0,
        }
    }

    #[inline]
    pub fn z() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 1.0,
            w: 0.0,
        }
    }

    #[inline]
    pub fn w() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            w: 1.0,
        }
    }

    #[inline]
    pub fn length_square(self) -> f32 {
        self * self
    }

    #[inline]
    pub fn length(self) -> f32 {
        (self * self).sqrt()
    }

    #[inline]
    pub fn norm(self) -> Self {
        self / self.length()
    }

    #[inline]
    pub fn is_valid(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite() && self.w.is_finite()
    }

    #[inline]
    pub fn approx_equal(self, rhs: Self) -> bool {
        (self.x - rhs.x).abs() < EPS
            && (self.y - rhs.y).abs() < EPS
            && (self.z - rhs.z).abs() < EPS
            && (self.w - rhs.w).abs() < EPS
    }
}
