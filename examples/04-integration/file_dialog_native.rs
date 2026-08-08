//! Native (rfd) File Dialog example via dear-file-browser
//! - Demonstrates non-blocking dialogs on a background thread
//! - Wakes the event loop when a dialog finishes instead of polling
//! - Buttons for: Open File(s), Pick Folder, Save File

use std::{fmt::Write as _, num::NonZeroU32, sync::Arc, thread, time::Instant};

use dear_file_browser::{Backend, DialogMode, FileDialog};
use dear_imgui_glow::GlowRenderer;
use dear_imgui_rs::*;
use dear_imgui_winit::WinitPlatform;
use glow::HasContext;
use glutin::{
    config::ConfigTemplateBuilder,
    context::{
        ContextAttributesBuilder, NotCurrentGlContext, PossiblyCurrentContext,
        PossiblyCurrentGlContext,
    },
    display::{GetGlDisplay, GlDisplay},
    surface::{GlSurface, Surface, SurfaceAttributesBuilder, WindowSurface},
};
use raw_window_handle::HasWindowHandle;
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy},
    window::{Window, WindowId},
};

enum UserEvent {
    DialogFinished(String),
}

struct ImguiState {
    renderer: GlowRenderer,
    platform: WinitPlatform,
    last_frame: Instant,
    renderer_shutdown_complete: bool,
    platform_shutdown_complete: bool,
    // Context must outlive every attachment, including fallback field drops after failed shutdown.
    context: Context,
}

struct CurrentGlContext {
    context: PossiblyCurrentContext,
    bound: bool,
}

impl CurrentGlContext {
    fn new(context: PossiblyCurrentContext) -> Self {
        Self {
            context,
            bound: true,
        }
    }

    fn get(&self) -> &PossiblyCurrentContext {
        &self.context
    }

    fn unbind(&mut self) -> glutin::error::Result<()> {
        if self.bound {
            self.context.make_not_current_in_place()?;
            self.bound = false;
        }
        Ok(())
    }
}

impl Drop for CurrentGlContext {
    fn drop(&mut self) {
        if let Err(error) = self.unbind() {
            eprintln!("Native file dialog fallback context unbind failed: {error}");
        }
    }
}

fn initialization_failure(
    cause: impl std::fmt::Display,
    context: &mut Context,
    platform: Option<&mut WinitPlatform>,
    renderer: Option<&mut GlowRenderer>,
    gl_context: &mut CurrentGlContext,
) -> Box<dyn std::error::Error> {
    context.end_frame();
    let mut errors = vec![format!("Native file dialog initialization failed: {cause}")];
    let mut attachments_shutdown = true;

    if let Some(renderer) = renderer
        && let Err(error) = renderer.shutdown(context)
    {
        errors.push(format!("Glow renderer rollback failed: {error}"));
        attachments_shutdown = false;
    }
    if let Some(platform) = platform
        && let Err(error) = platform.shutdown(context)
    {
        errors.push(format!("Winit platform rollback failed: {error}"));
        attachments_shutdown = false;
    }
    if attachments_shutdown && let Err(error) = gl_context.unbind() {
        errors.push(format!("OpenGL context rollback failed: {error}"));
    }

    errors.join("; ").into()
}

struct AppWindow {
    imgui: ImguiState,
    status: String,
    busy: bool,
    gl_context: CurrentGlContext,
    surface: Surface<WindowSurface>,
    window: Arc<Window>,
}

struct App {
    window: Option<AppWindow>,
    event_proxy: EventLoopProxy<UserEvent>,
}

impl App {
    fn new(event_proxy: EventLoopProxy<UserEvent>) -> Self {
        Self {
            window: None,
            event_proxy,
        }
    }
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
        let mut gl_context = CurrentGlContext::new(context.make_current(&surface)?);

        let mut imgui_context = Context::create();
        if let Err(error) = imgui_context.set_ini_filename(None::<String>) {
            return Err(initialization_failure(
                error,
                &mut imgui_context,
                None,
                None,
                &mut gl_context,
            ));
        }
        let mut platform = match WinitPlatform::new(&mut imgui_context) {
            Ok(platform) => platform,
            Err(error) => {
                return Err(initialization_failure(
                    error,
                    &mut imgui_context,
                    None,
                    None,
                    &mut gl_context,
                ));
            }
        };
        if let Err(error) = platform.attach_window(
            Arc::clone(&window),
            dear_imgui_winit::HiDpiMode::Default,
            &mut imgui_context,
        ) {
            return Err(initialization_failure(
                error,
                &mut imgui_context,
                Some(&mut platform),
                None,
                &mut gl_context,
            ));
        }

        let gl = unsafe {
            glow::Context::from_loader_function_cstr(|s| {
                gl_context.get().display().get_proc_address(s).cast()
            })
        };
        let mut renderer = match GlowRenderer::new(gl, &mut imgui_context) {
            Ok(renderer) => renderer,
            Err(error) => {
                return Err(initialization_failure(
                    error,
                    &mut imgui_context,
                    Some(&mut platform),
                    None,
                    &mut gl_context,
                ));
            }
        };
        if let Err(error) = renderer.set_framebuffer_srgb_enabled(false) {
            return Err(initialization_failure(
                error,
                &mut imgui_context,
                Some(&mut platform),
                Some(&mut renderer),
                &mut gl_context,
            ));
        }

