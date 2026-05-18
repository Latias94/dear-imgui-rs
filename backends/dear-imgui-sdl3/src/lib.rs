//! SDL3 platform backend bindings for `dear-imgui-rs`.
//!
//! This crate is a thin, opinionated wrapper around the official C++ SDL3
//! platform backend (`imgui_impl_sdl3.cpp`). When the `opengl3-renderer`
//! feature is enabled, the renderer path uses the shared OpenGL3 backend shim
//! exported by `dear-imgui-sys`.
//!
//! The intent is to provide a simple, safe-ish API that:
//! - plugs into an existing `dear-imgui-rs::Context`
//! - integrates with an SDL3 window and OpenGL context
//! - supports Dear ImGui multi-viewport via the official backend behavior.
//!
//! By default, this crate builds the SDL3 platform backend only. Enable
//! `opengl3-renderer` to pair it with the official OpenGL3 renderer shim
//! or `sdlrenderer3-renderer` to pair it with the official SDLRenderer3 shim.

#[cfg(feature = "opengl3-renderer")]
use std::ffi::CString;
use std::ffi::c_void;

use dear_imgui_rs::{Context, ContextAliveToken};
#[cfg(any(feature = "opengl3-renderer", feature = "sdlrenderer3-renderer"))]
use dear_imgui_rs::{TextureData, render::DrawData};
use dear_imgui_sys as sys;
#[cfg(feature = "opengl3-renderer")]
use dear_imgui_sys::backend_shim::opengl3 as opengl3_backend;
#[cfg(feature = "sdlrenderer3-renderer")]
use dear_imgui_sys::backend_shim::sdlrenderer3 as sdlrenderer3_backend;
#[cfg(feature = "sdlrenderer3-renderer")]
use sdl3::render::WindowCanvas;
use sdl3::video::{GLContext, Window};
use sdl3_sys::events::SDL_Event;

#[derive(Clone, Debug)]
struct ContextBinding {
    raw: *mut sys::ImGuiContext,
    alive: ContextAliveToken,
}

impl ContextBinding {
    fn capture(imgui: &Context) -> Self {
        Self {
            raw: imgui.as_raw(),
            alive: imgui.alive_token(),
        }
    }

    fn assert_matches(&self, imgui: &Context, caller: &str) {
        assert!(
            self.alive.is_alive(),
            "{caller} requires the captured Dear ImGui context to still be alive"
        );
        assert_eq!(
            self.raw,
            imgui.as_raw(),
            "{caller} received a different Dear ImGui context than the one used during backend initialization"
        );
    }

    fn bind(&self, caller: &str) -> CurrentContextGuard {
        assert!(
            self.alive.is_alive(),
            "{caller} requires the captured Dear ImGui context to still be alive"
        );
        assert!(
            !self.raw.is_null(),
            "{caller} requires a non-null Dear ImGui context"
        );
        unsafe { CurrentContextGuard::bind(self.raw) }
    }

    fn bind_for_drop(&self) -> Option<CurrentContextGuard> {
        if self.alive.is_alive() && !self.raw.is_null() {
            Some(unsafe { CurrentContextGuard::bind(self.raw) })
        } else {
            None
        }
    }

    #[cfg(any(feature = "opengl3-renderer", feature = "sdlrenderer3-renderer"))]
    fn assert_current_draw_data(&self, draw_data: &mut DrawData, caller: &str) {
        let expected = unsafe { sys::igGetDrawData() as *mut sys::ImDrawData };
        let actual = draw_data as *mut DrawData as *mut sys::ImDrawData;
        assert_eq!(
            expected, actual,
            "{caller} received draw data that does not belong to the captured Dear ImGui context"
        );
    }
}

#[derive(Debug)]
struct CurrentContextGuard {
    previous: *mut sys::ImGuiContext,
    restore: bool,
}

impl CurrentContextGuard {
    unsafe fn bind(raw: *mut sys::ImGuiContext) -> Self {
        let previous = unsafe { sys::igGetCurrentContext() };
        let restore = previous != raw;
        if restore {
            unsafe {
                sys::igSetCurrentContext(raw);
            }
        }
        Self { previous, restore }
    }
}

impl Drop for CurrentContextGuard {
    fn drop(&mut self) {
        if self.restore {
            unsafe {
                sys::igSetCurrentContext(self.previous);
            }
        }
    }
}

fn with_context<R>(imgui: &mut Context, caller: &str, f: impl FnOnce() -> R) -> R {
    let context = ContextBinding::capture(imgui);
    let _guard = context.bind(caller);
    f()
}

/// FFI bindings to the C wrappers defined in `wrapper.cpp`.
mod ffi {
    use super::*;

    unsafe extern "C" {
        pub fn ImGui_ImplSDL3_InitForOpenGL_Rust(
            window: *mut sdl3_sys::video::SDL_Window,
            sdl_gl_context: *mut c_void,
        ) -> bool;
        pub fn ImGui_ImplSDL3_InitForVulkan_Rust(window: *mut sdl3_sys::video::SDL_Window) -> bool;
        pub fn ImGui_ImplSDL3_InitForD3D_Rust(window: *mut sdl3_sys::video::SDL_Window) -> bool;
        pub fn ImGui_ImplSDL3_InitForMetal_Rust(window: *mut sdl3_sys::video::SDL_Window) -> bool;
        pub fn ImGui_ImplSDL3_InitForSDLRenderer_Rust(
            window: *mut sdl3_sys::video::SDL_Window,
            renderer: *mut sdl3_sys::render::SDL_Renderer,
        ) -> bool;
        pub fn ImGui_ImplSDL3_InitForSDLGPU_Rust(window: *mut sdl3_sys::video::SDL_Window) -> bool;
        pub fn ImGui_ImplSDL3_InitForOther_Rust(window: *mut sdl3_sys::video::SDL_Window) -> bool;
        pub fn ImGui_ImplSDL3_Shutdown_Rust();
        pub fn ImGui_ImplSDL3_NewFrame_Rust();
        pub fn ImGui_ImplSDL3_ProcessEvent_Rust(event: *const SDL_Event) -> bool;

        pub fn ImGui_ImplSDL3_SetGamepadMode_AutoFirst_Rust();
        pub fn ImGui_ImplSDL3_SetGamepadMode_AutoAll_Rust();
        pub fn ImGui_ImplSDL3_SetGamepadMode_Manual_Rust(
            manual_gamepads_array: *const *mut sdl3_sys::gamepad::SDL_Gamepad,
            manual_gamepads_count: i32,
        );
    }
}

