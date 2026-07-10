//! Winit surface adapter for the shared Vulkan multi-viewport runtime.

use super::{
    AshRenderer,
    vulkan_viewport::{self, SurfaceAdapter, SurfaceCreateError},
};
use ash::vk;
use dear_imgui_rs::{Context, platform_io::Viewport};
use std::sync::Arc;
use winit::raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use winit::window::Window;

pub use super::vulkan_viewport::{
    CallbackOwnershipError, SurfaceSupportError, VulkanViewportConfig,
    shutdown_multi_viewport_support,
};

struct WinitSurfaceAdapter;

impl SurfaceAdapter for WinitSurfaceAdapter {
    unsafe fn create_surface(
        &self,
        entry: &ash::Entry,
        instance: &ash::Instance,
        viewport: &mut Viewport,
    ) -> Result<vk::SurfaceKHR, SurfaceCreateError> {
        let window_ptr = viewport.platform_handle();
        if window_ptr.is_null() {
            return Err(SurfaceCreateError::MissingPlatformHandle);
        }
        let window = unsafe { &*(window_ptr as *const Window) };
        let display = window
            .display_handle()
            .map_err(|_| SurfaceCreateError::DisplayHandleUnavailable)?;
        let window_handle = window
            .window_handle()
            .map_err(|_| SurfaceCreateError::WindowHandleUnavailable)?;

        unsafe {
            ash_window::create_surface(
                entry,
                instance,
                display.as_raw(),
                window_handle.as_raw(),
                None,
            )
            .map_err(Into::into)
        }
    }
}

/// Enable Vulkan multi-viewport rendering for a Winit platform backend.
///
/// # Safety
///
/// `renderer` must remain at a stable address until [`shutdown_multi_viewport_support`] returns.
/// All viewport callbacks and renderer access must be serialized on the enabling thread; no other
/// reference may access, reinitialize, shut down, move, or drop the renderer while a callback is
/// executing. The context must use `dear-imgui-winit`, and every viewport platform handle must
/// remain a live `winit::window::Window` until its renderer destroy callback runs. Call shutdown
/// before dropping the context, renderer, platform backend/windows, Vulkan device, or instance,
/// and do not replace any viewport's `RendererUserData` while support is active. Every Vulkan
/// handle and queue family in `config` must satisfy [`VulkanViewportConfig`]'s device-lineage
/// contract.
pub unsafe fn enable(
    renderer: &mut AshRenderer,
    imgui_context: &mut Context,
    config: VulkanViewportConfig,
) -> Result<(), CallbackOwnershipError> {
    unsafe {
        vulkan_viewport::enable_with_adapter(
            renderer,
            imgui_context,
            config,
            Arc::new(WinitSurfaceAdapter),
        )
    }
}
