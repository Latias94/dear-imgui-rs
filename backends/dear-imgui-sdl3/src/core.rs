use super::*;
#[cfg(any(
    feature = "opengl3-renderer",
    feature = "sdlrenderer3-renderer",
    feature = "sdlgpu3-renderer"
))]
use dear_imgui_rs::sys;

pub(super) fn with_context<R>(imgui: &Context, caller: &str, f: impl FnOnce() -> R) -> R {
    imgui
        .binding()
        .try_with_bound_context(f)
        .unwrap_or_else(|error| panic!("{caller} could not bind its Dear ImGui context: {error}"))
}

#[cfg(any(
    feature = "opengl3-renderer",
    feature = "sdlrenderer3-renderer",
    feature = "sdlgpu3-renderer"
))]
pub(super) fn assert_current_draw_data(draw_data: &DrawData, caller: &str) {
    let expected = unsafe { sys::igGetDrawData() as *const sys::ImDrawData };
    let actual = draw_data as *const DrawData as *const sys::ImDrawData;
    assert_eq!(
        expected, actual,
        "{caller} received draw data that does not belong to the captured Dear ImGui context"
    );
}

/// FFI bindings to the C wrappers defined in `wrapper.cpp`.
pub(super) mod ffi {
    use super::*;

    #[cfg(feature = "sdlgpu3-renderer")]
    #[repr(C)]
    #[derive(Copy, Clone)]
    pub(crate) struct ImGuiImplSdlGpu3InitInfo {
        pub device: *mut SDL_GPUDevice,
        pub color_target_format: SDL_GPUTextureFormat,
        pub msaa_samples: SDL_GPUSampleCount,
        pub swapchain_composition: SDL_GPUSwapchainComposition,
        pub present_mode: SDL_GPUPresentMode,
    }

    unsafe extern "C" {
        #[cfg(test)]
        pub fn dear_imgui_sdl3_backend_sizeof_imgui_io() -> usize;
        #[cfg(all(test, debug_assertions))]
        pub fn dear_imgui_sdl3_native_contract_self_test() -> u64;
        #[cfg(all(
            test,
            debug_assertions,
            any(
                feature = "opengl3-renderer",
                feature = "sdlrenderer3-renderer",
                feature = "sdlgpu3-renderer"
            )
        ))]
        pub fn dear_imgui_sdl3_destroy_platform_windows_for_test(
            viewport: *mut sys::ImGuiViewportP,
        );
        #[cfg(test)]
        pub fn dear_imgui_sdl3_mouse_leave_due_for_test(
            pending_frame: i32,
            current_frame: i32,
            buttons_down: i32,
        ) -> bool;
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
        pub fn dear_imgui_sdl3_native_begin(
            phase: u32,
            expects_opengl: u32,
            swap_interval_policy: u32,
            explicit_swap_interval: i32,
            viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
        ) -> u64;
        pub fn dear_imgui_sdl3_native_end() -> u64;
        pub fn dear_imgui_sdl3_backend_clear_platform_monitors();
        #[cfg(any(
            feature = "opengl3-renderer",
            feature = "sdlrenderer3-renderer",
            feature = "sdlgpu3-renderer"
        ))]
        pub fn dear_imgui_sdl3_backend_set_texture_updates(
            texture: *mut sys::ImTextureData,
            updates: *const sys::ImTextureRect,
            update_count: i32,
        );

        pub fn ImGui_ImplSDL3_SetGamepadMode_AutoFirst_Rust();
        pub fn ImGui_ImplSDL3_SetGamepadMode_AutoAll_Rust();
        pub fn ImGui_ImplSDL3_SetGamepadMode_Manual_Rust(
            manual_gamepads_array: *const *mut sdl3_sys::gamepad::SDL_Gamepad,
            manual_gamepads_count: i32,
        );
        pub fn ImGui_ImplSDL3_SetMouseCaptureMode_Enabled_Rust();
        pub fn ImGui_ImplSDL3_SetMouseCaptureMode_EnabledAfterDrag_Rust();
        pub fn ImGui_ImplSDL3_SetMouseCaptureMode_Disabled_Rust();

        #[cfg(feature = "sdlrenderer3-renderer")]
        pub fn dear_imgui_sdl3_backend_sdlrenderer3_init(renderer: *mut SDL_Renderer) -> bool;
        #[cfg(feature = "sdlrenderer3-renderer")]
        pub fn dear_imgui_sdl3_backend_sdlrenderer3_shutdown();
        #[cfg(feature = "sdlrenderer3-renderer")]
        pub fn dear_imgui_sdl3_backend_sdlrenderer3_new_frame();
        #[cfg(feature = "sdlrenderer3-renderer")]
        pub fn dear_imgui_sdl3_backend_sdlrenderer3_render_draw_data(
            draw_data: *mut sys::ImDrawData,
            renderer: *mut SDL_Renderer,
        );
        #[cfg(feature = "sdlrenderer3-renderer")]
        pub fn dear_imgui_sdl3_backend_sdlrenderer3_create_device_objects();
        #[cfg(feature = "sdlrenderer3-renderer")]
        pub fn dear_imgui_sdl3_backend_sdlrenderer3_destroy_device_objects();
        #[cfg(feature = "sdlrenderer3-renderer")]
        pub fn dear_imgui_sdl3_backend_sdlrenderer3_update_texture(
            texture: *mut sys::ImTextureData,
        );

        #[cfg(feature = "sdlgpu3-renderer")]
        pub fn dear_imgui_sdl3_backend_sdlgpu3_init(info: *mut ImGuiImplSdlGpu3InitInfo) -> bool;
        #[cfg(feature = "sdlgpu3-renderer")]
        pub fn dear_imgui_sdl3_backend_sdlgpu3_shutdown();
        #[cfg(feature = "sdlgpu3-renderer")]
        pub fn dear_imgui_sdl3_backend_sdlgpu3_new_frame();
        #[cfg(feature = "sdlgpu3-renderer")]
        pub fn dear_imgui_sdl3_backend_sdlgpu3_prepare_draw_data(
            draw_data: *mut sys::ImDrawData,
            command_buffer: *mut SDL_GPUCommandBuffer,
        );
        #[cfg(feature = "sdlgpu3-renderer")]
        pub fn dear_imgui_sdl3_backend_sdlgpu3_render_draw_data(
            draw_data: *mut sys::ImDrawData,
            command_buffer: *mut SDL_GPUCommandBuffer,
            render_pass: *mut SDL_GPURenderPass,
            pipeline: *mut SDL_GPUGraphicsPipeline,
        );
        #[cfg(feature = "sdlgpu3-renderer")]
        pub fn dear_imgui_sdl3_backend_sdlgpu3_render_viewport(
            viewport: *mut sys::ImGuiViewport,
        ) -> u64;
        #[cfg(feature = "sdlgpu3-renderer")]
        pub fn dear_imgui_sdl3_backend_sdlgpu3_create_device_objects();
        #[cfg(feature = "sdlgpu3-renderer")]
        pub fn dear_imgui_sdl3_backend_sdlgpu3_destroy_device_objects();
        #[cfg(feature = "sdlgpu3-renderer")]
        pub fn dear_imgui_sdl3_backend_sdlgpu3_update_texture(texture: *mut sys::ImTextureData);
    }
}