/// Errors that can occur when setting up the SDL3 + OpenGL backend.
#[derive(Debug, thiserror::Error)]
pub enum Sdl3BackendError {
    #[error("ImGui_ImplSDL3_InitForOpenGL returned false")]
    Sdl3InitFailed,
    #[error("ImGui_ImplOpenGL3_Init returned false")]
    OpenGlInitFailed,
    #[error("Invalid GLSL version string")]
    InvalidGlslVersion,
    #[error("ImGui_ImplSDLRenderer3_Init returned false")]
    Renderer3InitFailed,
}

/// Gamepad handling mode used by the SDL3 backend.
///
/// This controls how many SDL3 gamepads are opened and merged into ImGui's
/// gamepad input state.
#[derive(Copy, Clone, Debug)]
pub enum GamepadMode {
    /// Automatically open the first available gamepad (Dear ImGui default).
    AutoFirst,
    /// Automatically open all available gamepads and merge their state.
    AutoAll,
}

/// RAII owner for the SDL3 platform backend without an official renderer shim.
///
/// Dropping this value shuts down `imgui_impl_sdl3` while the captured Dear ImGui
/// context is still alive. If the context has already been dropped, `Drop` skips
/// the FFI call.
#[must_use = "dropping the backend owner shuts down the SDL3 platform backend"]
#[derive(Debug)]
pub struct Sdl3PlatformBackend {
    context: ContextBinding,
    shutdown_on_drop: bool,
}

impl Sdl3PlatformBackend {
    fn from_initialized_context(imgui: &Context) -> Self {
        Self {
            context: ContextBinding::capture(imgui),
            shutdown_on_drop: true,
        }
    }

    /// Initialize the SDL3 platform backend for non-OpenGL renderers.
    pub fn init_for_other(imgui: &mut Context, window: &Window) -> Result<Self, Sdl3BackendError> {
        init_for_other(imgui, window)?;
        Ok(Self::from_initialized_context(imgui))
    }

    /// Initialize the SDL3 platform backend for an OpenGL context without
    /// initializing the official OpenGL3 renderer.
    pub fn init_platform_for_opengl(
        imgui: &mut Context,
        window: &Window,
        gl_context: &GLContext,
    ) -> Result<Self, Sdl3BackendError> {
        init_platform_for_opengl(imgui, window, gl_context)?;
        Ok(Self::from_initialized_context(imgui))
    }

    /// Initialize the SDL3 platform backend for Vulkan renderers.
    pub fn init_for_vulkan(imgui: &mut Context, window: &Window) -> Result<Self, Sdl3BackendError> {
        init_for_vulkan(imgui, window)?;
        Ok(Self::from_initialized_context(imgui))
    }

    /// Initialize the SDL3 platform backend for Direct3D renderers.
    pub fn init_for_d3d(imgui: &mut Context, window: &Window) -> Result<Self, Sdl3BackendError> {
        init_for_d3d(imgui, window)?;
        Ok(Self::from_initialized_context(imgui))
    }

    /// Initialize the SDL3 platform backend for Metal renderers.
    pub fn init_for_metal(imgui: &mut Context, window: &Window) -> Result<Self, Sdl3BackendError> {
        init_for_metal(imgui, window)?;
        Ok(Self::from_initialized_context(imgui))
    }

    /// Initialize the SDL3 platform backend for SDL_Renderer-based renderers.
    ///
    /// # Safety
    ///
    /// The caller must provide a valid `SDL_Renderer` pointer associated with `window`.
    pub unsafe fn init_for_sdl_renderer(
        imgui: &mut Context,
        window: &Window,
        renderer: *mut sdl3_sys::render::SDL_Renderer,
    ) -> Result<Self, Sdl3BackendError> {
        unsafe {
            init_for_sdl_renderer(imgui, window, renderer)?;
        }
        Ok(Self::from_initialized_context(imgui))
    }

    /// Initialize the SDL3 platform backend for SDL GPU renderers.
    pub fn init_for_sdl_gpu(
        imgui: &mut Context,
        window: &Window,
    ) -> Result<Self, Sdl3BackendError> {
        init_for_sdl_gpu(imgui, window)?;
        Ok(Self::from_initialized_context(imgui))
    }

    /// Begin a new SDL3 platform frame.
    pub fn new_frame(&mut self, imgui: &mut Context) {
        self.context
            .assert_matches(imgui, "Sdl3PlatformBackend::new_frame()");
        let _guard = self.context.bind("Sdl3PlatformBackend::new_frame()");
        sdl3_new_frame_impl();
    }

