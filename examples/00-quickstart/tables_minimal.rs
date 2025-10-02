//! Tables minimal example (single file).
//! Shows a basic 3-column table with sorting and resizing.

use std::{cmp::Ordering, num::NonZeroU32, sync::Arc, time::Instant};

use dear_imgui_glow::GlowRenderer;
use dear_imgui_rs::*;
use dear_imgui_winit::WinitPlatform;
use glow::HasContext;
use glutin::{
    config::ConfigTemplateBuilder,
    context::{ContextAttributesBuilder, NotCurrentGlContext, PossiblyCurrentContext},
    display::{GetGlDisplay, GlDisplay},
    surface::{GlSurface, Surface, SurfaceAttributesBuilder, WindowSurface},
};
use raw_window_handle::HasWindowHandle;
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::{Window, WindowId},
};

#[derive(Clone)]
struct Row {
    name: String,
    qty: i32,
    price: f32,
}

struct ImguiState {
    context: Context,
    platform: WinitPlatform,
    renderer: GlowRenderer,
    last_frame: Instant,
}

struct AppWindow {
    window: Arc<Window>,
    surface: Surface<WindowSurface>,
    context: PossiblyCurrentContext,
    imgui: ImguiState,
    rows: Vec<Row>,
}

#[derive(Default)]
struct App {
    window: Option<AppWindow>,
}

