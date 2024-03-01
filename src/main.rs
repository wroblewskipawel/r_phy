use std::{error::Error, result::Result};
use winit::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{WindowBuilder, WindowButtons},
};

use r_phy::renderer::{mesh::Mesh, RendererBackend};

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
    let mesh = renderer.load_mesh(&Mesh::triangle())?;
    event_loop.set_control_flow(ControlFlow::Poll);
    event_loop.run(move |event, elwt| match event {
        Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } => {
            elwt.exit();
        }
        Event::AboutToWait => {
            let _ = renderer.begin_frame();
            let _ = renderer.draw(mesh);
            let _ = renderer.end_frame();
        }
        _ => (),
    })?;
    Ok(())
}
