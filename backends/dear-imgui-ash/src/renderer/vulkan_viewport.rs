//! Shared Vulkan multi-viewport renderer runtime.

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
use dear_imgui_rs::{Context, internal::RawCast, platform_io::Viewport, sys};
use std::{ffi::c_void, sync::Mutex};

pub(crate) use self::callbacks::{
    renderer_create_window_sys, renderer_destroy_window_sys, renderer_render_window_sys,
    renderer_set_window_size_sys, renderer_swap_buffers_sys,
};
#[cfg(test)]
use self::callbacks::{renderer_render_window, request_platform_close_after_create_failure};
use self::frame_sync::{
    FrameSync, create_command_pool, create_frame_syncs, create_present_semaphores,
    destroy_frame_syncs, destroy_present_semaphores, present_semaphore_for_image,
    replace_frame_sync,
};
pub use self::registry::{
    CallbackOwnershipError, SurfaceSupportError, shutdown_multi_viewport_support,
};
use self::registry::{
    GlobalHandles, borrow_renderer, global_handles, query_surface_support, register_viewport_data,
    take_viewport_data, viewport_user_data_mut,
};
pub(crate) use self::registry::{clear_for_drop, enable_with_adapter};
#[cfg(test)]
use self::registry::{
    insert_renderer_state, is_ash_viewport_data, remove_renderer_state_for_context,
    unregister_viewport_data,
};
use self::surface::{SwapchainResources, ViewportAshData, ViewportRuntimeState};
#[cfg(feature = "dynamic-rendering")]
use self::swapchain::transition_swapchain_image;
use self::swapchain::{
    desired_extent_from_imvec2, desired_extent_from_viewport, recreate_swapchain,
    recreate_swapchain_after_device_idle,
};

pub(crate) trait SurfaceAdapter: Send + Sync {
    /// Create a Vulkan surface for one platform-owned viewport window.
    ///
    /// # Safety
    ///
    /// `viewport` must belong to the current Dear ImGui context and its platform window must
    /// remain alive until the returned surface is destroyed.
    unsafe fn create_surface(
        &self,
        entry: &ash::Entry,
        instance: &ash::Instance,
        viewport: &mut Viewport,
    ) -> Result<vk::SurfaceKHR, SurfaceCreateError>;
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum SurfaceCreateError {
    #[cfg(feature = "multi-viewport-winit")]
    #[error("the viewport has no platform window handle")]
    MissingPlatformHandle,
    #[cfg(feature = "multi-viewport-winit")]
    #[error("the platform display handle is unavailable")]
    DisplayHandleUnavailable,
    #[cfg(feature = "multi-viewport-winit")]
    #[error("the platform window handle is unavailable")]
    WindowHandleUnavailable,
    #[cfg(feature = "multi-viewport-winit")]
    #[error("Vulkan surface creation failed: {0}")]
    Vulkan(#[from] vk::Result),
    #[cfg(feature = "multi-viewport-sdl3")]
    #[error("Platform_CreateVkSurface failed with code {code} and surface 0x{surface:X}")]
    PlatformCallbackFailed { code: i32, surface: u64 },
}

/// Vulkan handles and queue selection used by secondary viewport swapchains.
///
/// All handles must share the renderer's Vulkan device lineage: `instance` owns
/// `physical_device` and `validation_surface`; `AshRenderer`'s logical device was created from
/// that pair with `VK_KHR_swapchain` enabled; `present_queue` belongs to that logical device and
/// `present_queue_family_index`; and the renderer's graphics queue belongs to
/// `graphics_queue_family_index`. The platform adapter's unsafe `enable` function cannot validate
/// those raw handle relationships.
#[derive(Clone)]
pub struct VulkanViewportConfig {
    /// Vulkan entry used to create platform surfaces.
    pub entry: ash::Entry,
    /// Vulkan instance that owns all secondary viewport surfaces.
    pub instance: ash::Instance,
    /// Physical device used to query secondary viewport surface support.
    pub physical_device: vk::PhysicalDevice,
    /// Existing application-owned surface used to validate presentation support before callbacks
    /// are installed. The runtime never destroys this surface.
    pub validation_surface: vk::SurfaceKHR,
    /// Queue used to present secondary viewport swapchains.
    pub present_queue: vk::Queue,
    /// Queue family used by the renderer's graphics queue.
    pub graphics_queue_family_index: u32,
    /// Queue family used by `present_queue`.
    pub present_queue_family_index: u32,
}

#[cfg(test)]
pub(crate) fn test_context_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: std::sync::OnceLock<Mutex<()>> = std::sync::OnceLock::new();
    GUARD
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
}
