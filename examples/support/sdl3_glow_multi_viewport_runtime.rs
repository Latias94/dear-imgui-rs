//! Shared SDL3 + Glow multi-viewport application runtime.
//!
//! The teaching example and private runtime contract use this module for the actual SDL3, Glow,
//! Dear ImGui, and viewport-rendering lifecycle. Test Engine policy, environment variables, and
//! machine-readable evidence live only in the private CI binary.

use std::cell::RefCell;
use std::error::Error;
use std::fmt;
use std::rc::Rc;
use std::time::Instant;

use dear_imgui_examples::sdl3_callbacks::{
    Sdl3CallbackEventHandoff, configure_main_callback_rate, requests_exit,
};
use dear_imgui_glow::{
    ExternalTextureId, GlTexture, GlowRenderer, create_texture_from_rgba,
    multi_viewport::{GlowPreparedViewportFrame, GlowViewportRuntime},
};
use dear_imgui_rs::{Condition, ConfigFlags, Context, FrameToken, Id, TextureId, Ui};
use dear_imgui_sdl3::{self as imgui_sdl3_backend, Sdl3PlatformBackend};
use glow::HasContext;
use sdl3::video::{GLProfile, SwapInterval, WindowPos};
use sdl3_main::{AppResult, MainThreadData};

/// Policy hooks for the interactive example and the private SDL3/Glow runtime contract.
///
/// This trait is deliberately specific to this one route. It keeps the real renderer transaction
/// and teardown order in one place without turning the examples crate into a generic probe runner.
pub(crate) trait ViewportScenario: Sized {
    type Output;

    fn swap_interval(&self) -> SwapInterval {
        SwapInterval::VSync
    }

    fn configure_context(&mut self, _context: &mut Context) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    fn initialize(
        &mut self,
        _context: &mut Context,
        _renderer: &mut GlowRenderer,
        _gl: &glow::Context,
        _window: &sdl3::video::Window,
        _external_texture: ExternalTextureView,
    ) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    fn before_ui(&mut self, _viewport_count: i32) {}

    fn extend_main_window(&mut self, _ui: &Ui, _viewport_count: &mut i32) {}

    fn after_ui(&mut self, _ui: &Ui, _external_texture: TextureId) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    fn drive_frame<'frame>(
        &mut self,
        frame: FrameToken<'frame>,
        frame_index: u64,
        driver: &mut SdlGlowFrameDriver<'_>,
    ) -> Result<(), Box<dyn Error>>;

    fn after_frame(
        &mut self,
        _context: &Context,
        _gl: &glow::Context,
        _presented: bool,
        _main_report: Option<MainFrameReport>,
    ) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    fn complete(&self) -> bool {
        false
    }

    fn shutdown(&mut self) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    fn take_output(&mut self) -> Option<Self::Output> {
        None
    }

    fn finish_after_teardown(_output: Self::Output) -> Result<(), Box<dyn Error>> {
        Ok(())
    }
}

/// Interactive policy used by the copy-and-run teaching example.
#[derive(Default)]
pub(crate) struct InteractiveScenario;

impl ViewportScenario for InteractiveScenario {
    type Output = ();

    fn drive_frame<'frame>(
        &mut self,
        frame: FrameToken<'frame>,
        _frame_index: u64,
        driver: &mut SdlGlowFrameDriver<'_>,
    ) -> Result<(), Box<dyn Error>> {
        let prepared = driver.prepare_frame(frame)?;
        driver.render_main_frame(prepared)?;
        driver.present_frame()?;
        Ok(())
    }
}

struct TextureUnitZeroGuard<'a> {
    gl: &'a glow::Context,
    active_texture: u32,
    texture: Option<GlTexture>,
}

impl<'a> TextureUnitZeroGuard<'a> {
    fn bind(gl: &'a glow::Context, texture: GlTexture) -> Self {
        unsafe {
            let active_texture =
                u32::try_from(gl.get_parameter_i32(glow::ACTIVE_TEXTURE)).unwrap_or(glow::TEXTURE0);
            gl.active_texture(glow::TEXTURE0);
            let previous = gl.get_parameter_texture(glow::TEXTURE_BINDING_2D);
            gl.bind_texture(glow::TEXTURE_2D, Some(texture));
            Self {
                gl,
                active_texture,
                texture: previous,
            }
        }
    }
}

