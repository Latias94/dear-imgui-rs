use super::*;

/// Enable native IME UI for SDL3 (recommended on IME-heavy platforms).
///
/// This should be called before creating any SDL3 windows so that the
/// underlying backend can display the OS IME UI correctly.
pub fn enable_native_ime_ui() {
    // Best-effort: ignore return value; missing hints are not fatal.
    let _ = sdl3::hint::set("SDL_HINT_IME_SHOW_UI", "1");
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