    /// Process a single low-level SDL3 event with the captured ImGui context.
    pub fn process_event(&mut self, imgui: &mut Context, event: &SDL_Event) -> bool {
        self.context
            .assert_matches(imgui, "Sdl3PlatformBackend::process_event()");
        let _guard = self.context.bind("Sdl3PlatformBackend::process_event()");
        process_sys_event(event)
    }

    /// Configure how the SDL3 backend handles gamepads for the captured context.
    pub fn set_gamepad_mode(&mut self, imgui: &mut Context, mode: GamepadMode) {
        self.context
            .assert_matches(imgui, "Sdl3PlatformBackend::set_gamepad_mode()");
        let _guard = self.context.bind("Sdl3PlatformBackend::set_gamepad_mode()");
        set_gamepad_mode(mode);
    }

    /// Configure SDL3 backend to use manual gamepad selection for the captured context.
    ///
    /// # Safety
    ///
    /// - The caller must ensure every pointer in `gamepads` is a valid, opened `SDL_Gamepad`.
    /// - The caller is responsible for keeping those gamepads alive for the duration of ImGui usage.
    /// - The slice itself is only read during this call; the backend copies the pointers.
    pub unsafe fn set_gamepad_mode_manual(
        &mut self,
        imgui: &mut Context,
        gamepads: &[*mut sdl3_sys::gamepad::SDL_Gamepad],
    ) {
        self.context
            .assert_matches(imgui, "Sdl3PlatformBackend::set_gamepad_mode_manual()");
        let _guard = self
            .context
            .bind("Sdl3PlatformBackend::set_gamepad_mode_manual()");
        unsafe {
            set_gamepad_mode_manual(gamepads);
        }
    }

    /// Shut down the SDL3 platform backend before dropping the owner.
    pub fn shutdown(mut self, imgui: &mut Context) {
        self.context
            .assert_matches(imgui, "Sdl3PlatformBackend::shutdown()");
        {
            let _guard = self.context.bind("Sdl3PlatformBackend::shutdown()");
            shutdown_platform_impl();
        }
        self.shutdown_on_drop = false;
    }
}

impl Drop for Sdl3PlatformBackend {
    fn drop(&mut self) {
        if self.shutdown_on_drop {
            if let Some(_guard) = self.context.bind_for_drop() {
                shutdown_platform_impl();
            }
        }
    }
}

/// RAII owner for SDL3 platform + official OpenGL3 renderer backends.
#[cfg(feature = "opengl3-renderer")]
#[must_use = "dropping the backend owner shuts down the SDL3 + OpenGL3 backends"]
#[derive(Debug)]
pub struct Sdl3OpenGl3Backend {
    context: ContextBinding,
    shutdown_on_drop: bool,
}

#[cfg(feature = "opengl3-renderer")]
impl Sdl3OpenGl3Backend {
    fn from_initialized_context(imgui: &Context) -> Self {
        Self {
            context: ContextBinding::capture(imgui),
            shutdown_on_drop: true,
        }
    }

    /// Initialize the SDL3 platform backend and the official OpenGL3 renderer.
    pub fn init(
        imgui: &mut Context,
        window: &Window,
        gl_context: &GLContext,
        glsl_version: &str,
    ) -> Result<Self, Sdl3BackendError> {
        init_for_opengl(imgui, window, gl_context, glsl_version)?;
        Ok(Self::from_initialized_context(imgui))
    }

    /// Initialize the SDL3 + OpenGL3 backends with the upstream default GLSL version.
    pub fn init_default(
        imgui: &mut Context,
        window: &Window,
        gl_context: &GLContext,
    ) -> Result<Self, Sdl3BackendError> {
        init_for_opengl_default(imgui, window, gl_context)?;
        Ok(Self::from_initialized_context(imgui))
    }

    /// Begin a new SDL3 + OpenGL3 frame.
    pub fn new_frame(&mut self, imgui: &mut Context) {
        self.context
            .assert_matches(imgui, "Sdl3OpenGl3Backend::new_frame()");
        let _guard = self.context.bind("Sdl3OpenGl3Backend::new_frame()");
        new_frame_opengl3_impl();
    }

    /// Process a single low-level SDL3 event with the captured ImGui context.
    pub fn process_event(&mut self, imgui: &mut Context, event: &SDL_Event) -> bool {
        self.context
            .assert_matches(imgui, "Sdl3OpenGl3Backend::process_event()");
        let _guard = self.context.bind("Sdl3OpenGl3Backend::process_event()");
        process_sys_event(event)
    }

    /// Render Dear ImGui draw data using the official OpenGL3 renderer.
    pub fn render(&mut self, draw_data: &mut DrawData) {
        let _guard = self.context.bind("Sdl3OpenGl3Backend::render()");
        self.context
            .assert_current_draw_data(draw_data, "Sdl3OpenGl3Backend::render()");
        render_opengl3_impl(draw_data);
    }

    /// Update a single ImGui texture using the official OpenGL3 renderer.
    pub fn update_texture(&mut self, imgui: &mut Context, tex: &mut TextureData) {
        self.context
            .assert_matches(imgui, "Sdl3OpenGl3Backend::update_texture()");
        let _guard = self.context.bind("Sdl3OpenGl3Backend::update_texture()");
        update_texture(tex);
    }

    /// Configure how the SDL3 backend handles gamepads for the captured context.
    pub fn set_gamepad_mode(&mut self, imgui: &mut Context, mode: GamepadMode) {
        self.context
            .assert_matches(imgui, "Sdl3OpenGl3Backend::set_gamepad_mode()");
        let _guard = self.context.bind("Sdl3OpenGl3Backend::set_gamepad_mode()");
        set_gamepad_mode(mode);
    }

