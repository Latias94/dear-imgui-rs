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
//! - Secondary OS windows are enabled only on desktop native targets
//!   (Windows/macOS/Linux); Linux is currently untested.

use dear_imgui_rs::{Condition, Context, TextureId};
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
    start_time: Instant,
    enable_viewports: bool,
    // Offscreen "game view" texture and view
    // Keep the texture alive; the view alone doesn't own the resource.
    _game_tex: wgpu::Texture,
    game_tex_view: wgpu::TextureView,
    game_tex_id: TextureId,
}

impl Drop for AppWindow {
    fn drop(&mut self) {
        // Avoid ImGui's shutdown assertion by ensuring platform windows are destroyed before the
        // context is dropped.
        if self.enable_viewports {
            winit_mvp::shutdown_multi_viewport_support();
        }
    }
}

impl AppWindow {
    fn new(event_loop: &ActiveEventLoop) -> Result<Self, Box<dyn std::error::Error>> {
        // Winit + WGPU multi-viewport is experimental.
        // Enabled by default on desktop native targets; tested on Windows/macOS.
        // Linux is enabled but currently untested.
        let enable_viewports = cfg!(any(
            target_os = "windows",
            target_os = "macos",
            target_os = "linux"
        ));

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

        // Create a simple offscreen texture for a "game view" (rendered every frame).
        let game_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("mvw_game_view_texture"),
            size: wgpu::Extent3d {
                width: 512,
                height: 512,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let game_tex_view = game_tex.create_view(&wgpu::TextureViewDescriptor::default());

        // Dear ImGui context + platform
        let mut imgui = Context::create();

        if enable_viewports {
            imgui.enable_multi_viewport();
        }
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

        // Register the offscreen texture as an external ImGui texture.
        let game_tex_raw_id = renderer.register_external_texture(&game_tex, &game_tex_view);
        let game_tex_id = TextureId::from(game_tex_raw_id);

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
            start_time: Instant::now(),
            enable_viewports,
            _game_tex: game_tex,
            game_tex_view,
            game_tex_id,
        };

        if app.enable_viewports {
            // Install platform (winit) viewport handlers (required by Dear ImGui)
            winit_mvp::init_multi_viewport_support(&mut app.imgui, &app.window);

            // Renderer viewport callbacks (install AFTER winit so our callbacks take precedence)
            dear_imgui_wgpu::multi_viewport::enable(&mut app.renderer, &mut app.imgui);
        }

        Ok(app)
    }

    fn redraw(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Delta time is set by the platform backend in `prepare_frame()`.

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

        // First render a simple "game view" into the offscreen texture.
        {
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("mvw_game_view_encoder"),
                });

            let rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("mvw_game_view_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.game_tex_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        // Simple animated clear: color changes over time.
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: (self.start_time.elapsed().as_secs_f32().sin() * 0.5 + 0.5) as f64,
                            g: 0.2,
                            b: 0.4,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            drop(rpass);

            self.queue.submit(Some(encoder.finish()));
        }

        self.platform.prepare_frame(&self.window, &mut self.imgui);
        let ui = self.imgui.frame();

        // Keep a dockspace in the main viewport so it always has content
        ui.dockspace_over_main_viewport();

        // Simple UI that can be torn out into another viewport (when enabled)
        ui.window("Main")
            .size([420.0, 260.0], Condition::FirstUseEver)
            .build(|| {
                if self.enable_viewports {
                    ui.text("Drag this window outside to create a new OS window.");
                    ui.separator();
                    ui.text("Multi-viewport is enabled (experimental).");
                } else {
                    ui.text("Multi-viewport is disabled on this platform (winit + WGPU).");
                    ui.separator();
                    ui.text("Use the SDL3 + OpenGL example for a stable multi-viewport demo:");
                    ui.text("  cargo run -p dear-imgui-examples --bin sdl3_opengl_multi_viewport --features \"multi-viewport sdl3-opengl3\"");
                }
            });

        // "Game View" window showing the offscreen texture; you can drag this window
        // to any viewport (including secondary OS windows) and the texture will render
        // via the WGPU backend automatically.
        ui.window("Game View")
            .size([520.0, 540.0], Condition::FirstUseEver)
            .build(|| {
                // Fit the game view into the available region while keeping it square.
                let avail = ui.content_region_avail();
                let side = avail[0].min(avail[1]).max(64.0);
                let size = [side, side];
                ui.text("Offscreen WGPU texture rendered each frame:");
                ui.image(self.game_tex_id, size);
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
                multiview_mask: None,
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
        if self.enable_viewports {
            self.imgui.update_platform_windows();
            self.imgui.render_platform_windows_default();
        }
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
        match AppWindow::new(event_loop) {
            Ok(win) => {
                // Place the window struct first so its address is stable
                win.window.request_redraw();
                self.window = Some(win);

                // Now that AppWindow is in its final place, (re)install renderer callbacks
                if let Some(app) = self.window.as_mut() {
                    dear_imgui_wgpu::multi_viewport::enable(&mut app.renderer, &mut app.imgui);
                }
            }
            Err(_e) => {
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

        let is_main_window = window_id == app.window.id();

        let full: Event<()> = Event::WindowEvent {
            window_id,
            event: event.clone(),
        };
        // Let platform + multi-viewport helper route events to main + secondary windows
        let _ = winit_mvp::handle_event_with_multi_viewport(
            &mut app.platform,
            &mut app.imgui,
            &app.window,
            &full,
        );

        match event {
            WindowEvent::CloseRequested => {
                // Only exit when the main application window is closed.
                if is_main_window {
                    event_loop.exit();
                }
            }
            WindowEvent::Resized(size) => {
                // Only reconfigure the main WGPU surface for the main window.
                if is_main_window {
                    app.resize(size);
                }
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                if is_main_window {
                    app.resize(app.window.inner_size());
                }
            }
            WindowEvent::RedrawRequested => {
                // We drive rendering from the main window. Secondary viewport windows are
                // rendered via ImGui's platform callbacks during `app.redraw()`.
                if is_main_window {
                    // ImGui may create secondary OS windows during `update_platform_windows()`.
                    // Provide the current `ActiveEventLoop` for the duration of this callback.
                    let _el_guard = if app.enable_viewports {
                        Some(winit_mvp::set_event_loop_for_frame(event_loop))
                    } else {
                        None
                    };
                    if let Err(_e) = app.redraw() {}
                    app.window.request_redraw();
                }
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
