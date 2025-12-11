//! Integration: Logging Console with filter and history (single file)
//! - Text log with levels + timestamps
//! - Filter box (substring match)
//! - Command input with Enter to submit, history recall via Prev/Next buttons
//! - Context menu: Clear / Copy Visible

use std::{
    num::NonZeroU32,
    sync::Arc,
    time::{Duration, Instant},
};

use dear_imgui_glow::GlowRenderer;
use dear_imgui_rs::*;
use dear_imgui_rs::{ClipboardBackend, DummyClipboardBackend};
use dear_imgui_winit::WinitPlatform;
use glow::HasContext;
use glutin::{
    config::ConfigTemplateBuilder,
    context::{ContextAttributesBuilder, NotCurrentGlContext, PossiblyCurrentContext},
    display::{GetGlDisplay, GlDisplay},
    surface::{GlSurface, Surface, SurfaceAttributesBuilder, WindowSurface},
};
use raw_window_handle::HasWindowHandle;
use std::sync::Mutex;
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
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Clone, Debug)]
struct LogItem {
    t: Instant,
    lvl: Level,
    text: String,
}

impl LogItem {
    fn fmt_line(&self, start: Instant) -> String {
        let dt = self.t.saturating_duration_since(start);
        let ms = dt.as_millis();
        let lvl = match self.lvl {
            Level::Trace => "TRACE",
            Level::Debug => "DEBUG",
            Level::Info => "INFO ",
            Level::Warn => "WARN ",
            Level::Error => "ERROR",
        };
        format!("[{ms:05}] [{lvl}] {}", self.text)
    }
}

struct ConsoleState {
    start: Instant,
    logs: Vec<LogItem>,
    input: String,
    history: Vec<String>,
    hist_pos: isize, // -1 = current
    filter: String,
    autoscroll: bool,
    status: String,
}

impl Default for ConsoleState {
    fn default() -> Self {
        Self {
            start: Instant::now(),
            logs: Vec::new(),
            input: String::new(),
            history: Vec::new(),
            hist_pos: -1,
            filter: String::new(),
            autoscroll: true,
            status: String::new(),
        }
    }
}

