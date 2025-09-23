//! Minimal ImPlot sanity check to isolate crashes
//!
//! - Starts a WGPU + Winit window
//! - Creates Dear ImGui context + WGPU renderer
//! - Uses ImPlot via dear-implot
//! - UI toggles to call specific ImPlot API subsets

use dear_imgui::*;
use dear_imgui_wgpu::WgpuRenderer;
use dear_imgui_winit::WinitPlatform;
use dear_implot as implot;
use dear_implot::Plot;
use pollster::block_on;
use std::{sync::Arc, time::Instant};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

struct ImguiState {
    context: Context,
    platform: WinitPlatform,
    renderer: WgpuRenderer,
    plot_context: implot::PlotContext,
    clear_color: wgpu::Color,
    last_frame: Instant,
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
    // toggles
    begin_only: bool,
    plot_line: bool,
    plot_scatter: bool,
    call_hover: bool,
    call_mouse_pos: bool,
}

impl AppWindow {
    fn new(event_loop: &ActiveEventLoop) -> Result<Self, Box<dyn std::error::Error>> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let window = {
            let size = LogicalSize::new(960.0, 600.0);
            Arc::new(
                event_loop.create_window(
                    Window::default_attributes()
                        .with_title("ImPlot Basic Simple")
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
        .map_err(|e| format!("Adapter error: {e}"))?;
        let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))?;
        let size = LogicalSize::new(960.0, 600.0);
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

        // ImGui
        let mut context = Context::create_or_panic();
        context.set_ini_filename_or_panic(None::<String>);
        let mut platform = WinitPlatform::new(&mut context);
        platform.attach_window(&window, dear_imgui_winit::HiDpiMode::Default, &mut context);
        let init_info = dear_imgui_wgpu::WgpuInitInfo::new(device.clone(), queue.clone(), surface_desc.format);
        let renderer = WgpuRenderer::new(init_info, &mut context)?;

        let plot_context = implot::PlotContext::create(&context);
        println!("Plot context created");

        let imgui = ImguiState {
            context,
            platform,
            renderer,
            plot_context,
            clear_color: wgpu::Color { r: 0.06, g: 0.06, b: 0.08, a: 1.0 },
            last_frame: Instant::now(),
        };

        Ok(Self { device, queue, window, surface_desc, surface, imgui })
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.surface_desc.width = new_size.width;
            self.surface_desc.height = new_size.height;
            self.surface.configure(&self.device, &self.surface_desc);
        }
    }

    fn render(&mut self, app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
        let now = Instant::now();
        let delta_time = now - self.imgui.last_frame;
        self.imgui.last_frame = now;
        self.imgui.context.io_mut().set_delta_time(delta_time.as_secs_f32());
        self.imgui.platform.prepare_frame(&self.window, &mut self.imgui.context);

        let ui = self.imgui.context.frame();
        let plot_ui = self.imgui.plot_context.get_plot_ui(&ui);

        // test data
        let x_data: Vec<f64> = (0..64).map(|i| i as f64 * 0.1).collect();
        let y_data: Vec<f64> = x_data.iter().map(|x| x.sin()).collect();
        
        ui.window("Controls")
            .size([320.0, 200.0], Condition::FirstUseEver)
            .build(|| {
                ui.checkbox("BeginPlot only", &mut app.begin_only);
                ui.checkbox("PlotLine", &mut app.plot_line);
                ui.checkbox("PlotScatter", &mut app.plot_scatter);
                ui.checkbox("Call IsPlotHovered", &mut app.call_hover);
                ui.checkbox("Call GetPlotMousePos", &mut app.call_mouse_pos);
            });

        ui.window("Plot Window")
            .size([640.0, 360.0], Condition::FirstUseEver)
            .build(|| {
                if let Some(token) = plot_ui.begin_plot_with_size("Simple", [600.0, 320.0]) {
                    if app.plot_line {
                        implot::LinePlot::new("line", &x_data, &y_data).plot();
                    }
                    if app.plot_scatter {
                        implot::ScatterPlot::new("scatter", &x_data, &y_data).plot();
                    }
                    if app.call_hover {
                        let _ = plot_ui.is_plot_hovered();
                    }
                    if app.call_mouse_pos {
                        let _p = plot_ui.get_plot_mouse_pos(None);
                        let _ = _p.x + _p.y; // use values
                    }
                    token.end();
                }
            });

        // WGPU render
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("imgui encoder") });
        let output = self.surface.get_current_texture()?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("imgui render pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(self.imgui.clear_color), store: wgpu::StoreOp::Store },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        let draw_data = self.imgui.context.render();
        self.imgui.renderer.render_draw_data(draw_data, &mut rpass)?;
        drop(rpass);
        self.queue.submit(Some(encoder.finish()));
        output.present();
        Ok(())
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            match AppWindow::new(event_loop) {
                Ok(window) => self.window = Some(window),
                Err(e) => {
                    eprintln!("Create window failed: {}", e);
                    event_loop.exit();
                }
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if let Some(window) = &mut self.window {
            let winit_event: winit::event::Event<()> = winit::event::Event::WindowEvent { window_id: id, event: event.clone() };
            window.imgui.platform.handle_event(&mut window.imgui.context, &window.window, &winit_event);
            match event {
                WindowEvent::CloseRequested => event_loop.exit(),
                WindowEvent::Resized(new_size) => window.resize(new_size),
                WindowEvent::RedrawRequested => {
                    // avoid double mutable borrow: borrow only `begin_only` toggles snapshot
                    let begin_only = self.begin_only;
                    let plot_line = self.plot_line;
                    let plot_scatter = self.plot_scatter;
                    let call_hover = self.call_hover;
                    let call_mouse_pos = self.call_mouse_pos;
                    let mut tmp = App {
                        window: None,
                        begin_only,
                        plot_line,
                        plot_scatter,
                        call_hover,
                        call_mouse_pos,
                    };
                    if let Err(e) = window.render(&mut tmp) {
                        eprintln!("Render error: {}", e);
                    }
                    window.window.request_redraw();
                }
                _ => {}
            }
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::default();
    event_loop.run_app(&mut app)?;
    Ok(())
}
