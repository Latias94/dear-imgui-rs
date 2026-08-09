#![cfg(any(target_os = "linux", target_os = "windows"))]

//! Native regression for `GlowRenderer::with_external_context` and `render_with_context`.
//!
//! This exercises Dear ImGui managed texture create/update/destroy requests through a
//! `PendingFrame` while the OpenGL context is owned by the application. It is ignored by
//! default because it requires a real window system and OpenGL driver; the native runtime job
//! runs it explicitly.

use std::{num::NonZeroU32, sync::Arc, time::Instant};

use dear_imgui_glow::GlowRenderer;
use dear_imgui_rs::{
    Condition, Context as ImguiContext, ManagedTextureError, ManagedTextureId, TextureId,
    texture::{OwnedTextureData, TextureStatus},
};
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

const MAX_REGRESSION_FRAMES: u32 = 120;
const TEXTURE_WIDTH: u32 = 128;
const TEXTURE_HEIGHT: u32 = 128;

fn boxed_error(message: impl Into<String>) -> Box<dyn std::error::Error> {
    Box::new(std::io::Error::other(message.into()))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RegressionStage {
    AwaitCreate,
    SubmitUpdate,
    AwaitUpdate,
    SubmitDestroy,
    AwaitDestroy,
    Verified,
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
            eprintln!("external-context regression GL unbind failed: {error}");
        }
    }
}

impl RegressionStage {
    fn label(self) -> &'static str {
        match self {
            Self::AwaitCreate => "await_create",
            Self::SubmitUpdate => "submit_update",
            Self::AwaitUpdate => "await_update",
            Self::SubmitDestroy => "submit_destroy",
            Self::AwaitDestroy => "await_destroy",
            Self::Verified => "verified",
        }
    }
}

struct ImguiState {
    managed_texture: ManagedTextureId,
    renderer: GlowRenderer,
    platform: WinitPlatform,
    live_texture_id: Option<TextureId>,
    stage: RegressionStage,
    last_frame: Instant,
    clear_color: [f32; 4],
    frame_count: u32,
    renderer_shutdown_complete: bool,
    platform_shutdown_complete: bool,
    context: ImguiContext,
}

struct AppWindow {
    imgui: ImguiState,
    gl: glow::Context,
    context: CurrentGlContext,
    surface: Surface<WindowSurface>,
    window: Arc<Window>,
}

#[derive(Default)]
struct App {
    window: Option<AppWindow>,
    success_message: Option<String>,
    failure_message: Option<String>,
}

fn initialization_failure(
    cause: impl std::fmt::Display,
    context: &mut ImguiContext,
    platform: Option<&mut WinitPlatform>,
    renderer: Option<(&mut GlowRenderer, &glow::Context)>,
    gl_context: &mut CurrentGlContext,
) -> Box<dyn std::error::Error> {
    context.end_frame();
    let mut errors = vec![format!(
        "external-context regression initialization failed: {cause}"
    )];
    let mut attachments_shutdown = true;

    if let Some((renderer, gl)) = renderer
        && let Err(error) = renderer.shutdown_with_context(gl, context)
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

    boxed_error(errors.join("; "))
}