    /// Configure SDL3 backend to use manual gamepad selection for the captured context.
    ///
    /// # Safety
    ///
    /// - The caller must ensure every pointer in `gamepads` is a valid, opened `SDL_Gamepad`.
    /// - The caller is responsible for keeping those gamepads alive for the duration of ImGui usage.
    /// - The slice itself is only read during this call; the backend copies the pointers.
    pub unsafe fn set_gamepad_mode_manual(
        &mut self,
        imgui: &mut Context,
        gamepads: &[*mut sdl3_sys::gamepad::SDL_Gamepad],
    ) {
        self.context
            .assert_matches(imgui, "Sdl3OpenGl3Backend::set_gamepad_mode_manual()");
        let _guard = self
            .context
            .bind("Sdl3OpenGl3Backend::set_gamepad_mode_manual()");
        unsafe {
            set_gamepad_mode_manual(gamepads);
        }
    }

    /// Create OpenGL3 renderer device objects.
    pub fn create_device_objects(&mut self, imgui: &mut Context) -> bool {
        self.context
            .assert_matches(imgui, "Sdl3OpenGl3Backend::create_device_objects()");
        let _guard = self
            .context
            .bind("Sdl3OpenGl3Backend::create_device_objects()");
        create_device_objects()
    }

    /// Destroy OpenGL3 renderer device objects.
    pub fn destroy_device_objects(&mut self, imgui: &mut Context) {
        self.context
            .assert_matches(imgui, "Sdl3OpenGl3Backend::destroy_device_objects()");
        let _guard = self
            .context
            .bind("Sdl3OpenGl3Backend::destroy_device_objects()");
        destroy_device_objects();
    }

    /// Shut down the official OpenGL3 renderer and SDL3 platform backend.
    pub fn shutdown(mut self, imgui: &mut Context) {
        self.context
            .assert_matches(imgui, "Sdl3OpenGl3Backend::shutdown()");
        {
            let _guard = self.context.bind("Sdl3OpenGl3Backend::shutdown()");
            shutdown_opengl3_impl();
        }
        self.shutdown_on_drop = false;
    }
}

#[cfg(feature = "opengl3-renderer")]
impl Drop for Sdl3OpenGl3Backend {
    fn drop(&mut self) {
        if self.shutdown_on_drop {
            if let Some(_guard) = self.context.bind_for_drop() {
                shutdown_opengl3_impl();
            }
        }
    }
}

/// RAII owner for SDL3 platform + official SDLRenderer3 renderer backends.
#[cfg(feature = "sdlrenderer3-renderer")]
#[must_use = "dropping the backend owner shuts down the SDL3 + SDLRenderer3 backends"]
#[derive(Debug)]
pub struct Sdl3RendererBackend {
    context: ContextBinding,
    shutdown_on_drop: bool,
}

#[cfg(feature = "sdlrenderer3-renderer")]
impl Sdl3RendererBackend {
    fn from_initialized_context(imgui: &Context) -> Self {
        Self {
            context: ContextBinding::capture(imgui),
            shutdown_on_drop: true,
        }
    }

    /// Initialize the SDL3 platform backend and the official SDLRenderer3 renderer.
    pub fn init(
        imgui: &mut Context,
        window: &Window,
        canvas: &WindowCanvas,
    ) -> Result<Self, Sdl3BackendError> {
        init_for_canvas(imgui, window, canvas)?;
        Ok(Self::from_initialized_context(imgui))
    }

    /// Begin a new SDL3 + SDLRenderer3 frame.
    pub fn new_frame(&mut self, imgui: &mut Context) {
        self.context
            .assert_matches(imgui, "Sdl3RendererBackend::new_frame()");
        let _guard = self.context.bind("Sdl3RendererBackend::new_frame()");
        new_frame_sdlrenderer3_impl();
    }

    /// Process a single low-level SDL3 event with the captured ImGui context.
    pub fn process_event(&mut self, imgui: &mut Context, event: &SDL_Event) -> bool {
        self.context
            .assert_matches(imgui, "Sdl3RendererBackend::process_event()");
        let _guard = self.context.bind("Sdl3RendererBackend::process_event()");
        process_sys_event(event)
    }

    /// Render Dear ImGui draw data using the official SDLRenderer3 renderer.
    pub fn render(&mut self, draw_data: &mut DrawData, canvas: &WindowCanvas) {
        let _guard = self.context.bind("Sdl3RendererBackend::render()");
        self.context
            .assert_current_draw_data(draw_data, "Sdl3RendererBackend::render()");
        render_sdlrenderer3_impl(draw_data, canvas);
    }

    /// Update a single ImGui texture using the official SDLRenderer3 renderer.
    pub fn update_texture(&mut self, imgui: &mut Context, tex: &mut TextureData) {
        self.context
            .assert_matches(imgui, "Sdl3RendererBackend::update_texture()");
        let _guard = self.context.bind("Sdl3RendererBackend::update_texture()");
        canvas_update_texture(tex);
    }

    /// Configure how the SDL3 backend handles gamepads for the captured context.
    pub fn set_gamepad_mode(&mut self, imgui: &mut Context, mode: GamepadMode) {
        self.context
            .assert_matches(imgui, "Sdl3RendererBackend::set_gamepad_mode()");
        let _guard = self.context.bind("Sdl3RendererBackend::set_gamepad_mode()");
        set_gamepad_mode(mode);
    }

