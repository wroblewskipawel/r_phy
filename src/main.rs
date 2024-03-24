use std::{error::Error, result::Result, time::Instant};
use winit::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{WindowBuilder, WindowButtons},
};

use r_phy::{
    math::{
        transform::Transform,
        types::{Matrix4, Vector3},
    },
    physics::shape,
    renderer::{camera::Camera, RendererBackend},
};

fn main() -> Result<(), Box<dyn Error>> {
    let event_loop = EventLoop::new()?;
    let window = WindowBuilder::new()
        .with_inner_size(LogicalSize {
            width: 800,
            height: 600,
        })
        .with_resizable(false)
        .with_enabled_buttons(WindowButtons::CLOSE | WindowButtons::MINIMIZE)
        .with_title("r_phy")
        .with_transparent(false)
        .build(&event_loop)?;
    let mut renderer = RendererBackend::Vulkan.create(&window)?;
    let meshes = renderer.load_meshes(&[
        shape::Sphere::new(0.75).into(),
        shape::Cube::new(1.0).into(),
        shape::Box::new(3.0, 1.0, 1.0).into(),
    ])?;
    let camera = Camera {
        view: Matrix4::look_at(
            Vector3::new(0.0, -10.0, 10.0),
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::z(),
        ),
        proj: Matrix4::perspective(std::f32::consts::FRAC_PI_3, 600.0 / 800.0, 1e-4, 1e4),
    };
    let mut previous_frame_time = Instant::now();
    let mut model_rotation = 0.0;
    event_loop.set_control_flow(ControlFlow::Poll);
    event_loop.run(move |event, elwt| {
        let current_frame_time = Instant::now();
        let elapsed_time = (current_frame_time - previous_frame_time).as_secs_f32();
        previous_frame_time = current_frame_time;
        model_rotation += elapsed_time * (2.0 * std::f32::consts::PI);
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                elwt.exit();
            }
            Event::AboutToWait => {
                let _ = renderer.begin_frame(&camera);
                let _ = renderer.draw(
                    meshes[0],
                    &(Transform::identity()
                        .rotate(Vector3::z(), model_rotation)
                        .translate(Vector3::new(0.0, 3.0, 0.0))
                        .rotate(Vector3::z(), model_rotation / 10.0)
                        .translate(Vector3::new(0.0, 0.0, 1.0 + (model_rotation / 5.0).sin())))
                    .rotate(
                        Vector3::new(0.0, 1.0, 2.0).norm(),
                        std::f32::consts::FRAC_PI_2,
                    )
                    .into(),
                );
                let _ = renderer.draw(
                    meshes[2],
                    &(Transform::identity()
                        .rotate(Vector3::y(), -model_rotation / 2.0)
                        .rotate(Vector3::z(), -model_rotation / 3.0)
                        .translate(Vector3::new(0.0, 4.0, 0.0))
                        .rotate(Vector3::z(), -model_rotation / 6.0)
                        .translate(Vector3::new(0.0, 0.0, 1.0 + (model_rotation / 2.0).cos())))
                    .rotate(
                        Vector3::new(1.0, -1.0, 0.0).norm(),
                        std::f32::consts::FRAC_PI_2,
                    )
                    .into(),
                );
                let _ = renderer.draw(
                    meshes[1],
                    &(<Transform as Into<Matrix4>>::into(
                        Transform::identity().rotate(Vector3::z(), model_rotation / 3.0),
                    ) * Matrix4::scale(2.0)),
                );
                let _ = renderer.end_frame();
            }
            _ => (),
        }
    })?;
    Ok(())
}