impl AppWindow {
    fn new(event_loop: &ActiveEventLoop) -> Result<Self, Box<dyn std::error::Error>> {
        let window_attributes = Window::default_attributes()
            .with_title("Glow External Context Regression")
            .with_inner_size(LogicalSize::new(1280.0, 720.0));

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
            .with_srgb(Some(true))
            .build(
                window.window_handle()?.as_raw(),
                NonZeroU32::new(1280).unwrap(),
                NonZeroU32::new(720).unwrap(),
            );
        let surface = unsafe {
            cfg.display()
                .create_window_surface(&cfg, &surface_attribs)?
        };
        let mut current_context = CurrentGlContext::new(context.make_current(&surface)?);

        let mut imgui_context = ImguiContext::create();
        if let Err(error) = imgui_context.set_ini_filename(None::<String>) {
            return Err(initialization_failure(
                error,
                &mut imgui_context,
                None,
                None,
                &mut current_context,
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
                    &mut current_context,
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
                &mut current_context,
            ));
        }

        let gl = unsafe {
            glow::Context::from_loader_function_cstr(|s| {
                current_context.get().display().get_proc_address(s).cast()
            })
        };

        let mut renderer = match GlowRenderer::with_external_context(&gl, &mut imgui_context) {
            Ok(renderer) => renderer,
            Err(error) => {
                return Err(initialization_failure(
                    error,
                    &mut imgui_context,
                    Some(&mut platform),
                    None,
                    &mut current_context,
                ));
            }
        };
        if let Err(error) = renderer.set_framebuffer_srgb_enabled(true) {
            return Err(initialization_failure(
                error,
                &mut imgui_context,
                Some(&mut platform),
                Some((&mut renderer, &gl)),
                &mut current_context,
            ));
        }

        let managed_texture = match OwnedTextureData::from_pixels(
            dear_imgui_rs::texture::TextureFormat::RGBA32,
            TEXTURE_WIDTH,
            TEXTURE_HEIGHT,
            &texture_pixels(0),
        ) {
            Ok(texture) => texture,
            Err(error) => {
                return Err(initialization_failure(
                    error,
                    &mut imgui_context,
                    Some(&mut platform),
                    Some((&mut renderer, &gl)),
                    &mut current_context,
                ));
            }
        };

        let managed_texture = imgui_context.register_texture(managed_texture);

        let imgui = ImguiState {
            managed_texture,
            renderer,
            platform,
            live_texture_id: None,
            stage: RegressionStage::AwaitCreate,
            last_frame: Instant::now(),
            clear_color: [0.08, 0.12, 0.16, 1.0],
            frame_count: 0,
            renderer_shutdown_complete: false,
            platform_shutdown_complete: false,
            context: imgui_context,
        };

        Ok(Self {
            imgui,
            gl,
            context: current_context,
            surface,
            window,
        })
    }

    fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.imgui.context.end_frame();
        let mut errors = Vec::new();

        if !self.imgui.renderer_shutdown_complete {
            match self
                .imgui
                .renderer
                .shutdown_with_context(&self.gl, &mut self.imgui.context)
            {
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
            && let Err(error) = self.context.unbind()
        {
            errors.push(format!("OpenGL context unbind failed: {error}"));
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(boxed_error(errors.join("; ")))
        }
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.surface.resize(
                self.context.get(),
                NonZeroU32::new(new_size.width).unwrap(),
                NonZeroU32::new(new_size.height).unwrap(),
            );
        }
    }

    fn prepare_regression_step(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        match self.imgui.stage {
            RegressionStage::SubmitUpdate => {
                let phase = self.imgui.frame_count;
                self.imgui
                    .context
                    .try_with_texture_mut(self.imgui.managed_texture, |mut texture| {
                        texture.replace_pixels(&texture_pixels(phase))
                    })?;
                self.imgui.stage = RegressionStage::AwaitUpdate;
                println!("Submitted managed texture update request");
            }
            RegressionStage::SubmitDestroy => {
                self.imgui
                    .context
                    .remove_texture(self.imgui.managed_texture)
                    .expect("managed texture should begin retirement once");
                self.imgui.stage = RegressionStage::AwaitDestroy;
                println!("Submitted managed texture destroy request");
            }
            _ => {}
        }
        Ok(())
    }

