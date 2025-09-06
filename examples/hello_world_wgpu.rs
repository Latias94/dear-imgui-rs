//! Hello World WGPU Example
//!
//! This example demonstrates how to use dear-imgui with winit and wgpu.
//! It creates a simple window with ImGui widgets rendered using WGPU.

use dear_imgui::{FontSource, *};
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
    demo_open: bool,
    last_frame: Instant,
    last_cursor: Option<MouseCursor>,
    counter: i32,
    text_input: String,
    slider_value: f32,
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

        let window = {
            let version = env!("CARGO_PKG_VERSION");

            let size = LogicalSize::new(1280.0, 720.0);

            let attributes = Window::default_attributes()
                .with_inner_size(size)
                .with_title(format!("Dear ImGui Hello World - WGPU {version}"));
            Arc::new(event_loop.create_window(attributes).unwrap())
        };

        let size = window.inner_size();
        let hidpi_factor = window.scale_factor();
        let surface = instance.create_surface(window.clone()).unwrap();

        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .unwrap();

        let (device, queue) =
            block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();

        // Set up swap chain
        let surface_desc = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![wgpu::TextureFormat::Bgra8Unorm],
        };

        surface.configure(&device, &surface_desc);

        let imgui = None;
        Self {
            device,
            queue,
            window,
            surface_desc,
            surface,
            hidpi_factor,
            imgui,
        }
    }

    fn setup_imgui(&mut self) {
        let mut context = Context::create();
        let mut platform = WinitPlatform::new(&mut context);
        platform.attach_window(
            &self.window,
            dear_imgui_winit::HiDpiMode::Default,
            &mut context,
        );

        // Set INI filename to None to disable settings persistence
        context.set_ini_filename(None::<&str>);

        // Configure font scaling for HiDPI displays
        let _font_size = (13.0 * self.hidpi_factor) as f32;
        context
            .io_mut()
            .set_font_global_scale((1.0 / self.hidpi_factor) as f32);

        // Add default font with basic configuration
        // Temporarily disabled due to FontAtlas implementation issues
        // context.fonts().add_font(&[FontSource::DefaultFontData {
        //     config: Some(FontConfig::default()),
        // }]);

        println!("Skipping font loading for now");

        //
        // Set up dear imgui wgpu renderer
        //
        let clear_color = wgpu::Color {
            r: 0.1,
            g: 0.2,
            b: 0.3,
            a: 1.0,
        };

        let mut renderer = WgpuRenderer::new(&self.device, &self.queue, self.surface_desc.format);

        // Load font texture
        renderer.reload_font_texture(&mut context, &self.device, &self.queue);

        let last_frame = Instant::now();
        let last_cursor = None;
        let demo_open = true;

        self.imgui = Some(ImguiState {
            context,
            platform,
            renderer,
            clear_color,
            demo_open,
            last_frame,
            last_cursor,
            counter: 0,
            text_input: String::from("Hello, World!"),
            slider_value: 0.5,
        })
    }

    fn new(event_loop: &ActiveEventLoop) -> Self {
        let mut window = Self::setup_gpu(event_loop);
        window.setup_imgui();
        window
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        println!("App resumed, creating window...");
        self.window = Some(AppWindow::new(event_loop));
        println!("Window created successfully");
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let window = self.window.as_mut().unwrap();
        let imgui = window.imgui.as_mut().unwrap();

        match &event {
            WindowEvent::Resized(size) => {
                window.surface_desc = wgpu::SurfaceConfiguration {
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    format: wgpu::TextureFormat::Bgra8UnormSrgb,
                    width: size.width,
                    height: size.height,
                    present_mode: wgpu::PresentMode::Fifo,
                    desired_maximum_frame_latency: 2,
                    alpha_mode: wgpu::CompositeAlphaMode::Auto,
                    view_formats: vec![wgpu::TextureFormat::Bgra8Unorm],
                };

                window
                    .surface
                    .configure(&window.device, &window.surface_desc);
            }
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput { event, .. } => {
                if let Key::Named(NamedKey::Escape) = event.logical_key {
                    if event.state.is_pressed() {
                        event_loop.exit();
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                imgui
                    .context
                    .io_mut()
                    .set_delta_time((now - imgui.last_frame).as_secs_f32());
                imgui.last_frame = now;

                let frame = match window.surface.get_current_texture() {
                    Ok(frame) => frame,
                    Err(e) => {
                        eprintln!("dropped frame: {e:?}");
                        return;
                    }
                };

                imgui
                    .platform
                    .prepare_frame(&window.window, &mut imgui.context);

                let mouse_cursor = {
                    let ui = imgui.context.frame();

                    // Main Hello World window
                    {
                        let window = ui.window("Hello World!");
                        window
                            .size([400.0, 300.0], Condition::FirstUseEver)
                            .position([50.0, 50.0], Condition::FirstUseEver)
                            .build(ui, || {
                                ui.text("Welcome to Dear ImGui with WGPU!");
                                ui.separator();

                                ui.text("This is a simple hello world example.");
                                ui.text("You can interact with the widgets below:");

                                ui.spacing();

                                // Test DrawList functionality
                                ui.text("DrawList Test:");

                                // Get the window draw list
                                let draw_list = ui.get_window_draw_list();

                                // Draw a line
                                draw_list
                                    .add_line([50.0, 150.0], [150.0, 200.0], 0xFF_FF_FF_FF)
                                    .build();

                                // Draw a rectangle
                                draw_list
                                    .add_rect([200.0, 150.0], [300.0, 200.0], 0xFF_00_FF_00)
                                    .build();

                                // Draw a circle
                                draw_list
                                    .add_circle([100.0, 250.0], 20.0, 0xFF_FF_00_00)
                                    .build();

                                // Draw some text
                                draw_list.add_text(
                                    [50.0, 280.0],
                                    0xFF_FF_FF_FF,
                                    "Custom drawn text!",
                                );

                                ui.text("✅ DrawList functions are working!");
                                ui.spacing();

                                // Test new advanced DrawList functionality
                                ui.text("Advanced DrawList Test:");

                                // Test Bezier curve
                                draw_list
                                    .add_bezier_curve(
                                        [50.0, 320.0],  // start point
                                        [100.0, 300.0], // control point 1
                                        [150.0, 340.0], // control point 2
                                        [200.0, 320.0], // end point
                                        0xFF_FF_00_FF,  // magenta color
                                    )
                                    .thickness(2.0)
                                    .build();

                                // Test polyline (unfilled)
                                let triangle_points =
                                    vec![[250.0, 320.0], [275.0, 300.0], [300.0, 320.0]];
                                draw_list
                                    .add_polyline(triangle_points, 0xFF_00_FF_FF)
                                    .thickness(2.0)
                                    .build();

                                // Test filled polygon
                                let square_points = vec![
                                    [320.0, 300.0],
                                    [350.0, 300.0],
                                    [350.0, 330.0],
                                    [320.0, 330.0],
                                ];
                                draw_list
                                    .add_polyline(square_points, 0xFF_FF_FF_00)
                                    .filled(true)
                                    .build();

                                ui.text("✅ Advanced DrawList functions are working!");
                                ui.spacing();

                                // Test utility functions
                                ui.text("Utility Functions Test:");
                                ui.text(&format!("ImGui Time: {:.2}", ui.time()));
                                ui.text(&format!("Frame Count: {}", ui.frame_count()));
                                ui.text(&format!(
                                    "Button Color Name: {}",
                                    ui.style_color_name(dear_imgui::StyleColor::Button)
                                ));
                                ui.text("✅ Utility functions are working!");
                                ui.spacing();

                                // Test InputText functionality
                                ui.text("InputText Test:");

                                // Create InputText widget using the UI method
                                if ui.input_text("Text Input", &mut imgui.text_input).build() {
                                    println!("InputText changed: {}", imgui.text_input);
                                }

                                ui.text(format!("Current text: {}", imgui.text_input));
                                ui.text("✅ InputText functions are working!");
                                ui.spacing();

                                // Counter button
                                if ui.button("Click me!") {
                                    imgui.counter += 1;
                                }
                                ui.same_line();
                                ui.text(format!("Clicked {} times", imgui.counter));

                                ui.spacing();

                                // Text input
                                ui.input_text("Text input", &mut imgui.text_input).build();

                                // Slider
                                ui.slider("Slider", 0.0, 1.0, &mut imgui.slider_value);

                                ui.spacing();

                                // Color picker for background
                                let mut color = [
                                    imgui.clear_color.r as f32,
                                    imgui.clear_color.g as f32,
                                    imgui.clear_color.b as f32,
                                ];
                                if ui.color_edit3("Background Color", &mut color) {
                                    imgui.clear_color.r = color[0] as f64;
                                    imgui.clear_color.g = color[1] as f64;
                                    imgui.clear_color.b = color[2] as f64;
                                }
                            });
                    }

                    // Info window
                    {
                        let info_window = ui.window("Information");
                        info_window
                            .size([300.0, 200.0], Condition::FirstUseEver)
                            .position([500.0, 50.0], Condition::FirstUseEver)
                            .build(ui, || {
                                let delta_s = imgui.last_frame.elapsed();
                                ui.text(format!(
                                    "Frametime: {:.3}ms",
                                    delta_s.as_secs_f32() * 1000.0
                                ));
                                ui.text(format!("FPS: {:.1}", 1.0 / delta_s.as_secs_f32()));

                                ui.separator();

                                // Mouse position
                                let mouse_pos = ui.io().mouse_pos();
                                ui.text(format!(
                                    "Mouse Position: ({:.1}, {:.1})",
                                    mouse_pos[0], mouse_pos[1]
                                ));

                                ui.text(format!(
                                    "Window size: {}x{}",
                                    window.surface_desc.width, window.surface_desc.height
                                ));

                                ui.separator();

                                ui.checkbox("Show Demo Window", &mut imgui.demo_open);
                            });
                    }

                    // Show demo window if enabled
                    if imgui.demo_open {
                        ui.show_demo_window(&mut imgui.demo_open);
                    }

                    // Get mouse cursor before ui goes out of scope
                    ui.mouse_cursor()
                };

                // End the frame and get draw data
                let draw_data = imgui.context.render();

                // Debug: Check if we have draw data
                // println!("DrawData valid: {}, draw lists count: {}",
                //     draw_data.valid(),
                //     draw_data.draw_lists().count());

                // for (i, draw_list) in draw_data.draw_lists().enumerate() {
                //     println!("DrawList {}: vtx_buffer len: {}, idx_buffer len: {}, commands: {}",
                //         i,
                //         draw_list.vtx_buffer().len(),
                //         draw_list.idx_buffer().len(),
                //         draw_list.commands().count());
                // }

                let mut encoder: wgpu::CommandEncoder = window
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

                // Mouse cursor handling
                if imgui.last_cursor != mouse_cursor {
                    imgui.last_cursor = mouse_cursor;
                    // imgui.platform.prepare_render(&ui, &window.window);
                }

                let view = frame
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());
                let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: None,
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

                // Render ImGui
                imgui
                    .renderer
                    .render_with_renderpass(draw_data, &window.queue, &window.device, &mut rpass)
                    .expect("Rendering failed");

                drop(rpass);

                window.queue.submit(Some(encoder.finish()));

                frame.present();
            }
            _ => (),
        }

        let winit_event: Event<()> = Event::WindowEvent { window_id, event };
        imgui
            .platform
            .handle_event(&winit_event, &window.window, &mut imgui.context);
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        let window = self.window.as_mut().unwrap();
        window.window.request_redraw();
    }
}

fn main() {
    env_logger::init();
    println!("Starting Dear ImGui WGPU Example...");

    let event_loop = EventLoop::new().unwrap();
    println!("Created event loop");

    event_loop.set_control_flow(ControlFlow::Poll);
    println!("Starting event loop...");

    event_loop.run_app(&mut App::default()).unwrap();
}
