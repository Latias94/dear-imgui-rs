// Multi-viewport support (SDL3 platform backend + Ash renderer)
//
// This mirrors the winit multi-viewport renderer callbacks, but creates per-viewport Vulkan
// surfaces by calling the platform backend's `ImGuiPlatformIO::Platform_CreateVkSurface`
// callback (set by `imgui_impl_sdl3.cpp` when initialized for Vulkan).

mod callbacks;
mod frame_sync;
mod registry;
mod surface;
mod swapchain;
#[cfg(test)]
mod tests;

use super::*;

use ash::vk::Handle;
use ash::{
    khr::{surface as khr_surface, swapchain as khr_swapchain},
    vk,
};
use dear_imgui_rs::Context;
use dear_imgui_rs::internal::RawCast;
use dear_imgui_rs::platform_io::Viewport;
use dear_imgui_rs::sys;
use std::ffi::c_void;
use std::sync::Mutex;

type PlatformCreateVkSurfaceFn = unsafe extern "C" fn(
    vp: *mut sys::ImGuiViewport,
    vk_inst: sys::ImU64,
    vk_allocators: *const c_void,
    out_vk_surface: *mut sys::ImU64,
) -> std::os::raw::c_int;

pub use self::callbacks::{
    platform_render_window_sys, platform_swap_buffers_sys, renderer_create_window,
    renderer_destroy_window, renderer_render_window, renderer_set_window_size,
    renderer_swap_buffers,
};
use self::frame_sync::{FrameSync, create_command_pool, create_frame_syncs};
pub(crate) use self::registry::clear_for_drop;
use self::registry::{
    GlobalHandles, borrow_renderer, global_handles, register_viewport_data, take_viewport_data,
    viewport_user_data_mut,
};
pub use self::registry::{disable, enable, shutdown_multi_viewport_support};
#[cfg(test)]
use self::registry::{remove_renderer_state_for_context, upsert_renderer_state};
use self::surface::ViewportAshData;
#[cfg(feature = "dynamic-rendering")]
use self::swapchain::transition_swapchain_image;
use self::swapchain::{
    extent_from_imvec2, extent_from_viewport, pick_present_mode, pick_surface_format,
    recreate_swapchain,
};
