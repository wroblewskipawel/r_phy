pub struct Cube {
    pub side: f32,
}

pub struct Sphere {
    pub diameter: f32,
}

pub struct Box {
    pub width: f32,
    pub height: f32,
    pub depth: f32,
}

impl Cube {
    pub fn new(side: f32) -> Self {
        Self { side }
    }
}

impl Sphere {
    pub fn new(diameter: f32) -> Self {
        Self { diameter }
    }
}

// Find other name for the structure so it does not conflicts with Box pointer
impl Box {
    pub fn new(width: f32, height: f32, depth: f32) -> Self {
        Box {
            width,
            height,
            depth,
        }
    }
}
