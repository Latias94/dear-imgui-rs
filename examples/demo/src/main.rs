//! Dear ImGui Demo Application
//!
//! This example demonstrates how to use Dear ImGui with winit and WGPU,
//! showing a complete GUI application with various controls and features.

use std::sync::Arc;

use dear_imgui::*;
use dear_imgui_wgpu::WgpuRenderer;
use dear_imgui_winit::{create_window_attributes, WinitPlatform};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

/// Application state
struct App {
    // Window and graphics
    window: Option<Arc<Window>>,
    surface: Option<wgpu::Surface<'static>>,
    device: Option<wgpu::Device>,
    queue: Option<wgpu::Queue>,
    surface_config: Option<wgpu::SurfaceConfiguration>,

    // Dear ImGui
    imgui_context: Option<Context>,
    platform: Option<WinitPlatform>,
    renderer: Option<WgpuRenderer>,

    // Demo state
    show_demo_window: bool,
    counter: i32,
    text_input: String,
    float_value: f32,
    color_value: Color,
    checkbox_value: bool,
    combo_current: i32,
    listbox_current: i32,
    progress: f32,
    selectable_items: [bool; 3],
    show_color_picker: bool,
    show_tree_demo: bool,
    show_table_demo: bool,
}

impl Default for App {
    fn default() -> Self {
        Self {
            window: None,
            surface: None,
            device: None,
            queue: None,
            surface_config: None,
            imgui_context: None,
            platform: None,
            renderer: None,
            show_demo_window: true,
            counter: 0,
            text_input: String::from("Hello, Dear ImGui!"),
            float_value: 0.5,
            color_value: Color::rgb(1.0, 0.5, 0.0),
            checkbox_value: true,
            combo_current: 0,
            listbox_current: 0,
            progress: 0.0,
            selectable_items: [false, true, false],
            show_color_picker: true,
            show_tree_demo: true,
            show_table_demo: true,
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // Create window
        let window_attributes =
            create_window_attributes("Dear ImGui Demo", LogicalSize::new(1200.0, 800.0));

        let window = match event_loop.create_window(window_attributes) {
            Ok(window) => Arc::new(window),
            Err(err) => {
                eprintln!("Failed to create window: {}", err);
                event_loop.exit();
                return;
            }
        };

        // Initialize WGPU
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::from_env_or_default());

        let surface = instance.create_surface(window.clone()).unwrap();

        // Request adapter
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .unwrap();

        // Request device and queue
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("Dear ImGui Device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            memory_hints: wgpu::MemoryHints::MemoryUsage,
            trace: wgpu::Trace::Off,
        }))
        .unwrap();

        // Configure surface
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let size = window.inner_size();
        let surface_config = surface
            .get_default_config(&adapter, size.width, size.height)
            .unwrap();
        surface.configure(&device, &surface_config);

        // Initialize Dear ImGui
        let mut imgui_context = Context::new().unwrap();
        let mut platform = WinitPlatform::new(&mut imgui_context);
        let mut renderer = WgpuRenderer::new(&device, &queue, surface_format);

        // Load font texture
        renderer.reload_font_texture(&mut imgui_context, &device, &queue);

        // Store everything
        self.window = Some(window);
        self.surface = Some(surface);
        self.device = Some(device);
        self.queue = Some(queue);
        self.surface_config = Some(surface_config);
        self.imgui_context = Some(imgui_context);
        self.platform = Some(platform);
        self.renderer = Some(renderer);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let window = match &self.window {
            Some(window) if window.id() == window_id => window,
            _ => return,
        };

        // Handle platform events
        if let Some(platform) = &mut self.platform {
            let event: Event<()> = Event::WindowEvent {
                window_id,
                event: event.clone(),
            };
            platform.handle_event(&event, window.as_ref());
        }

        match event {
            WindowEvent::CloseRequested => {
                println!("Close requested");
                event_loop.exit();
            }
            WindowEvent::Resized(physical_size) => {
                if let (Some(surface), Some(device), Some(surface_config)) =
                    (&self.surface, &self.device, &mut self.surface_config)
                {
                    surface_config.width = physical_size.width;
                    surface_config.height = physical_size.height;
                    surface.configure(device, surface_config);
                }
                window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                self.render();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

impl App {
    fn render(&mut self) {
        let window = match &self.window {
            Some(window) => window,
            None => return,
        };

        let (surface, device, queue, imgui_context, platform, renderer) = match (
            &self.surface,
            &self.device,
            &self.queue,
            &mut self.imgui_context,
            &mut self.platform,
            &mut self.renderer,
        ) {
            (Some(s), Some(d), Some(q), Some(ic), Some(p), Some(r)) => (s, d, q, ic, p, r),
            _ => return,
        };

        // Prepare Dear ImGui frame
        platform.prepare_frame(window.as_ref(), imgui_context);
        let mut frame = imgui_context.frame();

        // Build UI
        let (
            counter,
            float_value,
            text_input,
            show_demo_window,
            combo_current,
            listbox_current,
            progress,
            selectable_items,
            checkbox_value,
            color_value,
            show_color_picker,
            show_tree_demo,
            show_table_demo,
        ) = (
            &mut self.counter,
            &mut self.float_value,
            &mut self.text_input,
            &mut self.show_demo_window,
            &mut self.combo_current,
            &mut self.listbox_current,
            &mut self.progress,
            &mut self.selectable_items,
            &mut self.checkbox_value,
            &mut self.color_value,
            &mut self.show_color_picker,
            &mut self.show_tree_demo,
            &mut self.show_table_demo,
        );

        // Main menu bar
        if frame.begin_main_menu_bar() {
            if frame.begin_menu("File") {
                if frame.menu_item("New") {
                    println!("New file");
                }
                if frame.menu_item("Open") {
                    println!("Open file");
                }
                if frame.menu_item("Save") {
                    println!("Save file");
                }
                frame.end_menu();
            }

            if frame.begin_menu("View") {
                frame.menu_item_bool("Show Color Picker", show_color_picker);
                frame.menu_item_bool("Show Tree Demo", show_tree_demo);
                frame.menu_item_bool("Show Table Demo", show_table_demo);
                frame.end_menu();
            }

            if frame.begin_menu("Help") {
                if frame.menu_item("About") {
                    println!("Dear ImGui Rust Demo v0.1.0");
                }
                frame.end_menu();
            }

            frame.end_main_menu_bar();
        }

        // Main demo window
        frame
            .window("Dear ImGui Demo")
            .size([400.0, 500.0])
            .position([50.0, 50.0])
            .show(|ui| {
                ui.text("Welcome to Dear ImGui!");
                ui.text_colored(Color::GREEN, "This is a demo application");
                ui.separator();

                if ui.button("Click me!") {
                    *counter += 1;
                }
                ui.same_line();
                ui.text(format!("Clicked {} times", counter));

                ui.separator();

                ui.checkbox("Show demo window", show_demo_window);
                ui.checkbox("Checkbox value", checkbox_value);
                ui.slider_float("Float value", float_value, 0.0, 1.0);
                ui.input_text("Text input", text_input);

                ui.separator();
                ui.text("Combo and ListBox:");

                let combo_items = ["Option A", "Option B", "Option C"];
                ui.combo("Combo", combo_current, &combo_items);

                let listbox_items = ["Item 1", "Item 2", "Item 3", "Item 4"];
                ui.listbox("ListBox", listbox_current, &listbox_items, 3);

                ui.separator();
                ui.text("Progress and Bullets:");

                // Update progress
                *progress += 0.01;
                if *progress > 1.0 {
                    *progress = 0.0;
                }
                ui.progress_bar(*progress, Some(&format!("{:.0}%", *progress * 100.0)));

                ui.bullet_text("This is a bullet point");
                ui.bullet_text("Another bullet point");

                ui.separator();
                ui.text("Color Controls:");

                if ui.color_edit("Edit Color", color_value) {
                    println!("Color changed: {:?}", color_value);
                }

                if ui.color_button("color_preview", *color_value) {
                    println!("Color button clicked!");
                }
                ui.same_line();
                ui.text("Click the color swatch");

                ui.separator();
                ui.text("Selectable items:");

                for (i, selected) in selectable_items.iter_mut().enumerate() {
                    ui.selectable(&format!("Selectable {}", i + 1), selected);
                }

                ui.separator();
                if ui.button("Reset") {
                    *counter = 0;
                    *float_value = 0.5;
                    *text_input = String::from("Hello, Dear ImGui!");
                    *combo_current = 0;
                    *listbox_current = 0;
                    *progress = 0.0;
                    *checkbox_value = true;
                    selectable_items.fill(false);
                }

                true // Keep window open
            });

        // Additional demo windows
        if *show_demo_window {
            frame
                .window("Another Window")
                .size([300.0, 200.0])
                .position([500.0, 100.0])
                .show(|ui| {
                    ui.text("This is another window!");
                    ui.text_disabled("You can have multiple windows");

                    if ui.button("Close") {
                        *show_demo_window = false;
                    }

                    true // Keep window open
                });
        }

        // Color picker window
        if *show_color_picker {
            frame
                .window("Color Picker")
                .size([400.0, 500.0])
                .position([750.0, 100.0])
                .show(|ui| {
                    ui.text("Advanced Color Controls:");
                    ui.separator();

                    if ui.color_picker("Color Picker", color_value) {
                        println!("Color picked: {:?}", color_value);
                    }

                    true
                });
        }

        // Tree demo window
        if *show_tree_demo {
            frame
                .window("Tree Demo")
                .size([300.0, 400.0])
                .position([750.0, 650.0])
                .show(|ui| {
                    ui.text("Tree Structure:");
                    ui.separator();

                    if ui.tree_node("Documents") {
                        ui.text("📄 document1.txt");
                        ui.text("📄 document2.txt");

                        if ui.tree_node("Subfolder") {
                            ui.text("📄 nested_file.txt");
                            ui.tree_pop();
                        }

                        ui.tree_pop();
                    }

                    if ui.tree_node("Images") {
                        ui.text("🖼️ photo1.jpg");
                        ui.text("🖼️ photo2.png");
                        ui.tree_pop();
                    }

                    ui.separator();

                    if ui.collapsing_header("Settings") {
                        ui.text("⚙️ Setting 1");
                        ui.text("⚙️ Setting 2");
                        ui.text("⚙️ Setting 3");
                    }

                    true
                });
        }

        // Table demo window
        if *show_table_demo {
            frame
                .window("Table Demo")
                .size([500.0, 300.0])
                .position([100.0, 950.0])
                .show(|ui| {
                    ui.text("Data Table:");
                    ui.separator();

                    if ui.begin_table("DataTable", 4) {
                        // Setup columns
                        ui.table_setup_column("ID");
                        ui.table_setup_column("Name");
                        ui.table_setup_column("Age");
                        ui.table_setup_column("Department");
                        ui.table_headers_row();

                        // Sample data
                        let data = [
                            ("001", "Alice Johnson", "28", "Engineering"),
                            ("002", "Bob Smith", "32", "Marketing"),
                            ("003", "Carol Davis", "25", "Design"),
                            ("004", "David Wilson", "35", "Sales"),
                            ("005", "Eva Brown", "29", "Engineering"),
                        ];

                        for (id, name, age, dept) in &data {
                            ui.table_next_row();
                            ui.table_next_column();
                            ui.text(id);
                            ui.table_next_column();
                            ui.text(name);
                            ui.table_next_column();
                            ui.text(age);
                            ui.table_next_column();
                            ui.text(dept);
                        }

                        ui.end_table();
                    }

                    true
                });
        }

        // Get draw data
        let draw_data = frame.draw_data();

        // Render
        let output = surface.get_current_texture().unwrap();
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Dear ImGui Render Encoder"),
        });

        // Clear the screen first
        {
            let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Clear Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
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
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        }

        // Render Dear ImGui
        renderer
            .render(device, queue, &mut encoder, &view, &draw_data)
            .unwrap();

        queue.submit(std::iter::once(encoder.finish()));
        window.pre_present_notify();
        output.present();
    }
}

fn main() -> Result<()> {
    env_logger::init();

    println!("Starting Dear ImGui Demo Application");

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::default();
    event_loop.run_app(&mut app).unwrap();

    Ok(())
}
