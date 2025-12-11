//! Virtualized Log Viewer with ImGuiListClipper (single file)
//! - 100K+ lines with filtering/search
//! - Right-click context menu on items (copy/duplicate/delete)
//! - Auto-scroll to bottom when new lines arrive

use std::{num::NonZeroU32, sync::Arc, time::Instant};

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

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Level {
    Debug,
    Info,
    Warn,
    Error,
}

impl Level {
    fn label(self) -> &'static str {
        match self {
            Level::Debug => "DEBUG",
            Level::Info => "INFO",
            Level::Warn => "WARN",
            Level::Error => "ERROR",
        }
    }
    fn color(self) -> [f32; 4] {
        match self {
            Level::Debug => [0.6, 0.6, 0.6, 1.0],
            Level::Info => [0.6, 0.9, 1.0, 1.0],
            Level::Warn => [1.0, 0.8, 0.2, 1.0],
            Level::Error => [1.0, 0.3, 0.3, 1.0],
        }
    }
}

#[derive(Clone)]
struct LogEntry {
    lvl: Level,
    msg: String,
    ts_ms: u64,
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
    // Log state
    logs: Vec<LogEntry>,
    filtered: Vec<usize>,
    filter_text: String,
    filter_text_lower: String,
    show_lvl: [bool; 4],
    autoscroll: bool,
    last_copied: Option<String>,
    gen_seq: u64,
}

#[derive(Default)]
struct App {
    window: Option<AppWindow>,
}

impl AppWindow {
    fn new(event_loop: &ActiveEventLoop) -> Result<Self, Box<dyn std::error::Error>> {
        let window_attributes = winit::window::Window::default_attributes()
            .with_title("Dear ImGui - List Clipper Log")
            .with_inner_size(LogicalSize::new(1200.0, 720.0));

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
                NonZeroU32::new(1200).unwrap(),
                NonZeroU32::new(720).unwrap(),
            );
        let surface = unsafe {
            cfg.display()
                .create_window_surface(&cfg, &surface_attribs)?
        };
        let context = context.make_current(&surface)?;

        let mut context_imgui = Context::create();
        context_imgui.set_ini_filename(None::<String>).unwrap();
        let mut platform = WinitPlatform::new(&mut context_imgui);
        platform.attach_window(
            &window,
            dear_imgui_winit::HiDpiMode::Default,
            &mut context_imgui,
        );

        let gl = unsafe {
            glow::Context::from_loader_function_cstr(|s| {
                context.display().get_proc_address(s).cast()
            })
        };
        let mut renderer = GlowRenderer::new(gl, &mut context_imgui)?;
        renderer.set_framebuffer_srgb_enabled(false);
        renderer.new_frame()?;

        let imgui = ImguiState {
            context: context_imgui,
            platform,
            renderer,
            last_frame: Instant::now(),
        };

        let mut app = Self {
            window,
            surface,
            context,
            imgui,
            logs: Vec::new(),
            filtered: Vec::new(),
            filter_text: String::new(),
            filter_text_lower: String::new(),
            show_lvl: [true, true, true, true],
            autoscroll: true,
            last_copied: None,
            gen_seq: 0,
        };

        // Seed some lines
        app.generate_lines(2_000);
        app.rebuild_filter();
        Ok(app)
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

    fn generate_lines(&mut self, count: usize) {
        for _ in 0..count {
            self.gen_seq += 1;
            let lvl = match (self.gen_seq % 17) {
                0 | 1 => Level::Warn,
                2 => Level::Error,
                3 | 4 => Level::Debug,
                _ => Level::Info,
            };
            let msg = format!(
                "Event #{:06} - simulated log line with some payload value={}",
                self.gen_seq,
                self.gen_seq % 97
            );
            let ts_ms = self.gen_seq * 16;
            self.logs.push(LogEntry { lvl, msg, ts_ms });
        }
    }

    fn rebuild_filter(&mut self) {
        self.filtered.clear();
        let has_text = !self.filter_text_lower.is_empty();
        for (i, e) in self.logs.iter().enumerate() {
            let lvl_idx = match e.lvl {
                Level::Debug => 0,
                Level::Info => 1,
                Level::Warn => 2,
                Level::Error => 3,
            };
            if !self.show_lvl[lvl_idx] {
                continue;
            }
            if has_text {
                let m = e.msg.to_lowercase();
                if !m.contains(&self.filter_text_lower) {
                    continue;
                }
            }
            self.filtered.push(i);
        }
    }

