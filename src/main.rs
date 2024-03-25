use std::{error::Error, result::Result};
use winit::{
    dpi::PhysicalSize,
    window::{WindowBuilder, WindowButtons},
};

use r_phy::{
    core::{LoopBuilder, Object},
    math::{
        transform::Transform,
        types::{Matrix4, Vector3},
    },
    physics::shape,
    renderer::{camera::CameraType, RendererBackend},
};

fn main() -> Result<(), Box<dyn Error>> {
    let meshes = vec![
        shape::Sphere::new(0.75).into(),
        shape::Cube::new(1.0).into(),
        shape::Box::new(3.0, 1.0, 1.0).into(),
    ];
    let proj = Matrix4::perspective(std::f32::consts::FRAC_PI_3, 600.0 / 800.0, 1e-4, 1e4);
    let window_builder = WindowBuilder::new()
        .with_inner_size(PhysicalSize {
            width: 800,
            height: 600,
        })
        .with_resizable(false)
        .with_enabled_buttons(WindowButtons::CLOSE | WindowButtons::MINIMIZE)
        .with_title("r_phy")
        .with_transparent(false);
    let (loop_builder, mesh_handles) = LoopBuilder::new()
        .with_window(window_builder)
        .with_renderer(RendererBackend::Vulkan)
        .with_camera(CameraType::FirstPerson, proj)
        .with_meshes(meshes);
    loop_builder
        .with_object(Object::new(
            mesh_handles[0],
            Transform::identity(),
            Box::new(|elapsed_time, transform| transform.rotate(Vector3::z(), elapsed_time)),
        ))
        .with_object(Object::new(
            mesh_handles[1],
            Transform::identity().translate(Vector3::new(0.0, 0.0, 3.0)),
            Box::new(|elapsed_time, transform| transform.rotate(Vector3::z(), 2.0 * elapsed_time)),
        ))
        .with_object(Object::new(
            mesh_handles[2],
            Transform::identity().translate(Vector3::new(3.0, 0.0, 0.0)),
            Box::new(|elapsed_time, transform| transform.rotate(Vector3::z(), 3.0 * elapsed_time)),
        ))
        .build()?
        .run()?;
    Ok(())
}