impl ConsoleState {
    fn log(&mut self, lvl: Level, s: impl Into<String>) {
        self.logs.push(LogItem {
            t: Instant::now(),
            lvl,
            text: s.into(),
        });
    }
    fn clear(&mut self) {
        self.logs.clear();
    }
    fn submit(&mut self) {
        let cmd = self.input.trim().to_string();
        if cmd.is_empty() {
            return;
        }
        // echo
        self.log(Level::Info, format!("> {}", cmd));
        // history (avoid dup of last)
        if self.history.last().map(|h| h != &cmd).unwrap_or(true) {
            self.history.push(cmd.clone());
        }
        self.hist_pos = -1;
        self.input.clear();
        // handle basic commands
        match cmd.to_lowercase().as_str() {
            "clear" => self.clear(),
            "help" => {
                self.log(
                    Level::Info,
                    "Commands: HELP, CLEAR, LOG <text>, WARN <text>, ERROR <text>",
                );
            }
            cmd if cmd.starts_with("log ") => {
                self.log(Level::Info, &cmd[4..]);
            }
            cmd if cmd.starts_with("warn ") => {
                self.log(Level::Warn, &cmd[5..]);
            }
            cmd if cmd.starts_with("error ") => {
                self.log(Level::Error, &cmd[6..]);
            }
            _ => self.log(Level::Warn, "Unknown command. Type HELP."),
        }
    }
    fn recall_prev(&mut self) {
        if self.history.is_empty() {
            return;
        }
        if self.hist_pos < 0 {
            self.hist_pos = self.history.len() as isize - 1;
        } else if self.hist_pos > 0 {
            self.hist_pos -= 1;
        }
        self.input = self.history[self.hist_pos as usize].clone();
    }
    fn recall_next(&mut self) {
        if self.history.is_empty() {
            return;
        }
        if self.hist_pos >= 0 {
            self.hist_pos += 1;
        }
        if self.hist_pos >= self.history.len() as isize {
            self.hist_pos = -1;
            self.input.clear();
        } else {
            self.input = self.history[self.hist_pos as usize].clone();
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
    console: ConsoleState,
    last_fake_tick: Instant,
}

#[derive(Default)]
struct App {
    window: Option<AppWindow>,
}

impl AppWindow {
    fn new(event_loop: &ActiveEventLoop) -> Result<Self, Box<dyn std::error::Error>> {
        let window_attributes = winit::window::Window::default_attributes()
            .with_title("Dear ImGui - Console (Integration)")
            .with_inner_size(LogicalSize::new(1100.0, 720.0));
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
                NonZeroU32::new(1100).unwrap(),
                NonZeroU32::new(720).unwrap(),
            );
        let surface = unsafe {
            cfg.display()
                .create_window_surface(&cfg, &surface_attribs)?
        };
        let context = context.make_current(&surface)?;

        let mut imgui_context = Context::create();
        imgui_context.set_ini_filename(None::<String>).unwrap();
        // Clipboard backend (system clipboard via arboard)
        struct ArboardClipboard {
            inner: Mutex<arboard::Clipboard>,
        }
        impl ClipboardBackend for ArboardClipboard {
            fn get(&mut self) -> Option<String> {
                match self.inner.get_mut().unwrap().get_text() {
                    Ok(s) => Some(s),
                    Err(_) => None,
                }
            }
            fn set(&mut self, value: &str) {
                let _ = self.inner.get_mut().unwrap().set_text(value.to_owned());
            }
        }
        match arboard::Clipboard::new() {
            Ok(cb) => imgui_context.set_clipboard_backend(ArboardClipboard {
                inner: Mutex::new(cb),
            }),
            Err(_) => imgui_context.set_clipboard_backend(DummyClipboardBackend),
        }
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

        Ok(Self {
            window,
            surface,
            context,
            imgui: ImguiState {
                context: imgui_context,
                platform,
                renderer,
                last_frame: Instant::now(),
            },
            console: ConsoleState::default(),
            last_fake_tick: Instant::now(),
        })
    }

    fn resize(&mut self, sz: winit::dpi::PhysicalSize<u32>) {
        if sz.width > 0 && sz.height > 0 {
            self.surface.resize(
                &self.context,
                NonZeroU32::new(sz.width).unwrap(),
                NonZeroU32::new(sz.height).unwrap(),
            );
        }
    }

    fn render(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Simulate incoming logs every second
        if self.last_fake_tick.elapsed() >= Duration::from_millis(1000) {
            self.console.log(Level::Info, "Tick...");
            self.last_fake_tick = Instant::now();
        }

        let now = Instant::now();
        let dt = now - self.imgui.last_frame;
        self.imgui.last_frame = now;
        self.imgui.context.io_mut().set_delta_time(dt.as_secs_f32());

        self.imgui
            .platform
            .prepare_frame(&self.window, &mut self.imgui.context);
        let ui = self.imgui.context.frame();

        ui.window("Console")
            .size([900.0, 600.0], Condition::FirstUseEver)
            .build(|| {
                // Toolbar
                if ui.button("Clear") {
                    self.console.clear();
                }
                ui.same_line();
                if ui.button("Copy Visible") {
                    let mut out = String::new();
                    let f = self.console.filter.to_lowercase();
                    for it in &self.console.logs {
                        let line = it.fmt_line(self.console.start);
                        if f.is_empty() || line.to_lowercase().contains(&f) {
                            out.push_str(&line);
                            out.push('\n');
                        }
                    }
                    // Use ImGui clipboard path if available (calls our backend)
                    if !out.is_empty() {
                        if let Ok(cstr) = std::ffi::CString::new(out.clone()) {
                            unsafe { dear_imgui_rs::sys::igSetClipboardText(cstr.as_ptr()) };
                            self.console.status =
                                format!("Copied {} lines to clipboard", out.lines().count());
                        } else {
                            self.console.status = "[ERR] Clipboard text contains NUL".to_string();
                        }
                    } else {
                        self.console.status = "Nothing to copy".to_string();
                    }
                }
                ui.same_line();
                ui.checkbox("Auto-scroll", &mut self.console.autoscroll);
                ui.same_line();
                ui.text_disabled(&self.console.status);

                // Filter
                ui.separator();
                ui.input_text("Filter", &mut self.console.filter)
                    .hint("substring...")
                    .build();

                // Log area
                ui.separator();
                ui.child_window("log_region")
                    .size([0.0, -68.0])
                    .build(&ui, || {
                        let f = self.console.filter.to_lowercase();
                        let mut visible: Vec<usize> = Vec::with_capacity(self.console.logs.len());
                        for (i, it) in self.console.logs.iter().enumerate() {
                            let line = it.fmt_line(self.console.start);
                            if f.is_empty() || line.to_lowercase().contains(&f) {
                                visible.push(i);
                            }
                        }
                        let mut clipper = dear_imgui_rs::ListClipper::new(visible.len() as i32)
                            .items_height(ui.text_line_height_with_spacing())
                            .begin(&ui);
                        while clipper.step() {
                            for row in clipper.display_start()..clipper.display_end() {
                                let idx = visible[row as usize];
                                let line = self.console.logs[idx].fmt_line(self.console.start);
                                let label = format!("{}##{idx}", line);
                                ui.selectable_config(&label)
                                    .allow_double_click(true)
                                    .build();
                            }
                        }
                        if self.console.autoscroll {
                            ui.set_scroll_here_y(1.0);
                        }
                    });

                // Input line
                ui.separator();
                let entered = ui
                    .input_text("Command", &mut self.console.input)
                    .enter_returns_true(true)
                    .build();
                ui.same_line();
                if ui.button("Prev") {
                    self.console.recall_prev();
                }
                ui.same_line();
                if ui.button("Next") {
                    self.console.recall_next();
                }
                ui.same_line();
                if ui.button("Submit") {
                    self.console.submit();
                }
                if entered {
                    self.console.submit();
                }
            });

        self.imgui
            .platform
            .prepare_render_with_ui(&ui, &self.window);
        let draw_data = self.imgui.context.render();

        if let Some(gl) = self.imgui.renderer.gl_context() {
            unsafe {
                gl.clear_color(0.1, 0.2, 0.3, 1.0);
                gl.clear(glow::COLOR_BUFFER_BIT);
            }
        }
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
                Ok(w) => {
                    self.window = Some(w);
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
        let Some(window) = self.window.as_mut() else {
            return;
        };
        window.imgui.platform.handle_window_event(
            &mut window.imgui.context,
            &window.window,
            &event,
        );
        match event {
            WindowEvent::Resized(sz) => {
                window.resize(sz);
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
        if let Some(w) = &self.window {
            w.window.request_redraw();
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