    fn verify_regression_step(&mut self) -> Result<Option<String>, Box<dyn std::error::Error>> {
        match self.imgui.stage {
            RegressionStage::AwaitCreate => {
                let (status, tex_id) = self
                    .imgui
                    .context
                    .with_texture(self.imgui.managed_texture, |texture| {
                        (texture.status(), texture.texture_id())
                    })
                    .map_err(|error| boxed_error(error.to_string()))?;
                if status == TextureStatus::OK {
                    if tex_id.is_null() {
                        return Err(boxed_error(
                            "managed texture create completed without assigning a TextureId",
                        ));
                    }
                    self.imgui.live_texture_id = Some(tex_id);
                    self.imgui.stage = RegressionStage::SubmitUpdate;
                    println!("Verified managed texture create request");
                }
            }
            RegressionStage::AwaitUpdate => {
                let (status, current_id) = self
                    .imgui
                    .context
                    .with_texture(self.imgui.managed_texture, |texture| {
                        (texture.status(), texture.texture_id())
                    })
                    .map_err(|error| boxed_error(error.to_string()))?;
                if status == TextureStatus::OK {
                    let tex_id = self.imgui.live_texture_id.ok_or_else(|| {
                        boxed_error("missing TextureId after create verification")
                    })?;
                    if current_id != tex_id {
                        return Err(boxed_error(
                            "managed texture update unexpectedly changed the TextureId",
                        ));
                    }
                    self.imgui.stage = RegressionStage::SubmitDestroy;
                    println!("Verified managed texture update request");
                }
            }
            RegressionStage::AwaitDestroy => {
                match self
                    .imgui
                    .context
                    .with_texture(self.imgui.managed_texture, |_| ())
                {
                    Err(ManagedTextureError::Retiring(_)) => {}
                    Err(ManagedTextureError::AlreadyRemoved(_)) => {
                        self.imgui.live_texture_id.ok_or_else(|| {
                            boxed_error("missing TextureId before destroy verification")
                        })?;
                        self.imgui.stage = RegressionStage::Verified;
                        let message = format!(
                            "Regression passed: external-context Glow renderer handled managed texture create/update/destroy in {} frames.",
                            self.imgui.frame_count
                        );
                        println!("{message}");
                        return Ok(Some(message));
                    }
                    Ok(()) => {}
                    Err(error) => return Err(boxed_error(error.to_string())),
                }
            }
            RegressionStage::Verified => {
                return Ok(Some(
                    "Regression already verified; exiting cleanly.".to_string(),
                ));
            }
            RegressionStage::SubmitUpdate | RegressionStage::SubmitDestroy => {}
        }

        Ok(None)
    }

    fn render(&mut self) -> Result<Option<String>, Box<dyn std::error::Error>> {
        let now = Instant::now();
        let delta_time = now - self.imgui.last_frame;
        self.imgui
            .context
            .io_mut()
            .set_delta_time(delta_time.as_secs_f32());
        self.imgui.last_frame = now;

        self.prepare_regression_step()?;

        let (texture_status, texture_id) = self
            .imgui
            .context
            .with_texture(self.imgui.managed_texture, |texture| {
                (texture.status(), texture.texture_id())
            })
            .unwrap_or_else(|error| match error {
                ManagedTextureError::Retiring(_) => (
                    TextureStatus::WantDestroy,
                    self.imgui.live_texture_id.unwrap_or(TextureId::null()),
                ),
                ManagedTextureError::AlreadyRemoved(_) => {
                    (TextureStatus::Destroyed, TextureId::null())
                }
                _ => panic!("managed texture inspection failed: {error}"),
            });

        self.imgui
            .platform
            .prepare_frame(&mut self.imgui.context, &self.window)?;
        let ui = self.imgui.context.frame();

        let stage = self.imgui.stage;
        let show_texture = !matches!(
            stage,
            RegressionStage::SubmitDestroy
                | RegressionStage::AwaitDestroy
                | RegressionStage::Verified
        ) && texture_status != TextureStatus::Destroyed;

        ui.window("Glow External Context Regression")
            .size([520.0, 420.0], Condition::FirstUseEver)
            .build(|| {
                ui.text("Reproduces issue #22 with with_external_context + render_with_context");
                ui.separator();
                ui.text(format!("Stage: {}", stage.label()));
                ui.text(format!("Texture status: {:?}", texture_status));
                ui.text(format!("TextureId: {}", texture_id.id()));
                ui.text(format!(
                    "Frame {}/{}",
                    self.imgui.frame_count + 1,
                    MAX_REGRESSION_FRAMES
                ));
                ui.separator();
                ui.text("Expected sequence:");
                ui.bullet_text("WantCreate -> OK");
                ui.bullet_text("WantUpdates -> OK");
                ui.bullet_text("WantDestroy -> Destroyed");

                if show_texture {
                    ui.separator();
                    ui.text("Managed texture preview:");
                    ui.image(self.imgui.managed_texture, [256.0, 256.0]);
                }
            });

        unsafe {
            self.gl.enable(glow::FRAMEBUFFER_SRGB);
            self.gl.clear_color(
                self.imgui.clear_color[0],
                self.imgui.clear_color[1],
                self.imgui.clear_color[2],
                self.imgui.clear_color[3],
            );
            self.gl.clear(glow::COLOR_BUFFER_BIT);
            self.gl.disable(glow::FRAMEBUFFER_SRGB);
        }

        self.imgui.platform.prepare_render(&ui, &self.window)?;
        let pending_frame = self
            .imgui
            .context
            .render(self.imgui.renderer.renderer_consumer()?);

        // The external GL capability is supplied at the render boundary. If device objects were
        // invalidated, the renderer recreates them transactionally before texture reconciliation.
        self.imgui
            .renderer
            .render_with_context(&self.gl, pending_frame)?;

        self.surface.swap_buffers(self.context.get())?;

        self.imgui.frame_count = self.imgui.frame_count.saturating_add(1);
        if self.imgui.frame_count > MAX_REGRESSION_FRAMES {
            return Err(boxed_error(format!(
                "regression scenario did not finish within {MAX_REGRESSION_FRAMES} frames"
            )));
        }

        self.verify_regression_step()
    }
}

