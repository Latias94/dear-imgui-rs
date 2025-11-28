#![cfg(target_arch = "wasm32")]

use dear_imgui_rs::*;
use dear_imgui_wgpu::WgpuRenderer;
use dear_imgui_winit::WinitPlatform;
use instant::Instant;
use log::info;
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::prelude::*;
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

// Use Rc instead of Arc on wasm to avoid cross-thread requirements
type WindowRc = Rc<Window>;

struct ImguiState {
    context: Context,
    platform: WinitPlatform,
    renderer: WgpuRenderer,
    clear_color: wgpu::Color,
    demo_open: bool,
    last_frame: Instant,
    frames: u32,
}

struct AppWindow {
    device: wgpu::Device,
    queue: wgpu::Queue,
    window: WindowRc,
    surface_desc: wgpu::SurfaceConfiguration,
    surface: wgpu::Surface<'static>,
    imgui: ImguiState,
}

#[derive(Default)]
struct App {
    window: Option<AppWindow>,
}

impl AppWindow {
    // Complete async GPU setup given an already-created window + surface.
    async fn new_with_window(
        window: WindowRc,
        surface: wgpu::Surface<'static>,
        size: LogicalSize<f64>,
        instance: wgpu::Instance,
    ) -> Result<Self, JsValue> {
        // Request adapter and device
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .map_err(|e| JsValue::from_str(&format!("request_adapter: {e}")))?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .map_err(|e| JsValue::from_str(&format!("request_device: {e}")))?;

        let capabilities = surface.get_capabilities(&adapter);
        let surface_format = capabilities
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(capabilities.formats[0]);

        // Use physical size of the canvas for surface configuration (matches scissor/render target)
        let physical = window.inner_size();
        let surface_desc = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: physical.width.max(1),
            height: physical.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&device, &surface_desc);

        // Setup ImGui
        let mut context = Context::create();
        context.set_ini_filename(None::<String>).unwrap();

        // Set initial display size before creating platform
        {
            let io = context.io_mut();
            io.set_display_size([1280.0, 720.0]);
            io.set_display_framebuffer_scale([1.0, 1.0]);
        }

        let mut platform = WinitPlatform::new(&mut context);
        platform.attach_window(&window, dear_imgui_winit::HiDpiMode::Default, &mut context);

        // Optional: experiment with custom font settings when the experimental
        // feature is enabled. This uses the modern FontAtlas API and verifies
        // that the shared-memory provider and bindings are wired correctly.
        #[cfg(feature = "experimental-fonts")]
        {
            use dear_imgui_rs::{FontConfig, FontSource};
            let mut fonts = context.font_atlas_mut();
            let cfg = FontConfig::default()
                .size_pixels(18.0)
                .name("Web Default 18px");
            fonts.add_font(&[FontSource::default_font().with_config(cfg)]);
        }

        let init_info =
            dear_imgui_wgpu::WgpuInitInfo::new(device.clone(), queue.clone(), surface_desc.format);
        let renderer = WgpuRenderer::new(init_info, &mut context)
            .map_err(|e| JsValue::from_str(&format!("init renderer: {e}")))?;

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
            frames: 0,
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

