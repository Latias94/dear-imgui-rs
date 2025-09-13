use dear_imgui::*;
use dear_imgui_wgpu::{WgpuInitInfo, WgpuRenderer};
use dear_imgui_winit::WinitPlatform;
use std::cell::Cell;
use std::sync::{Arc, Mutex};
use winit::dpi::PhysicalSize;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowId},
};

struct ImGuiWgpuApp {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    _adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    size_changed: bool,

    // Dear ImGui components
    imgui_context: Context,
    imgui_platform: WinitPlatform,
    imgui_renderer: WgpuRenderer,

    // Demo state
    demo_open: Cell<bool>,
    clear_color: Cell<[f32; 3]>,
}

impl ImGuiWgpuApp {
    async fn new(window: Arc<Window>) -> Self {
        if cfg!(not(target_arch = "wasm32")) {
            // Calculate a default display height
            let height = 600 * window.scale_factor() as u32;
            let width = (height as f32 * 1.6) as u32;
            let _ = window.request_inner_size(PhysicalSize::new(width, height));
        }

        #[cfg(target_arch = "wasm32")]
        {
            use winit::platform::web::WindowExtWebSys;
            let canvas = window.canvas().unwrap();

            // Append the canvas to the current web page
            web_sys::window()
                .and_then(|win| win.document())
                .map(|doc| {
                    let _ = canvas.set_attribute("id", "dear-imgui-canvas");
                    match doc.get_element_by_id("dear-imgui-container") {
                        Some(dst) => {
                            let _ = dst.append_child(canvas.as_ref());
                        }
                        None => {
                            let container = doc.create_element("div").unwrap();
                            let _ = container.set_attribute("id", "dear-imgui-container");
                            let _ = container.append_child(canvas.as_ref());

                            doc.body().map(|body| body.append_child(container.as_ref()));
                        }
                    };
                })
                .expect("Failed to append canvas to the current web page");

            // Ensure the canvas can receive focus
            canvas.set_tab_index(0);

            // Set the canvas to not show a highlight outline on focus
            let style = canvas.style();
            style.set_property("outline", "none").unwrap();
            canvas.focus().expect("Failed to focus the canvas");
        }

        // Create a WGPU instance
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
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
                // WebGL doesn't support all of wgpu's features
                required_limits: if cfg!(target_arch = "wasm32") {
                    wgpu::Limits::downlevel_webgl2_defaults()
                } else {
                    wgpu::Limits::default()
                },
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::Off,
            })
            .await
            .unwrap();

        let mut size = window.inner_size();
        size.width = size.width.max(1);
        size.height = size.height.max(1);
        let config = surface
            .get_default_config(&adapter, size.width, size.height)
            .unwrap();
        surface.configure(&device, &config);

        // Initialize Dear ImGui
        let mut imgui_context = Context::create_or_panic();
        imgui_context.set_ini_filename_or_panic(None::<String>);

        // Create the platform backend
        let mut imgui_platform = WinitPlatform::new(&mut imgui_context);
        imgui_platform.attach_window(
            &window,
            dear_imgui_winit::HiDpiMode::Default,
            &mut imgui_context,
        );

        // Create the renderer
        let init_info = WgpuInitInfo::new(device.clone(), queue.clone(), config.format);
        let imgui_renderer = WgpuRenderer::new(init_info, &mut imgui_context)
            .expect("Failed to initialize Dear ImGui WGPU renderer");

        log::info!("Dear ImGui WGPU + Winit demo initialized successfully!");

        Self {
            window,
            surface,
            _adapter: adapter,
            device,
            queue,
            config,
            size,
            size_changed: false,
            imgui_context,
            imgui_platform,
            imgui_renderer,
            demo_open: Cell::new(true),
            clear_color: Cell::new([0.1, 0.2, 0.3]),
        }
    }

    /// Records that the window size has changed
    fn set_window_resized(&mut self, new_size: PhysicalSize<u32>) {
        if new_size == self.size {
            return;
        }
        self.size = new_size;
        self.size_changed = true;
    }

    /// Resizes the surface if needed
    fn resize_surface_if_needed(&mut self) {
        if self.size_changed {
            self.config.width = self.size.width;
            self.config.height = self.size.height;
            self.surface.configure(&self.device, &self.config);
            self.size_changed = false;
        }
    }

    fn handle_event(&mut self, event: &winit::event::Event<()>) {
        self.imgui_platform
            .handle_event(&mut self.imgui_context, &self.window, event);
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        if self.size.width == 0 || self.size.height == 0 {
            return Ok(());
        }
        self.resize_surface_if_needed();

        // Prepare the Dear ImGui frame
        self.imgui_platform
            .prepare_frame(&self.window, &mut self.imgui_context);

        let draw_data = {
            let ui = self.imgui_context.frame();

            // Create the Dear ImGui UI - implemented inline
            // Main menu bar
            if let Some(_token) = ui.begin_main_menu_bar() {
                if let Some(_token) = ui.begin_menu("File") {
                    if ui.menu_item("New") {
                        log::info!("New file clicked");
                    }
                    if ui.menu_item("Open") {
                        log::info!("Open file clicked");
                    }
                    ui.separator();
                    if ui.menu_item("Exit") {
                        log::info!("Exit clicked");
                    }
                }

                if let Some(_token) = ui.begin_menu("View") {
                    let mut demo_open = self.demo_open.get();
                    if ui.checkbox("Show Demo Window", &mut demo_open) {
                        self.demo_open.set(demo_open);
                    }
                }
            }

            // Demo window
            if self.demo_open.get() {
                let mut demo_open = self.demo_open.get();
                ui.show_demo_window(&mut demo_open);
                self.demo_open.set(demo_open);
            }

            // Custom window
            ui.window("Dear ImGui WGPU + Winit Demo")
                .size([400.0, 300.0], Condition::FirstUseEver)
                .build(|| {
                    ui.text("Hello from Dear ImGui with WGPU backend!");
                    ui.text(format!("Dear ImGui version: {}", dear_imgui_version()));
                    ui.text(format!(
                        "Application average {:.3} ms/frame ({:.1} FPS)",
                        1000.0 / ui.io().framerate(),
                        ui.io().framerate()
                    ));

                    ui.separator();

                    ui.text("Background Color:");
                    let mut clear_color = self.clear_color.get();
                    if ui.color_edit3("Clear Color", &mut clear_color) {
                        self.clear_color.set(clear_color);
                    }

                    ui.separator();

                    if ui.button("Log Info") {
                        log::info!("Button clicked from Dear ImGui!");
                    }

                    ui.same_line();
                    if ui.button("Test Alert") {
                        #[cfg(target_arch = "wasm32")]
                        {
                            web_sys::window()
                                .unwrap()
                                .alert_with_message("Hello from Dear ImGui WASM!")
                                .unwrap();
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        {
                            log::info!("Alert button clicked (native mode)");
                        }
                    }
                });

            // Render Dear ImGui
            self.imgui_context.render()
        };

        // Get the render surface
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Dear ImGui Render Encoder"),
            });
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Dear ImGui Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: self.clear_color.get()[0] as f64,
                            g: self.clear_color.get()[1] as f64,
                            b: self.clear_color.get()[2] as f64,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });

            self.imgui_renderer
                .render_draw_data(draw_data, &mut render_pass)
                .expect("Failed to render Dear ImGui");
        }

        self.queue.submit(Some(encoder.finish()));
        output.present();

        Ok(())
    }
}