impl Drop for AppWindow {
    fn drop(&mut self) {
        if let Err(error) = self.shutdown() {
            eprintln!("external-context regression fallback shutdown failed: {error}");
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            match AppWindow::new(event_loop) {
                Ok(window) => {
                    window.window.request_redraw();
                    self.window = Some(window);
                }
                Err(err) => {
                    self.failure_message = Some(err.to_string());
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

        if let Err(error) = window.imgui.platform.handle_window_event(
            &mut window.imgui.context,
            &window.window,
            &event,
        ) {
            self.failure_message = Some(format!("Winit platform event error: {error}"));
            event_loop.exit();
            return;
        }

        match event {
            WindowEvent::Resized(physical_size) => {
                window.resize(physical_size);
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
            WindowEvent::RedrawRequested => match window.render() {
                Ok(Some(message)) => {
                    self.success_message = Some(message);
                    event_loop.exit();
                }
                Ok(None) => {
                    window.window.request_redraw();
                }
                Err(err) => {
                    self.failure_message = Some(err.to_string());
                    event_loop.exit();
                }
            },
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if self.success_message.is_none() && self.failure_message.is_none() {
            if let Some(window) = &self.window {
                window.window.request_redraw();
            }
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = self.window.as_mut()
            && let Err(error) = window.shutdown()
        {
            self.failure_message = Some(format!("external-context shutdown failed: {error}"));
        }
    }
}

fn texture_pixels(phase: u32) -> Vec<u8> {
    let mut pixels = vec![0u8; (TEXTURE_WIDTH * TEXTURE_HEIGHT * 4) as usize];
    let t = phase as f32 * 0.15;

    for y in 0..TEXTURE_HEIGHT {
        for x in 0..TEXTURE_WIDTH {
            let i = ((y * TEXTURE_WIDTH + x) * 4) as usize;
            let fx = x as f32 / TEXTURE_WIDTH as f32;
            let fy = y as f32 / TEXTURE_HEIGHT as f32;
            pixels[i] = ((fx * 255.0 + t.sin() * 96.0).clamp(0.0, 255.0)) as u8;
            pixels[i + 1] = ((fy * 255.0 + (t * 1.4).cos() * 96.0).clamp(0.0, 255.0)) as u8;
            pixels[i + 2] = (((fx + fy + t * 0.25).sin().abs()) * 255.0) as u8;
            pixels[i + 3] = 255;
        }
    }

    pixels
}

fn build_event_loop() -> Result<EventLoop<()>, winit::error::EventLoopError> {
    let mut builder = EventLoop::builder();

    #[cfg(target_os = "linux")]
    {
        use winit::platform::{wayland::EventLoopBuilderExtWayland, x11::EventLoopBuilderExtX11};

        EventLoopBuilderExtWayland::with_any_thread(&mut builder, true);
        EventLoopBuilderExtX11::with_any_thread(&mut builder, true);
    }

    #[cfg(target_os = "windows")]
    {
        use winit::platform::windows::EventLoopBuilderExtWindows;

        builder.with_any_thread(true);
    }

    builder.build()
}

fn run_regression() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = build_event_loop()?;

    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::default();
    event_loop.run_app(&mut app)?;

    if let Some(message) = app.failure_message.take() {
        return Err(boxed_error(message));
    }

    let message = app.success_message.take().ok_or_else(|| {
        boxed_error("event loop exited before the external-context lifecycle was verified")
    })?;
    println!("{message}");

    Ok(())
}

#[test]
#[ignore = "requires a native OpenGL display; run the named native-runtime regression"]
fn external_context_managed_texture_lifecycle() -> Result<(), Box<dyn std::error::Error>> {
    let _ = env_logger::builder().is_test(true).try_init();
    run_regression()
}