impl AppWindow {
    fn new(event_loop: &ActiveEventLoop) -> Result<Self, Box<dyn std::error::Error>> {
        let window_attributes = winit::window::Window::default_attributes()
            .with_title("Dear ImGui - Tables Minimal")
            .with_inner_size(LogicalSize::new(1000.0, 640.0));

        let (window, cfg) = glutin_winit::DisplayBuilder::new()
            .with_window_attributes(Some(window_attributes))
            .build(event_loop, ConfigTemplateBuilder::new(), |mut configs| {
                configs.next().unwrap()
            })?;

        let window = Arc::new(window.unwrap());

        let context_attribs =
            ContextAttributesBuilder::new().build(Some(window.window_handle()?.as_raw()));
        let context = unsafe { cfg.display().create_context(&cfg, &context_attribs)? };

        let surface_attribs = SurfaceAttributesBuilder::<WindowSurface>::new()
            .with_srgb(Some(false))
            .build(
                window.window_handle()?.as_raw(),
                NonZeroU32::new(1000).unwrap(),
                NonZeroU32::new(640).unwrap(),
            );
        let surface = unsafe {
            cfg.display()
                .create_window_surface(&cfg, &surface_attribs)?
        };
        let context = context.make_current(&surface)?;

        let mut imgui_context = Context::create();
        imgui_context.set_ini_filename(None::<String>).unwrap();

        let mut platform = WinitPlatform::new(&mut imgui_context);
        platform.attach_window(
            &window,
            dear_imgui_winit::HiDpiMode::Default,
            &mut imgui_context,
        );

        let gl = unsafe {
            glow::Context::from_loader_function_cstr(|s| {
                context.display().get_proc_address(s).cast()
            })
        };
        let mut renderer = GlowRenderer::new(gl, &mut imgui_context)?;
        renderer.set_framebuffer_srgb_enabled(false);
        renderer.new_frame()?;

        let imgui = ImguiState {
            context: imgui_context,
            platform,
            renderer,
            last_frame: Instant::now(),
        };

        // Demo dataset
        let rows = vec![
            Row {
                name: "Apples".into(),
                qty: 12,
                price: 1.2,
            },
            Row {
                name: "Bananas".into(),
                qty: 8,
                price: 0.9,
            },
            Row {
                name: "Cherries".into(),
                qty: 24,
                price: 2.6,
            },
            Row {
                name: "Dates".into(),
                qty: 5,
                price: 3.1,
            },
            Row {
                name: "Elderberry".into(),
                qty: 13,
                price: 4.2,
            },
        ];

        Ok(Self {
            window,
            surface,
            context,
            imgui,
            rows,
        })
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.surface.resize(
                &self.context,
                NonZeroU32::new(new_size.width).unwrap(),
                NonZeroU32::new(new_size.height).unwrap(),
            );
        }
    }

    fn apply_sort(ui: &Ui, rows: &mut [Row]) {
        use dear_imgui_rs::SortDirection;

        if let Some(mut specs) = ui.table_get_sort_specs() {
            if specs.is_dirty() {
                for s in specs.iter() {
                    // Sort by first spec only for minimal demo
                    match (s.column_index, s.sort_direction) {
                        (0, SortDirection::Ascending) => rows.sort_by(|a, b| a.name.cmp(&b.name)),
                        (0, SortDirection::Descending) => rows.sort_by(|a, b| b.name.cmp(&a.name)),
                        (1, SortDirection::Ascending) => rows.sort_by(|a, b| a.qty.cmp(&b.qty)),
                        (1, SortDirection::Descending) => rows.sort_by(|a, b| b.qty.cmp(&a.qty)),
                        (2, SortDirection::Ascending) => rows.sort_by(|a, b| {
                            a.price.partial_cmp(&b.price).unwrap_or(Ordering::Equal)
                        }),
                        (2, SortDirection::Descending) => rows.sort_by(|a, b| {
                            b.price.partial_cmp(&a.price).unwrap_or(Ordering::Equal)
                        }),
                        _ => {}
                    }
                    break;
                }
                specs.clear_dirty();
            }
        }

        // Prevent unused warning if flags import gets optimized out in some builds
        let _ = TableFlags::NONE;
    }

    fn render(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let now = Instant::now();
        let delta_time = now - self.imgui.last_frame;
        self.imgui
            .context
            .io_mut()
            .set_delta_time(delta_time.as_secs_f32());
        self.imgui.last_frame = now;

        self.imgui
            .platform
            .prepare_frame(&self.window, &mut self.imgui.context);
        let ui = self.imgui.context.frame();

        ui.window("Tables Minimal")
            .size([640.0, 400.0], Condition::FirstUseEver)
            .build(|| {
                use dear_imgui_rs::{TableColumnFlags, TableFlags};

                ui.text("Sortable, resizable 3-column table");
                ui.separator();

                let mut rows = self.rows.clone();

                ui.table("inventory")
                    .flags(
                        TableFlags::RESIZABLE
                            | TableFlags::REORDERABLE
                            | TableFlags::ROW_BG
                            | TableFlags::BORDERS_OUTER
                            | TableFlags::BORDERS_V
                            | TableFlags::SORTABLE,
                    )
                    .outer_size([0.0, 0.0])
                    .column("Name")
                    .flags(TableColumnFlags::PREFER_SORT_ASCENDING)
                    .done()
                    .column("Qty")
                    .done()
                    .column("Price")
                    .done()
                    .headers(true)
                    .build(|ui| {
                        // Apply sort if any
                        Self::apply_sort(ui, &mut rows);

                        for r in &rows {
                            ui.table_next_row();
                            ui.table_next_column();
                            ui.text(&r.name);
                            ui.table_next_column();
                            ui.text(format!("{}", r.qty));
                            ui.table_next_column();
                            ui.text(format!("${:.2}", r.price));
                        }
                    });

                // Keep a sorted copy in state (optional)
                self.rows = rows;
            });

        if let Some(gl) = self.imgui.renderer.gl_context() {
            unsafe {
                gl.clear_color(0.1, 0.2, 0.3, 1.0);
                gl.clear(glow::COLOR_BUFFER_BIT);
            }
        }

        self.imgui
            .platform
            .prepare_render_with_ui(&ui, &self.window);
        let draw_data = self.imgui.context.render();
        self.imgui.renderer.new_frame()?;
        self.imgui.renderer.render(&draw_data)?;
        self.surface.swap_buffers(&self.context)?;
        Ok(())
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            match AppWindow::new(event_loop) {
                Ok(window) => {
                    self.window = Some(window);
                    self.window.as_ref().unwrap().window.request_redraw();
                }
                Err(e) => {
                    eprintln!("Failed to create window: {e}");
                    event_loop.exit();
                }
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let window = match self.window.as_mut() {
            Some(window) => window,
            None => return,
        };
        let full_event: winit::event::Event<()> = winit::event::Event::WindowEvent {
            window_id,
            event: event.clone(),
        };
        window
            .imgui
            .platform
            .handle_event(&mut window.imgui.context, &window.window, &full_event);

        match event {
            WindowEvent::Resized(size) => {
                window.resize(size);
                window.window.request_redraw();
            }
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput { event, .. } => {
                if event.logical_key == Key::Named(NamedKey::Escape) {
                    event_loop.exit();
                }
            }
            WindowEvent::RedrawRequested => {
                if let Err(e) = window.render() {
                    eprintln!("Render error: {e}");
                }
                window.window.request_redraw();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.window.request_redraw();
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