impl Drop for TextureUnitZeroGuard<'_> {
    fn drop(&mut self) {
        unsafe {
            self.gl.bind_texture(glow::TEXTURE_2D, self.texture);
            self.gl.active_texture(self.active_texture);
        }
    }
}

struct ExternalTexture {
    id: Option<ExternalTextureId>,
    handle: Option<GlTexture>,
}

impl ExternalTexture {
    fn create(gl: &glow::Context, renderer: &mut GlowRenderer) -> Result<Self, Box<dyn Error>> {
        let pixels = [
            255, 255, 255, 255, 255, 0, 255, 255, 0, 255, 255, 255, 0, 0, 0, 255,
        ];
        let handle = create_texture_from_rgba(gl, 2, 2, &pixels)?;
        {
            let _binding = TextureUnitZeroGuard::bind(gl, handle);
            unsafe {
                gl.generate_mipmap(glow::TEXTURE_2D);
                gl.tex_parameter_i32(
                    glow::TEXTURE_2D,
                    glow::TEXTURE_MIN_FILTER,
                    glow::LINEAR_MIPMAP_NEAREST as i32,
                );
                gl.tex_parameter_i32(
                    glow::TEXTURE_2D,
                    glow::TEXTURE_MAG_FILTER,
                    glow::NEAREST as i32,
                );
            }
        }
        let id = match renderer.register_external_texture(handle) {
            Ok(id) => id,
            Err(error) => {
                unsafe { gl.delete_texture(handle) };
                return Err(error.into());
            }
        };
        Ok(Self {
            id: Some(id),
            handle: Some(handle),
        })
    }

    fn view(&self) -> Result<ExternalTextureView, Box<dyn Error>> {
        let id = self.id.ok_or("external texture was already unregistered")?;
        let handle = self
            .handle
            .ok_or("external texture GL object was already deleted")?;
        Ok(ExternalTextureView {
            texture_id: id.texture_id(),
            handle,
        })
    }

    fn unregister_from_renderer(
        &mut self,
        renderer: &mut GlowRenderer,
    ) -> Result<(), Box<dyn Error>> {
        let Some(id) = self.id else {
            return Ok(());
        };
        renderer.unregister_external_texture(id)?;
        self.id = None;
        Ok(())
    }

    fn unregister_from_runtime(
        &mut self,
        renderer: &GlowViewportRuntime,
    ) -> Result<(), Box<dyn Error>> {
        let Some(id) = self.id else {
            return Ok(());
        };
        renderer.unregister_external_texture(id)?;
        self.id = None;
        Ok(())
    }

    fn renderer_released(&mut self) {
        self.id = None;
    }

    fn delete(&mut self, gl: &glow::Context) {
        let Some(handle) = self.handle.take() else {
            return;
        };
        unsafe { gl.delete_texture(handle) };
    }
}

#[derive(Clone, Copy)]
pub(crate) struct ExternalTextureView {
    texture_id: TextureId,
    handle: GlTexture,
}

impl ExternalTextureView {
    pub(crate) fn texture_id(self) -> TextureId {
        self.texture_id
    }

    pub(crate) fn handle(self) -> GlTexture {
        self.handle
    }
}

fn finish_cleanup(errors: Vec<String>) -> Result<(), Box<dyn Error>> {
    if errors.is_empty() {
        Ok(())
    } else {
        Err(std::io::Error::other(errors.join("; ")).into())
    }
}

fn initialization_failure(
    stage: &str,
    error: impl fmt::Display,
    cleanup: Result<(), Box<dyn Error>>,
) -> Box<dyn Error> {
    let message = match cleanup {
        Ok(()) => format!("{stage}: {error}"),
        Err(cleanup_error) => {
            format!("{stage}: {error}; initialization rollback failed: {cleanup_error}")
        }
    };
    std::io::Error::other(message).into()
}

