//! SDL3 surface adapter for the shared Vulkan multi-viewport runtime.

use super::{
    AshRenderer,
    vulkan_viewport::{self, SurfaceAdapter, SurfaceCreateError},
};
use ash::vk::{self, Handle};
use dear_imgui_rs::{Context, platform_io::Viewport, sys};
use std::{ffi::c_void, sync::Arc};

type PlatformCreateVkSurfaceFn = unsafe extern "C" fn(
    vp: *mut sys::ImGuiViewport,
    vk_inst: sys::ImU64,
    vk_allocators: *const c_void,
    out_vk_surface: *mut sys::ImU64,
) -> std::os::raw::c_int;

pub use super::vulkan_viewport::{
    CallbackOwnershipError, SurfaceSupportError, VulkanViewportConfig,
    shutdown_multi_viewport_support,
};

struct Sdl3SurfaceAdapter {
    create_surface: PlatformCreateVkSurfaceFn,
}

fn platform_create_surface_callback(
    imgui_context: &Context,
) -> Result<PlatformCreateVkSurfaceFn, CallbackOwnershipError> {
    imgui_context
        .platform_io()
        .platform_create_vk_surface_raw()
        .ok_or(CallbackOwnershipError::PlatformCreateVkSurfaceUnavailable)
}

impl SurfaceAdapter for Sdl3SurfaceAdapter {
    unsafe fn create_surface(
        &self,
        _entry: &ash::Entry,
        instance: &ash::Instance,
        viewport: &mut Viewport,
    ) -> Result<vk::SurfaceKHR, SurfaceCreateError> {
        let mut out_surface: sys::ImU64 = 0;
        let code = unsafe {
            (self.create_surface)(
                viewport.as_raw_mut(),
                instance.handle().as_raw(),
                std::ptr::null(),
                &mut out_surface,
            )
        };
        if code != 0 || out_surface == 0 {
            return Err(SurfaceCreateError::PlatformCallbackFailed {
                code,
                surface: out_surface,
            });
        }
        Ok(vk::SurfaceKHR::from_raw(out_surface))
    }
}

/// Enable Vulkan multi-viewport rendering for an SDL3 platform backend.
///
/// # Safety
///
/// `renderer` must remain at a stable address until [`shutdown_multi_viewport_support`] returns.
/// All viewport callbacks and renderer access must be serialized on the enabling thread; no other
/// reference may access, reinitialize, shut down, move, or drop the renderer while a callback is
/// executing. The context must use SDL3's Vulkan platform backend, and its
/// `Platform_CreateVkSurface` callback and platform windows must remain valid until shutdown. Call
/// shutdown before dropping the context, renderer, platform backend/windows, Vulkan device, or
/// instance, and do not replace any viewport's `RendererUserData` while support is active. Every
/// Vulkan handle and queue family in `config` must satisfy [`VulkanViewportConfig`]'s device-lineage
/// contract.
pub unsafe fn enable(
    renderer: &mut AshRenderer,
    imgui_context: &mut Context,
    config: VulkanViewportConfig,
) -> Result<(), CallbackOwnershipError> {
    let create_surface = platform_create_surface_callback(imgui_context)?;

    unsafe {
        vulkan_viewport::enable_with_adapter(
            renderer,
            imgui_context,
            config,
            Arc::new(Sdl3SurfaceAdapter { create_surface }),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_platform_surface_callback_fails_without_claiming_renderer_slots() {
        let _guard = vulkan_viewport::test_context_guard();
        let context = Context::create();

        assert_eq!(
            platform_create_surface_callback(&context),
            Err(CallbackOwnershipError::PlatformCreateVkSurfaceUnavailable)
        );
        assert!(context.platform_io().renderer_callbacks_are_empty());
    }
}