    fn render(&mut self) -> Result<(), JsValue> {
        let now = Instant::now();
        let delta_time = now - self.imgui.last_frame;

        // Query window size + HiDPI factor and update ImGui IO before NewFrame
        {
            let scale_factor = self.window.scale_factor() as f32;
            let physical = self.window.inner_size();
            // ImGui expects logical size in points, with framebuffer scale carrying the DPI factor
            let logical_w = (physical.width as f32 / scale_factor).max(0.0);
            let logical_h = (physical.height as f32 / scale_factor).max(0.0);

            let io = self.imgui.context.io_mut();
            io.set_display_size([logical_w, logical_h]);
            io.set_display_framebuffer_scale([scale_factor, scale_factor]);
            io.set_delta_time(delta_time.as_secs_f32());

            // Log a few frames for debugging size plumbing
            if self.imgui.frames < 3 {
                info!(
                    "IO set: logical={}x{}, framebuffer_scale={} (winit scale_factor={}), physical={}x{}, surface={}x{}",
                    logical_w,
                    logical_h,
                    scale_factor,
                    self.window.scale_factor(),
                    physical.width,
                    physical.height,
                    self.surface_desc.width,
                    self.surface_desc.height
                );
            }
        }
        self.imgui.last_frame = now;
        self.imgui.frames = self.imgui.frames.saturating_add(1);

        // Allow winit platform backend to update IO (mouse/keyboard/text)
        self.imgui
            .platform
            .prepare_frame(&self.window, &mut self.imgui.context);

        // Skip rendering if surface is zero-sized (e.g., hidden tab)
        if self.surface_desc.width == 0 || self.surface_desc.height == 0 {
            return Ok(());
        }

        // Ensure surface matches current canvas/backing buffer size (DPR changes, CSS resize, etc.)
        let physical = self.window.inner_size();
        if physical.width != self.surface_desc.width || physical.height != self.surface_desc.height
        {
            self.resize(physical);
        }

        let frame = self
            .surface
            .get_current_texture()
            .map_err(|e| JsValue::from_str(&format!("get_current_texture: {e}")))?;

        let ui = self.imgui.context.frame();

        ui.window("Hello, Dear ImGui (Web)")
            .size([400.0, 300.0], Condition::FirstUseEver)
            .build(|| {
                ui.text("This is running under wasm32-unknown-unknown");
                ui.separator();
                if ui.button("Show Demo Window") {
                    self.imgui.demo_open = true;
                }
            });

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
            });

            self.imgui
                .renderer
                .new_frame()
                .map_err(|e| JsValue::from_str(&format!("new_frame: {e}")))?;
            self.imgui
                .renderer
                .render_draw_data(&draw_data, &mut rpass)
                .map_err(|e| JsValue::from_str(&format!("render_draw_data: {e}")))?;
        }

        self.queue.submit(Some(encoder.finish()));
        frame.present();
        Ok(())
    }
}

thread_local! {
    static APP_READY: RefCell<Option<AppWindow>> = RefCell::new(None);
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        // Create window + surface synchronously, then finish GPU init asynchronously.
        use winit::platform::web::WindowAttributesExtWebSys;
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());

        // Acquire target canvas from the page
        let win = web_sys::window().expect("no window");
        let document = win.document().expect("no document");
        let canvas: web_sys::HtmlCanvasElement = document
            .get_element_by_id("wasm-canvas")
            .expect("canvas not found")
            .dyn_into()
            .expect("element is not canvas");

        let size = LogicalSize::new(1280.0, 720.0);
        let window: WindowRc = Rc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title("Dear ImGui Web Demo")
                        .with_canvas(Some(canvas))
                        .with_inner_size(size),
                )
                .expect("create_window"),
        );

        let surface = instance
            .create_surface(window.clone())
            .expect("create_surface");

        // Finish GPU init on the JS task queue
        let window_for_redraw = window.clone();
        wasm_bindgen_futures::spawn_local(async move {
            match AppWindow::new_with_window(window, surface, size, instance).await {
                Ok(appwin) => {
                    APP_READY.with(|cell| *cell.borrow_mut() = Some(appwin));
                    window_for_redraw.request_redraw();
                }
                Err(e) => {
                    web_sys::console::error_1(&JsValue::from_str(&format!(
                        "Failed to init GPU: {:?}",
                        e
                    )));
                }
            }
        });
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        // If async init completed, pick up the window now
        if self.window.is_none() {
            if let Some(w) = APP_READY.with(|cell| cell.borrow_mut().take()) {
                self.window = Some(w);
            }
        }
        let Some(window) = self.window.as_mut() else {
            return;
        };

        let full_event: winit::event::Event<()> = winit::event::Event::WindowEvent {
            window_id: _window_id,
            event: event.clone(),
        };
        window
            .imgui
            .platform
            .handle_event(&mut window.imgui.context, &window.window, &full_event);

        match event {
            WindowEvent::Resized(physical_size) => {
                window.resize(physical_size);
                window.window.request_redraw();
            }
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                if let Err(e) = window.render() {
                    web_sys::console::error_1(&JsValue::from_str(&format!(
                        "Render error: {:?}",
                        e
                    )));
                }
                window.window.request_redraw();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        // Pick up async init completion if available
        if self.window.is_none() {
            if let Some(w) = APP_READY.with(|cell| cell.borrow_mut().take()) {
                self.window = Some(w);
            }
        }
        if let Some(window) = &self.window {
            window.window.request_redraw();
        }
    }
}

#[wasm_bindgen(start)]
pub fn start() {
    // Logging and panic hooks for the browser console
    console_error_panic_hook::set_once();
    let _ = console_log::init_with_level(log::Level::Info);

    // Create an event loop and run the application
    let event_loop = EventLoop::new().expect("failed to create event loop");
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::default();
    event_loop.run_app(&mut app).expect("failed to run app");
}
