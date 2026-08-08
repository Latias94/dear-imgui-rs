//! Glow renderer-owned texture lifecycle.
//!
//! The renderer allocates the OpenGL object and returns a nominal `RendererTextureId`. The handle
//! is used for updates and removal, and is converted to Dear ImGui's `TextureId` only when the UI
//! records an image command.

use std::{num::NonZeroU32, sync::Arc, time::Instant};

use dear_imgui_glow::{GlowRenderer, RendererTextureId};
use dear_imgui_rs::{Condition, Context, TextureFormat, TextureId};
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

const TEXTURE_SIDE: u32 = 128;

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
            eprintln!("Glow texture example fallback context unbind failed: {error}");
        }
    }
}

#[derive(Clone, Copy)]
enum TextureCommand {
    Register,
    Update,
    Unregister,
}

struct TextureDemo {
    texture: Option<RendererTextureId>,
    pending_command: Option<TextureCommand>,
    revision: u32,
}

impl TextureDemo {
    fn new(renderer: &mut GlowRenderer) -> Result<Self, dear_imgui_glow::RenderError> {
        let mut demo = Self {
            texture: None,
            pending_command: None,
            revision: 0,
        };
        demo.register(renderer)?;
        Ok(demo)
    }

    fn pixels(revision: u32) -> Vec<u8> {
        let palettes = [
            ([244, 114, 182], [30, 41, 59]),
            ([96, 165, 250], [49, 46, 129]),
            ([251, 191, 36], [127, 29, 29]),
        ];
        let (light, dark) = palettes[revision as usize % palettes.len()];
        let mut pixels = Vec::with_capacity((TEXTURE_SIDE * TEXTURE_SIDE * 4) as usize);

        for y in 0..TEXTURE_SIDE {
            for x in 0..TEXTURE_SIDE {
                let stripe = ((x + y) / 12) % 2 == 0;
                let color = if stripe { light } else { dark };
                pixels.extend_from_slice(&[color[0], color[1], color[2], 255]);
            }
        }

        pixels
    }

    fn register(
        &mut self,
        renderer: &mut GlowRenderer,
    ) -> Result<(), dear_imgui_glow::RenderError> {
        if self.texture.is_some() {
            return Ok(());
        }

        let revision = self.revision.wrapping_add(1);
        let texture = renderer.register_texture(
            TEXTURE_SIDE,
            TEXTURE_SIDE,
            TextureFormat::RGBA32,
            &Self::pixels(revision),
        )?;
        self.texture = Some(texture);
        self.revision = revision;
        Ok(())
    }

    fn update(&mut self, renderer: &mut GlowRenderer) -> Result<(), dear_imgui_glow::RenderError> {
        let Some(texture) = self.texture else {
            return Ok(());
        };

        let revision = self.revision.wrapping_add(1);
        renderer.update_texture(texture, TEXTURE_SIDE, TEXTURE_SIDE, &Self::pixels(revision))?;
        self.revision = revision;
        Ok(())
    }

    fn unregister(
        &mut self,
        renderer: &mut GlowRenderer,
    ) -> Result<(), dear_imgui_glow::RenderError> {
        let Some(texture) = self.texture.take() else {
            return Ok(());
        };

        if let Err(error) = renderer.unregister_texture(texture) {
            self.texture = Some(texture);
            return Err(error);
        }
        Ok(())
    }

    fn apply_pending(
        &mut self,
        renderer: &mut GlowRenderer,
    ) -> Result<(), dear_imgui_glow::RenderError> {
        match self.pending_command.take() {
            Some(TextureCommand::Register) => self.register(renderer),
            Some(TextureCommand::Update) => self.update(renderer),
            Some(TextureCommand::Unregister) => self.unregister(renderer),
            None => Ok(()),
        }
    }

    fn texture_id(&self) -> Option<TextureId> {
        self.texture.map(TextureId::from)
    }
}

struct ImguiState {
    renderer: GlowRenderer,
    platform: WinitPlatform,
    texture: TextureDemo,
    last_frame: Instant,
    renderer_shutdown_complete: bool,
    platform_shutdown_complete: bool,
    // Context must outlive every attachment, including fallback field drops after a failed shutdown.
    context: Context,
}

