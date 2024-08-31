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
        model::{
            CommonVertex, EmptyMaterial, Image, Mesh, Model, PbrMaterial, SimpleVertex,
            UnlitMaterial,
        },
        shader::Shader,
        vulkan::VulkanRendererBuilder,
    },
};

fn main() -> Result<(), Box<dyn Error>> {
    let (gltf_mesh, gltf_material) =
        Mesh::load_gltf(Path::new("assets/gltf/WaterBottle/glTF/WaterBottle.gltf"))?;
    let meshes: Vec<Mesh<CommonVertex>> = vec![
        shape::Sphere::new(1.5).into(),
        shape::Cube::new(1.0).into(),
        shape::Box::new(3.0, 1.0, 1.0).into(),
        gltf_mesh,
    ];
    let simple_meshes: Vec<Mesh<SimpleVertex>> = vec![
        shape::Sphere::new(1.5).into(),
        shape::Cube::new(1.0).into(),
        shape::Box::new(3.0, 1.0, 1.0).into(),
    ];
    let unlit_materials = vec![
        UnlitMaterial::builder()
            .with_albedo(Image::File(Path::new("assets/textures/tile_1.png").into()))
            .build()?,
        UnlitMaterial::builder()
            .with_albedo(Image::File(Path::new("assets/textures/tile_2.png").into()))
            .build()?,
    ];
    let empty_material = vec![EmptyMaterial::default()];
    let prb_materials = vec![gltf_material];

    let renderer_builder = VulkanRendererBuilder::new()
        .with_material_type::<UnlitMaterial>()
        .with_material_type::<PbrMaterial>()
        .with_material_type::<EmptyMaterial>()
        .with_vertex_type::<CommonVertex>()
        .with_vertex_type::<SimpleVertex>()
        .with_shader_type(Shader::<CommonVertex, EmptyMaterial>::marker())
        .with_shader_type(Shader::<CommonVertex, UnlitMaterial>::marker())
        .with_shader_type(Shader::<CommonVertex, PbrMaterial>::marker())
        .with_meshes(simple_meshes)
        .with_meshes(meshes)
        .with_materials(unlit_materials)
        .with_materials(prb_materials)
        .with_materials(empty_material)
        .with_shaders(vec![Shader::<CommonVertex, EmptyMaterial>::new(
            "shaders/spv/deferred/gbuffer_write/checker",
        )])
        .with_shaders(vec![Shader::<CommonVertex, UnlitMaterial>::new(
            "shaders/spv/deferred/gbuffer_write/unlit",
        )])
        .with_shaders(vec![Shader::<CommonVertex, PbrMaterial>::new(
            "shaders/spv/deferred/gbuffer_write/pbr",
        )]);
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
    let camera_builder = FirstPersonCameraBuilder::new(proj);
    let game_loop = LoopBuilder::new()
        .with_window(window_builder)
        .with_renderer(renderer_builder)
        .with_camera(camera_builder)
        .build()?;
    let meshes = game_loop.get_mesh_handles::<CommonVertex>().unwrap();
    let empty_materials = game_loop.get_material_handles::<EmptyMaterial>().unwrap();
    let unlit_materials = game_loop.get_material_handles::<UnlitMaterial>().unwrap();
    let pbr_materials = game_loop.get_material_handles::<PbrMaterial>().unwrap();
    let empty_complex_shader_handles = game_loop
        .get_shader_handles::<Shader<CommonVertex, EmptyMaterial>>()
        .unwrap();
    let pbr_complex_shader_handles = game_loop
        .get_shader_handles::<Shader<CommonVertex, PbrMaterial>>()
        .unwrap();
    let pbr_unlit_shader_handles = game_loop
        .get_shader_handles::<Shader<CommonVertex, UnlitMaterial>>()
        .unwrap();
    let scene = game_loop
        .scene()
        .with_objects(
            pbr_complex_shader_handles[0],
            vec![Object::new(
                Model::new(meshes[3], pbr_materials[0]),
                Transform::identity().translate(Vector3::new(0.0, 0.0, -3.0)),
                Box::new(|elapsed_time, transform| {
                    transform.rotate(Vector3::z(), 3.0 * elapsed_time)
                }),
            )],
        )
        .with_objects(
            pbr_unlit_shader_handles[0],
            vec![
                Object::new(
                    Model::new(meshes[0], unlit_materials[0]),
                    Transform::identity(),
                    Box::new(|elapsed_time, transform| {
                        transform.rotate(Vector3::z(), elapsed_time)
                    }),
                ),
                Object::new(
                    Model::new(meshes[0], unlit_materials[0]),
                    Transform::identity().translate(Vector3::new(0.0, 4.0, 4.0)),
                    Box::new(|elapsed_time, transform| {
                        transform.rotate(Vector3::z(), 2.0 * elapsed_time)
                    }),
                ),
                Object::new(
                    Model::new(meshes[0], unlit_materials[0]),
                    Transform::identity().translate(Vector3::new(0.0, 2.0, 2.0)),
                    Box::new(|elapsed_time, transform| {
                        transform.rotate(Vector3::z(), 3.0 * elapsed_time)
                    }),
                ),
                Object::new(
                    Model::new(meshes[1], unlit_materials[1]),
                    Transform::identity().translate(Vector3::new(0.0, 0.0, 3.0)),
                    Box::new(|elapsed_time, transform| {
                        transform.rotate(Vector3::z(), 2.0 * elapsed_time)
                    }),
                ),
            ],
        )
        .with_objects(
            empty_complex_shader_handles[0],
            vec![
                Object::new(
                    Model::new(meshes[2], empty_materials[0]),
                    Transform::identity().translate(Vector3::new(3.0, 0.0, 0.0)),
                    Box::new(|elapsed_time, transform| {
                        transform.rotate(Vector3::z(), 3.0 * elapsed_time)
                    }),
                ),
                Object::new(
                    Model::new(meshes[2], empty_materials[0]),
                    Transform::identity().translate(Vector3::new(-4.0, -4.0, 0.0)),
                    Box::new(|elapsed_time, transform| {
                        transform
                            .rotate(Vector3::z(), 3.0 * elapsed_time)
                            .rotate(Vector3::y(), 3.0 * elapsed_time)
                    }),
                ),
            ],
        );
    game_loop.run(scene)?;
    Ok(())
}