    /// Configure SDL3 backend to use manual gamepad selection for the captured context.
    ///
    /// # Safety
    ///
    /// - The caller must ensure every pointer in `gamepads` is a valid, opened `SDL_Gamepad`.
    /// - The caller is responsible for keeping those gamepads alive for the duration of ImGui usage.
    /// - The slice itself is only read during this call; the backend copies the pointers.
    pub unsafe fn set_gamepad_mode_manual(
        &mut self,
        imgui: &mut Context,
        gamepads: &[*mut sdl3_sys::gamepad::SDL_Gamepad],
    ) {
        self.context
            .assert_matches(imgui, "Sdl3RendererBackend::set_gamepad_mode_manual()");
        let _guard = self
            .context
            .bind("Sdl3RendererBackend::set_gamepad_mode_manual()");
        unsafe {
            set_gamepad_mode_manual(gamepads);
        }
    }

    /// Create SDLRenderer3 renderer device objects.
    pub fn create_device_objects(&mut self, imgui: &mut Context) {
        self.context
            .assert_matches(imgui, "Sdl3RendererBackend::create_device_objects()");
        let _guard = self
            .context
            .bind("Sdl3RendererBackend::create_device_objects()");
        canvas_create_device_objects();
    }

    /// Destroy SDLRenderer3 renderer device objects.
    pub fn destroy_device_objects(&mut self, imgui: &mut Context) {
        self.context
            .assert_matches(imgui, "Sdl3RendererBackend::destroy_device_objects()");
        let _guard = self
            .context
            .bind("Sdl3RendererBackend::destroy_device_objects()");
        canvas_destroy_device_objects();
    }

    /// Shut down the official SDLRenderer3 renderer and SDL3 platform backend.
    pub fn shutdown(mut self, imgui: &mut Context) {
        self.context
            .assert_matches(imgui, "Sdl3RendererBackend::shutdown()");
        {
            let _guard = self.context.bind("Sdl3RendererBackend::shutdown()");
            shutdown_sdlrenderer3_impl();
        }
        self.shutdown_on_drop = false;
    }
}

#[cfg(feature = "sdlrenderer3-renderer")]
impl Drop for Sdl3RendererBackend {
    fn drop(&mut self) {
        if self.shutdown_on_drop {
            if let Some(_guard) = self.context.bind_for_drop() {
                shutdown_sdlrenderer3_impl();
            }
        }
    }
}

/// Configure how the SDL3 backend handles gamepads.
///
/// Call this after backend initialization if you want a mode other than the
/// default `AutoFirst`.
///
/// This thin compatibility helper operates on Dear ImGui's current context.
/// Prefer [`set_gamepad_mode_for_context`] or an RAII backend owner in
/// multi-context code.
pub fn set_gamepad_mode(mode: GamepadMode) {
    unsafe {
        match mode {
            GamepadMode::AutoFirst => ffi::ImGui_ImplSDL3_SetGamepadMode_AutoFirst_Rust(),
            GamepadMode::AutoAll => ffi::ImGui_ImplSDL3_SetGamepadMode_AutoAll_Rust(),
        }
    }
}

/// Configure how the SDL3 backend handles gamepads for a specific context.
pub fn set_gamepad_mode_for_context(imgui: &mut Context, mode: GamepadMode) {
    with_context(imgui, "set_gamepad_mode_for_context()", || {
        set_gamepad_mode(mode);
    });
}

/// Configure SDL3 backend to use manual gamepad selection.
///
/// This thin compatibility helper operates on Dear ImGui's current context.
/// Prefer [`set_gamepad_mode_manual_for_context`] or an RAII backend owner in
/// multi-context code.
///
/// # Safety
///
/// - The caller must ensure every pointer in `gamepads` is a valid, opened `SDL_Gamepad`.
/// - The caller is responsible for keeping those gamepads alive for the duration of ImGui usage.
/// - The slice itself is only read during this call; the backend copies the pointers.
pub unsafe fn set_gamepad_mode_manual(gamepads: &[*mut sdl3_sys::gamepad::SDL_Gamepad]) {
    unsafe {
        ffi::ImGui_ImplSDL3_SetGamepadMode_Manual_Rust(gamepads.as_ptr(), gamepads.len() as i32);
    }
}

/// Enable native IME UI for SDL3 (recommended on IME-heavy platforms).
///
/// This should be called before creating any SDL3 windows so that the
/// underlying backend can display the OS IME UI correctly.
pub fn enable_native_ime_ui() {
    // Best-effort: ignore return value; missing hints are not fatal.
    let _ = sdl3::hint::set("SDL_HINT_IME_SHOW_UI", "1");
}

#[cfg(feature = "opengl3-renderer")]
fn init_opengl3_impl(glsl_version: *const std::ffi::c_char) -> Result<(), Sdl3BackendError> {
    unsafe {
        if !opengl3_backend::dear_imgui_backend_opengl3_init(glsl_version) {
            ffi::ImGui_ImplSDL3_Shutdown_Rust();
            return Err(Sdl3BackendError::OpenGlInitFailed);
        }
    }
    Ok(())
}

fn shutdown_platform_impl() {
    unsafe {
        ffi::ImGui_ImplSDL3_Shutdown_Rust();
    }
}

/// Configure SDL3 backend to use manual gamepad selection for a specific context.
///
/// # Safety
///
/// - The caller must ensure every pointer in `gamepads` is a valid, opened `SDL_Gamepad`.
/// - The caller is responsible for keeping those gamepads alive for the duration of ImGui usage.
/// - The slice itself is only read during this call; the backend copies the pointers.
pub unsafe fn set_gamepad_mode_manual_for_context(
    imgui: &mut Context,
    gamepads: &[*mut sdl3_sys::gamepad::SDL_Gamepad],
) {
    with_context(imgui, "set_gamepad_mode_manual_for_context()", || unsafe {
        set_gamepad_mode_manual(gamepads);
    });
}

