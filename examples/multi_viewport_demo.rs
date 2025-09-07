//! Multi-viewport demo for Dear ImGui
//!
//! This example demonstrates how to use Dear ImGui's multi-viewport feature
//! with the winit backend. Windows can be dragged outside the main viewport
//! to create separate OS windows.

use dear_imgui::*;
use dear_imgui_wgpu::Renderer;
use dear_imgui_winit::{HiDpiMode, WinitPlatform};
use pollster::FutureExt as _;
use std::time::Instant;
use wgpu::util::DeviceExt;
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

#[cfg(feature = "multi-viewport")]
use dear_imgui_winit::WinitViewportBackend;

struct MultiViewportApp {
    window: Option<Window>,
    wgpu_state: Option<WgpuState>,
    imgui_ctx: Option<Context>,
    platform: Option<WinitPlatform>,
    renderer: Option<Renderer>,
    last_frame: Instant,
    demo_open: bool,
    another_window_open: bool,
}

struct WgpuState {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
}

impl WgpuState {
    async fn new(window: &Window) -> Self {
        let size = window.inner_size();
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = instance.create_surface(window).unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    label: None,
                    memory_hints: wgpu::MemoryHints::default(),
                },
                None,
            )
            .await
            .unwrap();

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&device, &config);

        Self {
            surface,
            device,
            queue,
            config,
            size,
        }
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
        }
    }
}

impl Default for MultiViewportApp {
    fn default() -> Self {
        Self {
            window: None,
            wgpu_state: None,
            imgui_ctx: None,
            platform: None,
            renderer: None,
            last_frame: Instant::now(),
            demo_open: true,
            another_window_open: true,
        }
    }
}

impl ApplicationHandler for MultiViewportApp {
    fn can_create_surfaces(&mut self, event_loop: &ActiveEventLoop) {
        let window = event_loop
            .create_window(
                Window::default_attributes()
                    .with_title("Dear ImGui Multi-Viewport Demo")
                    .with_inner_size(LogicalSize::new(1280.0, 720.0)),
            )
            .expect("Failed to create window");

        let wgpu_state = WgpuState::new(&window).block_on();

        // Initialize Dear ImGui
        let mut imgui_ctx = Context::create();
        let mut platform = WinitPlatform::new(&mut imgui_ctx);
        platform.attach_window(&window, HiDpiMode::Default, &mut imgui_ctx);

        // Enable multi-viewport support
        #[cfg(feature = "multi-viewport")]
        {
            platform.setup_multi_viewport(event_loop, &mut imgui_ctx);
        }

        // Initialize renderer
        let renderer = Renderer::new(
            &mut imgui_ctx,
            &wgpu_state.device,
            &wgpu_state.queue,
            wgpu_state.config.format,
        );

        self.window = Some(window);
        self.wgpu_state = Some(wgpu_state);
        self.imgui_ctx = Some(imgui_ctx);
        self.platform = Some(platform);
        self.renderer = Some(renderer);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, window_id: WindowId, event: WindowEvent) {
        if let (Some(window), Some(platform), Some(imgui_ctx)) = 
            (&self.window, &mut self.platform, &mut self.imgui_ctx) 
        {
            if window.id() == window_id {
                let response = platform.handle_event(&Event::WindowEvent { window_id, event: event.clone() }, window, imgui_ctx);
                
                match event {
                    WindowEvent::CloseRequested => {
                        event_loop.exit();
                    }
                    WindowEvent::Resized(physical_size) => {
                        if let Some(wgpu_state) = &mut self.wgpu_state {
                            wgpu_state.resize(physical_size);
                        }
                    }
                    WindowEvent::RedrawRequested => {
                        self.render();
                    }
                    _ => {}
                }
            }
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

impl MultiViewportApp {
    fn render(&mut self) {
        let (window, wgpu_state, imgui_ctx, platform, renderer) = match (
            &self.window,
            &mut self.wgpu_state,
            &mut self.imgui_ctx,
            &mut self.platform,
            &mut self.renderer,
        ) {
            (Some(w), Some(wgpu), Some(ctx), Some(p), Some(r)) => (w, wgpu, ctx, p, r),
            _ => return,
        };

        let now = Instant::now();
        let delta_time = now.duration_since(self.last_frame);
        self.last_frame = now;

        // Prepare frame
        platform.prepare_frame(window, imgui_ctx);
        let ui = imgui_ctx.frame();

        // Main menu bar
        ui.main_menu_bar(|| {
            ui.menu("File", || {
                if ui.menu_item("Exit") {
                    std::process::exit(0);
                }
            });
            ui.menu("Windows", || {
                ui.checkbox("Demo Window", &mut self.demo_open);
                ui.checkbox("Another Window", &mut self.another_window_open);
            });
        });

        // Demo window that can be dragged out
        if self.demo_open {
            ui.window("Demo Window")
                .size([400.0, 300.0], Condition::FirstUseEver)
                .build(|| {
                    ui.text("Hello from Dear ImGui!");
                    ui.text("This window can be dragged outside the main viewport.");
                    ui.separator();
                    
                    ui.text(format!("Frame time: {:.3} ms", delta_time.as_secs_f32() * 1000.0));
                    ui.text(format!("FPS: {:.1}", 1.0 / delta_time.as_secs_f32()));
                    
                    if ui.button("Close") {
                        self.demo_open = false;
                    }
                });
        }

        // Another window
        if self.another_window_open {
            ui.window("Another Window")
                .size([300.0, 200.0], Condition::FirstUseEver)
                .build(|| {
                    ui.text("This is another window that supports multi-viewport!");
                    ui.text("Try dragging it outside the main window.");
                    
                    if ui.button("Close") {
                        self.another_window_open = false;
                    }
                });
        }

        // Render
        let output = wgpu_state.surface.get_current_texture().unwrap();
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = wgpu_state.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
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
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            renderer.render(imgui_ctx.render(), &wgpu_state.queue, &wgpu_state.device, &mut render_pass)
                .expect("Rendering failed");
        }

        wgpu_state.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        // Update platform windows (for multi-viewport)
        #[cfg(feature = "multi-viewport")]
        {
            unsafe {
                dear_imgui_sys::ImGui_UpdatePlatformWindows();
                dear_imgui_sys::ImGui_RenderPlatformWindowsDefault(std::ptr::null_mut(), std::ptr::null_mut());
            }
        }
    }
}

fn main() {
    env_logger::init();

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = MultiViewportApp::default();
    event_loop.run_app(&mut app).unwrap();
}
