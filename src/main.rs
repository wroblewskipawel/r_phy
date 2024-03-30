use std::{error::Error, path::Path, result::Result};
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
    renderer::{
        camera::CameraType,
        model::{Material, Model},
        RendererBackend,
    },
};

fn main() -> Result<(), Box<dyn Error>> {
    let meshes = vec![
        shape::Sphere::new(1.5).into(),
        shape::Cube::new(1.0).into(),
        shape::Box::new(3.0, 1.0, 1.0).into(),
    ];
    let materials = vec![
        Material::builder()
            .with_albedo(Path::new("assets/textures/tile_1.png"))
            .build()?,
        Material::builder()
            .with_albedo(Path::new("assets/textures/tile_2.png"))
            .build()?,
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
    let (game_loop, mesh_handles, material_handles) = LoopBuilder::new()
        .with_window(window_builder)
        .with_renderer(RendererBackend::Vulkan)
        .with_camera(CameraType::FirstPerson, proj)
        .with_meshes(meshes)
        .with_materials(materials)
        .build()?;
    let objects = vec![
        Object::new(
            Model::new(mesh_handles[0], material_handles[0]),
            Transform::identity(),
            Box::new(|elapsed_time, transform| transform.rotate(Vector3::z(), elapsed_time)),
        ),
        Object::new(
            Model::new(mesh_handles[1], material_handles[1]),
            Transform::identity().translate(Vector3::new(0.0, 0.0, 3.0)),
            Box::new(|elapsed_time, transform| transform.rotate(Vector3::z(), 2.0 * elapsed_time)),
        ),
        Object::new(
            Model::new(mesh_handles[2], material_handles[1]),
            Transform::identity().translate(Vector3::new(3.0, 0.0, 0.0)),
            Box::new(|elapsed_time, transform| transform.rotate(Vector3::z(), 3.0 * elapsed_time)),
        ),
    ];
    game_loop.with_objects(objects).run()?;
    Ok(())
}
