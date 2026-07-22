//! Shared Vulkan multi-viewport renderer runtime.

mod callbacks;
mod frame_sync;
mod registry;
mod runtime;
mod surface;
mod swapchain;

#[cfg(test)]
mod runtime_contract_tests;
#[cfg(test)]
mod tests;

use super::*;
use ash::{
    khr::{surface as khr_surface, swapchain as khr_swapchain},
    vk,
};
use dear_imgui_rs::{platform_io::Viewport, sys};
#[cfg(test)]
use std::sync::Mutex;

use self::frame_sync::{
    FrameSync, create_command_pool, create_frame_syncs, create_present_semaphores,
    destroy_frame_syncs, destroy_present_semaphores, present_semaphore_for_image,
    replace_frame_sync,
};
pub use self::registry::SurfaceSupportError;
use self::registry::{GlobalHandles, query_surface_support};
pub(crate) use self::runtime::OwningViewportRuntime;
pub(crate) use self::runtime::attach_with_adapter;
pub use self::runtime::{AshViewportAttachError, AshViewportError};
use self::surface::{SwapchainResources, ViewportAshData, ViewportRuntimeState};

pub(super) fn first_renderer_callback_drift(
    platform_io: &dear_imgui_rs::platform_io::PlatformIo,
) -> Option<&'static str> {
    callbacks::first_renderer_callback_drift(platform_io)
}
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
pub enum SurfaceCreateError {
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

/// Policy used to select a complete surface format and color-space pair for secondary viewports.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SurfaceFormatPolicy {
    /// Select an 8-bit sRGB format paired with `SRGB_NONLINEAR`.
    AutoSrgb,
    /// Require the exact pair used by the application's main swapchain.
    Exact(vk::SurfaceFormatKHR),
}

/// Presentation timing policy for secondary viewport swapchains.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PresentModePolicy {
    /// Prefer `FIFO_RELAXED`, then the universally supported `FIFO` mode.
    AutoVsync,
    /// Prefer `IMMEDIATE`, then `MAILBOX`, with `FIFO` as the portable fallback.
    AutoNoVsync,
    /// Require one exact Vulkan present mode on every secondary surface.
    Exact(vk::PresentModeKHR),
}

/// Surface format and presentation policy shared by every secondary viewport swapchain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ViewportSwapchainPolicy {
    /// Complete surface format and color-space selection policy.
    pub surface_format: SurfaceFormatPolicy,
    /// Presentation timing selection policy.
    pub present_mode: PresentModePolicy,
}

impl ViewportSwapchainPolicy {
    /// Copy the main swapchain's exact surface pair and infer its VSync intent.
    ///
    /// `FIFO` and `FIFO_RELAXED` preserve a VSync policy across heterogeneous surfaces. Other
    /// main-window modes preserve a no-VSync policy while still permitting a portable fallback.
    pub fn from_main_surface(
        surface_format: vk::SurfaceFormatKHR,
        present_mode: vk::PresentModeKHR,
    ) -> Self {
        let present_mode = if present_mode == vk::PresentModeKHR::FIFO
            || present_mode == vk::PresentModeKHR::FIFO_RELAXED
        {
            PresentModePolicy::AutoVsync
        } else {
            PresentModePolicy::AutoNoVsync
        };
        Self {
            surface_format: SurfaceFormatPolicy::Exact(surface_format),
            present_mode,
        }
    }

    /// Copy the main swapchain's exact surface pair and present mode.
    pub const fn exact_from_main_surface(
        surface_format: vk::SurfaceFormatKHR,
        present_mode: vk::PresentModeKHR,
    ) -> Self {
        Self {
            surface_format: SurfaceFormatPolicy::Exact(surface_format),
            present_mode: PresentModePolicy::Exact(present_mode),
        }
    }
}

impl Default for ViewportSwapchainPolicy {
    fn default() -> Self {
        Self {
            surface_format: SurfaceFormatPolicy::AutoSrgb,
            present_mode: PresentModePolicy::AutoNoVsync,
        }
    }
}

/// Vulkan handles and queue selection used by secondary viewport swapchains.
///
/// All handles must share the renderer's Vulkan device lineage: `instance` owns
/// `physical_device` and `validation_surface`; `AshRenderer`'s logical device was created from
/// that pair with `VK_KHR_swapchain` enabled; `present_queue` belongs to that logical device and
/// `present_queue_family_index`; and the renderer's graphics queue belongs to
/// `graphics_queue_family_index`. The platform adapter's unsafe `attach` function cannot validate
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
    /// Surface pair and presentation policy used by every secondary viewport swapchain.
    pub swapchain_policy: ViewportSwapchainPolicy,
}

#[cfg(test)]
pub(crate) fn test_context_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: std::sync::OnceLock<Mutex<()>> = std::sync::OnceLock::new();
    GUARD
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
}