fn initialization_failure(
    cause: impl std::fmt::Display,
    context: &mut Context,
    platform: Option<&mut WinitPlatform>,
    renderer: Option<&mut GlowRenderer>,
    gl_context: &mut CurrentGlContext,
) -> Box<dyn std::error::Error> {
    context.end_frame();
    let mut errors = vec![format!(
        "Glow texture example initialization failed: {cause}"
    )];
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
    gl_context: CurrentGlContext,
    surface: Surface<WindowSurface>,
    window: Arc<Window>,
}

#[derive(Default)]
struct App {
    window: Option<AppWindow>,
}

impl AppWindow {
    fn new(event_loop: &ActiveEventLoop) -> Result<Self, Box<dyn std::error::Error>> {
        let attributes = Window::default_attributes()
            .with_title("Dear ImGui - Glow Renderer Texture")
            .with_inner_size(LogicalSize::new(840.0, 620.0));
        let (window, config) = glutin_winit::DisplayBuilder::new()
            .with_window_attributes(Some(attributes))
            .build(event_loop, ConfigTemplateBuilder::new(), |mut configs| {
                configs.next().expect("OpenGL config should be available")
            })?;
        let window = Arc::new(window.expect("DisplayBuilder should create the requested window"));

        let context_attributes =
            ContextAttributesBuilder::new().build(Some(window.window_handle()?.as_raw()));
        let not_current = unsafe {
            config
                .display()
                .create_context(&config, &context_attributes)?
        };
        let size = window.inner_size();
        let surface_attributes = SurfaceAttributesBuilder::<WindowSurface>::new()
            .with_srgb(Some(true))
            .build(
                window.window_handle()?.as_raw(),
                NonZeroU32::new(size.width.max(1)).expect("clamped width is non-zero"),
                NonZeroU32::new(size.height.max(1)).expect("clamped height is non-zero"),
            );
        let surface = unsafe {
            config
                .display()
                .create_window_surface(&config, &surface_attributes)?
        };
        let mut gl_context = CurrentGlContext::new(not_current.make_current(&surface)?);

        let mut context = Context::create();
        if let Err(error) = context.set_ini_filename(None::<String>) {
            return Err(initialization_failure(
                error,
                &mut context,
                None,
                None,
                &mut gl_context,
            ));
        }
        let mut platform = match WinitPlatform::new(&mut context) {
            Ok(platform) => platform,
            Err(error) => {
                return Err(initialization_failure(
                    error,
                    &mut context,
                    None,
                    None,
                    &mut gl_context,
                ));
            }
        };
        if let Err(error) = platform.attach_window(
            Arc::clone(&window),
            dear_imgui_winit::HiDpiMode::Default,
            &mut context,
        ) {
            return Err(initialization_failure(
                error,
                &mut context,
                Some(&mut platform),
                None,
                &mut gl_context,
            ));
        }

        let gl = unsafe {
            glow::Context::from_loader_function_cstr(|symbol| {
                gl_context.get().display().get_proc_address(symbol).cast()
            })
        };
        let mut renderer = match GlowRenderer::new(gl, &mut context) {
            Ok(renderer) => renderer,
            Err(error) => {
                return Err(initialization_failure(
                    error,
                    &mut context,
                    Some(&mut platform),
                    None,
                    &mut gl_context,
                ));
            }
        };
        if let Err(error) = renderer.set_framebuffer_srgb_enabled(true) {
            return Err(initialization_failure(
                error,
                &mut context,
                Some(&mut platform),
                Some(&mut renderer),
                &mut gl_context,
            ));
        }
        let texture = match TextureDemo::new(&mut renderer) {
            Ok(texture) => texture,
            Err(error) => {
                return Err(initialization_failure(
                    error,
                    &mut context,
                    Some(&mut platform),
                    Some(&mut renderer),
                    &mut gl_context,
                ));
            }
        };

        Ok(Self {
            imgui: ImguiState {
                platform,
                renderer,
                texture,
                last_frame: Instant::now(),
                renderer_shutdown_complete: false,
                platform_shutdown_complete: false,
                context,
            },
            gl_context,
            surface,
            window,
        })
    }

    fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.imgui.context.end_frame();
        let mut errors = Vec::new();