    fn render(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // timing
        let now = Instant::now();
        let dt = now - self.imgui.last_frame;
        self.imgui.last_frame = now;
        self.imgui.context.io_mut().set_delta_time(dt.as_secs_f32());

        // frame + UI scope (drop `ui` before applying mutations)
        self.imgui
            .platform
            .prepare_frame(&self.window, &mut self.imgui.context);
        let (add_1k, add_10k, do_clear, mut recalc_filter, ctx_copy, ctx_dup, ctx_del, draw_data) = {
            let ui = self.imgui.context.frame();
            // pending actions variables captured by closure
            let mut add_1k = false;
            let mut add_10k = false;
            let mut do_clear = false;
            let mut recalc_filter = false;
            let mut ctx_copy: Option<String> = None;
            let mut ctx_dup: Option<usize> = None;
            let mut ctx_del: Option<usize> = None;

            ui.window("Virtualized Log (ListClipper)")
                .size([1000.0, 640.0], Condition::FirstUseEver)
                .build(|| {
                    // pending actions are captured from the outer scope
                    // Filter row
                    let _w = ui.push_item_width(280.0);
                    let changed = ui
                        .input_text("Filter (substring)", &mut self.filter_text)
                        .build();
                    drop(_w);
                    ui.same_line();
                    let mut lvl_dbg = self.show_lvl[0];
                    let mut lvl_inf = self.show_lvl[1];
                    let mut lvl_wrn = self.show_lvl[2];
                    let mut lvl_err = self.show_lvl[3];
                    ui.checkbox("Debug", &mut lvl_dbg);
                    ui.same_line();
                    ui.checkbox("Info", &mut lvl_inf);
                    ui.same_line();
                    ui.checkbox("Warn", &mut lvl_wrn);
                    ui.same_line();
                    ui.checkbox("Error", &mut lvl_err);
                    ui.same_line();
                    ui.checkbox("Auto-scroll", &mut self.autoscroll);

                    if changed || self.show_lvl != [lvl_dbg, lvl_inf, lvl_wrn, lvl_err] {
                        self.show_lvl = [lvl_dbg, lvl_inf, lvl_wrn, lvl_err];
                        self.filter_text_lower = self.filter_text.to_lowercase();
                        recalc_filter = true;
                    }

                    // Actions
                    if ui.button("Add 1K") {
                        add_1k = true;
                    }
                    ui.same_line();
                    if ui.button("Add 10K") {
                        add_10k = true;
                    }
                    ui.same_line();
                    if ui.button("Clear") {
                        do_clear = true;
                    }

                    if let Some(last) = &self.last_copied {
                        ui.same_line();
                        ui.text_disabled(format!("Copied: {}", last));
                    }

                    ui.separator();

                    // Log viewport
                    ui.child_window("log_view").size([0.0, 0.0]).build(&ui, || {
                        let should_scroll = self.autoscroll && (ui.scroll_y() >= ui.scroll_max_y());
                        let total = self.filtered.len() as i32;
                        let clipper = ListClipper::new(total).begin(&ui).iter();
                        for i in clipper {
                            let idx = self.filtered[i as usize];
                            let e = &self.logs[idx];
                            // One item per line using Selectable (gives a valid ID for context menu)
                            let line_display = format!("[{}] {}", e.lvl.label(), e.msg);
                            let label = format!("{}##{}", line_display, idx);
                            let color = ui.push_style_color(StyleColor::Text, e.lvl.color());
                            ui.selectable_config(label).selected(false).build();
                            color.pop();

                            // Per-item context menu
                            if let Some(_ctx) = ui.begin_popup_context_item() {
                                if ui.menu_item("Copy line") {
                                    ctx_copy = Some(e.msg.clone());
                                }
                                if ui.menu_item("Duplicate") {
                                    ctx_dup = Some(idx);
                                }
                                if ui.menu_item("Delete") {
                                    ctx_del = Some(idx);
                                }
                            }
                        }
                        if should_scroll {
                            ui.set_scroll_here_y(1.0);
                        }
                    });
                });

            // finalize draw data while `ui` is alive
            self.imgui
                .platform
                .prepare_render_with_ui(&ui, &self.window);
            let draw_data = self.imgui.context.render();
            (
                add_1k,
                add_10k,
                do_clear,
                recalc_filter,
                ctx_copy,
                ctx_dup,
                ctx_del,
                draw_data,
            )
        };

        // Render GL clear
        if let Some(gl) = self.imgui.renderer.gl_context() {
            unsafe {
                gl.clear_color(0.1, 0.2, 0.3, 1.0);
                gl.clear(glow::COLOR_BUFFER_BIT);
            }
        }
        self.imgui.renderer.new_frame()?;
        self.imgui.renderer.render(&draw_data)?;
        self.surface.swap_buffers(&self.context)?;

        // Apply pending actions after rendering
        if do_clear {
            self.logs.clear();
            self.filtered.clear();
        }
        if add_1k {
            self.generate_lines(1_000);
            recalc_filter = true;
        }
        if add_10k {
            self.generate_lines(10_000);
            recalc_filter = true;
        }
        if let Some(i) = ctx_dup {
            if i < self.logs.len() {
                let clone = self.logs[i].clone();
                self.logs.insert(i + 1, clone);
                recalc_filter = true;
            }
        }
        if let Some(i) = ctx_del {
            if i < self.logs.len() {
                self.logs.remove(i);
                recalc_filter = true;
            }
        }
        if let Some(s) = ctx_copy {
            self.last_copied = Some(s);
        }
        if recalc_filter {
            self.rebuild_filter();
        }

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
            Some(w) => w,
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