#[derive(Debug)]
pub(crate) struct SdlGlowFrameError {
    source: Box<dyn Error>,
}

impl SdlGlowFrameError {
    pub(crate) fn new(source: impl Error + 'static) -> Self {
        Self {
            source: Box::new(source),
        }
    }

    pub(crate) fn boxed(source: Box<dyn Error>) -> Self {
        Self { source }
    }

    pub(crate) fn message(message: impl Into<String>) -> Self {
        Self::new(std::io::Error::other(message.into()))
    }
}

impl fmt::Display for SdlGlowFrameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.source.fmt(formatter)
    }
}

impl Error for SdlGlowFrameError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.source.as_ref())
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct MainDrawDataTransform {
    display_pos: [f32; 2],
    framebuffer_scale: [f32; 2],
}

impl MainDrawDataTransform {
    pub(crate) fn display_pos(self) -> [f32; 2] {
        self.display_pos
    }

    pub(crate) fn framebuffer_scale(self) -> [f32; 2] {
        self.framebuffer_scale
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct SecondaryViewportFrameEvidence {
    pub(crate) glow_rendered_viewports: Vec<Id>,
}

impl SecondaryViewportFrameEvidence {
    fn from_report(glow: &dear_imgui_glow::multi_viewport::GlowViewportFrameReport) -> Self {
        Self {
            glow_rendered_viewports: glow.rendered_viewports().to_vec(),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct MainFrameReport {
    draw_data_transform: MainDrawDataTransform,
    pub(crate) secondary_viewports: Option<SecondaryViewportFrameEvidence>,
}

impl MainFrameReport {
    pub(crate) fn draw_data_transform(&self) -> MainDrawDataTransform {
        self.draw_data_transform
    }
}

pub(crate) struct SdlGlowFrameDriver<'a> {
    sdl3_backend: &'a Sdl3PlatformBackend,
    renderer: &'a GlowViewportRuntime,
    gl: &'a glow::Context,
    window: &'a sdl3::video::Window,
    gl_context: &'a sdl3::video::GLContext,
    prepared: bool,
    main_target_ready: bool,
    main_ready: bool,
    main_report: Option<MainFrameReport>,
    presented: bool,
}

impl<'a> SdlGlowFrameDriver<'a> {
    fn new(
        sdl3_backend: &'a Sdl3PlatformBackend,
        renderer: &'a GlowViewportRuntime,
        gl: &'a glow::Context,
        window: &'a sdl3::video::Window,
        gl_context: &'a sdl3::video::GLContext,
    ) -> Self {
        Self {
            sdl3_backend,
            renderer,
            gl,
            window,
            gl_context,
            prepared: false,
            main_target_ready: false,
            main_ready: false,
            main_report: None,
            presented: false,
        }
    }

    pub(crate) fn gl(&self) -> &glow::Context {
        self.gl
    }

    pub(crate) fn window(&self) -> &sdl3::video::Window {
        self.window
    }

    pub(crate) fn main_report(&self) -> Option<&MainFrameReport> {
        self.main_report.as_ref()
    }

    fn restore_main_context(&self, operation: &str) -> Result<(), SdlGlowFrameError> {
        self.window
            .gl_make_current(self.gl_context)
            .map_err(|error| {
                SdlGlowFrameError::message(format!(
                    "failed to restore the main OpenGL context before {operation}: {error}"
                ))
            })
    }

    pub(crate) fn prepare_frame<'frame>(
        &mut self,
        frame: FrameToken<'frame>,
    ) -> Result<GlowPreparedViewportFrame<'frame>, SdlGlowFrameError> {
        if self.prepared {
            return Err(SdlGlowFrameError::message(
                "OpenGL frame was prepared more than once",
            ));
        }

        let prepared = self
            .renderer
            .prepare_frame(frame)
            .map_err(SdlGlowFrameError::new)?;
        self.prepared = true;
        Ok(prepared)
    }

    pub(crate) fn prepare_main_target(&mut self) -> Result<(), SdlGlowFrameError> {
        if !self.prepared {
            return Err(SdlGlowFrameError::message(
                "main OpenGL target was prepared before the renderer frame",
            ));
        }
        if self.main_target_ready {
            return Err(SdlGlowFrameError::message(
                "main OpenGL target was prepared more than once",
            ));
        }
        self.restore_main_context("preparing the main render target")?;
        unsafe {
            let (width, height) = self.window.size_in_pixels();
            self.gl.viewport(0, 0, width as i32, height as i32);
            self.gl.clear_color(0.1, 0.12, 0.15, 1.0);
            self.gl.clear(glow::COLOR_BUFFER_BIT);
        }
        self.main_target_ready = true;
        Ok(())
    }

    pub(crate) fn render_main_frame(
        &mut self,
        frame: GlowPreparedViewportFrame<'_>,
    ) -> Result<(), SdlGlowFrameError> {
        if !self.prepared {
            return Err(SdlGlowFrameError::message(
                "main OpenGL frame reached render-main before preparation",
            ));
        }
        if self.main_ready {
            return Err(SdlGlowFrameError::message(
                "main OpenGL frame reached render-main more than once",
            ));
        }
        if !self.main_target_ready {
            self.prepare_main_target()?;
        }

        // OpenGL draws the main viewport first, switches through secondary contexts, restores the
        // main context, drains route faults, and only then presents the main window.
        let rendered = self
            .renderer
            .render_main(
                frame,
                || {
                    self.window
                        .gl_make_current(self.gl_context)
                        .map_err(|error| {
                            SdlGlowFrameError::message(format!(
                                "failed to restore the main OpenGL context: {error}"
                            ))
                        })
                },
                || self.sdl3_backend.drain_faults(),
            )
            .map_err(SdlGlowFrameError::new)?;
        let secondary_viewports = Some(SecondaryViewportFrameEvidence::from_report(
            rendered.secondary_report(),
        ));
        let draw_data = rendered.draw_data();
        let draw_data_transform = MainDrawDataTransform {
            display_pos: draw_data.display_pos(),
            framebuffer_scale: draw_data.framebuffer_scale(),
        };
        drop(rendered);
        self.main_report = Some(MainFrameReport {
            draw_data_transform,
            secondary_viewports,
        });
        self.main_ready = true;
        Ok(())
    }

    pub(crate) fn present_frame(&mut self) -> Result<(), SdlGlowFrameError> {
        if !self.main_ready || self.main_report.is_none() {
            return Err(SdlGlowFrameError::message(
                "main OpenGL window was presented before render-main completed",
            ));
        }
        if self.presented {
            return Err(SdlGlowFrameError::message(
                "main OpenGL window was presented more than once",
            ));
        }
        self.restore_main_context("presenting the main window")?;
        dear_imgui_examples::sdl3_gl::swap_window(self.window).map_err(|error| {
            SdlGlowFrameError::message(format!("failed to present the main OpenGL window: {error}"))
        })?;
        self.presented = true;
        Ok(())
    }

    fn was_presented(&self) -> bool {
        self.presented
    }

    fn take_main_report(&mut self) -> Option<MainFrameReport> {
        self.main_report.take()
    }
}

pub(crate) struct GlowApp<S: ViewportScenario> {
    main: MainThreadData<RefCell<Option<MainData<S>>>>,
    events: Sdl3CallbackEventHandoff,
}

struct MainData<S: ViewportScenario> {
    scenario: S,
    sdl3_backend: Sdl3PlatformBackend,
    imgui: Context,
    renderer: GlowViewportRuntime,
    gl: Rc<glow::Context>,
    gl_context: sdl3::video::GLContext,
    window: sdl3::video::Window,
    external_texture: ExternalTexture,
    _video: sdl3::VideoSubsystem,
    _sdl: sdl3::Sdl,
    last_frame: Instant,
    frame_index: u64,
    scenario_shutdown_complete: bool,
    renderer_shutdown_complete: bool,
    platform_shutdown_complete: bool,
}

impl<S: ViewportScenario> Drop for MainData<S> {
    fn drop(&mut self) {
        if let Err(error) = self.shutdown() {
            eprintln!("SDL3/Glow fallback shutdown failed: {error}");
        }
    }
}

impl<S: ViewportScenario> GlowApp<S> {
    pub(crate) fn new(scenario: S) -> Result<Self, Box<dyn Error>> {
        imgui_sdl3_backend::enable_native_ime_ui();
        configure_main_callback_rate();
        Ok(Self {
            main: MainThreadData::assert_new(RefCell::new(Some(MainData::new(scenario)?))),
            events: Sdl3CallbackEventHandoff::default(),
        })
    }

    fn process_events(&self) -> Result<AppResult, Box<dyn Error>> {
        let mut events = self.events.drain();
        if let Some(error) = events.first_fault() {
            return Err(error.into());
        }
        let mut main = self.main.assert_get().borrow_mut();
        let main = main
            .as_mut()
            .expect("SDL3 Glow state must be active while callbacks run");
        while let Some(event) = events.pop() {
            let _ = main
                .sdl3_backend
                .process_callback_event(&mut main.imgui, &event)?;
            if requests_exit(&event, main.window.id()) {
                return Ok(AppResult::Success);
            }
        }
        Ok(AppResult::Continue)
    }

    fn render(&self) -> Result<bool, Box<dyn Error>> {
        self.main
            .assert_get()
            .borrow_mut()
            .as_mut()
            .expect("SDL3 Glow state must be active while callbacks run")
            .render()
    }

    pub(crate) fn iterate(&self) -> AppResult {
        match self.process_events() {
            Ok(AppResult::Continue) => {}
            Ok(result) => return result,
            Err(error) => {
                eprintln!("SDL3 Glow event processing failed: {error}");
                return AppResult::Failure;
            }
        }
        match self.render() {
            Ok(true) => AppResult::Success,
            Ok(false) => AppResult::Continue,
            Err(error) => {
                eprintln!("SDL3 Glow frame failed: {error}");
                AppResult::Failure
            }
        }
    }

    /// Queue an SDL event for processing on the SDL main thread.
    ///
    /// # Safety
    ///
    /// `raw` must be the event supplied by SDL and remain valid for this call.
    pub(crate) unsafe fn queue_event(&self, raw: &sdl3::sys::events::SDL_Event) {
        // SAFETY: the caller upholds SDL's callback event lifetime contract.
        unsafe { self.events.push_from_callback(raw) };
    }

    pub(crate) fn shutdown(&self) {
        let main = self.main.assert_get().borrow_mut().take();
        let Some(mut main) = main else {
            return;
        };
        let shutdown = main.shutdown();
        drop(main);
        match shutdown {
            Ok(Some(output)) => {
                if let Err(error) = S::finish_after_teardown(output) {
                    eprintln!("failed to finish SDL3/Glow scenario output: {error}");
                }
            }
            Ok(None) => {}
            Err(error) => eprintln!("SDL3/Glow shutdown failed: {error}"),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn rollback_unattached_runtime<S: ViewportScenario>(
    scenario: &mut S,
    scenario_initialization_attempted: bool,
    sdl3_backend: &mut Sdl3PlatformBackend,
    imgui: &mut Context,
    mut renderer: Option<&mut GlowRenderer>,
    gl: &glow::Context,
    window: &sdl3::video::Window,
    gl_context: &sdl3::video::GLContext,
    mut external_texture: Option<&mut ExternalTexture>,
) -> Result<(), Box<dyn Error>> {
    imgui.end_frame();
    let mut errors = Vec::new();

    if scenario_initialization_attempted && let Err(error) = scenario.shutdown() {
        errors.push(format!("scenario rollback failed: {error}"));
    }

    let main_context_current = match window.gl_make_current(gl_context) {
        Ok(()) => true,
        Err(error) => {
            errors.push(format!(
                "failed to restore the main OpenGL context before renderer rollback: {error}"
            ));
            false
        }
    };

    if let (Some(renderer), Some(texture)) =
        (renderer.as_deref_mut(), external_texture.as_deref_mut())
        && let Err(error) = texture.unregister_from_renderer(renderer)
    {
        errors.push(format!(
            "failed to unregister the application texture during rollback: {error}"
        ));
    }

    if let Some(renderer) = renderer.as_deref_mut() {
        if main_context_current {
            match renderer.shutdown(imgui) {
                Ok(()) => {
                    if let Some(texture) = external_texture.as_deref_mut() {
                        texture.renderer_released();
                    }
                }
                Err(error) => errors.push(format!("Glow renderer rollback failed: {error}")),
            }
        } else {
            errors.push(
                "Glow renderer rollback could not run without the main OpenGL context".to_owned(),
            );
        }
    }

    if let Err(error) = sdl3_backend.shutdown(imgui) {
        errors.push(format!("SDL3 platform rollback failed: {error}"));
    }

    let main_context_current = match window.gl_make_current(gl_context) {
        Ok(()) => true,
        Err(error) => {
            errors.push(format!(
                "failed to restore the main OpenGL context before texture rollback: {error}"
            ));
            false
        }
    };
    if let Some(texture) = external_texture.as_deref_mut() {
        if main_context_current {
            texture.delete(gl);
        } else if texture.handle.is_some() {
            errors.push(
                "application texture rollback could not run without the main OpenGL context"
                    .to_owned(),
            );
        }
    }

    finish_cleanup(errors)
}

impl<S: ViewportScenario> MainData<S> {
    fn new(mut scenario: S) -> Result<Self, Box<dyn Error>> {
        let sdl = sdl3::init()?;
        let video = sdl.video()?;

        let gl_attr = video.gl_attr();
        gl_attr.set_context_version(3, 2);
        gl_attr.set_context_profile(GLProfile::Core);
        gl_attr.set_depth_size(0);

        let main_scale = video
            .get_primary_display()?
            .get_content_scale()
            .unwrap_or(1.0);

        let mut window = video
            .window(
                "Dear ImGui + SDL3 + Glow (multi-viewport)",
                (800.0 * main_scale) as u32,
                (600.0 * main_scale) as u32,
            )
            .opengl()
            .resizable()
            .hidden()
            .high_pixel_density()
            .build()
            .map_err(|error| format!("failed to create SDL3 window: {error}"))?;

        let gl_context = window
            .gl_create_context()
            .map_err(|error| format!("SDL_GL_CreateContext failed: {error}"))?;
        window
            .gl_make_current(&gl_context)
            .map_err(|error| format!("SDL_GL_MakeCurrent failed: {error}"))?;
        let _ = video.gl_set_swap_interval(scenario.swap_interval());
        window.set_position(WindowPos::Centered, WindowPos::Centered);
        window.show();

        // SAFETY: the window's OpenGL context is current on this thread.
        let gl = Rc::new(unsafe { create_glow_context(&video) });

        let mut imgui = Context::create();
        scenario.configure_context(&mut imgui)?;
        let window_scale = window.display_scale();
        let window_scale = if window_scale.is_finite() && window_scale > 0.0 {
            window_scale
        } else {
            1.0
        };
        {
            let io = imgui.io_mut();
            let mut flags = io.config_flags();
            flags.insert(ConfigFlags::DOCKING_ENABLE | ConfigFlags::VIEWPORTS_ENABLE);
            io.set_config_flags(flags);
            io.set_config_dpi_scale_fonts(true);
            io.set_config_dpi_scale_viewports(true);
        }
        {
            let style = imgui.style_mut();
            style.scale_all_sizes(window_scale);
            style.set_font_scale_dpi(window_scale);
        }

        // SAFETY: `window` and `gl_context` outlive renderer/platform shutdown and Context teardown.
        let mut sdl3_backend = unsafe {
            Sdl3PlatformBackend::init_platform_for_opengl(&mut imgui, &window, &gl_context)?
        };

        let mut renderer = match GlowRenderer::with_shared_context(Rc::clone(&gl), &mut imgui) {
            Ok(renderer) => renderer,
            Err(error) => {
                let cleanup = rollback_unattached_runtime(
                    &mut scenario,
                    false,
                    &mut sdl3_backend,
                    &mut imgui,
                    None,
                    gl.as_ref(),
                    &window,
                    &gl_context,
                    None,
                );
                return Err(initialization_failure(
                    "Glow renderer initialization failed",
                    error,
                    cleanup,
                ));
            }
        };
        let mut external_texture = match ExternalTexture::create(&gl, &mut renderer) {
            Ok(texture) => texture,
            Err(error) => {
                let cleanup = rollback_unattached_runtime(
                    &mut scenario,
                    false,
                    &mut sdl3_backend,
                    &mut imgui,
                    Some(&mut renderer),
                    gl.as_ref(),
                    &window,
                    &gl_context,
                    None,
                );
                return Err(initialization_failure(
                    "external texture initialization failed",
                    error,
                    cleanup,
                ));
            }
        };
        let external_texture_view = match external_texture.view() {
            Ok(view) => view,
            Err(error) => {
                let cleanup = rollback_unattached_runtime(
                    &mut scenario,
                    false,
                    &mut sdl3_backend,
                    &mut imgui,
                    Some(&mut renderer),
                    gl.as_ref(),
                    &window,
                    &gl_context,
                    Some(&mut external_texture),
                );
                return Err(initialization_failure(
                    "external texture publication failed",
                    error,
                    cleanup,
                ));
            }
        };
        if let Err(error) = scenario.initialize(
            &mut imgui,
            &mut renderer,
            &gl,
            &window,
            external_texture_view,
        ) {
            let cleanup = rollback_unattached_runtime(
                &mut scenario,
                true,
                &mut sdl3_backend,
                &mut imgui,
                Some(&mut renderer),
                gl.as_ref(),
                &window,
                &gl_context,
                Some(&mut external_texture),
            );
            return Err(initialization_failure(
                "scenario initialization failed",
                error,
                cleanup,
            ));
        }
        // SAFETY: SDL3 creates every secondary OpenGL context in the main context's share group
        // and makes the matching context current for renderer callbacks. The prepared transaction
        // restores `gl_context` before it returns.
        let renderer = match unsafe { GlowViewportRuntime::attach(&mut imgui, renderer) } {
            Ok(renderer) => renderer,
            Err(failure) => {
                let (attach_error, mut renderer) = failure.into_parts();
                let cleanup = rollback_unattached_runtime(
                    &mut scenario,
                    true,
                    &mut sdl3_backend,
                    &mut imgui,
                    Some(&mut renderer),
                    gl.as_ref(),
                    &window,
                    &gl_context,
                    Some(&mut external_texture),
                );
                return Err(initialization_failure(
                    "Glow multi-viewport attachment failed",
                    attach_error,
                    cleanup,
                ));
            }
        };

        Ok(Self {
            scenario,
            sdl3_backend,
            imgui,
            renderer,
            gl,
            gl_context,
            window,
            external_texture,
            _video: video,
            _sdl: sdl,
            last_frame: Instant::now(),
            frame_index: 0,
            scenario_shutdown_complete: false,
            renderer_shutdown_complete: false,
            platform_shutdown_complete: false,
        })
    }

    fn render(&mut self) -> Result<bool, Box<dyn Error>> {
        let now = Instant::now();
        self.imgui
            .io_mut()
            .set_delta_time((now - self.last_frame).as_secs_f32());
        self.last_frame = now;

        self.sdl3_backend.new_frame(&mut self.imgui)?;
        let mut viewport_count =
            i32::try_from(self.imgui.platform_io().viewports_iter().count()).unwrap_or(i32::MAX);
        self.scenario.before_ui(viewport_count);

        let frame = self.imgui.begin_frame();
        let ui = frame.ui();
        ui.dockspace().build()?;
        let external_texture = self.external_texture.view()?;
        let scenario = &mut self.scenario;
        ui.window("Main")
            .size([420.0, 260.0], Condition::FirstUseEver)
            .build(|| {
                ui.text("SDL3 + Glow + Dear ImGui multi-viewport");
                ui.separator();
                ui.text("Drag this window outside the main viewport to spawn an OS window.");
                scenario.extend_main_window(ui, &mut viewport_count);
                let draw_list = ui.get_window_draw_list();
                draw_list.set_sampler_nearest();
                ui.image(external_texture.texture_id(), [64.0, 64.0]);
                draw_list.set_sampler_linear();
            });
        self.scenario.after_ui(ui, external_texture.texture_id())?;

        self.frame_index = self
            .frame_index
            .checked_add(1)
            .ok_or("SDL3/Glow example frame index exhausted")?;
        let mut driver = SdlGlowFrameDriver::new(
            &self.sdl3_backend,
            &self.renderer,
            self.gl.as_ref(),
            &self.window,
            &self.gl_context,
        );
        self.scenario
            .drive_frame(frame, self.frame_index, &mut driver)?;
        let presented = driver.was_presented();
        let main_report = driver.take_main_report();
        drop(driver);
        self.scenario
            .after_frame(&self.imgui, &self.gl, presented, main_report)?;
        Ok(self.scenario.complete())
    }

    fn shutdown(&mut self) -> Result<Option<S::Output>, Box<dyn Error>> {
        if self.scenario_shutdown_complete
            && self.renderer_shutdown_complete
            && self.platform_shutdown_complete
            && self.external_texture.handle.is_none()
        {
            return Ok(self.scenario.take_output());
        }
        self.imgui.end_frame();
        let mut errors = Vec::new();

        if !self.scenario_shutdown_complete {
            match self.scenario.shutdown() {
                Ok(()) => self.scenario_shutdown_complete = true,
                Err(error) => errors.push(format!("scenario shutdown failed: {error}")),
            }
        }

        let main_context_current = match self.window.gl_make_current(&self.gl_context) {
            Ok(()) => true,
            Err(error) => {
                errors.push(format!(
                    "failed to restore the main OpenGL context before renderer shutdown: {error}"
                ));
                false
            }
        };

        if !self.renderer_shutdown_complete {
            if let Err(error) = self
                .external_texture
                .unregister_from_runtime(&self.renderer)
            {
                errors.push(format!(
                    "failed to unregister the application texture: {error}"
                ));
            }

            if main_context_current {
                match self.renderer.shutdown(&mut self.imgui) {
                    Ok(()) => {
                        self.renderer_shutdown_complete = true;
                        self.external_texture.renderer_released();
                    }
                    Err(error) => errors.push(format!("Glow renderer shutdown failed: {error}")),
                }
            } else {
                errors.push(
                    "Glow renderer shutdown could not run without the main OpenGL context"
                        .to_owned(),
                );
            }
        }

        if !self.platform_shutdown_complete {
            match self.sdl3_backend.shutdown(&mut self.imgui) {
                Ok(()) => self.platform_shutdown_complete = true,
                Err(error) => errors.push(format!("SDL3 platform shutdown failed: {error}")),
            }
        }

        let main_context_current = match self.window.gl_make_current(&self.gl_context) {
            Ok(()) => true,
            Err(error) => {
                errors.push(format!(
                    "failed to restore the main OpenGL context before deleting the application texture: {error}"
                ));
                false
            }
        };
        if main_context_current {
            self.external_texture.delete(&self.gl);
        } else if self.external_texture.handle.is_some() {
            errors.push(
                "application texture deletion could not run without the main OpenGL context"
                    .to_owned(),
            );
        }

        finish_cleanup(errors)?;
        Ok(self.scenario.take_output())
    }
}

/// Create a Glow context from an SDL3 `VideoSubsystem`.
///
/// # Safety
///
/// Call this only after there is a current OpenGL context for the thread.
unsafe fn create_glow_context(video: &sdl3::VideoSubsystem) -> glow::Context {
    use std::ffi::c_void;

    unsafe {
        glow::Context::from_loader_function(|name| {
            video
                .gl_get_proc_address(name)
                .map(|function| function as *const c_void)
                .unwrap_or(std::ptr::null())
        })
    }
}
