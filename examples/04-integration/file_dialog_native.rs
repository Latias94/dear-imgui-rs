//! Native (rfd) File Dialog example via dear-file-browser
//! - Demonstrates non-blocking dialogs on a background thread
//! - Buttons for: Open File(s), Pick Folder, Save File

use std::{
    num::NonZeroU32,
    sync::{Arc, mpsc},
    thread,
    time::Instant,
};

use dear_file_browser::{Backend, DialogMode, FileDialog};
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
    // demo state
    status: String,
    busy: bool,
    rx: mpsc::Receiver<String>,
}

#[derive(Default)]
struct App {
    window: Option<AppWindow>,
}

impl AppWindow {
    fn new(event_loop: &ActiveEventLoop) -> Result<Self, Box<dyn std::error::Error>> {
        let window_attributes = winit::window::Window::default_attributes()
            .with_title("Dear ImGui - File Dialog (Native)")
            .with_inner_size(LogicalSize::new(980.0, 640.0));
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
                NonZeroU32::new(980).unwrap(),
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

        let (_tx, rx) = mpsc::channel();
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
            status: String::new(),
            busy: false,
            rx,
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
        let dt = now - self.imgui.last_frame;
        self.imgui.last_frame = now;
        self.imgui.context.io_mut().set_delta_time(dt.as_secs_f32());

        // Poll results from background threads
        while let Ok(msg) = self.rx.try_recv() {
            self.status = msg;
            self.busy = false;
        }

        self.imgui
            .platform
            .prepare_frame(&self.window, &mut self.imgui.context);
        let ui = self.imgui.context.frame();

        // Defer actions until after UI borrow ends to avoid E0500
        let mut do_open_file = false;
        let mut do_open_files = false;
        let mut do_pick_folder = false;
        let mut do_save_file = false;

        ui.window("File Dialog (Native)")
            .size([700.0, 520.0], Condition::FirstUseEver)
            .build(|| {
                let can = !self.busy;
                if can && ui.button("Open File") {
                    do_open_file = true;
                }
                ui.same_line();
                if can && ui.button("Open Files") {
                    do_open_files = true;
                }
                ui.same_line();
                if can && ui.button("Pick Folder") {
                    do_pick_folder = true;
                }
                ui.same_line();
                if can && ui.button("Save File") {
                    do_save_file = true;
                }

                ui.separator();
                if self.busy {
                    ui.text_colored([1.0, 0.8, 0.2, 1.0], "Dialog open...");
                }
                ui.text(&self.status);
            });

        if let Some(gl) = self.imgui.renderer.gl_context() {
            unsafe {
                gl.clear_color(0.05, 0.06, 0.08, 1.0);
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

        // Trigger actions after UI/render to avoid borrow conflicts
        if do_open_file {
            self.spawn(DialogMode::OpenFile);
        }
        if do_open_files {
            self.spawn(DialogMode::OpenFiles);
        }
        if do_pick_folder {
            self.spawn(DialogMode::PickFolder);
        }
        if do_save_file {
            self.spawn(DialogMode::SaveFile);
        }
        Ok(())
    }

    fn spawn(&mut self, mode: DialogMode) {
        let (tx, rx) = mpsc::channel();
        self.rx = rx;
        self.busy = true;
        thread::spawn(move || {
            let res = FileDialog::new(mode).backend(Backend::Auto).open_blocking();
            let msg = match res {
                Ok(sel) => {
                    let mut s = format!("OK ({} path(s))\n", sel.paths.len());
                    for p in sel.paths {
                        s.push_str(&format!("  - {}\n", p.display()));
                    }
                    s
                }
                Err(e) => format!("ERR: {e}"),
            };
            let _ = tx.send(msg);
        });
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
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(w) = &mut self.window else {
            return;
        };
        if w.window.id() != window_id {
            return;
        }
        // Feed to ImGui platform first
        let full_event: winit::event::Event<()> = winit::event::Event::WindowEvent {
            window_id,
            event: event.clone(),
        };
        w.imgui
            .platform
            .handle_event(&mut w.imgui.context, &w.window, &full_event);
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => w.resize(size),
            WindowEvent::RedrawRequested => {
                if let Err(e) = w.render() {
                    eprintln!("render error: {e}");
                    event_loop.exit();
                    return;
                }
                w.window.request_redraw();
            }
            _ => {}
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::default();
    event_loop.run_app(&mut app)?;
    Ok(())
}
