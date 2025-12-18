use dear_imgui_rs::*;
use dear_imgui_wgpu::WgpuRenderer;
use dear_imgui_winit::WinitPlatform;
use pollster::block_on;
use std::{sync::Arc, time::Instant};
use tracing::{debug, error, info, trace, warn};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::{Window, WindowId},
};

struct ImguiState {
    context: Context,
    platform: WinitPlatform,
    renderer: WgpuRenderer,
    clear_color: wgpu::Color,
    demo_open: bool,
    last_frame: Instant,
    // Logging demo state
    log_counter: i32,
    frame_count: u64,
    total_frame_time: f32,
}

struct AppWindow {
    device: wgpu::Device,
    queue: wgpu::Queue,
    window: Arc<Window>,
    surface_desc: wgpu::SurfaceConfiguration,
    surface: wgpu::Surface<'static>,
    imgui: ImguiState,
}

#[derive(Default)]
struct App {
    window: Option<AppWindow>,
}

impl AppWindow {
    fn new(event_loop: &ActiveEventLoop) -> Result<Self, Box<dyn std::error::Error>> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });

        let window = {
            let version = env!("CARGO_PKG_VERSION");
            let size = LogicalSize::new(1280.0, 720.0);

            Arc::new(
                event_loop.create_window(
                    Window::default_attributes()
                        .with_title(format!("Dear ImGui Hello World - {version}"))
                        .with_inner_size(size),
                )?,
            )
        };

        let surface = instance.create_surface(window.clone())?;

        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .expect("Failed to find an appropriate adapter");

        let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))?;

        // Use the window's actual physical size for the surface, not a fixed logical size.
        let physical_size = window.inner_size();
        // Pick an sRGB surface format when available for consistent visuals
        let caps = surface.get_capabilities(&adapter);
        let preferred_srgb = [
            wgpu::TextureFormat::Bgra8UnormSrgb,
            wgpu::TextureFormat::Rgba8UnormSrgb,
        ];
        let format = preferred_srgb
            .iter()
            .cloned()
            .find(|f| caps.formats.contains(f))
            .unwrap_or(caps.formats[0]);

        let surface_desc = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: physical_size.width,
            height: physical_size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&device, &surface_desc);

        // Setup ImGui immediately
        let mut context = Context::create();
        context.set_ini_filename(None::<String>).unwrap();

        let mut platform = WinitPlatform::new(&mut context);
        platform.attach_window(&window, dear_imgui_winit::HiDpiMode::Default, &mut context);

        // Method 1: One-step initialization (recommended)
        let init_info =
            dear_imgui_wgpu::WgpuInitInfo::new(device.clone(), queue.clone(), surface_desc.format);
        let mut renderer =
            WgpuRenderer::new(init_info, &mut context).expect("Failed to initialize WGPU renderer");
        // Unify visuals (sRGB): auto gamma by format, matches official practice
        renderer.set_gamma_mode(dear_imgui_wgpu::GammaMode::Auto);

        // Log successful initialization
        dear_imgui_rs::logging::log_context_created();
        dear_imgui_rs::logging::log_platform_init("Winit");
        dear_imgui_rs::logging::log_renderer_init("WGPU");

        let imgui = ImguiState {
            context,
            platform,
            renderer,
            clear_color: wgpu::Color {
                r: 0.1,
                g: 0.2,
                b: 0.3,
                a: 1.0,
            },
            demo_open: true,
            last_frame: Instant::now(),
            log_counter: 0,
            frame_count: 0,
            total_frame_time: 0.0,
        };

        Ok(Self {
            device,
            queue,
            window,
            surface_desc,
            surface,
            imgui,
        })
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.surface_desc.width = new_size.width;
            self.surface_desc.height = new_size.height;
            self.surface.configure(&self.device, &self.surface_desc);
        }
    }

    fn render(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let now = Instant::now();
        let delta_time = now - self.imgui.last_frame;
        let delta_secs = delta_time.as_secs_f32();

        // Update frame statistics
        self.imgui.frame_count += 1;
        self.imgui.total_frame_time += delta_secs;

        // Log frame statistics every 60 frames
        if self.imgui.frame_count % 60 == 0 {
            let avg_frame_time = self.imgui.total_frame_time / 60.0;
            dear_imgui_rs::logging::log_frame_stats(avg_frame_time, 1.0 / avg_frame_time);
            self.imgui.total_frame_time = 0.0;
        }

        self.imgui.context.io_mut().set_delta_time(delta_secs);
        self.imgui.last_frame = now;

        let frame = match self.surface.get_current_texture() {
            Ok(frame) => frame,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                // Surface changed (e.g., moved between monitors / DPI change); reconfigure
                self.surface.configure(&self.device, &self.surface_desc);
                return Ok(());
            }
            Err(wgpu::SurfaceError::Timeout) => {
                // Non-fatal; skip this frame
                return Ok(());
            }
            Err(e) => return Err(Box::new(e)),
        };

        self.imgui
            .platform
            .prepare_frame(&self.window, &mut self.imgui.context);
        let ui = self.imgui.context.frame();

        // Main window content
        ui.window("Hello, Dear ImGui!")
            .size([400.0, 300.0], Condition::FirstUseEver)
            .build(|| {
                ui.text("Welcome to Dear ImGui Rust bindings!");
                ui.separator();

                ui.text(format!(
                    "Application average {:.3} ms/frame ({:.1} FPS)",
                    1000.0 / ui.io().framerate(),
                    ui.io().framerate()
                ));

                let mut color = [
                    self.imgui.clear_color.r as f32,
                    self.imgui.clear_color.g as f32,
                    self.imgui.clear_color.b as f32,
                    self.imgui.clear_color.a as f32,
                ];

                if ui.color_edit4("Clear color", &mut color) {
                    self.imgui.clear_color.r = color[0] as f64;
                    self.imgui.clear_color.g = color[1] as f64;
                    self.imgui.clear_color.b = color[2] as f64;
                    self.imgui.clear_color.a = color[3] as f64;
                }

                if ui.button("Show Demo Window") {
                    self.imgui.demo_open = true;
                }
            });

        // Logging Demo Window
        ui.window("Logging Demo")
            .size([350.0, 250.0], Condition::FirstUseEver)
            .build(|| {
                ui.text("Dear ImGui Logging Features");
                ui.separator();

                // Counter with logging
                if ui.button("Increment Counter") {
                    self.imgui.log_counter += 1;
                    info!("Counter incremented to: {}", self.imgui.log_counter);
                }
                ui.same_line();
                ui.text(format!("Count: {}", self.imgui.log_counter));

                ui.separator();

                // Log level buttons
                ui.text("Generate log messages:");

                if ui.button("Trace") {
                    trace!("This is a trace message - very detailed debugging info");
                }
                ui.same_line();
                if ui.button("Debug") {
                    debug!("This is a debug message - general debugging info");
                }
                ui.same_line();
                if ui.button("Info") {
                    info!("This is an info message - general information");
                }

                if ui.button("Warn") {
                    warn!("This is a warning message - something might be wrong");
                }
                ui.same_line();
                if ui.button("Error") {
                    error!("This is an error message - something went wrong!");
                }

                ui.separator();

                // Error handling demo
                if ui.button("Test Error Handling") {
                    let result: Result<(), ImGuiError> =
                        Err(ImGuiError::resource_allocation("Demo texture"));
                    if let Err(e) = result {
                        error!("Simulated error: {}", e);
                    }
                }

                ui.separator();
                ui.text_wrapped("Check your console/terminal for log output!");
                ui.text_wrapped("Set RUST_LOG environment variable to control verbosity:");
                ui.text("  RUST_LOG=debug cargo run --example wgpu_basic");
            });

        // Show demo window if requested
        if self.imgui.demo_open {
            ui.show_demo_window(&mut self.imgui.demo_open);
        }

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        let draw_data = self.imgui.context.render();

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(self.imgui.clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            // Call new_frame before rendering
            self.imgui
                .renderer
                .new_frame()
                .expect("Failed to prepare new frame");

            self.imgui
                .renderer
                .render_draw_data(draw_data, &mut rpass)?;
        }

        self.queue.submit(Some(encoder.finish()));
        frame.present();
        Ok(())
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // For compatibility with older winit versions and mobile platforms
        if self.window.is_none() {
            match AppWindow::new(event_loop) {
                Ok(window) => {
                    self.window = Some(window);
                    info!("Window created successfully in resumed");
                }
                Err(e) => {
                    error!("Failed to create window in resumed: {e}");
                    event_loop.exit();
                }
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let window = match self.window.as_mut() {
            Some(window) => window,
            None => return,
        };

        // Handle the event with ImGui first (window-local path)
        window.imgui.platform.handle_window_event(
            &mut window.imgui.context,
            &window.window,
            &event,
        );

        match event {
            WindowEvent::Resized(physical_size) => {
                window.resize(physical_size);
                window.window.request_redraw();
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                // DPI changed: update surface to new physical size
                let new_size = window.window.inner_size();
                window.resize(new_size);
                window.window.request_redraw();
            }
            WindowEvent::CloseRequested => {
                info!("Close requested");
                event_loop.exit();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.logical_key == Key::Named(NamedKey::Escape) {
                    event_loop.exit();
                }
            }
            WindowEvent::RedrawRequested => {
                if let Err(e) = window.render() {
                    error!("Render error: {e}");
                }
                window.window.request_redraw();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        // Request redraw if we have a window
        if let Some(window) = &self.window {
            window.window.request_redraw();
        }
    }
}

fn main() {
    // Initialize tracing with custom filter for demo
    dear_imgui_rs::logging::init_tracing_with_filter("dear_imgui=debug,wgpu_basic=info,wgpu=warn");

    info!("Starting Dear ImGui WGPU Basic Example with Logging Demo");
    info!("This example demonstrates:");
    info!("  - Basic Dear ImGui usage with WGPU backend");
    info!("  - Integrated tracing/logging support");
    info!("  - Error handling with thiserror");
    info!("  - Frame statistics logging");

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::default();

    info!("Starting event loop...");
    event_loop.run_app(&mut app).unwrap();

    info!("Dear ImGui WGPU Basic Example finished");
}
