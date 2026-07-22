use std::ffi::c_void;

use crate::sys;

use super::super::PlatformIo;

impl PlatformIo {
    /// Get platform create VkSurface callback (raw).
    ///
    /// This optional callback is typically set by platform backends (SDL2/SDL3/GLFW/Win32) to let
    /// Vulkan renderers create a per-viewport `VkSurfaceKHR` without reaching into platform-owned
    /// window handles or user data.
    #[cfg(feature = "multi-viewport")]
    pub fn platform_create_vk_surface_raw(
        &self,
    ) -> Option<
        unsafe extern "C" fn(
            vp: *mut sys::ImGuiViewport,
            vk_inst: sys::ImU64,
            vk_allocators: *const c_void,
            out_vk_surface: *mut sys::ImU64,
        ) -> std::os::raw::c_int,
    > {
        self.inner().Platform_CreateVkSurface
    }

    /// Set platform create VkSurface callback (raw).
    ///
    /// # Safety
    ///
    /// When present, the callback must remain callable until it is replaced or cleared, must not
    /// unwind across the C ABI, and must uphold Dear ImGui's `Platform_CreateVkSurface` contract.
    /// In particular, it must validate the Vulkan instance and allocator pointers supplied by the
    /// renderer and initialize `out_vk_surface` according to its return value. Replacing or
    /// clearing it is valid only after every consumer has stopped invoking it.
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_platform_create_vk_surface_raw(
        &mut self,
        callback: Option<
            unsafe extern "C" fn(
                vp: *mut sys::ImGuiViewport,
                vk_inst: sys::ImU64,
                vk_allocators: *const c_void,
                out_vk_surface: *mut sys::ImU64,
            ) -> std::os::raw::c_int,
        >,
    ) {
        self.inner_mut().Platform_CreateVkSurface = callback;
    }
}
