use super::*;

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
