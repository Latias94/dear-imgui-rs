//! Dear ImPlot Basic Example - Modular API Showcase
//!
//! This example demonstrates the new modular ImPlot API with comprehensive
//! plot type support. It showcases:
//!
//! - Line plots and scatter plots
//! - Bar charts (simple and positional)
//! - Histograms (1D and 2D)
//! - Heatmaps with configurable scaling
//! - Pie charts with various options
//! - Error bars and shaded plots
//! - NEW: Stairs plots, Digital plots, Text annotations
//! - NEW: Bar groups, Dummy plots, Stems plots
//! - Plot combinations in single charts
//! - Error handling and validation
//!
//! The example uses a tabbed interface to organize different plot types
//! and demonstrates the builder pattern API for ergonomic plot creation.

use dear_imgui::*;
use dear_imgui_wgpu::WgpuRenderer;
use dear_imgui_winit::WinitPlatform;
use dear_implot::*;
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
    plot_context: PlotContext,
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
}

impl AppWindow {
    fn new(event_loop: &ActiveEventLoop) -> Result<Self, Box<dyn std::error::Error>> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });

        let window = {
            let version = env!("CARGO_PKG_VERSION");
            let size = LogicalSize::new(1280.0, 720.0);

            Arc::new(
                event_loop.create_window(
                    Window::default_attributes()
                        .with_title(&format!("Dear ImGui + ImPlot Example - {version}"))
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
        .expect("Failed to find an appropriate adapter");

        let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))?;

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

        // Setup ImGui
        let mut context = Context::create_or_panic();
        context.set_ini_filename_or_panic(None::<String>);

        let mut platform = WinitPlatform::new(&mut context);
        platform.attach_window(&window, dear_imgui_winit::HiDpiMode::Default, &mut context);

        let mut renderer = WgpuRenderer::new();

        // Initialize the renderer with the new API
        let init_info =
            dear_imgui_wgpu::WgpuInitInfo::new(device.clone(), queue.clone(), surface_desc.format);
        renderer
            .init(init_info)
            .expect("Failed to initialize WGPU renderer");

        // Prepare font atlas
        renderer
            .prepare_font_atlas(&mut context)
            .expect("Failed to prepare font atlas");

        // Setup ImPlot
        let plot_context = PlotContext::create(&context);

        let imgui = ImguiState {
            context,
            platform,
            renderer,
            plot_context,
            clear_color: wgpu::Color {
                r: 0.1,
                g: 0.2,
                b: 0.3,
                a: 1.0,
            },
            last_frame: Instant::now(),
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

    fn render(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let now = Instant::now();
        let delta_time = now - self.imgui.last_frame;
        self.imgui.last_frame = now;

        self.imgui
            .context
            .io_mut()
            .set_delta_time(delta_time.as_secs_f32());

        self.imgui
            .platform
            .prepare_frame(&self.window, &mut self.imgui.context);

        let ui = self.imgui.context.frame();
        let plot_ui = self.imgui.plot_context.get_plot_ui(&ui);

        // Sample data for plots
        let x_data: Vec<f64> = (0..100).map(|i| i as f64 * 0.1).collect();
        let y_data: Vec<f64> = x_data.iter().map(|x| x.sin()).collect();
        let scatter_x: Vec<f64> = (0..50).map(|i| i as f64 * 0.2).collect();
        let scatter_y: Vec<f64> = scatter_x.iter().map(|x| (x * 2.0).cos()).collect();

        // Additional data for new plot types
        let bar_values = vec![1.0, 3.0, 2.0, 4.0, 3.0, 5.0, 4.0, 2.0];
        let histogram_data = vec![1.0, 2.0, 1.0, 3.0, 2.0, 4.0, 3.0, 2.0, 1.0, 2.0, 3.0, 1.0];
        let heatmap_data: Vec<f64> = (0..100)
            .map(|i| ((i / 10) as f64 * 0.5).sin() * ((i % 10) as f64 * 0.3).cos())
            .collect();
        let errors = vec![0.1, 0.15, 0.2, 0.1, 0.25, 0.15, 0.1, 0.2];

        // Data for new plot types
        let stairs_x = vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let stairs_y = vec![1.0, 3.0, 2.0, 4.0, 2.0, 5.0, 3.0];
        let digital_x = vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0];
        let digital_y = vec![0.0, 1.0, 1.0, 0.0, 1.0, 0.0, 0.0, 1.0];

        // Bar groups data: 3 series, 4 groups each
        let group_labels = vec!["Series A", "Series B", "Series C"];
        let group_values = vec![
            2.0, 3.0, 1.0, 2.5, // Series A values for 4 groups
            1.5, 2.5, 2.0, 3.0, // Series B values for 4 groups
            3.0, 1.0, 2.5, 1.5, // Series C values for 4 groups
        ];

        // Main window with tabbed interface
        ui.window("ImPlot Demo - Modular API Showcase")
            .size([1200.0, 800.0], Condition::FirstUseEver)
            .build(|| {
                ui.text("Welcome to Dear ImGui + ImPlot with New Modular API!");
                ui.text("This demo showcases all the new plot types and features.");
                ui.separator();

                if let Some(tab_bar) = ui.tab_bar("PlotTabs") {
                    // Basic Plots Tab
                    if let Some(tab) = ui.tab_item("Basic Plots") {
                        ui.columns(2, "basic_plots", true);

                        // Line plot
                        ui.text("Line Plot:");
                        if let Some(token) = plot_ui.begin_plot("Line Plot") {
                            LinePlot::new("sin(x)", &x_data, &y_data).plot();
                            token.end();
                        }

                        ui.next_column();

                        // Scatter plot
                        ui.text("Scatter Plot:");
                        if let Some(token) = plot_ui.begin_plot("Scatter Plot") {
                            ScatterPlot::new("cos(2x)", &scatter_x, &scatter_y).plot();
                            token.end();
                        }

                        ui.columns(1, "reset_basic", false); // Reset column state
                        tab.end();
                    }

                    // Bar Charts Tab
                    if let Some(tab) = ui.tab_item("Bar Charts") {
                        ui.columns(2, "bar_plots", true);

                        // Simple bar chart
                        ui.text("Bar Chart:");
                        if let Some(token) = plot_ui.begin_plot("Bar Chart") {
                            BarPlot::new("Values", &bar_values)
                                .with_bar_size(0.8)
                                .plot();
                            token.end();
                        }

                        ui.next_column();

                        // Bar Groups
                        ui.text("Bar Groups:");
                        if let Some(token) = plot_ui.begin_plot("Bar Groups") {
                            BarGroupsPlot::new(group_labels.clone(), &group_values, 3, 4)
                                .with_group_size(0.75)
                                .plot();
                            token.end();
                        }

                        ui.columns(1, "reset_bar", false); // Reset column state
                        tab.end();
                    }

                    // Signal & Step Plots Tab
                    if let Some(tab) = ui.tab_item("Signal & Step Plots") {
                        ui.columns(2, "signal_plots", true);

                        // Stairs Plot
                        ui.text("Stairs Plot:");
                        if let Some(token) = plot_ui.begin_plot("Stairs Plot") {
                            StairsPlot::new("Steps", &stairs_x, &stairs_y)
                                .pre_step()
                                .plot();
                            token.end();
                        }

                        ui.next_column();

                        // Digital Plot
                        ui.text("Digital Plot:");
                        if let Some(token) = plot_ui.begin_plot("Digital Signals") {
                            DigitalPlot::new("Signal", &digital_x, &digital_y).plot();
                            token.end();
                        }

                        ui.columns(1, "reset_signal", false); // Reset column state
                        tab.end();
                    }

                    // Histograms Tab
                    if let Some(tab) = ui.tab_item("Histograms") {
                        ui.columns(2, "hist_plots", true);

                        // 1D Histogram
                        ui.text("1D Histogram:");
                        if let Some(token) = plot_ui.begin_plot("Histogram") {
                            HistogramPlot::new("Distribution", &histogram_data)
                                .with_bins(8)
                                .density()
                                .plot();
                            token.end();
                        }

                        ui.next_column();

                        // 2D Histogram
                        ui.text("2D Histogram:");
                        if let Some(token) = plot_ui.begin_plot("2D Histogram") {
                            let x_hist = vec![1.0, 2.0, 3.0, 2.0, 1.0, 3.0, 2.0, 1.0];
                            let y_hist = vec![1.0, 1.0, 2.0, 3.0, 2.0, 1.0, 3.0, 2.0];
                            Histogram2DPlot::new("2D Data", &x_hist, &y_hist)
                                .with_bins(4, 4)
                                .plot();
                            token.end();
                        }

                        ui.columns(1, "reset_hist", false); // Reset column state
                        tab.end();
                    }

                    // Advanced Plots Tab
                    if let Some(tab) = ui.tab_item("Advanced Plots") {
                        ui.columns(2, "advanced_plots", true);

                        // Heatmap
                        ui.text("Heatmap:");
                        if let Some(token) = plot_ui.begin_plot("Heatmap") {
                            HeatmapPlot::new("Temperature", &heatmap_data, 10, 10)
                                .with_scale(-1.0, 1.0)
                                .with_bounds(0.0, 0.0, 1.0, 1.0)
                                .with_label_format(Some("%.2f"))
                                .plot();
                            token.end();
                        }

                        ui.next_column();

                        // Text Annotations
                        ui.text("Text Annotations:");
                        if let Some(token) = plot_ui.begin_plot("Text Plot") {
                            // Plot some data first
                            LinePlot::new("Data", &x_data[..20], &y_data[..20]).plot();

                            // Add text annotations
                            TextPlot::new("Peak", 1.5, 0.9)
                                .with_pixel_offset(10.0, -10.0)
                                .plot();
                            TextPlot::new("Valley", 4.7, -0.9)
                                .with_pixel_offset(-20.0, 10.0)
                                .plot();
                            token.end();
                        }

                        ui.columns(1, "reset_advanced", false); // Reset column state
                        tab.end();
                    }

                    // Error & Shaded Plots Tab
                    if let Some(tab) = ui.tab_item("Error & Shaded") {
                        ui.columns(2, "error_plots", true);

                        // Error bars
                        ui.text("Error Bars:");
                        if let Some(token) = plot_ui.begin_plot("Error Bars") {
                            let x_err = vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0];
                            let y_err = vec![1.0, 2.5, 2.0, 3.5, 3.0, 4.0, 3.5, 2.0];

                            // Line plot first
                            LinePlot::new("Data", &x_err, &y_err).plot();

                            // Then error bars
                            ErrorBarsPlot::new("Errors", &x_err, &y_err, &errors).plot();
                            token.end();
                        }

                        ui.next_column();

                        // Pie Chart
                        ui.text("Pie Chart:");
                        if let Some(token) = plot_ui.begin_plot("Pie Chart") {
                            let pie_labels = vec!["Apples", "Bananas", "Cherries", "Dates"];
                            let pie_values = [0.35, 0.25, 0.25, 0.15];
                            PieChartPlot::new(pie_labels, &pie_values, 0.5, 0.5, 0.35)
                                .normalize()
                                .exploding()
                                .plot();
                            token.end();
                        }

                        ui.columns(1, "reset_error", false); // Reset column state
                        tab.end();
                    }

                    // Utility Plots Tab
                    if let Some(tab) = ui.tab_item("Utility Plots") {
                        ui.columns(2, "utility_plots", true);

                        // Stems Plot
                        ui.text("Stems Plot:");
                        if let Some(token) = plot_ui.begin_plot("Stems Plot") {
                            StemPlot::new("Stems", &stairs_x, &stairs_y)
                                .with_y_ref(0.0)
                                .plot();
                            token.end();
                        }

                        ui.next_column();

                        // Dummy Plot for Legend
                        ui.text("Legend with Dummy Items:");
                        if let Some(token) = plot_ui.begin_plot("Legend Demo") {
                            // Real plots
                            LinePlot::new("Real Data", &x_data[..10], &y_data[..10]).plot();

                            // Dummy items for legend organization
                            DummyPlot::new("--- Separator ---").plot();
                            DummyPlot::new("Future Feature 1").plot();
                            DummyPlot::new("Future Feature 2").plot();
                            token.end();
                        }

                        ui.columns(1, "reset_utility", false); // Reset column state
                        tab.end();
                    }

                    // Combination Tab
                    if let Some(tab) = ui.tab_item("Combinations") {
                        ui.text("Multiple plot types in one chart:");

                        if let Some(token) = plot_ui.begin_plot("Combined Plot") {
                            // Multiple plot types in one chart
                            LinePlot::new("Trend", &x_data[..30], &y_data[..30]).plot();
                            ScatterPlot::new("Points", &scatter_x[..15], &scatter_y[..15]).plot();

                            // Add stairs and digital plots
                            StairsPlot::new("Steps", &stairs_x[..5], &stairs_y[..5]).plot();

                            // Shaded area
                            let x_combined = vec![0.0, 1.0, 2.0, 3.0];
                            let y_combined = vec![0.5, 0.8, 0.6, 0.9];
                            ShadedPlot::new("Background", &x_combined, &y_combined)
                                .with_y_ref(0.0)
                                .plot();

                            token.end();
                        }

                        tab.end();
                    }

                    tab_bar.end();
                }
            });

        // Advanced Features Window
        ui.window("Advanced Features")
            .size([600.0, 500.0], Condition::FirstUseEver)
            .position([850.0, 50.0], Condition::FirstUseEver)
            .build(|| {
                ui.text("Advanced ImPlot Features:");
                ui.separator();

                // Subplot example (commented out as it requires additional bindings)
                ui.text("Subplots (Future Feature):");
                ui.text_disabled("SubplotGrid::new(\"Grid\", 2, 2).begin()");
                ui.text_disabled("  // Multiple plots in grid layout");
                ui.text_disabled("  // Each subplot can have different plot types");
                ui.separator();

                // Multi-axis example (commented out as it requires additional bindings)
                ui.text("Multi-Axis Plots (Future Feature):");
                ui.text_disabled("MultiAxisPlot::new(\"Multi\")");
                ui.text_disabled("  .add_y_axis(YAxisConfig { ... })");
                ui.text_disabled("  // Multiple Y-axes with different scales");
                ui.separator();

                // Error handling demonstration
                ui.text("Error Handling:");
                ui.text("The new API provides comprehensive error handling:");

                if ui.button("Test Empty Data Error") {
                    let empty_data: &[f64] = &[];
                    let x_data = [1.0, 2.0, 3.0];
                    match LinePlot::new("Empty", empty_data, &x_data).validate() {
                        Err(e) => println!("Caught error: {}", e),
                        Ok(_) => println!("Unexpected success"),
                    }
                }

                if ui.button("Test Length Mismatch Error") {
                    let short_data = [1.0, 2.0];
                    let long_data = [1.0, 2.0, 3.0, 4.0];
                    match LinePlot::new("Mismatch", &short_data, &long_data).validate() {
                        Err(e) => println!("Caught error: {}", e),
                        Ok(_) => println!("Unexpected success"),
                    }
                }

                ui.separator();
                ui.text("API Comparison:");
                ui.text("Old API: plot_ui.plot_line(\"label\", &x, &y)");
                ui.text("New API: LinePlot::new(\"label\", &x, &y).plot()");
                ui.text("Benefits:");
                ui.bullet_text("Type safety and validation");
                ui.bullet_text("Builder pattern for configuration");
                ui.bullet_text("Modular design");
                ui.bullet_text("Comprehensive error handling");
            });

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

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
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

            let draw_data = self.imgui.context.render();
            self.imgui
                .renderer
                .render_draw_data(&draw_data, &mut render_pass)?;
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            match AppWindow::new(event_loop) {
                Ok(window) => {
                    self.window = Some(window);
                }
                Err(e) => {
                    eprintln!("Failed to create window: {}", e);
                    event_loop.exit();
                }
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if let Some(window) = &mut self.window {
            let winit_event: winit::event::Event<()> = winit::event::Event::WindowEvent {
                window_id: id,
                event: event.clone(),
            };
            window.imgui.platform.handle_event(
                &mut window.imgui.context,
                &window.window,
                &winit_event,
            );

            match event {
                WindowEvent::CloseRequested => {
                    event_loop.exit();
                }
                WindowEvent::Resized(new_size) => {
                    window.resize(new_size);
                }
                WindowEvent::RedrawRequested => {
                    if let Err(e) = window.render() {
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
