use ash::vk;
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
    physics::shape::Cube,
    renderer::{
        camera::first_person::FirstPersonCameraBuilder,
        model::{CommonVertex, EmptyMaterial, Model, PbrMaterial, SimpleVertex, UnlitMaterial},
        shader::Shader,
        vulkan::{DeferredShader, VulkanRendererBuilder, VulkanRendererConfig},
        ContextBuilder,
    },
};

const RENDERER_MEM_ALLOC_PAGE_SIZE: vk::DeviceSize = 128 * 1024 * 1024;

fn main() -> Result<(), Box<dyn Error>> {
    let renderer_builder = VulkanRendererBuilder::new()
        .with_config(
            VulkanRendererConfig::builder()
                .with_page_size(RENDERER_MEM_ALLOC_PAGE_SIZE)
                .build()?,
        )
        .with_material_type::<UnlitMaterial>()
        .with_material_type::<PbrMaterial>()
        .with_material_type::<EmptyMaterial>()
        .with_vertex_type::<CommonVertex>()
        .with_vertex_type::<SimpleVertex>()
        .with_shader_type(Shader::<CommonVertex, EmptyMaterial>::marker())
        .with_shader_type(Shader::<CommonVertex, UnlitMaterial>::marker())
        .with_shader_type(Shader::<CommonVertex, PbrMaterial>::marker());
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
    let mut context_builder = game_loop.context_builder();
    let empty_material = context_builder.add_material(EmptyMaterial::default());
    let cube_mesh = context_builder.add_mesh::<CommonVertex, _>(Cube::new(1.0f32).into());
    // TODO: Explicit type conversion to the type used by selected renderer should not be visible at the front-end
    let checker_shader =
        context_builder.add_shader::<Shader<_, _>, DeferredShader<_>, _>(Shader::<
            CommonVertex,
            EmptyMaterial,
        >::new(
            "shaders/spv/deferred/gbuffer_write/checker",
        ));
    let scene = game_loop.scene(context_builder)?.with_objects(
        checker_shader,
        vec![
            Object::new(
                Model::new(cube_mesh, empty_material),
                Transform::identity().translate(Vector3::new(4.0, 0.0, 0.0)),
                Box::new(|elapsed_time, transform| {
                    Transform::identity()
                        .rotate(Vector3::z(), elapsed_time * std::f32::consts::FRAC_PI_2)
                        * transform
                }),
            ),
            Object::new(
                Model::new(cube_mesh, empty_material),
                Transform::identity().translate(Vector3::new(4.0, 2.0, 0.0)),
                Box::new(|elapsed_time, transform| {
                    Transform::identity()
                        .rotate(Vector3::z(), elapsed_time * std::f32::consts::FRAC_PI_2)
                        * transform
                }),
            ),
        ],
    );
    game_loop.run(scene)?;
    Ok(())
}
