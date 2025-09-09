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
    clear_color: wgpu::Color,
    last_frame: Instant,
    // Widget demo state
    text_input: String,
    slider_value: f32,
    checkbox_value: bool,
    radio_value: i32,
    combo_current: usize,
    color_value: [f32; 4],
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
struct App {
    window: Option<AppWindow>,
}

impl AppWindow {
    fn setup_gpu(event_loop: &ActiveEventLoop) -> Self {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });

        let version = env!("CARGO_PKG_VERSION");
        let size = LogicalSize::new(1280.0, 720.0);

        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title(&format!("Dear ImGui Widgets Demo - {version}"))
                        .with_inner_size(size),
                )
                .expect("Failed to create window"),
        );

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
        context.set_ini_filename(None::<String>);

        let mut platform = WinitPlatform::new(&mut context);
        platform.attach_window(
            &self.window,
            dear_imgui_winit::HiDpiMode::Default,
            &mut context,
        );

        let mut renderer = WgpuRenderer::new(&self.device, &self.queue, self.surface_desc.format);

        // Load font texture
        renderer.reload_font_texture(&mut context, &self.device, &self.queue);

        self.imgui = Some(ImguiState {
            context,
            platform,
            renderer,
            clear_color: wgpu::Color {
                r: 0.1,
                g: 0.2,
                b: 0.3,
                a: 1.0,
            },
            last_frame: Instant::now(),
            text_input: String::from("Hello, World!"),
            slider_value: 0.5,
            checkbox_value: true,
            radio_value: 0,
            combo_current: 0,
            color_value: [1.0, 0.5, 0.0, 1.0],
        });
    }

    fn render(&mut self) {
        let imgui = self.imgui.as_mut().unwrap();

        let now = Instant::now();
        let delta_time = (now - imgui.last_frame).as_secs_f32();
        imgui.context.io_mut().set_delta_time(delta_time);
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

        // Main widgets demo window
        ui.window("Widgets Demo")
            .size([600.0, 500.0], Condition::FirstUseEver)
            .position([50.0, 50.0], Condition::FirstUseEver)
            .build(|| {
                ui.text("Dear ImGui Widgets Showcase");
                ui.separator();

                // Basic widgets
                ui.text("Basic Widgets:");

                if ui.button("Click me!") {
                    println!("Button clicked!");
                }

                ui.same_line();
                if ui.small_button("Small") {
                    println!("Small button clicked!");
                }

                ui.checkbox("Checkbox", &mut imgui.checkbox_value);

                if ui.radio_button("Radio A", imgui.radio_value == 0) {
                    imgui.radio_value = 0;
                }
                ui.same_line();
                if ui.radio_button("Radio B", imgui.radio_value == 1) {
                    imgui.radio_value = 1;
                }
                ui.same_line();
                if ui.radio_button("Radio C", imgui.radio_value == 2) {
                    imgui.radio_value = 2;
                }

                ui.separator();

                // Input widgets
                ui.text("Input Widgets:");
                ui.input_text("Text Input", &mut imgui.text_input).build();

                ui.slider("Slider", 0.0, 1.0, &mut imgui.slider_value);

                let combo_items = ["Item 1", "Item 2", "Item 3", "Item 4"];
                ui.combo("Combo", &mut imgui.combo_current, &combo_items, |item| {
                    (*item).into()
                });

                ui.separator();

                // Color widgets - 暂时注释掉测试
                ui.text("Color Widgets:");
                // ui.color_edit4("Color Editor", &mut imgui.color_value);

                ui.separator();

                // Information display
                ui.text("System Information:");
                ui.text(&format!(
                    "Application average {:.3} ms/frame ({:.1} FPS)",
                    1000.0 / ui.io().framerate(),
                    ui.io().framerate()
                ));

                // 恢复系统信息显示功能
                ui.text(&format!("ImGui Time: {:.2}", ui.time()));
                ui.text(&format!("Frame Count: {}", ui.frame_count()));

                let cursor_pos = ui.get_cursor_screen_pos();
                ui.text(&format!(
                    "Cursor Screen Pos: [{:.1}, {:.1}]",
                    cursor_pos[0], cursor_pos[1]
                ));

                // 测试修复后的 content_region_avail 函数
                let content_avail = ui.content_region_avail();
                ui.text(&format!(
                    "Content Region Avail: [{:.1}, {:.1}]",
                    content_avail[0], content_avail[1]
                ));
            });

        // Style editor window
        ui.window("Style Editor")
            .size([400.0, 600.0], Condition::FirstUseEver)
            .position([670.0, 50.0], Condition::FirstUseEver)
            .build(|| {
                ui.text("Style Customization");
                ui.separator();

                // 颜色编辑功能有问题 - 暂时注释
                ui.text("Color Editor Test:");
                // ui.color_edit4("Test Color", &mut imgui.color_value); // 导致访问违例

                ui.separator();
                ui.text("Successfully restored: time(), frame_count(), get_cursor_screen_pos()");
            });

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

            let draw_data = imgui.context.render();
            imgui
                .renderer
                .render_with_renderpass(draw_data, &self.queue, &self.device, &mut rpass)
                .expect("Rendering failed");
        }

        self.queue.submit(Some(encoder.finish()));
        frame.present();
    }
}

impl ApplicationHandler for App {
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

        let full_event: winit::event::Event<()> = winit::event::Event::WindowEvent {
            window_id: window.window.id(),
            event: event.clone(),
        };
        imgui
            .platform
            .handle_event(&mut imgui.context, &window.window, &full_event);

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

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::default();
    event_loop.run_app(&mut app).unwrap();
}
