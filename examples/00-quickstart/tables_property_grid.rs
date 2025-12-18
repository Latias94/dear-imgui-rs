//! Tables as a Property Grid (single file, quickstart)
//! - 2-column table: labels on the left, editors on the right
//! - Freeze first column, resizable columns
//! - Mix inputs: text, checkbox, drag, combo, color edit, multiline

use std::{num::NonZeroU32, sync::Arc, time::Instant};

use dear_imgui_glow::GlowRenderer;
use dear_imgui_rs::TableColumnSetup;
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
struct Props {
    name: String,
    visible: bool,
    speed: f32,
    size: [f32; 2],
    color: [f32; 4],
    mode: usize,
    notes: String,
}

impl Default for Props {
    fn default() -> Self {
        Self {
            name: "Player".into(),
            visible: true,
            speed: 3.5,
            size: [1.0, 2.0],
            color: [0.15, 0.65, 0.95, 1.0],
            mode: 0,
            notes: "Supports multi-line text.\nUseful for descriptions.".into(),
        }
    }
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
    props: Props,
}

#[derive(Default)]
struct App {
    window: Option<AppWindow>,
}

impl AppWindow {
    fn new(event_loop: &ActiveEventLoop) -> Result<Self, Box<dyn std::error::Error>> {
        let window_attributes = winit::window::Window::default_attributes()
            .with_title("Dear ImGui - Tables Property Grid")
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

        Ok(Self {
            window,
            surface,
            context,
            imgui,
            props: Props::default(),
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

        ui.window("Property Grid")
            .size([760.0, 520.0], Condition::FirstUseEver)
            .build(|| {
                use dear_imgui_rs::{TableColumnFlags, TableFlags};

                // Two columns: Label | Editor
                let cols = [
                    TableColumnSetup::new("Property")
                        .flags(TableColumnFlags::WIDTH_FIXED)
                        .init_width_or_weight(180.0),
                    TableColumnSetup::new("Value")
                        .flags(TableColumnFlags::WIDTH_STRETCH)
                        .init_width_or_weight(1.0),
                ];

                ui.table("props")
                    .flags(
                        TableFlags::RESIZABLE
                            | TableFlags::BORDERS_OUTER
                            | TableFlags::BORDERS_V
                            | TableFlags::ROW_BG
                            | TableFlags::SIZING_STRETCH_PROP,
                    )
                    .columns(cols)
                    .freeze(1, 0)
                    .headers(true)
                    .build(|ui| {
                        // Helper: one row with label left and editor right
                        let row = |label: &str, f: &mut dyn FnMut(&Ui)| {
                            ui.table_next_row();
                            ui.table_set_column_index(0);
                            ui.text(label);
                            ui.table_set_column_index(1);
                            let _w = ui.push_item_width(-1.0);
                            f(ui);
                        };

                        // Name
                        row("Name", &mut |ui| {
                            ui.input_text("##name", &mut self.props.name).build();
                        });
                        // Visible
                        row("Visible", &mut |ui| {
                            ui.checkbox("##visible", &mut self.props.visible);
                        });
                        // Speed
                        row("Speed", &mut |ui| {
                            ui.drag_float_config("##speed")
                                .range(0.0, 20.0)
                                .speed(0.1)
                                .display_format("%.2f")
                                .build(ui, &mut self.props.speed);
                        });
                        // Size (2 floats)
                        row("Size", &mut |ui| {
                            ui.input_float2("##size", &mut self.props.size).build();
                        });
                        // Color
                        row("Color", &mut |ui| {
                            ui.color_edit4("##color", &mut self.props.color);
                        });
                        // Mode (combo)
                        row("Mode", &mut |ui| {
                            const ITEMS: [&str; 3] = ["Idle", "Walking", "Running"];
                            let preview = ITEMS[self.props.mode];
                            if let Some(_c) = ui.begin_combo("##mode", preview) {
                                for (i, it) in ITEMS.iter().enumerate() {
                                    let sel = self.props.mode == i;
                                    if ui.selectable_config(it).selected(sel).build() {
                                        self.props.mode = i;
                                    }
                                }
                            }
                        });
                        // Notes (multiline)
                        row("Notes", &mut |ui| {
                            ui.input_text_multiline("##notes", &mut self.props.notes, [0.0, 80.0])
                                .build();
                        });
                    });
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
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let window = match self.window.as_mut() {
            Some(window) => window,
            None => return,
        };

        window.imgui.platform.handle_window_event(
            &mut window.imgui.context,
            &window.window,
            &event,
        );

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