        if let Err(error) = self.imgui.texture.unregister(&mut self.imgui.renderer) {
            errors.push(format!("Glow renderer texture release failed: {error}"));
        }
        if !self.imgui.renderer_shutdown_complete {
            match self.imgui.renderer.shutdown(&mut self.imgui.context) {
                Ok(()) => {
                    self.imgui.renderer_shutdown_complete = true;
                    self.imgui.texture.texture = None;
                }
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
            Err(errors.join("; ").into())
        }
    }

    fn resize(&mut self, size: winit::dpi::PhysicalSize<u32>) {
        if size.width == 0 || size.height == 0 {
            return;
        }
        self.surface.resize(
            self.gl_context.get(),
            NonZeroU32::new(size.width).expect("checked width is non-zero"),
            NonZeroU32::new(size.height).expect("checked height is non-zero"),
        );
    }

    fn render(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let ImguiState {
            context,
            platform,
            renderer,
            texture,
            last_frame,
            ..
        } = &mut self.imgui;

        texture.apply_pending(renderer)?;
        let now = Instant::now();
        context
            .io_mut()
            .set_delta_time(now.duration_since(*last_frame).as_secs_f32());
        *last_frame = now;
        platform.prepare_frame(context, &self.window)?;

        // RendererTextureId remains the ownership handle; TextureId is only for Dear ImGui draw
        // commands. `From` is intentionally one-way.
        let texture_id = texture.texture_id();
        let revision = texture.revision;
        let mut requested_command = None;
        let ui = context.frame();
        ui.window("Glow Renderer Texture")
            .size([520.0, 500.0], Condition::FirstUseEver)
            .build(|| {
                ui.text("Renderer-owned OpenGL texture");
                ui.separator();
                ui.text(format!("Texture revision: {revision}"));

                if let Some(texture_id) = texture_id {
                    ui.image(texture_id, [256.0, 256.0]);
                    if ui.button("Update pixels") {
                        requested_command = Some(TextureCommand::Update);
                    }
                    ui.same_line();
                    if ui.button("Unregister and delete") {
                        requested_command = Some(TextureCommand::Unregister);
                    }
                } else if ui.button("Register renderer texture") {
                    requested_command = Some(TextureCommand::Register);
                }

                ui.separator();
                ui.text_wrapped(
                    "register_texture allocates the GL object and returns RendererTextureId. \
                     update_texture and unregister_texture require that nominal handle, preventing \
                     accidental operations on managed or application-owned textures.",
                );
                ui.text_wrapped(
                    "Commands run at the start of the next frame so removal never races the draw \
                     data recorded by this frame.",
                );
            });
        texture.pending_command = requested_command;

        platform.prepare_render(&ui, &self.window)?;
        let gl = renderer
            .gl_context()
            .expect("GlowRenderer should retain its OpenGL function table");
        unsafe {
            gl.enable(glow::FRAMEBUFFER_SRGB);
            gl.clear_color(0.07, 0.09, 0.13, 1.0);
            gl.clear(glow::COLOR_BUFFER_BIT);
            gl.disable(glow::FRAMEBUFFER_SRGB);
        }
        let pending_frame = context.render(renderer.renderer_consumer()?);
        renderer.render(pending_frame)?;
        self.surface.swap_buffers(self.gl_context.get())?;
        Ok(())
    }
}

impl Drop for AppWindow {
    fn drop(&mut self) {
        if let Err(error) = self.shutdown() {
            eprintln!("Glow texture example fallback shutdown failed: {error}");
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        match AppWindow::new(event_loop) {
            Ok(window) => {
                window.window.request_redraw();
                self.window = Some(window);
            }
            Err(error) => {
                eprintln!("failed to initialize Glow example: {error}");
                event_loop.exit();
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

        if let Err(error) = window.imgui.platform.handle_window_event(
            &mut window.imgui.context,
            &window.window,
            &event,
        ) {
            eprintln!("Winit platform event error: {error}");
            event_loop.exit();
            return;
        }

        match event {
            WindowEvent::Resized(size) => {
                window.resize(size);
                window.window.request_redraw();
            }
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput { event, .. }
                if event.logical_key == Key::Named(NamedKey::Escape) =>
            {
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                if let Err(error) = window.render() {
                    eprintln!("render failed: {error}");
                    event_loop.exit();
                }
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
            eprintln!("Glow texture example shutdown failed: {error}");
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    event_loop.run_app(&mut App::default())?;
    Ok(())
}