#[cfg(feature = "opengl3-renderer")]
fn shutdown_opengl3_impl() {
    unsafe {
        opengl3_backend::dear_imgui_backend_opengl3_shutdown();
        ffi::ImGui_ImplSDL3_Shutdown_Rust();
    }
}

#[cfg(feature = "sdlrenderer3-renderer")]
fn shutdown_sdlrenderer3_impl() {
    unsafe {
        sdlrenderer3_backend::dear_imgui_backend_sdlrenderer3_shutdown();
        ffi::ImGui_ImplSDL3_Shutdown_Rust();
    }
}

#[cfg(feature = "opengl3-renderer")]
fn new_frame_opengl3_impl() {
    unsafe {
        opengl3_backend::dear_imgui_backend_opengl3_new_frame();
        ffi::ImGui_ImplSDL3_NewFrame_Rust();
    }
}

fn sdl3_new_frame_impl() {
    unsafe {
        ffi::ImGui_ImplSDL3_NewFrame_Rust();
    }
}

#[cfg(feature = "sdlrenderer3-renderer")]
fn new_frame_sdlrenderer3_impl() {
    unsafe {
        sdlrenderer3_backend::dear_imgui_backend_sdlrenderer3_new_frame();
        ffi::ImGui_ImplSDL3_NewFrame_Rust();
    }
}

/// Initialize the Dear ImGui SDL3 + OpenGL3 backends.
///
/// This assumes that:
/// - a `dear_imgui_rs::Context` already exists;
/// - `window` has an active OpenGL context (`gl_context`);
/// - the same context will be current when rendering.
///
/// `glsl_version` should be a GLSL version string such as `"#version 150"`.
///
/// Requires the `opengl3-renderer` feature.
#[cfg(feature = "opengl3-renderer")]
pub fn init_for_opengl(
    imgui: &mut Context,
    window: &Window,
    gl_context: &GLContext,
    glsl_version: &str,
) -> Result<(), Sdl3BackendError> {
    let glsl = CString::new(glsl_version).map_err(|_| Sdl3BackendError::InvalidGlslVersion)?;

    let sdl_window = window.raw();
    let sdl_gl = unsafe { gl_context.raw() };

    with_context(imgui, "init_for_opengl()", || {
        unsafe {
            if !ffi::ImGui_ImplSDL3_InitForOpenGL_Rust(sdl_window, sdl_gl as *mut c_void) {
                return Err(Sdl3BackendError::Sdl3InitFailed);
            }
        }
        init_opengl3_impl(glsl.as_ptr())
    })
}

/// Initialize the Dear ImGui SDL3 + OpenGL3 backends using the default GLSL version.
///
/// This matches the upstream behavior of passing `nullptr` for `glsl_version`.
///
/// Requires the `opengl3-renderer` feature.
#[cfg(feature = "opengl3-renderer")]
pub fn init_for_opengl_default(
    imgui: &mut Context,
    window: &Window,
    gl_context: &GLContext,
) -> Result<(), Sdl3BackendError> {
    let sdl_window = window.raw();
    let sdl_gl = unsafe { gl_context.raw() };

    with_context(imgui, "init_for_opengl_default()", || {
        unsafe {
            if !ffi::ImGui_ImplSDL3_InitForOpenGL_Rust(sdl_window, sdl_gl as *mut c_void) {
                return Err(Sdl3BackendError::Sdl3InitFailed);
            }
        }
        init_opengl3_impl(std::ptr::null())
    })
}

/// Initialize only the Dear ImGui SDL3 *platform* backend for an OpenGL context.
///
/// This is useful when you want to use a Rust renderer (e.g. `dear-imgui-glow`)
/// instead of the official C++ OpenGL3 renderer. It:
/// - configures the SDL3 platform backend (including multi-viewport support);
/// - does **not** initialize `imgui_impl_opengl3`.
///
/// This assumes that:
/// - a `dear_imgui_rs::Context` already exists;
/// - `window` has an active OpenGL context (`gl_context`);
/// - the same context will be current when rendering.
pub fn init_platform_for_opengl(
    imgui: &mut Context,
    window: &Window,
    gl_context: &GLContext,
) -> Result<(), Sdl3BackendError> {
    let sdl_window = window.raw();
    let sdl_gl = unsafe { gl_context.raw() };

    with_context(imgui, "init_platform_for_opengl()", || {
        unsafe {
            if !ffi::ImGui_ImplSDL3_InitForOpenGL_Rust(sdl_window, sdl_gl as *mut c_void) {
                return Err(Sdl3BackendError::Sdl3InitFailed);
            }
        }
        Ok(())
    })
}

/// Initialize the Dear ImGui SDL3 platform backend only.
///
/// This is useful when using a non-OpenGL renderer (e.g. WGPU) and only
/// want SDL3 to drive the platform layer.
///
/// This assumes that:
/// - a `dear_imgui_rs::Context` already exists;
/// - `window` is a valid SDL3 window handle.
pub fn init_for_other(imgui: &mut Context, window: &Window) -> Result<(), Sdl3BackendError> {
    let sdl_window = window.raw();

    with_context(imgui, "init_for_other()", || {
        unsafe {
            if !ffi::ImGui_ImplSDL3_InitForOther_Rust(sdl_window) {
                return Err(Sdl3BackendError::Sdl3InitFailed);
            }
        }
        Ok(())
    })
}

