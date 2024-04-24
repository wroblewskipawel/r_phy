// TYpe list idea:
// Implement generic trait that builds type list node based on trait generic parameter
// NodeBuilder<Node> -> Builds TypeLIst Node
// End -> Builds TypeList Terminator

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
        camera::first_person::FirstPersonCameraBuilder,
        model::{CommonVertex, Image, Mesh, Model, PbrMaterial, UnlitMaterial},
        vulkan::VulkanRendererBuilder,
    },
};

fn main() -> Result<(), Box<dyn Error>> {
    let (gltf_mesh, gltf_material) =
        Mesh::load_gltf(Path::new("assets/gltf/WaterBottle/glTF/WaterBottle.gltf"))?;
    let meshes = vec![
        shape::Sphere::new(1.5).into(),
        shape::Cube::new(1.0).into(),
        shape::Box::new(3.0, 1.0, 1.0).into(),
        gltf_mesh,
    ];
    let unlit_materials = vec![
        UnlitMaterial::builder()
            .with_albedo(Image::File(Path::new("assets/textures/tile_1.png").into()))
            .build()?,
        UnlitMaterial::builder()
            .with_albedo(Image::File(Path::new("assets/textures/tile_2.png").into()))
            .build()?,
    ];
    let prb_materials = vec![gltf_material];
    let renderer = VulkanRendererBuilder::new()
        .with_materials(
            unlit_materials,
            Path::new("shaders/spv/deferred/gbuffer_write/unlit").to_owned(),
        )
        .with_materials(
            prb_materials,
            Path::new("shaders/spv/deferred/gbuffer_write/pbr").to_owned(),
        )
        .with_meshes(meshes);
    let proj = Matrix4::perspective(std::f32::consts::FRAC_PI_3, 600.0 / 800.0, 1e-3, 1e3);
    let window_builder = WindowBuilder::new()
        .with_inner_size(PhysicalSize {
            width: 800,
            height: 600,
        })
        .with_resizable(false)
        .with_enabled_buttons(WindowButtons::CLOSE | WindowButtons::MINIMIZE)
        .with_title("r_phy")
        .with_transparent(false);
    let camera = FirstPersonCameraBuilder::new(proj);
    let game_loop = LoopBuilder::new()
        .with_window(window_builder)
        .with_renderer(renderer)
        .with_camera(camera)
        .build()?;
    let meshes = game_loop.get_mesh_handles::<CommonVertex>().unwrap();
    let unlit_materials = game_loop.get_material_handles::<UnlitMaterial>().unwrap();
    let pbr_materials = game_loop.get_material_handles::<PbrMaterial>().unwrap();
    let scene = game_loop
        .scene()
        .with_objects(vec![Object::new(
            Model::new(meshes[3], pbr_materials[0]),
            Transform::identity().translate(Vector3::new(0.0, 0.0, -3.0)),
            Box::new(|elapsed_time, transform| transform.rotate(Vector3::z(), 3.0 * elapsed_time)),
        )])
        .with_objects(vec![
            Object::new(
                Model::new(meshes[0], unlit_materials[0]),
                Transform::identity(),
                Box::new(|elapsed_time, transform| transform.rotate(Vector3::z(), elapsed_time)),
            ),
            Object::new(
                Model::new(meshes[1], unlit_materials[1]),
                Transform::identity().translate(Vector3::new(0.0, 0.0, 3.0)),
                Box::new(|elapsed_time, transform| {
                    transform.rotate(Vector3::z(), 2.0 * elapsed_time)
                }),
            ),
            Object::new(
                Model::new(meshes[2], unlit_materials[1]),
                Transform::identity().translate(Vector3::new(3.0, 0.0, 0.0)),
                Box::new(|elapsed_time, transform| {
                    transform.rotate(Vector3::z(), 3.0 * elapsed_time)
                }),
            ),
        ]);
    game_loop.run(scene)?;
    Ok(())
}
