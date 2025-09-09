use dear_imgui::*;
use dear_imgui_wgpu::WgpuRenderer;
use dear_imgui_winit::WinitPlatform;
use std::time::Instant;
use wgpu::{
    Backends, Color as WgpuColor, CommandEncoderDescriptor, Device, DeviceDescriptor, Features,
    Instance, InstanceDescriptor, Limits, LoadOp, MemoryHints, Operations, PowerPreference, Queue,
    RenderPassColorAttachment, RenderPassDescriptor, RequestAdapterOptions, StoreOp, Surface,
    SurfaceConfiguration, TextureUsages, TextureViewDescriptor, Trace,
};
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

struct SimpleApp {
    window: Option<Window>,
    device: Option<Device>,
    queue: Option<Queue>,
    surface_config: Option<SurfaceConfiguration>,
    imgui_ctx: Option<Context>,
    platform: Option<WinitPlatform>,
    renderer: Option<WgpuRenderer>,
    last_frame: Instant,
}

impl SimpleApp {
    fn new() -> Self {
        Self {
            window: None,
            device: None,
            queue: None,
            surface_config: None,
            imgui_ctx: None,
            platform: None,
            renderer: None,
            last_frame: Instant::now(),
        }
    }

    fn render(&mut self) {
        if let (Some(imgui_ctx), Some(platform), Some(window)) =
            (&mut self.imgui_ctx, &mut self.platform, &self.window)
        {
            platform.prepare_frame(window, imgui_ctx);
            let ui = imgui_ctx.frame();

            // Simple UI
            ui.window("Hello World")
                .size([300.0, 200.0], Condition::FirstUseEver)
                .build(|| {
                    ui.text("Hello, Dear ImGui!");
                    ui.separator();
                    ui.text("This is a simple test without multi-viewport.");

                    if ui.button("Test Button") {
                        println!("Button clicked!");
                    }
                });

            // Skip rendering for now to test basic setup
            println!("Rendering frame...");
        }
    }
}

impl ApplicationHandler for SimpleApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        println!("Application resumed");

        let window = event_loop
            .create_window(
                Window::default_attributes()
                    .with_title("Simple Dear ImGui Test")
                    .with_inner_size(winit::dpi::LogicalSize::new(800, 600)),
            )
            .unwrap();

        // Initialize wgpu
        let instance = Instance::new(&InstanceDescriptor {
            backends: Backends::PRIMARY,
            ..Default::default()
        });

        // Store window first to avoid lifetime issues
        self.window = Some(window);

        // Skip wgpu setup for now

        // Initialize Dear ImGui
        let mut imgui_ctx = Context::create();
        imgui_ctx.set_ini_filename(Some(std::path::PathBuf::from("simple_test.ini")));

        let mut platform = WinitPlatform::new(&mut imgui_ctx);
        platform.attach_window(
            self.window.as_ref().unwrap(),
            dear_imgui_winit::HiDpiMode::Default,
            &mut imgui_ctx,
        );

        self.imgui_ctx = Some(imgui_ctx);
        self.platform = Some(platform);

        println!("Initialization complete");
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                println!("Close requested");
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                self.render();
            }
            _ => {}
        }

        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

fn main() {
    println!("Starting Simple Dear ImGui Test");

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = SimpleApp::new();
    event_loop.run_app(&mut app).unwrap();
}
