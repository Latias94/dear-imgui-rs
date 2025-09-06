use dear_imgui::*;
use dear_imgui_wgpu::WgpuRenderer;
use dear_imgui_winit::WinitPlatform;
use pollster::block_on;
use std::{sync::Arc, time::Instant};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::Window,
};

struct ImguiState {
    context: Context,
    platform: WinitPlatform,
    renderer: WgpuRenderer,
    last_frame: Instant,
    colors: [[f32; 4]; 4],
    thickness: f32,
    curve_segments: i32,
    draw_bg: bool,
}

struct DrawDemoApp {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface_desc: wgpu::SurfaceConfiguration,
    imgui: Option<ImguiState>,
}

impl ApplicationHandler for DrawDemoApp {
    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {
        // Window is already created in new()
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _window_id: winit::window::WindowId, event: WindowEvent) {
        if let Some(imgui) = &mut self.imgui {
            let full_event: winit::event::Event<()> = winit::event::Event::WindowEvent {
                window_id: self.window.id(),
                event: event.clone()
            };
            imgui.platform.handle_event(&full_event, &self.window, &mut imgui.context);
        }

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.logical_key == Key::Named(NamedKey::Escape) {
                    event_loop.exit();
                }
            }
            WindowEvent::Resized(new_size) => {
                self.surface_desc.width = new_size.width.max(1);
                self.surface_desc.height = new_size.height.max(1);
                self.surface.configure(&self.device, &self.surface_desc);
            }
            WindowEvent::RedrawRequested => {
                self.render();
                self.window.request_redraw();
            }
            _ => {}
        }
    }
}

impl DrawDemoApp {
    fn new(event_loop: &EventLoop<()>) -> Self {
        let size = LogicalSize::new(1280.0, 720.0);
        let window = Arc::new(
            event_loop.create_window(
                winit::window::Window::default_attributes()
                    .with_title("Dear ImGui Draw Demo")
                    .with_inner_size(size)
            ).unwrap(),
        );

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });

        let surface = instance.create_surface(window.clone()).unwrap();

        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .unwrap();

        let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let window_size = window.inner_size();
        let surface_desc = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: window_size.width,
            height: window_size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&device, &surface_desc);

        let mut app = Self {
            window,
            surface,
            device,
            queue,
            surface_desc,
            imgui: None,
        };

        app.setup_imgui();
        app
    }

    fn setup_imgui(&mut self) {
        let mut context = Context::create();
        context.set_ini_filename::<&str>(None);

        let mut platform = WinitPlatform::new(&mut context);
        platform.attach_window(&self.window, dear_imgui_winit::HiDpiMode::Default, &mut context);

        let mut renderer = WgpuRenderer::new(
            &self.device,
            &self.queue,
            self.surface_desc.format,
        );

        // Load font texture - this is crucial for text rendering!
        renderer.reload_font_texture(&mut context, &self.device, &self.queue);

        self.imgui = Some(ImguiState {
            context,
            platform,
            renderer,
            last_frame: Instant::now(),
            colors: [
                [1.0, 0.0, 0.0, 1.0], // Red
                [0.0, 1.0, 0.0, 1.0], // Green
                [0.0, 0.0, 1.0, 1.0], // Blue
                [1.0, 1.0, 0.0, 1.0], // Yellow
            ],
            thickness: 2.0,
            curve_segments: 16,
            draw_bg: true,
        });
    }

    fn render(&mut self) {
        let imgui = self.imgui.as_mut().unwrap();

        let now = Instant::now();
        imgui.context.io_mut().set_delta_time((now - imgui.last_frame).as_secs_f32());
        imgui.last_frame = now;

        let frame = match self.surface.get_current_texture() {
            Ok(frame) => frame,
            Err(e) => {
                eprintln!("dropped frame: {e:?}");
                return;
            }
        };

        imgui.platform.prepare_frame(&self.window, &mut imgui.context);
        let ui = imgui.context.frame();

        // Control panel
        ui.window("Draw Demo Controls")
            .size([300.0, 200.0], Condition::FirstUseEver)
            .build(|| {
                ui.text("Drawing Controls");
                ui.separator();

                ui.checkbox("Draw background", &mut imgui.draw_bg);
                ui.slider("Thickness", 0.5, 10.0, &mut imgui.thickness);
                ui.slider("Curve segments", 3, 64, &mut imgui.curve_segments);

                ui.separator();
                ui.text("Colors:");
                ui.color_edit4("Red", &mut imgui.colors[0]);
                ui.color_edit4("Green", &mut imgui.colors[1]);
                ui.color_edit4("Blue", &mut imgui.colors[2]);
                ui.color_edit4("Yellow", &mut imgui.colors[3]);
            });

        // Simple test window without drawing
        ui.window("Simple Test")
            .size([400.0, 300.0], Condition::FirstUseEver)
            .build(|| {
                ui.text("Hello from Draw Demo!");
                ui.text("This is a simplified version to test basic functionality.");

                if ui.button("Test Button") {
                    println!("Button clicked!");
                }

                ui.separator();
                ui.text("If you can see this, the basic UI is working.");
            });

        // Render
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.2,
                            b: 0.3,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            let _ = imgui.renderer.render_with_renderpass(imgui.context.render(), &self.queue, &self.device, &mut rpass);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
    }
}

// Helper function to convert f32 color to u32
fn color_u32_from_f32(color: [f32; 4]) -> u32 {
    let r = (color[0] * 255.0) as u32;
    let g = (color[1] * 255.0) as u32;
    let b = (color[2] * 255.0) as u32;
    let a = (color[3] * 255.0) as u32;
    (a << 24) | (b << 16) | (g << 8) | r
}

fn main() {
    env_logger::init();

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = DrawDemoApp::new(&event_loop);
    app.window.request_redraw();

    event_loop.run_app(&mut app).unwrap();
}
