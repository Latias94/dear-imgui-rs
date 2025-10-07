//! Minimal multi-viewport sample using winit + wgpu backends
//!
//! ⚠️ **EXPERIMENTAL TEST EXAMPLE ONLY** ⚠️
//!
//! Multi-viewport support is currently **NOT PRODUCTION-READY**.
//! This example is for testing and development purposes only.
//!
//! Run with:
//! ```bash
//! cargo run --bin multi_viewport_wgpu --features multi-viewport
//! ```
//!
//! What this example demonstrates:
//! - Creates a main window with WGPU rendering
//! - Enables Dear ImGui multi-viewport (experimental)
//! - Routes input events for secondary windows
//! - Lets Dear ImGui create/update/destroy platform windows and renders them
//!
//! Known limitations:
//! - Multi-viewport functionality may have bugs or incomplete features
//! - Not recommended for production use

use dear_imgui_rs::{Condition, Context};
use dear_imgui_wgpu::{GammaMode, WgpuInitInfo, WgpuRenderer};
use dear_imgui_winit::{HiDpiMode, WinitPlatform, multi_viewport as winit_mvp};
use pollster::block_on;
use std::{sync::Arc, time::Instant};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

struct AppWindow {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    device: wgpu::Device,
    queue: wgpu::Queue,
    imgui: Context,
    platform: WinitPlatform,
    renderer: WgpuRenderer,
    last_frame: Instant,
}

impl AppWindow {
    fn new(event_loop: &ActiveEventLoop) -> Result<Self, Box<dyn std::error::Error>> {
        // Create WGPU instance first (also used by renderer for per-viewport surfaces)
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());

        let window: Arc<Window> = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title("Dear ImGui Multi-Viewport (wgpu)")
                        .with_inner_size(LogicalSize::new(1200.0, 720.0)),
                )?
                .into(),
        );

        let surface = instance.create_surface(window.clone())?;

        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))?;

        let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))?;

        let size = window.inner_size();
        let caps = surface.get_capabilities(&adapter);
        let format = [
            wgpu::TextureFormat::Bgra8UnormSrgb,
            wgpu::TextureFormat::Rgba8UnormSrgb,
        ]
        .into_iter()
        .find(|f| caps.formats.contains(f))
        .unwrap_or(caps.formats[0]);

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Opaque,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        // Dear ImGui context + platform
        let mut imgui = Context::create();

        imgui.enable_multi_viewport();
        {
            let io = imgui.io_mut();
            let mut flags = io.config_flags();
            flags.insert(dear_imgui_rs::ConfigFlags::DOCKING_ENABLE);
            io.set_config_flags(flags);
        }

        let mut platform = WinitPlatform::new(&mut imgui);
        platform.attach_window(&window, HiDpiMode::Default, &mut imgui);

        // WGPU renderer
        let init_info = WgpuInitInfo::new(device.clone(), queue.clone(), surface_config.format)
            .with_instance(instance.clone())
            .with_adapter(adapter.clone());
        let mut renderer = WgpuRenderer::new(init_info, &mut imgui)?;
        renderer.set_gamma_mode(GammaMode::Auto);

        // Build Self first to pin renderer and imgui in their final locations
        let mut app = Self {
            window,
            surface,
            surface_config,
            device,
            queue,
            imgui,
            platform,
            renderer,
            last_frame: Instant::now(),
        };

        // Install platform (winit) viewport handlers (required by Dear ImGui)
        winit_mvp::init_multi_viewport_support(&mut app.imgui, &app.window);

        // Renderer viewport callbacks (install AFTER winit so our callbacks take precedence)
        dear_imgui_wgpu::multi_viewport::enable(&mut app.renderer, &mut app.imgui);

        Ok(app)
    }

    fn redraw(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let dt = self.last_frame.elapsed().as_secs_f32();
        self.last_frame = Instant::now();
        self.imgui.io_mut().set_delta_time(dt);

        let frame = match self.surface.get_current_texture() {
            Ok(f) => f,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                self.surface.configure(&self.device, &self.surface_config);
                return Ok(());
            }
            Err(wgpu::SurfaceError::Timeout) => {
                return Ok(());
            }
            Err(e) => {
                return Err(Box::new(e));
            }
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        self.platform.prepare_frame(&self.window, &mut self.imgui);
        let ui = self.imgui.frame();

        // Keep a dockspace in the main viewport so it always has content
        ui.dockspace_over_main_viewport();

        // Simple UI that can be torn out into another viewport
        ui.window("Main")
            .size([420.0, 260.0], Condition::FirstUseEver)
            .build(|| {
                ui.text("Drag this window outside to create a new OS window.");
                ui.separator();
                ui.text("Multi-viewport is enabled.");
            });

        // Optionally show demo to validate interaction
        // let mut show_demo = true; ui.show_demo_window(&mut show_demo);

        let draw_data = self.imgui.render();
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("imgui-main-encoder"),
            });

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("imgui-main-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.12,
                            b: 0.15,
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

            self.renderer.new_frame()?;
            self.renderer.render_draw_data_with_fb_size(
                draw_data,
                &mut rpass,
                self.surface_config.width,
                self.surface_config.height,
            )?;
        }

        // Submit and present main frame first to avoid cross-surface validation hazards
        self.queue.submit(Some(encoder.finish()));
        frame.present();

        // Update + render all platform windows (secondary viewports)
        self.imgui.update_platform_windows();
        self.imgui.render_platform_windows_default();
        Ok(())
    }

    fn resize(&mut self, size: winit::dpi::PhysicalSize<u32>) {
        if size.width > 0 && size.height > 0 {
            self.surface_config.width = size.width;
            self.surface_config.height = size.height;
            self.surface.configure(&self.device, &self.surface_config);
        }
    }
}

#[derive(Default)]
struct App {
    window: Option<AppWindow>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // Allow Dear ImGui to create windows using the event loop
        winit_mvp::set_event_loop(event_loop);

        match AppWindow::new(event_loop) {
            Ok(mut win) => {
                // Place the window struct first so its address is stable
                win.window.request_redraw();
                self.window = Some(win);

                // Now that AppWindow is in its final place, (re)install renderer callbacks
                if let Some(app) = self.window.as_mut() {
                    dear_imgui_wgpu::multi_viewport::enable(&mut app.renderer, &mut app.imgui);
                }
            }
            Err(e) => {
                event_loop.exit();
            }
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        // Continuously request redraw in Poll mode
        if let Some(app) = &self.window {
            app.window.request_redraw();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(app) = self.window.as_mut() else {
            return;
        };

        // Forward event to main window platform integration ONLY if it targets the main window
        let full: Event<()> = Event::WindowEvent {
            window_id,
            event: event.clone(),
        };
        if window_id == app.window.id() {
            app.platform
                .handle_event(&mut app.imgui, &app.window, &full);
        }
        // Route to ImGui-created windows (secondary viewports)
        let _ = winit_mvp::route_event_to_viewports(&mut app.imgui, &full);

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                app.resize(size);
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                app.resize(app.window.inner_size());
            }
            WindowEvent::RedrawRequested => {
                if let Err(e) = app.redraw() {}
                app.window.request_redraw();
            }
            _ => {}
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::default();
    event_loop.run_app(&mut app)?;
    Ok(())
}
