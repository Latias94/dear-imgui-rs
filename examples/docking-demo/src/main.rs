use dear_imgui::{Context, Vec2};
use dear_imgui_wgpu::WgpuRenderer;
use dear_imgui_winit::WinitPlatform;
use std::sync::Arc;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowId},
};

struct DockingApp {
    window: Arc<Window>,
    imgui_context: Context,
    platform: WinitPlatform,
    renderer: WgpuRenderer,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    size_changed: bool,
}

impl DockingApp {
    async fn new(window: Arc<Window>) -> Self {
        // Initialize WGPU
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });

        let surface = instance.create_surface(window.clone()).unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::default(),
                trace: wgpu::Trace::Off,
            })
            .await
            .unwrap();

        let mut size = window.inner_size();
        size.width = size.width.max(1);
        size.height = size.height.max(1);

        let surface_config = surface
            .get_default_config(&adapter, size.width, size.height)
            .unwrap();
        surface.configure(&device, &surface_config);

        // Initialize Dear ImGui
        let mut imgui_context = Context::new().unwrap();

        // Enable docking - TODO: Add ConfigFlags support
        // let io = imgui_context.io_mut();
        // io.config_flags |= ConfigFlags::DOCKING_ENABLE;

        let platform = WinitPlatform::new(&mut imgui_context);
        let renderer = WgpuRenderer::new(&device, &queue, surface_config.format);

        Self {
            window,
            imgui_context,
            platform,
            renderer,
            device,
            queue,
            surface,
            surface_config,
            size,
            size_changed: false,
        }
    }

    fn set_window_resized(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size == self.size {
            return;
        }
        self.size = new_size;
        self.size_changed = true;
    }

    fn resize_surface_if_needed(&mut self) {
        if self.size_changed {
            self.surface_config.width = self.size.width;
            self.surface_config.height = self.size.height;
            self.surface.configure(&self.device, &self.surface_config);
            self.size_changed = false;
        }
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        if self.size.width == 0 || self.size.height == 0 {
            return Ok(());
        }
        self.resize_surface_if_needed();

        self.platform
            .prepare_frame(&self.window, &mut self.imgui_context);

        let mut frame = self.imgui_context.frame();

        // Create a simple test window first
        frame
            .window("Hello World")
            .size(Vec2::new(400.0, 300.0))
            .show(|ui| {
                ui.text("Hello, Dear ImGui with Docking!");
                ui.separator();

                if ui.button("Test Button") {
                    println!("Button clicked!");
                }

                ui.text("This is a basic test window.");
                ui.text("If you see this, the basic rendering is working!");

                // Test basic docking functions (but don't create dock space yet)
                #[cfg(feature = "docking")]
                {
                    ui.separator();
                    ui.text("Docking feature is enabled!");

                    // Test if we can call docking functions without crashing
                    let is_docked = ui.is_window_docked();
                    let dock_id = ui.get_window_dock_id();
                    ui.text(format!("Window docked: {}", is_docked));
                    ui.text(format!("Window dock ID: {}", dock_id));
                }

                #[cfg(not(feature = "docking"))]
                {
                    ui.separator();
                    ui.text("Docking feature is NOT enabled.");
                }

                true
            });

        let draw_data = frame.draw_data();

        // Render
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        // Clear the screen
        {
            let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.1,
                            b: 0.1,
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
        }

        // Render ImGui
        self.renderer
            .render(&self.device, &self.queue, &mut encoder, &view, &draw_data)
            .unwrap();

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}

#[derive(Default)]
struct DockingAppHandler {
    app: Option<DockingApp>,
}

impl ApplicationHandler for DockingAppHandler {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.app.is_some() {
            return;
        }

        // Create window
        let window_attributes = Window::default_attributes()
            .with_title("Dear ImGui Docking Demo")
            .with_inner_size(winit::dpi::LogicalSize::new(1200, 800));

        let window = Arc::new(event_loop.create_window(window_attributes).unwrap());

        // Initialize app asynchronously
        let app = pollster::block_on(DockingApp::new(window));
        self.app = Some(app);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        if let Some(app) = self.app.as_mut() {
            // TODO: Fix event handling - platform.handle_event expects Event<T>, not WindowEvent
            // app.platform.handle_event(&event, &app.window);

            match event {
                WindowEvent::CloseRequested => {
                    event_loop.exit();
                }
                WindowEvent::Resized(physical_size) => {
                    if physical_size.width == 0 || physical_size.height == 0 {
                        // Handle minimized window
                        return;
                    }
                    app.set_window_resized(physical_size);
                }
                WindowEvent::RedrawRequested => {
                    app.window.pre_present_notify();
                    match app.render() {
                        Ok(_) => {}
                        Err(wgpu::SurfaceError::Lost) => eprintln!("Surface is lost"),
                        Err(e) => eprintln!("{e:?}"),
                    }
                    // 请求下一帧重绘
                    app.window.request_redraw();
                }
                _ => {}
            }
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        // 在初始化完成后请求第一次重绘
        if let Some(app) = self.app.as_ref() {
            app.window.request_redraw();
        }
    }
}

fn main() {
    env_logger::init();

    let event_loop = EventLoop::new().unwrap();

    let mut app = DockingAppHandler::default();
    event_loop.run_app(&mut app).unwrap();
}