/// Swap-interval policy applied when SDL3 creates a secondary OpenGL context.
///
/// [`Immediate`](Self::Immediate) matches the upstream multi-viewport default and avoids serial
/// VSync waits when several platform windows are presented in one frame.
///
/// Drivers may reject a requested interval after creating a secondary context. That is treated as
/// a presentation fallback to the driver's default rather than a viewport-creation failure.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Sdl3OpenGlViewportSwapInterval {
    /// Disable VSync for secondary viewport contexts.
    #[default]
    Immediate,
    /// Wait for one vertical refresh when swapping each secondary viewport.
    VSync,
    /// Request adaptive VSync (`-1`) where the SDL video driver supports it.
    Adaptive,
    /// Read and copy the main OpenGL context's current swap interval at viewport creation.
    MatchMain,
}

impl Sdl3OpenGlViewportSwapInterval {
    pub(crate) fn native_policy(self) -> (u32, i32) {
        match self {
            Self::Immediate => (0, 0),
            Self::VSync => (0, 1),
            Self::Adaptive => (0, -1),
            Self::MatchMain => (1, 0),
        }
    }
}

/// Errors reported by the SDL3 platform and optional renderer runtimes.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Sdl3BackendError {
    #[error("ImGui_ImplSDL3_InitForOpenGL returned false")]
    Sdl3InitFailed,
    #[error("ImGui_ImplOpenGL3_Init returned false")]
    OpenGlInitFailed,
    #[error("Invalid GLSL version string")]
    InvalidGlslVersion,
    #[error("ImGui_ImplSDLRenderer3_Init returned false")]
    Renderer3InitFailed,
    #[error("ImGui_ImplSDLGPU3_Init returned false")]
    Gpu3InitFailed,
    #[error("SDL3 runtime belongs to Context {expected:?}, but received Context {actual:?}")]
    ContextMismatch {
        expected: dear_imgui_rs::ContextId,
        actual: dear_imgui_rs::ContextId,
    },
    #[error("another SDL3 platform runtime already owns the process-wide SDL session")]
    PlatformSessionOccupied,
    #[error("SDL3 text input contains an interior NUL byte")]
    TextInputContainsNul,
    #[error(transparent)]
    Attachment(#[from] dear_imgui_rs::ContextAttachmentError),
    #[error(transparent)]
    PlatformAttachmentRelease(#[from] dear_imgui_rs::ContextPlatformAttachmentReleaseError),
    #[error(transparent)]
    Context(#[from] dear_imgui_rs::ContextBindingError),
    #[error("another platform backend already owns `{callback}`")]
    PlatformCallbackOccupied { callback: &'static str },
    #[error("another platform backend already owns `{field}`")]
    PlatformStateOccupied { field: &'static str },
    #[error("another platform backend already owns SDL3-reserved capability bits {flags:#x}")]
    PlatformCapabilityOccupied { flags: i32 },
    #[error("another renderer backend already owns `{callback}`")]
    RendererCallbackOccupied { callback: &'static str },
    #[error("another renderer backend already owns `{field}`")]
    RendererStateOccupied { field: &'static str },
    #[error("another renderer backend already owns SDL3-reserved capability bits {flags:#x}")]
    RendererCapabilityOccupied { flags: i32 },
    #[error("SDL3 platform callback `{callback}` was replaced while the runtime was attached")]
    PlatformCallbackReplaced { callback: &'static str },
    #[error("SDL3-owned platform state `{field}` was replaced while the runtime was attached")]
    PlatformStateReplaced { field: &'static str },
    #[error("SDL3 renderer callback `{callback}` was replaced while the runtime was attached")]
    RendererCallbackReplaced { callback: &'static str },
    #[error("SDL3-owned renderer state `{field}` was replaced while the runtime was attached")]
    RendererStateReplaced { field: &'static str },
    #[error("SDL3 platform callback `{callback}` panicked")]
    PlatformCallbackPanicked { callback: &'static str },
    #[error("another platform backend already owns BackendPlatformUserData")]
    PlatformBackendOccupied,
    #[error("viewport PlatformUserData was replaced by another platform backend")]
    ForeignPlatformUserData,
    #[error("SDL3 failed to create a secondary viewport window")]
    ViewportCreationFailed,
    #[error("SDL3 failed to capture the OpenGL state required for viewport creation")]
    ViewportOpenGlStateCaptureFailed,
    #[error("SDL3 failed to create or activate a distinct OpenGL context for a secondary viewport")]
    ViewportOpenGlContextFailed,
    #[error(
        "SDL3 failed to maintain the OpenGL swap-interval transaction for a secondary viewport"
    )]
    ViewportOpenGlSwapIntervalFailed,
    #[error("SDL3 failed to restore the previous OpenGL window, context, or share attribute")]
    ViewportOpenGlStateRestoreFailed,
    #[error("SDL3 failed to activate the OpenGL context required to render a secondary viewport")]
    ViewportOpenGlRenderContextFailed,
    #[error("SDL3 failed to activate or swap a secondary viewport OpenGL window")]
    ViewportOpenGlSwapFailed,
    #[error("SDL3 failed to claim a secondary viewport window for the SDL GPU device")]
    ViewportSdlGpuClaimFailed,
    #[error("SDL3 failed to configure a secondary viewport SDL GPU swapchain")]
    ViewportSdlGpuConfigureFailed,
    #[error("SDL3 failed to acquire a command buffer for a secondary viewport")]
    ViewportSdlGpuCommandBufferFailed,
    #[error("SDL3 failed to acquire a swapchain texture for a secondary viewport")]
    ViewportSdlGpuSwapchainFailed,
    #[error("SDL3 failed to begin a render pass for a secondary viewport")]
    ViewportSdlGpuRenderPassFailed,
    #[error("SDL3 failed to submit a command buffer for a secondary viewport")]
    ViewportSdlGpuSubmitFailed,
    #[error("SDLRenderer3 received a WindowCanvas other than the renderer used at initialization")]
    RendererMismatch,
    #[error("the SDL3 native callback bridge observed a reentrant or unbalanced transaction")]
    NativeBridgeProtocolFailed,
    #[error("Dear ImGui platform state is unavailable")]
    PlatformStateUnavailable,
    #[error("the SDL3 runtime is no longer attached")]
    RuntimeDetached,
    #[cfg(feature = "multi-viewport")]
    #[error("the SDL3 platform backend was not initialized with init_for_vulkan")]
    VulkanSurfaceProviderRequiresVulkan,
    #[cfg(feature = "multi-viewport")]
    #[error("the SDL3 Vulkan surface provider is already leased by a renderer")]
    VulkanSurfaceProviderAlreadyLeased,
    #[cfg(feature = "multi-viewport")]
    #[error("the SDL3 Vulkan platform runtime has no Platform_CreateVkSurface callback")]
    VulkanSurfaceCallbackUnavailable,
    #[cfg(feature = "multi-viewport")]
    #[error(
        "the SDL3 platform backend cannot shut down while its Vulkan surface provider is leased"
    )]
    VulkanSurfaceProviderActive,
    #[error("SDL3 shutdown panicked while releasing {phase}")]
    ShutdownPanicked { phase: &'static str },
    #[error("SDL3 shutdown is already releasing {phase}")]
    ShutdownInProgress { phase: &'static str },
    #[error(transparent)]
    RendererConsumer(#[from] dear_imgui_rs::render::RendererConsumerError),
    #[error(transparent)]
    TextureFeedback(#[from] dear_imgui_rs::render::TextureFeedbackError),
    #[error("managed texture {texture:?} received an update before renderer creation")]
    ManagedTextureNotCreated {
        texture: dear_imgui_rs::render::SnapshotTextureId,
    },
    #[error("managed texture {texture:?} request is invalid: {reason}")]
    InvalidTextureRequest {
        texture: dear_imgui_rs::render::SnapshotTextureId,
        reason: &'static str,
    },
    #[error("managed texture {texture:?} uses unsupported format {format:?}")]
    UnsupportedTextureFormat {
        texture: dear_imgui_rs::render::SnapshotTextureId,
        format: dear_imgui_rs::TextureFormat,
    },
    #[error("official SDL3 renderer failed to {operation} managed texture {texture:?}")]
    TextureOperationFailed {
        texture: dear_imgui_rs::render::SnapshotTextureId,
        operation: &'static str,
    },
}

/// Failure to create a Vulkan surface through an SDL3 platform-owner lease.
#[cfg(feature = "multi-viewport")]
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Sdl3VulkanSurfaceError {
    /// The provider no longer identifies the active SDL3 platform runtime.
    #[error("the SDL3 Vulkan surface provider no longer owns the active platform runtime")]
    OwnerUnavailable,
    /// The SDL3 runtime rejected entry or observed platform-contract drift.
    #[error(transparent)]
    Backend(#[from] Sdl3BackendError),
    /// The Vulkan platform callback is absent from the validated SDL3 callback table.
    #[error("the SDL3 platform runtime has no Platform_CreateVkSurface callback")]
    CallbackUnavailable,
    /// SDL3 failed to create a non-null surface.
    #[error("Platform_CreateVkSurface failed with code {code} and surface 0x{surface:X}")]
    CallbackFailed { code: i32, surface: u64 },
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
    info: crate::viewport::SdlGpu3InitInfo<'_>,
) -> Result<(), Sdl3BackendError> {
    let mut init_info = ffi::ImGuiImplSdlGpu3InitInfo {
        device: info.device.raw(),
        color_target_format: SDL_GPUTextureFormat(info.color_target_format as i32),
        msaa_samples: SDL_GPUSampleCount(info.msaa_samples as i32),
        swapchain_composition: SDL_GPUSwapchainComposition(info.swapchain_composition as i32),
        present_mode: SDL_GPUPresentMode(info.present_mode as i32),
    };
    unsafe {
        if !ffi::dear_imgui_sdl3_backend_sdlgpu3_init(&mut init_info) {
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
pub(super) fn shutdown_opengl3_renderer_impl() {
    unsafe {
        opengl3_backend::dear_imgui_backend_opengl3_shutdown();
    }
}

#[cfg(feature = "sdlgpu3-renderer")]
pub(super) fn shutdown_sdlgpu3_renderer_impl() {
    unsafe {
        ffi::dear_imgui_sdl3_backend_sdlgpu3_shutdown();
    }
}

#[cfg(feature = "sdlrenderer3-renderer")]
pub(super) fn shutdown_sdlrenderer3_renderer_impl() {
    unsafe {
        ffi::dear_imgui_sdl3_backend_sdlrenderer3_shutdown();
    }
}

#[cfg(feature = "sdlgpu3-renderer")]
pub(super) fn new_frame_sdlgpu3_impl() {
    unsafe {
        ffi::dear_imgui_sdl3_backend_sdlgpu3_new_frame();
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
        ffi::dear_imgui_sdl3_backend_sdlrenderer3_new_frame();
        ffi::ImGui_ImplSDL3_NewFrame_Rust();
    }
}
