use super::*;

#[derive(Clone, Debug)]
pub(super) struct ContextBinding {
    raw: *mut sys::ImGuiContext,
    alive: ContextAliveToken,
}

impl ContextBinding {
    pub(super) fn capture(imgui: &Context) -> Self {
        Self {
            raw: imgui.as_raw(),
            alive: imgui.alive_token(),
        }
    }

    pub(super) fn assert_matches(&self, imgui: &Context, caller: &str) {
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

    pub(super) fn bind(&self, caller: &str) -> CurrentContextGuard {
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

    pub(super) fn bind_for_drop(&self) -> Option<CurrentContextGuard> {
        if self.alive.is_alive() && !self.raw.is_null() {
            Some(unsafe { CurrentContextGuard::bind(self.raw) })
        } else {
            None
        }
    }

    #[cfg(any(feature = "opengl3-renderer", feature = "sdlrenderer3-renderer", feature = "sdlgpu3-renderer"))]
    pub(super) fn assert_current_draw_data(&self, draw_data: &mut DrawData, caller: &str) {
        let expected = unsafe { sys::igGetDrawData() as *mut sys::ImDrawData };
        let actual = draw_data as *mut DrawData as *mut sys::ImDrawData;
        assert_eq!(
            expected, actual,
            "{caller} received draw data that does not belong to the captured Dear ImGui context"
        );
    }
}

#[derive(Debug)]
pub(super) struct CurrentContextGuard {
    previous: *mut sys::ImGuiContext,
    restore: bool,
}

impl CurrentContextGuard {
    pub(super) unsafe fn bind(raw: *mut sys::ImGuiContext) -> Self {
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

pub(super) fn with_context<R>(imgui: &mut Context, caller: &str, f: impl FnOnce() -> R) -> R {
    let context = ContextBinding::capture(imgui);
    let _guard = context.bind(caller);
    f()
}

/// FFI bindings to the C wrappers defined in `wrapper.cpp`.
pub(super) mod ffi {
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
    #[error("ImGui_ImplSDLGpu3_Init returned false")]
    Gpu3InitFailed,
}

#[cfg(feature = "opengl3-renderer")]
pub(super) fn init_opengl3_impl(
    glsl_version: *const std::ffi::c_char,
) -> Result<(), Sdl3BackendError> {
    unsafe {
        if !opengl3_backend::dear_imgui_backend_opengl3_init(glsl_version) {
            ffi::ImGui_ImplSDL3_Shutdown_Rust();
            return Err(Sdl3BackendError::OpenGlInitFailed);
        }
    }
    Ok(())
}

#[cfg(feature = "sdlgpu3-renderer")]
pub(super) fn init_sdlgpu3_impl(
    device: *mut sdl3_sys::gpu::SDL_GPUDevice,
    texture: c_int,
    sample_count: c_int,
    swap_chain: c_int,
    preset_mode: c_int
) -> Result<(), Sdl3BackendError> {
    let mut sd = Box::new(sdlgpu3_backend::ImGui_ImplSDLGPU3_InitInfo {
        device: device as *mut _ as *mut c_void,
        colorTargetFormat: texture as c_int,
        MSAASamples: sample_count as c_int,
        SwapchainComposition: swap_chain as c_int,
        PresentMode: preset_mode as c_int,
    });
    unsafe {
        if !sdlgpu3_backend::dear_imgui_backend_sdlgpu3_init(sd.as_mut()) {
            ffi::ImGui_ImplSDL3_Shutdown_Rust();
            return Err(Sdl3BackendError::Gpu3InitFailed);
        }
    }
    Ok(())
}

pub(super) fn shutdown_platform_impl() {
    unsafe {
        ffi::ImGui_ImplSDL3_Shutdown_Rust();
    }
}

#[cfg(feature = "opengl3-renderer")]
pub(super) fn shutdown_opengl3_impl() {
    unsafe {
        opengl3_backend::dear_imgui_backend_opengl3_shutdown();
        ffi::ImGui_ImplSDL3_Shutdown_Rust();
    }
}

#[cfg(feature = "sdlgpu3-renderer")]
pub(super) fn shutdown_gpu_impl() {
    unsafe {
        sdlgpu3_backend::dear_imgui_backend_sdlgpu3_shutdown();
        ffi::ImGui_ImplSDL3_Shutdown_Rust();
    }
}

#[cfg(feature = "sdlrenderer3-renderer")]
pub(super) fn shutdown_sdlrenderer3_impl() {
    unsafe {
        sdlrenderer3_backend::dear_imgui_backend_sdlrenderer3_shutdown();
        ffi::ImGui_ImplSDL3_Shutdown_Rust();
    }
}

#[cfg(feature = "sdlgpu3-renderer")]
pub(super) fn new_frame_gpu3_impl() {
    unsafe {
        sdlgpu3_backend::dear_imgui_backend_sdlgpu3_new_frame();
        ffi::ImGui_ImplSDL3_NewFrame_Rust();
    }
}

#[cfg(feature = "opengl3-renderer")]
pub(super) fn new_frame_opengl3_impl() {
    unsafe {
        opengl3_backend::dear_imgui_backend_opengl3_new_frame();
        ffi::ImGui_ImplSDL3_NewFrame_Rust();
    }
}

pub(super) fn sdl3_new_frame_impl() {
    unsafe {
        ffi::ImGui_ImplSDL3_NewFrame_Rust();
    }
}

#[cfg(feature = "sdlrenderer3-renderer")]
pub(super) fn new_frame_sdlrenderer3_impl() {
    unsafe {
        sdlrenderer3_backend::dear_imgui_backend_sdlrenderer3_new_frame();
        ffi::ImGui_ImplSDL3_NewFrame_Rust();
    }
}