/// Initialize the Dear ImGui SDL3 platform backend for Vulkan renderers.
///
/// This is equivalent to `ImGui_ImplSDL3_InitForVulkan` and is required for
/// Vulkan multi-viewport support (sets Vulkan window flags for secondary viewports).
pub fn init_for_vulkan(imgui: &mut Context, window: &Window) -> Result<(), Sdl3BackendError> {
    let sdl_window = window.raw();
    with_context(imgui, "init_for_vulkan()", || {
        unsafe {
            if !ffi::ImGui_ImplSDL3_InitForVulkan_Rust(sdl_window) {
                return Err(Sdl3BackendError::Sdl3InitFailed);
            }
        }
        Ok(())
    })
}

/// Initialize the Dear ImGui SDL3 platform backend for Direct3D (Windows only).
pub fn init_for_d3d(imgui: &mut Context, window: &Window) -> Result<(), Sdl3BackendError> {
    let sdl_window = window.raw();
    with_context(imgui, "init_for_d3d()", || {
        unsafe {
            if !ffi::ImGui_ImplSDL3_InitForD3D_Rust(sdl_window) {
                return Err(Sdl3BackendError::Sdl3InitFailed);
            }
        }
        Ok(())
    })
}

/// Initialize the Dear ImGui SDL3 platform backend for Metal renderers.
pub fn init_for_metal(imgui: &mut Context, window: &Window) -> Result<(), Sdl3BackendError> {
    let sdl_window = window.raw();
    with_context(imgui, "init_for_metal()", || {
        unsafe {
            if !ffi::ImGui_ImplSDL3_InitForMetal_Rust(sdl_window) {
                return Err(Sdl3BackendError::Sdl3InitFailed);
            }
        }
        Ok(())
    })
}

/// Initialize the Dear ImGui SDL3 platform backend for SDL_Renderer-based renderers.
///
/// # Safety
///
/// The caller must provide a valid `SDL_Renderer` pointer associated with `window`.
pub unsafe fn init_for_sdl_renderer(
    imgui: &mut Context,
    window: &Window,
    renderer: *mut sdl3_sys::render::SDL_Renderer,
) -> Result<(), Sdl3BackendError> {
    let sdl_window = window.raw();
    with_context(imgui, "init_for_sdl_renderer()", || {
        unsafe {
            if !ffi::ImGui_ImplSDL3_InitForSDLRenderer_Rust(sdl_window, renderer) {
                return Err(Sdl3BackendError::Sdl3InitFailed);
            }
        }
        Ok(())
    })
}

/// Initialize the Dear ImGui SDL3 platform backend for SDL GPU (SDL_gpu3) renderers.
pub fn init_for_sdl_gpu(imgui: &mut Context, window: &Window) -> Result<(), Sdl3BackendError> {
    let sdl_window = window.raw();
    with_context(imgui, "init_for_sdl_gpu()", || {
        unsafe {
            if !ffi::ImGui_ImplSDL3_InitForSDLGPU_Rust(sdl_window) {
                return Err(Sdl3BackendError::Sdl3InitFailed);
            }
        }
        Ok(())
    })
}

/// Initialize the Dear ImGui SDL3 + SDLRenderer3 backend.
///
/// This assumes that:
/// - a `dear_imgui_rs::Context` already exists;
/// - the window related to the renderer/canvas.
/// - the canvas will exist for at least until `shutdown_for_canvas` is called.
///
/// Requires the `sdlrenderer3-renderer` feature.
#[cfg(feature = "sdlrenderer3-renderer")]
pub fn init_for_canvas(
    imgui: &mut Context,
    window: &Window,
    canvas: &WindowCanvas,
) -> Result<(), Sdl3BackendError> {
    let sdl_window = window.raw();
    let sdl_renderer = canvas.raw();

    with_context(imgui, "init_for_canvas()", || {
        unsafe {
            if !ffi::ImGui_ImplSDL3_InitForSDLRenderer_Rust(sdl_window, sdl_renderer) {
                return Err(Sdl3BackendError::Sdl3InitFailed);
            }
            if !sdlrenderer3_backend::dear_imgui_backend_sdlrenderer3_init(
                sdl_renderer as *mut std::ffi::c_void,
            ) {
                ffi::ImGui_ImplSDL3_Shutdown_Rust();
                return Err(Sdl3BackendError::Renderer3InitFailed);
            }
        }
        Ok(())
    })
}

/// Shutdown the SDL3 + OpenGL3 backends.
///
/// Call this before destroying the ImGui context or the SDL3 window.
#[cfg(feature = "opengl3-renderer")]
pub fn shutdown_for_opengl(imgui: &mut Context) {
    with_context(imgui, "shutdown_for_opengl()", shutdown_opengl3_impl);
}

/// Shutdown the SDL3 platform backend only.
///
/// This is the counterpart to [`init_for_other`] and should be called before
/// destroying the ImGui context when using a non-OpenGL renderer (e.g. WGPU).
pub fn shutdown(imgui: &mut Context) {
    with_context(imgui, "shutdown()", shutdown_platform_impl);
}

/// Shutdown the SDL3 + SDLRenderer3 backend.
///
/// Call this before destroying the ImGui context or the SDL3 canvas or window.
#[cfg(feature = "sdlrenderer3-renderer")]
pub fn shutdown_for_canvas(imgui: &mut Context) {
    with_context(imgui, "shutdown_for_canvas()", shutdown_sdlrenderer3_impl);
}

/// Begin a new ImGui frame for SDL3 + OpenGL.
///
/// Call this before `imgui.frame()`.
#[cfg(feature = "opengl3-renderer")]
pub fn new_frame(imgui: &mut Context) {
    with_context(imgui, "new_frame()", new_frame_opengl3_impl);
}

/// Begin a new ImGui frame for SDL3 platform backend only.
///
/// This is intended for non-OpenGL renderers such as WGPU.
pub fn sdl3_new_frame(imgui: &mut Context) {
    with_context(imgui, "sdl3_new_frame()", sdl3_new_frame_impl);
}

