//! SDL3 platform backend bindings for `dear-imgui-rs`.
//!
//! This crate is a thin, opinionated wrapper around the official C++ SDL3
//! platform backend (`imgui_impl_sdl3.cpp`). When the `opengl3-renderer`,
//! `sdlrenderer3-renderer`, or `sdlgpu3-renderer` features are enabled, this
//! crate compiles the matching official renderer backend and local C shim.
//!
//! The intent is to provide a simple, ownership-aware API that:
//! - plugs into an existing `dear-imgui-rs::Context`
//! - integrates with an SDL3 window and OpenGL context
//! - supports Dear ImGui multi-viewport when the active SDL video driver provides the global
//!   mouse state and capture capabilities required by the official backend.
//!
//! The embedded upstream backend currently advertises native platform viewports on the Windows,
//! Cocoa, X11, DIVE, and VMAN SDL video drivers. Wayland intentionally remains a single native
//! window: docking inside the host window works, but detached Dear ImGui viewports are not created
//! as OS windows because Wayland does not expose the required global pointer model.
//!
//! By default, this crate builds the SDL3 platform backend only. Enable
//! `opengl3-renderer`, `sdlrenderer3-renderer`, or `sdlgpu3-renderer` to pair
//! it with the matching official renderer shim.

#[cfg(doctest)]
mod removed_free_api_contracts {
    /// ```compile_fail
    /// use dear_imgui_sdl3::process_sys_event;
    /// ```
    struct ProcessSysEvent;

    /// ```compile_fail
    /// use dear_imgui_sdl3::process_sys_event_for_context;
    /// ```
    struct ProcessSysEventForContext;

    /// ```compile_fail
    /// use dear_imgui_sdl3::sdl3_poll_event_ll;
    /// ```
    struct Sdl3PollEventLl;

    /// ```compile_fail
    /// use dear_imgui_sdl3::set_gamepad_mode;
    /// ```
    struct SetGamepadMode;

    /// ```compile_fail
    /// use dear_imgui_sdl3::set_gamepad_mode_for_context;
    /// ```
    struct SetGamepadModeForContext;

    /// ```compile_fail
    /// use dear_imgui_sdl3::set_gamepad_mode_manual;
    /// ```
    struct SetGamepadModeManual;

    /// ```compile_fail
    /// use dear_imgui_sdl3::set_gamepad_mode_manual_for_context;
    /// ```
    struct SetGamepadModeManualForContext;
}

mod backend;
mod callback_ownership;
mod clipboard;
mod core;
mod cursors;
mod events;
mod input;
#[cfg(any(
    feature = "opengl3-renderer",
    feature = "sdlrenderer3-renderer",
    feature = "sdlgpu3-renderer"
))]
mod renderer_textures;
mod runtime;
#[cfg(test)]
mod tests;
mod viewport;

#[cfg(feature = "opengl3-renderer")]
use std::ffi::CString;
use std::ffi::c_void;

#[cfg(any(
    feature = "opengl3-renderer",
    feature = "sdlrenderer3-renderer",
    feature = "sdlgpu3-renderer"
))]
use dear_imgui_rs::ContextBinding;
#[cfg(any(
    feature = "opengl3-renderer",
    feature = "sdlrenderer3-renderer",
    feature = "sdlgpu3-renderer"
))]
use dear_imgui_rs::render::{DrawData, PendingFrame, ReconciledFrame};
use dear_imgui_rs::{Context, ContextId};
#[cfg(feature = "opengl3-renderer")]
use dear_imgui_sys::backend_shim::opengl3 as opengl3_backend;
#[cfg(feature = "sdlgpu3-renderer")]
use sdl3::gpu::CommandBuffer;
#[cfg(feature = "sdlgpu3-renderer")]
use sdl3::gpu::Device;
#[cfg(feature = "sdlgpu3-renderer")]
use sdl3::gpu::RenderPass;
#[cfg(feature = "sdlrenderer3-renderer")]
use sdl3::render::WindowCanvas;
use sdl3::video::{GLContext, Window};
use sdl3_sys::events::SDL_Event;
#[cfg(feature = "sdlgpu3-renderer")]
use sdl3_sys::gpu::{
    SDL_GPUCommandBuffer, SDL_GPUDevice, SDL_GPUGraphicsPipeline, SDL_GPUPresentMode,
    SDL_GPURenderPass, SDL_GPUSampleCount, SDL_GPUSwapchainComposition, SDL_GPUTextureFormat,
};
#[cfg(feature = "sdlrenderer3-renderer")]
use sdl3_sys::render::SDL_Renderer;

#[cfg(feature = "opengl3-renderer")]
pub use self::backend::Sdl3OpenGl3Backend;
pub use self::backend::Sdl3PlatformBackend;
#[cfg(feature = "sdlrenderer3-renderer")]
pub use self::backend::Sdl3RendererBackend;
#[cfg(feature = "sdlgpu3-renderer")]
pub use self::backend::{SdlGpu3PreparedFrame, SdlGpu3RendererBackend};
#[cfg(feature = "multi-viewport")]
pub use self::core::Sdl3VulkanSurfaceError;
pub use self::core::{Sdl3BackendError, Sdl3OpenGlViewportSwapInterval};
use self::core::{ffi, sdl3_new_frame_impl, with_context};
#[cfg(feature = "opengl3-renderer")]
use self::core::{init_opengl3_impl, new_frame_opengl3_impl, shutdown_opengl3_renderer_impl};
#[cfg(feature = "sdlrenderer3-renderer")]
use self::core::{new_frame_sdlrenderer3_impl, shutdown_sdlrenderer3_renderer_impl};
use self::events::{process_owned_event, process_raw_sys_event};
pub use self::input::{GamepadMode, MouseCaptureMode};
use self::input::{set_gamepad_mode, set_gamepad_mode_manual, set_mouse_capture_mode};
#[cfg(feature = "multi-viewport")]
pub use self::runtime::Sdl3VulkanSurfaceProvider;
pub use self::runtime::{
    Sdl3OpenGlViewportFrameReport, Sdl3OpenGlViewportFrameTrace, Sdl3OpenGlViewportFrameTraceError,
};
#[cfg(feature = "sdlgpu3-renderer")]
pub use self::viewport::SdlGpu3InitInfo;
pub use self::viewport::enable_native_ime_ui;

#[cfg(feature = "sdlgpu3-renderer")]
use self::core::{init_sdlgpu3_impl, new_frame_sdlgpu3_impl, shutdown_sdlgpu3_renderer_impl};
use self::runtime::{NativeRendererKind, PlatformGraphicsKind, RuntimeRegistration};
#[cfg(feature = "sdlrenderer3-renderer")]
use self::viewport::init_for_canvas;
use self::viewport::{
    init_for_d3d, init_for_metal, init_for_other, init_for_sdl_gpu, init_for_sdl_renderer,
    init_for_vulkan, init_platform_for_opengl,
};
#[cfg(feature = "opengl3-renderer")]
use self::viewport::{init_for_opengl, init_for_opengl_default};
#[cfg(feature = "sdlgpu3-renderer")]
use self::viewport::{init_for_sdlgpu3, init_for_sdlgpu3_default};
