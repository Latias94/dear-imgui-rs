//! Minimal Docking example (single file).
//! Shows how to enable docking and create a fullscreen DockSpace.

use std::{num::NonZeroU32, sync::Arc, time::Instant};

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
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::{Window, WindowId},
};

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
            eprintln!("Dockspace fallback context unbind failed: {error}");
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
    let mut errors = vec![format!("Dockspace initialization failed: {cause}")];
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
    dock_windows: DockWindows,
    gl_context: CurrentGlContext,
    surface: Surface<WindowSurface>,
    window: Arc<Window>,
}

struct DockWindows {
    james_1: WindowKey,
    james_2: WindowKey,
    james_3: WindowKey,
    james_4: WindowKey,
}

impl DockWindows {
    fn new() -> Result<Self, WindowKeyError> {
        Ok(Self {
            james_1: WindowKey::new("james-1", "James_1")?,
            james_2: WindowKey::new("james-2", "James_2")?,
            james_3: WindowKey::new("james-3", "James_3")?,
            james_4: WindowKey::new("james-4", "James_4")?,
        })
    }

    fn layout(&self) -> DockLayout {
        DockLayout::split(
            DockSplit::Left,
            0.20,
            DockLayout::tabs([&self.james_1]),
            DockLayout::split(
                DockSplit::Right,
                0.20,
                DockLayout::tabs([&self.james_3]),
                DockLayout::split(
                    DockSplit::Down,
                    0.20,
                    DockLayout::tabs([&self.james_4]),
                    DockLayout::tabs([&self.james_2]),
                ),
            ),
        )
    }
}

#[derive(Default)]
struct App {
    window: Option<AppWindow>,
}

impl AppWindow {
    fn new(event_loop: &ActiveEventLoop) -> Result<Self, Box<dyn std::error::Error>> {
        let dock_windows = DockWindows::new()?;

        // Create window with OpenGL context
        let window_attributes = winit::window::Window::default_attributes()
            .with_title("Dear ImGui - Dockspace Minimal")
            .with_inner_size(LogicalSize::new(1200.0, 720.0));

        let (window, cfg) = glutin_winit::DisplayBuilder::new()
            .with_window_attributes(Some(window_attributes))
            .build(event_loop, ConfigTemplateBuilder::new(), |mut configs| {
                configs.next().unwrap()
            })?;

        let window = Arc::new(window.unwrap());

        // OpenGL context
        let context_attribs =
            ContextAttributesBuilder::new().build(Some(window.window_handle()?.as_raw()));
        let context = unsafe { cfg.display().create_context(&cfg, &context_attribs)? };

        // Linear framebuffer for simplicity
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
        let mut gl_context = CurrentGlContext::new(context.make_current(&surface)?);

        // Dear ImGui
        let mut imgui_context = Context::create();
        // Deterministic layout for a minimal sample
        if let Err(error) = imgui_context.set_ini_filename(None::<String>) {
            return Err(initialization_failure(
                error,
                &mut imgui_context,
                None,
                None,
                &mut gl_context,
            ));
        }

        // Enable docking
        let io = imgui_context.io_mut();
        let mut flags = io.config_flags();
        flags.insert(ConfigFlags::DOCKING_ENABLE);
        io.set_config_flags(flags);

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

        // OpenGL renderer
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

        let imgui = ImguiState {
            renderer,
            platform,
            last_frame: Instant::now(),
            renderer_shutdown_complete: false,
            platform_shutdown_complete: false,
            context: imgui_context,
        };

        Ok(Self {
            imgui,
            dock_windows,
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
            .prepare_frame(&mut self.imgui.context, &self.window)?;
        let ui = self.imgui.context.frame();

        // 1) Host a fullscreen window for the DockSpace (mirrors minimal C++ docking example)
        use dear_imgui_rs::{DockLayoutApply, StyleColor, StyleVar, WindowFlags};

        let viewport = ui.main_viewport();
        // Ensure this window is associated with the main viewport (safe wrapper)
        ui.set_next_window_viewport(viewport.id());
        let pos = viewport.pos();
        let size = viewport.size();

        let mut window_flags = WindowFlags::NO_DOCKING;
        window_flags |= WindowFlags::NO_TITLE_BAR
            | WindowFlags::NO_COLLAPSE
            | WindowFlags::NO_RESIZE
            | WindowFlags::NO_MOVE
            | WindowFlags::NO_BRING_TO_FRONT_ON_FOCUS
            | WindowFlags::NO_NAV_FOCUS;

        // Zero rounding/border and remove padding for a clean host window
        let rounding = ui.push_style_var(StyleVar::WindowRounding(0.0));
        let border = ui.push_style_var(StyleVar::WindowBorderSize(0.0));
        let padding = ui.push_style_var(StyleVar::WindowPadding([0.0, 0.0]));
        let dockspace_id = ui.get_id("MyDockspace");
        let dockspace_layout = self.dock_windows.layout();
        let mut dockspace_result = Ok(dockspace_id);

        ui.window("DockSpace Demo")
            .flags(window_flags)
            .position(pos, Condition::Always)
            .size(size, Condition::Always)
            .build(|| {
                // Pop padding/border/rounding to restore defaults
                padding.pop();
                border.pop();
                rounding.pop();

                // Render DockSpace inside the host window
                let color = ui.push_style_color(StyleColor::DockingEmptyBg, [1.0, 0.0, 0.0, 1.0]);
                dockspace_result = ui
                    .dockspace()
                    .root_id(dockspace_id)
                    .current_window(ui.content_region_avail())
                    .layout(&dockspace_layout, DockLayoutApply::IfMissing)
                    .build();
                color.pop();
            });
        dockspace_result?;

        // 2) Create docked windows
        ui.window(&self.dock_windows.james_1)
            .build(|| ui.text("Text 1"));
        ui.window(&self.dock_windows.james_2)
            .build(|| ui.text("Text 2"));
        ui.window(&self.dock_windows.james_3)
            .build(|| ui.text("Text 3"));
        ui.window(&self.dock_windows.james_4)
            .build(|| ui.text("Text 4"));

        // Clear and render
        if let Some(gl) = self.imgui.renderer.gl_context() {
            unsafe {
                gl.clear_color(0.1, 0.2, 0.3, 1.0);
                gl.clear(glow::COLOR_BUFFER_BIT);
            }
        }

        self.imgui.platform.prepare_render(&ui, &self.window)?;
        let pending_frame = self
            .imgui
            .context
            .render(self.imgui.renderer.renderer_consumer()?);

        self.imgui.renderer.render(pending_frame)?;

        self.surface.swap_buffers(self.gl_context.get())?;
        Ok(())
    }
}

impl Drop for AppWindow {
    fn drop(&mut self) {
        if let Err(error) = self.shutdown() {
            eprintln!("Dockspace fallback shutdown failed: {error}");
        }
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

        // Pass to ImGui (window-local path)
        if let Err(error) = window.imgui.platform.handle_window_event(
            &mut window.imgui.context,
            &window.window,
            &event,
        ) {
            eprintln!("Winit platform error: {error}");
            event_loop.exit();
            return;
        }

        match event {
            WindowEvent::Resized(size) => {
                window.resize(size);
                window.window.request_redraw();
            }
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
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

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = self.window.as_mut()
            && let Err(error) = window.shutdown()
        {
            eprintln!("Dockspace shutdown failed: {error}");
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
