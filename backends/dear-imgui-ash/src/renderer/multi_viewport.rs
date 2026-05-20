// Multi-viewport support (Renderer_* callbacks and helpers)

mod callbacks;
mod frame_sync;
mod registry;
mod surface;
mod swapchain;
#[cfg(test)]
mod tests;

use super::*;

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
use winit::raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use winit::window::Window;

pub use self::callbacks::{
    platform_render_window_sys, platform_swap_buffers_sys, renderer_create_window,
    renderer_destroy_window, renderer_render_window, renderer_set_window_size,
    renderer_swap_buffers,
};
use self::frame_sync::{FrameSync, create_command_pool, create_frame_syncs};
pub(crate) use self::registry::clear_for_drop;
use self::registry::{GlobalHandles, borrow_renderer, global_handles, viewport_user_data_mut};
pub use self::registry::{disable, enable, shutdown_multi_viewport_support};
#[cfg(test)]
use self::registry::{remove_renderer_state_for_context, upsert_renderer_state};
use self::surface::ViewportAshData;
#[cfg(feature = "dynamic-rendering")]
use self::swapchain::transition_swapchain_image;
use self::swapchain::{
    extent_from_window, pick_present_mode, pick_surface_format, recreate_swapchain,
};
