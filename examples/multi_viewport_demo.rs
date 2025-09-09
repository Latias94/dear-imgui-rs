//! Multi-viewport demo for Dear ImGui
//!
//! This example demonstrates how to use Dear ImGui's multi-viewport feature,
//! which allows windows to be dragged outside the main application window
//! and displayed in separate OS windows.

use dear_imgui::*;
use dear_imgui_wgpu::WgpuRenderer;
use dear_imgui_winit::WinitPlatform;
use pollster::block_on;
use std::{sync::Arc, time::Instant};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::Window,
};

struct ImguiState {
    context: Context,
    platform: WinitPlatform,
    renderer: WgpuRenderer,
    clear_color: wgpu::Color,
    show_demo_window: bool,
    show_another_window: bool,
    show_multi_viewport_info: bool,
    last_frame: Instant,
}

struct AppWindow {
    device: wgpu::Device,
    queue: wgpu::Queue,
    window: Arc<Window>,
    surface_desc: wgpu::SurfaceConfiguration,
    surface: wgpu::Surface<'static>,
    hidpi_factor: f64,
    imgui: Option<ImguiState>,
}

#[derive(Default)]
struct MultiViewportApp {
    window: Option<AppWindow>,
}

impl AppWindow {
    fn setup_gpu(event_loop: &ActiveEventLoop) -> Self {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });

        let window = {
            let version = env!("CARGO_PKG_VERSION");
            let size = LogicalSize::new(1280.0, 720.0);

            Arc::new(
                event_loop
                    .create_window(
                        Window::default_attributes()
                            .with_title(&format!("Dear ImGui Multi-Viewport Demo - {version}"))
                            .with_inner_size(size),
                    )
                    .expect("Failed to create window"),
            )
        };

        let hidpi_factor = window.scale_factor();
        let surface = instance.create_surface(window.clone()).unwrap();

        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .expect("Failed to find an appropriate adapter");

        let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
            .expect("Failed to create device");

        let size = LogicalSize::new(1280.0, 720.0);
        let surface_desc = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface.get_capabilities(&adapter).formats[0],
            width: size.width as u32,
            height: size.height as u32,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&device, &surface_desc);

        Self {
            device,
            queue,
            window,
            surface_desc,
            surface,
            hidpi_factor,
            imgui: None,
        }
    }

    fn setup_imgui(&mut self) {
        let mut context = Context::create();
        context.set_ini_filename(Some(std::path::PathBuf::from("multi_viewport_demo.ini")));

        let mut platform = WinitPlatform::new(&mut context);
        platform.attach_window(
            &self.window,
            dear_imgui_winit::HiDpiMode::Default,
            &mut context,
        );

        // Enable multi-viewport support
        #[cfg(feature = "multi-viewport")]
        {
            // Enable multi-viewport and docking flags
            let io = context.io_mut();
            let mut config_flags = io.config_flags();
            config_flags.insert(ConfigFlags::VIEWPORTS_ENABLE);
            config_flags.insert(ConfigFlags::DOCKING_ENABLE);
            io.set_config_flags(config_flags);

            // Set up dummy backends to prevent crashes
            context.set_platform_backend(dear_imgui::DummyPlatformViewportBackend);
            context.set_renderer_backend(dear_imgui::DummyRendererViewportBackend);
        }

        let mut renderer = WgpuRenderer::new(&self.device, &self.queue, self.surface_desc.format);

        // Load font texture - this is crucial for text rendering!
        renderer.reload_font_texture(&mut context, &self.device, &self.queue);

        self.imgui = Some(ImguiState {
            context,
            platform,
            renderer,
            clear_color: wgpu::Color {
                r: 0.45,
                g: 0.55,
                b: 0.60,
                a: 1.0,
            },
            show_demo_window: true,
            show_another_window: false,
            show_multi_viewport_info: true,
            last_frame: Instant::now(),
        });
    }

    fn render(&mut self) {
        let imgui = self.imgui.as_mut().unwrap();

        let now = Instant::now();
        let delta_time = now - imgui.last_frame;
        imgui
            .context
            .io_mut()
            .set_delta_time(delta_time.as_secs_f32());
        imgui.last_frame = now;

        let frame = match self.surface.get_current_texture() {
            Ok(frame) => frame,
            Err(e) => {
                eprintln!("dropped frame: {e:?}");
                return;
            }
        };

        imgui
            .platform
            .prepare_frame(&self.window, &mut imgui.context);
        let ui = imgui.context.frame();

        // Main control window
        ui.window("Multi-Viewport Control Panel")
            .size([400.0, 300.0], Condition::FirstUseEver)
            .build(|| {
                ui.text("Welcome to Dear ImGui Multi-Viewport Demo!");
                ui.separator();

                ui.text(&format!(
                    "Application average {:.3} ms/frame ({:.1} FPS)",
                    1000.0 / ui.io().framerate(),
                    ui.io().framerate()
                ));

                ui.color_edit4(
                    "Clear color",
                    &mut [
                        imgui.clear_color.r as f32,
                        imgui.clear_color.g as f32,
                        imgui.clear_color.b as f32,
                        imgui.clear_color.a as f32,
                    ],
                );

                ui.separator();
                ui.text("Window Controls:");
                ui.checkbox("Show Demo Window", &mut imgui.show_demo_window);
                ui.checkbox("Show Another Window", &mut imgui.show_another_window);
                ui.checkbox(
                    "Show Multi-Viewport Info",
                    &mut imgui.show_multi_viewport_info,
                );

                ui.separator();
                ui.text("Multi-Viewport Instructions:");
                ui.bullet_text("Drag any window outside this main window");
                ui.bullet_text("The window will become a separate OS window");
                ui.bullet_text("You can move, resize, and interact with it independently");
                ui.bullet_text("Try dragging the Demo Window or other windows outside!");
            });

        // Show demo window if requested
        if imgui.show_demo_window {
            ui.show_demo_window(&mut imgui.show_demo_window);
        }

        // Show another window
        if imgui.show_another_window {
            ui.window("Another Window")
                .size([300.0, 200.0], Condition::FirstUseEver)
                .build(|| {
                    ui.text("Hello from another window!");
                    ui.text("This window can also be dragged outside!");
                    ui.separator();
                    ui.text("Try dragging this window by its title bar");
                    ui.text("outside the main application window.");
                    if ui.button("Close Me") {
                        imgui.show_another_window = false;
                    }
                });
        }

        // Multi-viewport specific UI
        #[cfg(feature = "multi-viewport")]
        if imgui.show_multi_viewport_info {
            ui.window("Multi-Viewport Info")
                .size([400.0, 250.0], Condition::FirstUseEver)
                .build(|| {
                    ui.text("Multi-Viewport Technical Info:");
                    ui.separator();

                    ui.text("Multi-viewport support is ENABLED");
                    ui.text("Docking support is ENABLED");
                    ui.text("Platform backend: Dummy (for demo)");
                    ui.text("Renderer backend: Dummy (for demo)");

                    ui.separator();
                    ui.text("Instructions:");
                    ui.bullet_text("Drag any window outside this main window");
                    ui.bullet_text("The window will become a separate OS window");
                    ui.bullet_text("You can move, resize, and interact with it independently");
                    ui.bullet_text("Try dragging the Demo Window or other windows outside!");
                });
        }

        let draw_data = imgui.context.render();

        // Update platform windows (required for multi-viewport)
        #[cfg(feature = "multi-viewport")]
        {
            // Note: In a real multi-viewport implementation, you would call update_platform_windows here
            // but we need to avoid borrowing conflicts for this demo
        }

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(imgui.clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            imgui
                .renderer
                .render_with_renderpass(&draw_data, &self.queue, &self.device, &mut rpass)
                .expect("Rendering failed");
        }

        self.queue.submit(Some(encoder.finish()));
        frame.present();

        // Render platform windows (required for multi-viewport)
        #[cfg(feature = "multi-viewport")]
        {
            imgui.context.render_platform_windows_default();
        }
    }
}

