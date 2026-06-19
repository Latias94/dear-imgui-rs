//! SDL3 platform backend bindings for `dear-imgui-rs`.
//!
//! This crate is a thin, opinionated wrapper around the official C++ SDL3
//! platform backend (`imgui_impl_sdl3.cpp`). When the `opengl3-renderer`,
//! `sdlrenderer3-renderer`, or `sdlgpu3-renderer` features are enabled, the
//! renderer path uses the matching shared backend shim exported by
//! `dear-imgui-sys`.
//!
//! The intent is to provide a simple, safe-ish API that:
//! - plugs into an existing `dear-imgui-rs::Context`
//! - integrates with an SDL3 window and OpenGL context
//! - supports Dear ImGui multi-viewport via the official backend behavior.
//!
//! By default, this crate builds the SDL3 platform backend only. Enable
//! `opengl3-renderer`, `sdlrenderer3-renderer`, or `sdlgpu3-renderer` to pair
//! it with the matching official renderer shim.

mod backend;
mod clipboard;
mod core;
mod cursors;
mod events;
mod gamepad;
#[cfg(test)]
mod tests;
mod viewport;

#[cfg(feature = "opengl3-renderer")]
use std::ffi::CString;
use std::ffi::c_void;

use dear_imgui_rs::{Context, ContextAliveToken};
#[cfg(any(
    feature = "opengl3-renderer",
    feature = "sdlrenderer3-renderer",
    feature = "sdlgpu3-renderer"
))]
use dear_imgui_rs::{TextureData, render::DrawData};
use dear_imgui_sys as sys;
#[cfg(feature = "opengl3-renderer")]
use dear_imgui_sys::backend_shim::opengl3 as opengl3_backend;
#[cfg(feature = "sdlgpu3-renderer")]
use dear_imgui_sys::backend_shim::sdlgpu3 as sdlgpu3_backend;
#[cfg(feature = "sdlrenderer3-renderer")]
use dear_imgui_sys::backend_shim::sdlrenderer3 as sdlrenderer3_backend;
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

pub use self::backend::Sdl3PlatformBackend;
#[cfg(feature = "opengl3-renderer")]
pub use self::backend::{
    Sdl3OpenGl3Backend, create_device_objects, destroy_device_objects, render, update_texture,
};
#[cfg(feature = "sdlrenderer3-renderer")]
pub use self::backend::{
    Sdl3RendererBackend, canvas_create_device_objects, canvas_destroy_device_objects,
    canvas_render, canvas_update_texture,
};
#[cfg(feature = "sdlgpu3-renderer")]
pub use self::backend::{
    SdlGpu3RendererBackend, create_gpu3_device_objects, destroy_gpu3_device_objects,
    update_gp3_texture, update_gpu3_texture,
};
pub use self::core::Sdl3BackendError;
pub use self::events::{process_sys_event, process_sys_event_for_context, sdl3_poll_event_ll};
pub use self::gamepad::{
    GamepadMode, set_gamepad_mode, set_gamepad_mode_for_context, set_gamepad_mode_manual,
    set_gamepad_mode_manual_for_context,
};
#[cfg(feature = "sdlgpu3-renderer")]
pub use self::viewport::{SdlGpu3InitInfo, init_for_sdlgpu3, init_for_sdlgpu3_default};
#[cfg(feature = "sdlrenderer3-renderer")]
pub use self::viewport::{canvas_new_frame, init_for_canvas, shutdown_for_canvas};
pub use self::viewport::{
    enable_native_ime_ui, init_for_d3d, init_for_metal, init_for_other, init_for_sdl_gpu,
    init_for_sdl_gpu as init_for_platform_sdl_gpu, init_for_sdl_renderer, init_for_vulkan,
    init_platform_for_opengl, sdl3_new_frame, shutdown,
};
#[cfg(feature = "opengl3-renderer")]
pub use self::viewport::{
    init_for_opengl, init_for_opengl_default, new_frame, shutdown_for_opengl,
};

use self::core::{ContextBinding, ffi, sdl3_new_frame_impl, shutdown_platform_impl, with_context};
#[cfg(feature = "opengl3-renderer")]
use self::core::{init_opengl3_impl, new_frame_opengl3_impl, shutdown_opengl3_impl};
#[cfg(feature = "sdlrenderer3-renderer")]
use self::core::{new_frame_sdlrenderer3_impl, shutdown_sdlrenderer3_impl};

#[cfg(feature = "sdlgpu3-renderer")]
use self::core::{init_sdlgpu3_impl, new_frame_sdlgpu3_impl, shutdown_sdlgpu3_impl};