#[derive(Default)]
struct ImGuiWgpuAppHandler {
    app: Arc<Mutex<Option<ImGuiWgpuApp>>>,
    /// Missed window resize event
    ///
    /// # NOTE:
    /// On the web, app initialization is asynchronous. When a resize event is received,
    /// initialization might not be complete, causing the event to be missed. When the app is initialized,
    /// `set_window_resized` will be called to apply the missed resize event.
    #[allow(dead_code)]
    missed_resize: Arc<Mutex<Option<PhysicalSize<u32>>>>,
}

impl ApplicationHandler for ImGuiWgpuAppHandler {
    /// Resumed event
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // If the app is already initialized, just return
        if self.app.as_ref().lock().unwrap().is_some() {
            return;
        }

        let window_attributes =
            Window::default_attributes().with_title("Dear ImGui WGPU + Winit Demo");

        let window = Arc::new(event_loop.create_window(window_attributes).unwrap());
        cfg_if::cfg_if! {
            if #[cfg(target_arch = "wasm32")] {
                let app = self.app.clone();
                let missed_resize = self.missed_resize.clone();

                wasm_bindgen_futures::spawn_local(async move {
                    let window_cloned = window.clone();

                    let imgui_app = ImGuiWgpuApp::new(window).await;
                    let mut app = app.lock().unwrap();
                    *app = Some(imgui_app);

                    // If a resize event was missed, apply it now
                    if let Some(resize) = *missed_resize.lock().unwrap() {
                        app.as_mut().unwrap().set_window_resized(resize);
                        window_cloned.request_redraw();
                    }
                });
            } else {
                let imgui_app = pollster::block_on(ImGuiWgpuApp::new(window));
                self.app.lock().unwrap().replace(imgui_app);
                // NOTE: On non-web platforms, resize and redraw events are not missed
            }
        }
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        // Suspended event
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let mut app = self.app.lock().unwrap();
        if app.as_ref().is_none() {
            // If the app is not yet initialized, record the missed window event
            if let WindowEvent::Resized(physical_size) = event {
                if physical_size.width > 0 && physical_size.height > 0 {
                    let mut missed_resize = self.missed_resize.lock().unwrap();
                    *missed_resize = Some(physical_size);
                }
            }
            return;
        }

        let app = app.as_mut().unwrap();

        // Construct the full event structure for Dear ImGui to handle
        let full_event = winit::event::Event::WindowEvent {
            window_id,
            event: event.clone(),
        };
        app.handle_event(&full_event);

        // Window events
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(physical_size) => {
                if physical_size.width == 0 || physical_size.height == 0 {
                    // Handle window minimization event
                    log::info!("Window minimized!");
                } else {
                    log::info!("Window resized: {:?}", physical_size);
                    app.set_window_resized(physical_size);
                }
            }
            WindowEvent::RedrawRequested => {
                // Surface redraw event
                app.window.pre_present_notify();

                match app.render() {
                    Ok(_) => {}
                    // Reconfigure the surface if the context is lost
                    Err(wgpu::SurfaceError::Lost) => {
                        eprintln!("Surface is lost");
                        app.size_changed = true;
                    }
                    // All other errors (outdated, timeout, etc.) should be resolved on the next frame
                    Err(e) => eprintln!("{e:?}"),
                }
                // RedrawRequested will only be triggered once unless we manually request it.
                app.window.request_redraw();
            }
            _ => (),
        }
    }
}

fn init_logger() {
    cfg_if::cfg_if! {
        if #[cfg(target_arch = "wasm32")] {
            fern::Dispatch::new()
                .level(log::LevelFilter::Info)
                .level_for("wgpu_core", log::LevelFilter::Error)
                .level_for("wgpu_hal", log::LevelFilter::Error)
                .level_for("naga", log::LevelFilter::Error)
                .chain(fern::Output::call(console_log::log))
                .apply()
                .expect("Could not initialize logger");
            std::panic::set_hook(Box::new(console_error_panic_hook::hook));
        } else {
            env_logger::init();
        }
    }
}

fn main() -> Result<(), impl std::error::Error> {
    init_logger();
    log::info!("Starting Dear ImGui WGPU + Winit Demo...");

    let events_loop = EventLoop::new().unwrap();
    let mut app = ImGuiWgpuAppHandler::default();
    events_loop.run_app(&mut app)
}