/// Begin a new ImGui frame for SDL3 + SDLRenderer3.
///
/// Call this before `imgui.frame()`.
#[cfg(feature = "sdlrenderer3-renderer")]
pub fn canvas_new_frame(imgui: &mut Context) {
    with_context(imgui, "canvas_new_frame()", new_frame_sdlrenderer3_impl);
}

/// Poll the next SDL3 event as a low-level `SDL_Event`.
///
/// This mirrors the C++ SDL3 examples and is useful when you want to feed both
/// Dear ImGui and your own event handling from the same low-level event stream.
pub fn sdl3_poll_event_ll() -> Option<SDL_Event> {
    let mut raw = std::mem::MaybeUninit::<SDL_Event>::uninit();
    let has_event = unsafe { sdl3_sys::events::SDL_PollEvent(raw.as_mut_ptr()) };
    if has_event {
        Some(unsafe { raw.assume_init() })
    } else {
        None
    }
}

/// Process a single low-level SDL3 event with ImGui's SDL3 backend.
///
/// Returns `true` if Dear ImGui consumed the event.
///
/// This thin compatibility helper operates on Dear ImGui's current context.
/// Prefer [`process_sys_event_for_context`] or an RAII backend owner in
/// multi-context code.
pub fn process_sys_event(event: &SDL_Event) -> bool {
    unsafe { ffi::ImGui_ImplSDL3_ProcessEvent_Rust(event) }
}

/// Process a single low-level SDL3 event for a specific ImGui context.
///
/// Returns `true` if Dear ImGui consumed the event.
pub fn process_sys_event_for_context(imgui: &mut Context, event: &SDL_Event) -> bool {
    with_context(imgui, "process_sys_event_for_context()", || {
        process_sys_event(event)
    })
}

/// Render Dear ImGui draw data using the OpenGL3 backend.
///
/// This assumes an OpenGL context is current.
#[cfg(feature = "opengl3-renderer")]
pub fn render(draw_data: &mut DrawData) {
    render_opengl3_impl(draw_data);
}

#[cfg(feature = "opengl3-renderer")]
fn render_opengl3_impl(draw_data: &mut DrawData) {
    unsafe {
        let raw = draw_data as *mut DrawData as *mut sys::ImDrawData;
        opengl3_backend::dear_imgui_backend_opengl3_render_draw_data(raw);
    }
}

/// Render Dear ImGui draw data using the SDLRenderer3 backend.
#[cfg(feature = "sdlrenderer3-renderer")]
pub fn canvas_render(draw_data: &mut DrawData, canvas: &WindowCanvas) {
    render_sdlrenderer3_impl(draw_data, canvas);
}

#[cfg(feature = "sdlrenderer3-renderer")]
fn render_sdlrenderer3_impl(draw_data: &mut DrawData, canvas: &WindowCanvas) {
    let sdl_renderer = canvas.raw();
    unsafe {
        let raw = draw_data as *mut DrawData as *mut sys::ImDrawData;
        sdlrenderer3_backend::dear_imgui_backend_sdlrenderer3_render_draw_data(
            raw,
            sdl_renderer as *mut std::ffi::c_void,
        );
    }
}

/// Update a single ImGui texture using the OpenGL3 backend.
///
/// This is an advanced helper that delegates to `ImGui_ImplOpenGL3_UpdateTexture`.
#[cfg(feature = "opengl3-renderer")]
pub fn update_texture(tex: &mut TextureData) {
    unsafe {
        opengl3_backend::dear_imgui_backend_opengl3_update_texture(tex.as_raw_mut());
    }
}

/// Update a single ImGui texture using the SDLRenderer3 backend.
///
/// This is an advanced helper that delegates to `ImGui_ImplSDLRenderer3_UpdateTexture`.
#[cfg(feature = "sdlrenderer3-renderer")]
pub fn canvas_update_texture(tex: &mut TextureData) {
    unsafe {
        sdlrenderer3_backend::dear_imgui_backend_sdlrenderer3_update_texture(tex.as_raw_mut());
    }
}

/// Create OpenGL3 renderer device objects.
///
/// This is an optional advanced helper mirroring `ImGui_ImplOpenGL3_CreateDeviceObjects`.
#[cfg(feature = "opengl3-renderer")]
pub fn create_device_objects() -> bool {
    unsafe { opengl3_backend::dear_imgui_backend_opengl3_create_device_objects() }
}

/// Destroy OpenGL3 renderer device objects.
///
/// This is an optional advanced helper mirroring `ImGui_ImplOpenGL3_DestroyDeviceObjects`.
#[cfg(feature = "opengl3-renderer")]
pub fn destroy_device_objects() {
    unsafe {
        opengl3_backend::dear_imgui_backend_opengl3_destroy_device_objects();
    }
}

/// Create SDLRenderer3 renderer device objects.
///
/// This is an optional advanced helper mirroring `ImGui_ImplSDLRenderer3_CreateDeviceObjects`.
#[cfg(feature = "sdlrenderer3-renderer")]
pub fn canvas_create_device_objects() {
    unsafe { sdlrenderer3_backend::dear_imgui_backend_sdlrenderer3_create_device_objects() }
}

/// Destroy SDLRenderer3 renderer device objects.
///
/// This is an optional advanced helper mirroring `ImGui_ImplSDLRenderer3_DestroyDeviceObjects`.
#[cfg(feature = "sdlrenderer3-renderer")]
pub fn canvas_destroy_device_objects() {
    unsafe {
        sdlrenderer3_backend::dear_imgui_backend_sdlrenderer3_destroy_device_objects();
    }
}