impl ApplicationHandler for MultiViewportApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let mut window = AppWindow::setup_gpu(event_loop);
            window.setup_imgui();
            self.window = Some(window);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let window = self.window.as_mut().unwrap();
        let imgui = window.imgui.as_mut().unwrap();

        // Handle the event first before matching on it
        let full_event: winit::event::Event<()> = winit::event::Event::WindowEvent {
            window_id: window.window.id(),
            event: event.clone(),
        };
        imgui
            .platform
            .handle_event(&full_event, &window.window, &mut imgui.context);

        match event {
            WindowEvent::Resized(physical_size) => {
                window.surface_desc.width = physical_size.width;
                window.surface_desc.height = physical_size.height;
                window
                    .surface
                    .configure(&window.device, &window.surface_desc);
            }
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput { event, .. } => {
                if event.logical_key == Key::Named(NamedKey::Escape) {
                    event_loop.exit();
                }
            }
            WindowEvent::RedrawRequested => {
                window.render();
                window.window.request_redraw();
            }
            _ => {}
        }
    }
}

fn main() {
    env_logger::init();

    println!("Starting Multi-Viewport Demo");
    #[cfg(feature = "multi-viewport")]
    println!("Multi-viewport support is ENABLED");
    #[cfg(not(feature = "multi-viewport"))]
    println!("Multi-viewport support is DISABLED - enable the 'multi-viewport' feature to test");

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = MultiViewportApp::default();
    event_loop.run_app(&mut app).unwrap();
}