        Ok(Self {
            imgui: ImguiState {
                renderer,
                platform,
                last_frame: Instant::now(),
                renderer_shutdown_complete: false,
                platform_shutdown_complete: false,
                context: imgui_context,
            },
            status: String::new(),
            busy: false,
            gl_context,
            surface,
            window,
        })
    }

    fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.imgui.context.end_frame();
        let mut errors = Vec::new();

        if !self.imgui.renderer_shutdown_complete {
            match self.imgui.renderer.shutdown(&mut self.imgui.context) {
                Ok(()) => self.imgui.renderer_shutdown_complete = true,
                Err(error) => errors.push(format!("Glow renderer shutdown failed: {error}")),
            }
        }
        if !self.imgui.platform_shutdown_complete {
            match self.imgui.platform.shutdown(&mut self.imgui.context) {
                Ok(()) => self.imgui.platform_shutdown_complete = true,
                Err(error) => errors.push(format!("Winit platform shutdown failed: {error}")),
            }
        }
        if self.imgui.renderer_shutdown_complete
            && self.imgui.platform_shutdown_complete
            && let Err(error) = self.gl_context.unbind()
        {
            errors.push(format!("OpenGL context unbind failed: {error}"));
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(std::io::Error::other(errors.join("; ")).into())
        }
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.surface.resize(
                self.gl_context.get(),
                NonZeroU32::new(new_size.width).unwrap(),
                NonZeroU32::new(new_size.height).unwrap(),
            );
        }
    }

    fn render(
        &mut self,
        event_proxy: &EventLoopProxy<UserEvent>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let now = Instant::now();
        let dt = now - self.imgui.last_frame;
        self.imgui.last_frame = now;
        self.imgui.context.io_mut().set_delta_time(dt.as_secs_f32());

        self.imgui
            .platform
            .prepare_frame(&mut self.imgui.context, &self.window)?;
        let ui = self.imgui.context.frame();

        let mut requested_dialog = None;

        ui.window("File Dialog (Native)")
            .size([700.0, 520.0], Condition::FirstUseEver)
            .build(|| {
                let can = !self.busy;
                if can && ui.button("Open File") {
                    requested_dialog = Some(DialogMode::OpenFile);
                }
                ui.same_line();
                if can && ui.button("Open Files") {
                    requested_dialog = Some(DialogMode::OpenFiles);
                }
                ui.same_line();
                if can && ui.button("Pick Folder") {
                    requested_dialog = Some(DialogMode::PickFolder);
                }
                ui.same_line();
                if can && ui.button("Save File") {
                    requested_dialog = Some(DialogMode::SaveFile);
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
        self.imgui.platform.prepare_render(ui, &self.window)?;
        let pending_frame = self
            .imgui
            .context
            .render(self.imgui.renderer.renderer_consumer()?);
        self.imgui.renderer.render(pending_frame)?;
        self.surface.swap_buffers(self.gl_context.get())?;

        if let Some(mode) = requested_dialog {
            self.spawn(mode, event_proxy.clone());
            self.window.request_redraw();
        }
        Ok(())
    }

    fn spawn(&mut self, mode: DialogMode, event_proxy: EventLoopProxy<UserEvent>) {
        self.busy = true;
        thread::spawn(move || {
            let status = match FileDialog::new(mode).backend(Backend::Auto).open_blocking() {
                Ok(selection) => {
                    let paths = selection.into_paths();
                    let mut status = format!("OK ({} path(s))\n", paths.len());
                    for path in paths {
                        let _ = writeln!(status, "  - {}", path.display());
                    }
                    status
                }
                Err(error) => format!("ERR: {error}"),
            };
            let _ = event_proxy.send_event(UserEvent::DialogFinished(status));
        });
    }

    fn finish_dialog(&mut self, status: String) {
        self.status = status;
        self.busy = false;
    }
}

impl Drop for AppWindow {
    fn drop(&mut self) {
        if let Err(error) = self.shutdown() {
            eprintln!("Native file dialog fallback shutdown failed: {error}");
        }
    }
}

impl ApplicationHandler<UserEvent> for App {
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
        // Feed to ImGui platform first (window-local path)
        if let Err(error) =
            w.imgui
                .platform
                .handle_window_event(&mut w.imgui.context, &w.window, &event)
        {
            eprintln!("Winit platform error: {error}");
            event_loop.exit();
            return;
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                w.resize(size);
                w.window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                if let Err(e) = w.render(&self.event_proxy) {
                    eprintln!("render error: {e}");
                    event_loop.exit();
                }
            }
            WindowEvent::Destroyed => {}
            _ => w.window.request_redraw(),
        }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: UserEvent) {
        let Some(window) = &mut self.window else {
            return;
        };
        match event {
            UserEvent::DialogFinished(status) => window.finish_dialog(status),
        }
        window.window.request_redraw();
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = self.window.as_mut()
            && let Err(error) = window.shutdown()
        {
            eprintln!("Native file dialog shutdown failed: {error}");
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::<UserEvent>::with_user_event().build()?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = App::new(event_loop.create_proxy());
    event_loop.run_app(&mut app)?;
    Ok(())
}
