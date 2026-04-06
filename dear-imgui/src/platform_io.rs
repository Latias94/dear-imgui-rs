//! Platform IO functionality for Dear ImGui
//!
//! This module provides access to Dear ImGui's platform IO system, which handles
//! multi-viewport and platform-specific functionality.

// Multi-viewport requires platform callbacks (C calling back into Rust). On the
// web (wasm32 import-style build), this is not supported yet, so we currently
// disable the `multi-viewport` feature for wasm at compile time.
#[cfg(all(target_arch = "wasm32", feature = "multi-viewport"))]
compile_error!("The `multi-viewport` feature is not supported on wasm32 targets yet.");

use crate::sys;
#[cfg(feature = "multi-viewport")]
use core::ffi::c_char;
use std::cell::UnsafeCell;
#[cfg(feature = "multi-viewport")]
use std::ffi::c_void;

/// Platform IO structure for multi-viewport support
///
/// This is a transparent wrapper around `ImGuiPlatformIO` that provides
/// safe access to platform-specific functionality.
#[repr(transparent)]
pub struct PlatformIo {
    raw: UnsafeCell<sys::ImGuiPlatformIO>,
}

// Ensure the wrapper stays layout-compatible with the sys bindings.
const _: [(); std::mem::size_of::<sys::ImGuiPlatformIO>()] =
    [(); std::mem::size_of::<PlatformIo>()];
const _: [(); std::mem::align_of::<sys::ImGuiPlatformIO>()] =
    [(); std::mem::align_of::<PlatformIo>()];

// Typed-callback trampolines (avoid transmute) --------------------------------
#[cfg(feature = "multi-viewport")]
mod trampolines;
mod viewport;

pub use viewport::Viewport;

impl PlatformIo {
    #[inline]
    fn inner(&self) -> &sys::ImGuiPlatformIO {
        // Safety: `PlatformIo` is a view into ImGui-owned platform state which may be mutated by
        // Dear ImGui while Rust holds `&PlatformIo`, so we store it behind `UnsafeCell` to make
        // that interior mutability explicit.
        unsafe { &*self.raw.get() }
    }

    #[inline]
    fn inner_mut(&mut self) -> &mut sys::ImGuiPlatformIO {
        // Safety: caller has `&mut PlatformIo`, so this is a unique Rust borrow for this wrapper.
        unsafe { &mut *self.raw.get() }
    }

    /// Get a reference to the platform IO from a raw pointer
    ///
    /// # Safety
    ///
    /// The caller must ensure that:
    /// - `raw` is non-null and points to a valid `ImGuiPlatformIO`.
    /// - The platform IO outlives the returned reference (e.g. it belongs to the
    ///   currently active ImGui context).
    pub unsafe fn from_raw<'a>(raw: *const sys::ImGuiPlatformIO) -> &'a Self {
        unsafe { &*(raw as *const Self) }
    }

    /// Get a mutable reference to the platform IO from a raw pointer
    ///
    /// # Safety
    ///
    /// The caller must ensure that:
    /// - `raw` is non-null and points to a valid `ImGuiPlatformIO`.
    /// - The platform IO outlives the returned reference (e.g. it belongs to the
    ///   currently active ImGui context).
    /// - No other references (shared or mutable) to the same platform IO are alive.
    pub unsafe fn from_raw_mut<'a>(raw: *mut sys::ImGuiPlatformIO) -> &'a mut Self {
        unsafe { &mut *(raw as *mut Self) }
    }

    /// Get the raw pointer to the underlying `ImGuiPlatformIO`
    pub fn as_raw(&self) -> *const sys::ImGuiPlatformIO {
        self.raw.get().cast_const()
    }

    /// Get the raw mutable pointer to the underlying `ImGuiPlatformIO`
    pub fn as_raw_mut(&mut self) -> *mut sys::ImGuiPlatformIO {
        self.raw.get()
    }

    /// Clear all platform backend handlers.
    ///
    /// This resets the `Platform_*` callback table stored in `ImGuiPlatformIO`.
    #[cfg(feature = "multi-viewport")]
    pub fn clear_platform_handlers(&mut self) {
        unsafe { sys::ImGuiPlatformIO_ClearPlatformHandlers(self.as_raw_mut()) }
    }

    /// Clear all renderer backend handlers.
    ///
    /// This resets the `Renderer_*` callback table stored in `ImGuiPlatformIO`.
    #[cfg(feature = "multi-viewport")]
    pub fn clear_renderer_handlers(&mut self) {
        unsafe { sys::ImGuiPlatformIO_ClearRendererHandlers(self.as_raw_mut()) }
    }

    /// Set platform create window callback (raw sys pointer)
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_create_window_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport)>,
    ) {
        self.inner_mut().Platform_CreateWindow = callback;
    }

    /// Set platform create window callback (typed Viewport).
    ///
    /// # Safety
    ///
    /// If `callback` is `Some`, it must be safe for Dear ImGui to call it through a C ABI:
    /// - It must not unwind across the FFI boundary.
    /// - The `Viewport` pointer is only valid for the duration of the call and must not be stored.
    /// - The callback must uphold Dear ImGui's `Platform_CreateWindow` contract.
    ///
    /// Note: the typed callback is stored in a global slot shared by all ImGui contexts.
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_platform_create_window(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport)>,
    ) {
        use trampolines::*;
        store_cb(&PLATFORM_CREATE_WINDOW_CB, callback);
        self.set_platform_create_window_raw(callback.map(|_| {
            trampolines::platform_create_window as unsafe extern "C" fn(*mut sys::ImGuiViewport)
        }));
    }

    /// Set platform destroy window callback (raw)
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_destroy_window_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport)>,
    ) {
        self.inner_mut().Platform_DestroyWindow = callback;
    }

    /// Set platform destroy window callback (typed Viewport).
    ///
    /// # Safety
    ///
    /// See [`Self::set_platform_create_window`].
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_platform_destroy_window(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport)>,
    ) {
        use trampolines::*;
        store_cb(&PLATFORM_DESTROY_WINDOW_CB, callback);
        self.set_platform_destroy_window_raw(callback.map(|_| {
            trampolines::platform_destroy_window as unsafe extern "C" fn(*mut sys::ImGuiViewport)
        }));
    }

    /// Set platform show window callback (raw)
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_show_window_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport)>,
    ) {
        self.inner_mut().Platform_ShowWindow = callback;
    }

    /// Set platform show window callback (typed Viewport).
    ///
    /// # Safety
    ///
    /// See [`Self::set_platform_create_window`].
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_platform_show_window(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport)>,
    ) {
        use trampolines::*;
        store_cb(&PLATFORM_SHOW_WINDOW_CB, callback);
        self.set_platform_show_window_raw(callback.map(|_| {
            trampolines::platform_show_window as unsafe extern "C" fn(*mut sys::ImGuiViewport)
        }));
    }

    /// Set platform set window position callback (raw)
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_set_window_pos_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, sys::ImVec2)>,
    ) {
        self.inner_mut().Platform_SetWindowPos = callback;
    }

    /// Set platform set window position callback (typed Viewport).
    ///
    /// # Safety
    ///
    /// See [`Self::set_platform_create_window`].
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_platform_set_window_pos(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport, sys::ImVec2)>,
    ) {
        use trampolines::*;
        store_cb(&PLATFORM_SET_WINDOW_POS_CB, callback);
        self.set_platform_set_window_pos_raw(callback.map(|_| {
            trampolines::platform_set_window_pos
                as unsafe extern "C" fn(*mut sys::ImGuiViewport, sys::ImVec2)
        }));
    }

    /// Set platform get window position callback (raw)
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_get_window_pos_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport) -> sys::ImVec2>,
    ) {
        use trampolines::*;

        store_cb(&PLATFORM_GET_WINDOW_POS_RAW_CB, callback);
        store_cb(&PLATFORM_GET_WINDOW_POS_CB, None);

        unsafe {
            match callback {
                Some(_) => sys::ImGuiPlatformIO_Set_Platform_GetWindowPos(
                    self.as_raw_mut(),
                    Some(
                        trampolines::platform_get_window_pos_out
                            as unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut sys::ImVec2),
                    ),
                ),
                None => self.inner_mut().Platform_GetWindowPos = None,
            }
        }
    }

    /// Set platform get window position callback (typed Viewport).
    ///
    /// # Safety
    ///
    /// See [`Self::set_platform_create_window`].
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_platform_get_window_pos(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport) -> sys::ImVec2>,
    ) {
        use trampolines::*;
        store_cb(&PLATFORM_GET_WINDOW_POS_RAW_CB, None);
        store_cb(&PLATFORM_GET_WINDOW_POS_CB, callback);

        unsafe {
            match callback {
                Some(_) => sys::ImGuiPlatformIO_Set_Platform_GetWindowPos(
                    self.as_raw_mut(),
                    Some(
                        trampolines::platform_get_window_pos_out
                            as unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut sys::ImVec2),
                    ),
                ),
                None => self.inner_mut().Platform_GetWindowPos = None,
            }
        }
    }

    /// Set platform set window size callback (raw)
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_set_window_size_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, sys::ImVec2)>,
    ) {
        self.inner_mut().Platform_SetWindowSize = callback;
    }

    /// Set platform set window size callback (typed Viewport).
    ///
    /// # Safety
    ///
    /// See [`Self::set_platform_create_window`].
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_platform_set_window_size(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport, sys::ImVec2)>,
    ) {
        use trampolines::*;
        store_cb(&PLATFORM_SET_WINDOW_SIZE_CB, callback);
        self.set_platform_set_window_size_raw(callback.map(|_| {
            trampolines::platform_set_window_size
                as unsafe extern "C" fn(*mut sys::ImGuiViewport, sys::ImVec2)
        }));
    }

    /// Set platform get window size callback (raw)
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_get_window_size_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport) -> sys::ImVec2>,
    ) {
        use trampolines::*;

        store_cb(&PLATFORM_GET_WINDOW_SIZE_RAW_CB, callback);
        store_cb(&PLATFORM_GET_WINDOW_SIZE_CB, None);

        unsafe {
            match callback {
                Some(_) => sys::ImGuiPlatformIO_Set_Platform_GetWindowSize(
                    self.as_raw_mut(),
                    Some(
                        trampolines::platform_get_window_size_out
                            as unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut sys::ImVec2),
                    ),
                ),
                None => self.inner_mut().Platform_GetWindowSize = None,
            }
        }
    }

    /// Set platform get window size callback (typed Viewport).
    ///
    /// # Safety
    ///
    /// See [`Self::set_platform_create_window`].
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_platform_get_window_size(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport) -> sys::ImVec2>,
    ) {
        use trampolines::*;
        store_cb(&PLATFORM_GET_WINDOW_SIZE_RAW_CB, None);
        store_cb(&PLATFORM_GET_WINDOW_SIZE_CB, callback);

        unsafe {
            match callback {
                Some(_) => sys::ImGuiPlatformIO_Set_Platform_GetWindowSize(
                    self.as_raw_mut(),
                    Some(
                        trampolines::platform_get_window_size_out
                            as unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut sys::ImVec2),
                    ),
                ),
                None => self.inner_mut().Platform_GetWindowSize = None,
            }
        }
    }

    /// Set platform set window focus callback (raw)
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_set_window_focus_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport)>,
    ) {
        self.inner_mut().Platform_SetWindowFocus = callback;
    }

    /// Set platform set window focus callback (typed Viewport).
    ///
    /// # Safety
    ///
    /// See [`Self::set_platform_create_window`].
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_platform_set_window_focus(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport)>,
    ) {
        use trampolines::*;
        store_cb(&PLATFORM_SET_WINDOW_FOCUS_CB, callback);
        self.set_platform_set_window_focus_raw(callback.map(|_| {
            trampolines::platform_set_window_focus as unsafe extern "C" fn(*mut sys::ImGuiViewport)
        }));
    }

    /// Set platform get window DPI scale callback (raw)
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_get_window_dpi_scale_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport) -> f32>,
    ) {
        self.inner_mut().Platform_GetWindowDpiScale = callback;
    }

    /// Set platform get window DPI scale callback (typed Viewport).
    ///
    /// # Safety
    ///
    /// See [`Self::set_platform_create_window`].
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_platform_get_window_dpi_scale(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport) -> f32>,
    ) {
        use trampolines::*;
        store_cb(&PLATFORM_GET_WINDOW_DPI_SCALE_CB, callback);
        self.set_platform_get_window_dpi_scale_raw(callback.map(|_| {
            trampolines::platform_get_window_dpi_scale
                as unsafe extern "C" fn(*mut sys::ImGuiViewport) -> f32
        }));
    }

    /// Set platform get window focus callback (raw)
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_get_window_focus_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport) -> bool>,
    ) {
        self.inner_mut().Platform_GetWindowFocus = callback;
    }

    /// Set platform get window focus callback (typed Viewport).
    ///
    /// # Safety
    ///
    /// See [`Self::set_platform_create_window`].
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_platform_get_window_focus(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport) -> bool>,
    ) {
        use trampolines::*;
        store_cb(&PLATFORM_GET_WINDOW_FOCUS_CB, callback);
        self.set_platform_get_window_focus_raw(callback.map(|_| {
            trampolines::platform_get_window_focus
                as unsafe extern "C" fn(*mut sys::ImGuiViewport) -> bool
        }));
    }

    /// Set platform get window minimized callback (raw)
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_get_window_minimized_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport) -> bool>,
    ) {
        self.inner_mut().Platform_GetWindowMinimized = callback;
    }

    /// Set platform get window minimized callback (typed Viewport).
    ///
    /// # Safety
    ///
    /// See [`Self::set_platform_create_window`].
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_platform_get_window_minimized(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport) -> bool>,
    ) {
        use trampolines::*;
        store_cb(&PLATFORM_GET_WINDOW_MINIMIZED_CB, callback);
        self.set_platform_get_window_minimized_raw(callback.map(|_| {
            trampolines::platform_get_window_minimized
                as unsafe extern "C" fn(*mut sys::ImGuiViewport) -> bool
        }));
    }

    /// Set platform on changed viewport callback (raw)
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_on_changed_viewport_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport)>,
    ) {
        self.inner_mut().Platform_OnChangedViewport = callback;
    }

    /// Set platform on changed viewport callback (typed Viewport).
    ///
    /// # Safety
    ///
    /// See [`Self::set_platform_create_window`].
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_platform_on_changed_viewport(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport)>,
    ) {
        use trampolines::*;
        store_cb(&PLATFORM_ON_CHANGED_VIEWPORT_CB, callback);
        self.set_platform_on_changed_viewport_raw(callback.map(|_| {
            trampolines::platform_on_changed_viewport
                as unsafe extern "C" fn(*mut sys::ImGuiViewport)
        }));
    }

    /// Set platform set window title callback (raw)
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_set_window_title_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, *const c_char)>,
    ) {
        self.inner_mut().Platform_SetWindowTitle = callback;
    }

    /// Set platform set window title callback (typed Viewport).
    ///
    /// # Safety
    ///
    /// See [`Self::set_platform_create_window`].
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_platform_set_window_title(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport, *const c_char)>,
    ) {
        use trampolines::*;
        store_cb(&PLATFORM_SET_WINDOW_TITLE_CB, callback);
        self.set_platform_set_window_title_raw(callback.map(|_| {
            trampolines::platform_set_window_title
                as unsafe extern "C" fn(*mut sys::ImGuiViewport, *const c_char)
        }));
    }

    /// Set platform set window alpha callback (raw)
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_set_window_alpha_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, f32)>,
    ) {
        self.inner_mut().Platform_SetWindowAlpha = callback;
    }

    /// Set platform set window alpha callback (typed Viewport).
    ///
    /// # Safety
    ///
    /// See [`Self::set_platform_create_window`].
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_platform_set_window_alpha(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport, f32)>,
    ) {
        use trampolines::*;
        store_cb(&PLATFORM_SET_WINDOW_ALPHA_CB, callback);
        self.set_platform_set_window_alpha_raw(callback.map(|_| {
            trampolines::platform_set_window_alpha
                as unsafe extern "C" fn(*mut sys::ImGuiViewport, f32)
        }));
    }

    /// Set platform update window callback (raw)
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_update_window_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport)>,
    ) {
        self.inner_mut().Platform_UpdateWindow = callback;
    }

    /// Set platform update window callback (typed Viewport).
    ///
    /// # Safety
    ///
    /// See [`Self::set_platform_create_window`].
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_platform_update_window(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport)>,
    ) {
        use trampolines::*;
        store_cb(&PLATFORM_UPDATE_WINDOW_CB, callback);
        self.set_platform_update_window_raw(callback.map(|_| {
            trampolines::platform_update_window as unsafe extern "C" fn(*mut sys::ImGuiViewport)
        }));
    }

    /// Set platform render window callback (raw)
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_render_window_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void)>,
    ) {
        self.inner_mut().Platform_RenderWindow = callback;
    }

    /// Set platform render window callback (typed Viewport).
    ///
    /// # Safety
    ///
    /// See [`Self::set_platform_create_window`].
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_platform_render_window(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport, *mut c_void)>,
    ) {
        use trampolines::*;
        store_cb(&PLATFORM_RENDER_WINDOW_CB, callback);
        self.set_platform_render_window_raw(callback.map(|_| {
            trampolines::platform_render_window
                as unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void)
        }));
    }

    /// Set platform swap buffers callback (raw)
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_swap_buffers_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void)>,
    ) {
        self.inner_mut().Platform_SwapBuffers = callback;
    }

    /// Set platform swap buffers callback (typed Viewport).
    ///
    /// # Safety
    ///
    /// See [`Self::set_platform_create_window`].
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_platform_swap_buffers(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport, *mut c_void)>,
    ) {
        use trampolines::*;
        store_cb(&PLATFORM_SWAP_BUFFERS_CB, callback);
        self.set_platform_swap_buffers_raw(callback.map(|_| {
            trampolines::platform_swap_buffers
                as unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void)
        }));
    }

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
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_create_vk_surface_raw(
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

    /// Set renderer create window callback (raw)
    #[cfg(feature = "multi-viewport")]
    pub fn set_renderer_create_window_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport)>,
    ) {
        self.inner_mut().Renderer_CreateWindow = callback;
    }

    /// Set renderer create window callback (typed Viewport).
    ///
    /// # Safety
    ///
    /// Same requirements as [`Self::set_platform_create_window`], but for Dear ImGui's
    /// `Renderer_CreateWindow` callback.
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_renderer_create_window(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport)>,
    ) {
        use trampolines::*;
        store_cb(&RENDERER_CREATE_WINDOW_CB, callback);
        self.set_renderer_create_window_raw(callback.map(|_| {
            trampolines::renderer_create_window as unsafe extern "C" fn(*mut sys::ImGuiViewport)
        }));
    }

    /// Set renderer destroy window callback (raw)
    #[cfg(feature = "multi-viewport")]
    pub fn set_renderer_destroy_window_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport)>,
    ) {
        self.inner_mut().Renderer_DestroyWindow = callback;
    }

    /// Set renderer destroy window callback (typed Viewport).
    ///
    /// # Safety
    ///
    /// See [`Self::set_renderer_create_window`].
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_renderer_destroy_window(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport)>,
    ) {
        use trampolines::*;
        store_cb(&RENDERER_DESTROY_WINDOW_CB, callback);
        self.set_renderer_destroy_window_raw(callback.map(|_| {
            trampolines::renderer_destroy_window as unsafe extern "C" fn(*mut sys::ImGuiViewport)
        }));
    }

    /// Set renderer set window size callback (raw)
    #[cfg(feature = "multi-viewport")]
    pub fn set_renderer_set_window_size_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, sys::ImVec2)>,
    ) {
        self.inner_mut().Renderer_SetWindowSize = callback;
    }

    /// Set renderer set window size callback (typed Viewport).
    ///
    /// # Safety
    ///
    /// See [`Self::set_renderer_create_window`].
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_renderer_set_window_size(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport, sys::ImVec2)>,
    ) {
        use trampolines::*;
        store_cb(&RENDERER_SET_WINDOW_SIZE_CB, callback);
        self.set_renderer_set_window_size_raw(callback.map(|_| {
            trampolines::renderer_set_window_size
                as unsafe extern "C" fn(*mut sys::ImGuiViewport, sys::ImVec2)
        }));
    }

    /// Set renderer render window callback (raw)
    #[cfg(feature = "multi-viewport")]
    pub fn set_renderer_render_window_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void)>,
    ) {
        self.inner_mut().Renderer_RenderWindow = callback;
    }

    /// Set renderer render window callback (typed Viewport).
    ///
    /// # Safety
    ///
    /// See [`Self::set_renderer_create_window`].
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_renderer_render_window(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport, *mut c_void)>,
    ) {
        use trampolines::*;
        store_cb(&RENDERER_RENDER_WINDOW_CB, callback);
        self.set_renderer_render_window_raw(callback.map(|_| {
            trampolines::renderer_render_window
                as unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void)
        }));
    }

    /// Set renderer swap buffers callback (raw)
    #[cfg(feature = "multi-viewport")]
    pub fn set_renderer_swap_buffers_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void)>,
    ) {
        self.inner_mut().Renderer_SwapBuffers = callback;
    }

    /// Set renderer swap buffers callback (typed Viewport).
    ///
    /// # Safety
    ///
    /// See [`Self::set_renderer_create_window`].
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_renderer_swap_buffers(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport, *mut c_void)>,
    ) {
        use trampolines::*;
        store_cb(&RENDERER_SWAP_BUFFERS_CB, callback);
        self.set_renderer_swap_buffers_raw(callback.map(|_| {
            trampolines::renderer_swap_buffers
                as unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void)
        }));
    }

    /// Get access to the monitors vector
    #[cfg(feature = "multi-viewport")]
    pub fn monitors(&self) -> &crate::internal::ImVector<sys::ImGuiPlatformMonitor> {
        unsafe {
            crate::internal::imvector_cast_ref::<
                sys::ImGuiPlatformMonitor,
                sys::ImVector_ImGuiPlatformMonitor,
            >(&self.inner().Monitors)
        }
    }

    /// Get mutable access to the monitors vector
    #[cfg(feature = "multi-viewport")]
    pub fn monitors_mut(&mut self) -> &mut crate::internal::ImVector<sys::ImGuiPlatformMonitor> {
        unsafe {
            crate::internal::imvector_cast_mut::<
                sys::ImGuiPlatformMonitor,
                sys::ImVector_ImGuiPlatformMonitor,
            >(&mut self.inner_mut().Monitors)
        }
    }

    /// Get access to the viewports vector
    #[cfg(feature = "multi-viewport")]
    pub(crate) fn viewports(&self) -> &crate::internal::ImVector<*mut sys::ImGuiViewport> {
        unsafe {
            crate::internal::imvector_cast_ref::<
                *mut sys::ImGuiViewport,
                sys::ImVector_ImGuiViewportPtr,
            >(&self.inner().Viewports)
        }
    }

    /// Get mutable access to the viewports vector
    #[cfg(feature = "multi-viewport")]
    pub(crate) fn viewports_mut(
        &mut self,
    ) -> &mut crate::internal::ImVector<*mut sys::ImGuiViewport> {
        unsafe {
            crate::internal::imvector_cast_mut::<
                *mut sys::ImGuiViewport,
                sys::ImVector_ImGuiViewportPtr,
            >(&mut self.inner_mut().Viewports)
        }
    }

    /// Get an iterator over all viewports
    #[cfg(feature = "multi-viewport")]
    pub fn viewports_iter(&self) -> impl Iterator<Item = &Viewport> {
        self.viewports()
            .iter()
            .map(|&ptr| unsafe { Viewport::from_raw(ptr) })
    }

    /// Get a mutable iterator over all viewports
    #[cfg(feature = "multi-viewport")]
    pub fn viewports_iter_mut(&mut self) -> impl Iterator<Item = &mut Viewport> {
        self.viewports_mut()
            .iter_mut()
            .map(|&mut ptr| unsafe { Viewport::from_raw_mut(ptr) })
    }

    /// Get an iterator over all textures managed by the platform
    ///
    /// This is used by renderer backends during shutdown to destroy all textures.
    ///
    /// Note: items returned by this iterator provide a guarded mutable view; do not store them or
    /// hold them across iterations.
    pub fn textures(&self) -> crate::render::draw_data::TextureIterator<'_> {
        unsafe {
            let vector = &self.inner().Textures;
            let size = match usize::try_from(vector.Size) {
                Ok(size) => size,
                Err(_) => 0,
            };
            if size == 0 || vector.Data.is_null() {
                crate::render::draw_data::TextureIterator::new(std::ptr::null(), std::ptr::null())
            } else {
                crate::render::draw_data::TextureIterator::new(vector.Data, vector.Data.add(size))
            }
        }
    }

    /// Get the number of textures managed by the platform
    pub fn textures_count(&self) -> usize {
        let vector = &self.inner().Textures;
        if vector.Data.is_null() {
            return 0;
        }
        usize::try_from(vector.Size).unwrap_or(0)
    }

    /// Apply managed texture feedback produced by a renderer thread.
    ///
    /// In ImGui 1.92+, `DrawData::textures()` is built from `PlatformIO.Textures[]`. Renderer backends
    /// are expected to update each `TextureData`'s `Status` (and `TexID` on creation) after handling
    /// `WantCreate`/`WantUpdates`/`WantDestroy` requests.
    ///
    /// In a threaded engine, the renderer thread cannot safely mutate ImGui state directly. The
    /// intended flow is:
    /// - UI thread: snapshot texture requests and send to renderer
    /// - Render thread: create/update/destroy GPU resources and produce `TextureFeedback`
    /// - UI thread: call this function before the next frame to apply the feedback
    ///
    /// Returns the number of textures updated.
    pub fn apply_texture_feedback(
        &mut self,
        feedback: &[crate::render::snapshot::TextureFeedback],
    ) -> usize {
        if feedback.is_empty() {
            return 0;
        }

        let mut by_id: std::collections::HashMap<i32, crate::render::snapshot::TextureFeedback> =
            std::collections::HashMap::with_capacity(feedback.len());
        for &fb in feedback {
            by_id.insert(fb.id.0, fb);
        }

        let mut applied = 0usize;
        for mut tex in self.textures() {
            let uid = tex.unique_id();
            let Some(fb) = by_id.get(&uid) else { continue };

            // Destroyed clears backend bindings (TexID/BackendUserData) as expected by ImGui.
            if fb.status == crate::texture::TextureStatus::Destroyed {
                tex.set_status(fb.status);
            } else {
                if let Some(tex_id) = fb.tex_id {
                    tex.set_tex_id(tex_id);
                }
                tex.set_status(fb.status);
            }

            applied += 1;
        }

        applied
    }

    /// Get a specific texture by index
    ///
    /// Returns None if the index is out of bounds.
    pub fn texture(&self, index: usize) -> Option<&crate::texture::TextureData> {
        unsafe {
            crate::render::draw_data::assert_texture_data_not_borrowed();
            let vector = &self.inner().Textures;
            let size = usize::try_from(vector.Size).ok()?;
            if size == 0 || vector.Data.is_null() {
                return None;
            }
            if index >= size {
                return None;
            }
            let texture_ptr = *vector.Data.add(index);
            if texture_ptr.is_null() {
                return None;
            }
            Some(crate::texture::TextureData::from_raw_ref(
                texture_ptr as *const _,
            ))
        }
    }

    /// Get a mutable reference to a specific texture by index
    ///
    /// Returns None if the index is out of bounds.
    pub fn texture_mut(&mut self, index: usize) -> Option<&mut crate::texture::TextureData> {
        unsafe {
            let vector = &self.inner().Textures;
            let size = usize::try_from(vector.Size).ok()?;
            if size == 0 || vector.Data.is_null() {
                return None;
            }
            if index >= size {
                return None;
            }
            let texture_ptr = *vector.Data.add(index);
            if texture_ptr.is_null() {
                return None;
            }
            Some(crate::texture::TextureData::from_raw(texture_ptr))
        }
    }

    /// Set the renderer render state
    ///
    /// This is used by renderer backends to expose their current render state
    /// to draw callbacks during rendering. The pointer should remain valid
    /// during the entire render_draw_data() call.
    ///
    /// # Safety
    ///
    /// The caller must ensure that:
    /// - The pointer is valid for the duration of the render call
    /// - The pointed-to data matches the expected render state structure for the backend
    /// - The pointer is set to null after rendering is complete
    pub unsafe fn set_renderer_render_state(&mut self, render_state: *mut std::ffi::c_void) {
        self.inner_mut().Renderer_RenderState = render_state;
    }

    /// Get the current renderer render state
    ///
    /// Returns the render state pointer that was set by the renderer backend.
    /// This is typically used by draw callbacks to access the current render state.
    ///
    /// # Safety
    ///
    /// The caller must ensure that:
    /// - The returned pointer is cast to the correct render state type for the backend
    /// - The pointer is only used during the render_draw_data() call
    pub unsafe fn renderer_render_state(&self) -> *mut std::ffi::c_void {
        self.inner().Renderer_RenderState
    }
}

// TODO: Add safe wrappers for platform IO functionality:
// - Viewport management
// - Platform backend callbacks
// - Renderer backend callbacks
// - Monitor information
// - Platform-specific settings

#[cfg(test)]
mod tests {
    use super::*;

    fn new_platform_io() -> sys::ImGuiPlatformIO {
        unsafe {
            let p = sys::ImGuiPlatformIO_ImGuiPlatformIO();
            assert!(
                !p.is_null(),
                "ImGuiPlatformIO_ImGuiPlatformIO() returned null"
            );
            let v = *p;
            sys::ImGuiPlatformIO_destroy(p);
            v
        }
    }

    #[test]
    fn platform_io_textures_empty_is_safe() {
        let mut raw: sys::ImGuiPlatformIO = new_platform_io();

        raw.Textures.Size = 0;
        raw.Textures.Data = std::ptr::null_mut();
        let pio = PlatformIo {
            raw: UnsafeCell::new(raw),
        };
        assert_eq!(pio.textures().count(), 0);
        assert_eq!(pio.textures_count(), 0);

        let mut raw: sys::ImGuiPlatformIO = new_platform_io();
        raw.Textures.Size = 1;
        raw.Textures.Data = std::ptr::null_mut();
        let pio = PlatformIo {
            raw: UnsafeCell::new(raw),
        };
        assert_eq!(pio.textures().count(), 0);
        assert_eq!(pio.textures_count(), 0);
        assert!(pio.texture(0).is_none());
    }

    #[test]
    fn platform_io_from_raw_matches_mut_wrapper() {
        let mut raw: sys::ImGuiPlatformIO = new_platform_io();
        let raw_ptr = (&mut raw) as *mut sys::ImGuiPlatformIO;

        let shared = unsafe { PlatformIo::from_raw(raw_ptr.cast_const()) };
        let mutable = unsafe { PlatformIo::from_raw_mut(raw_ptr) };

        assert_eq!(shared.as_raw(), mutable.as_raw());
    }

    #[cfg(feature = "multi-viewport")]
    #[test]
    fn platform_io_clear_handlers_resets_platform_and_renderer_callbacks() {
        unsafe extern "C" fn platform_cb(_viewport: *mut sys::ImGuiViewport) {}
        unsafe extern "C" fn renderer_cb(_viewport: *mut sys::ImGuiViewport) {}
        unsafe extern "C" fn platform_dpi_scale_cb(_viewport: *mut sys::ImGuiViewport) -> f32 {
            1.0
        }

        let mut raw: sys::ImGuiPlatformIO = new_platform_io();
        raw.Platform_CreateWindow = Some(platform_cb);
        raw.Platform_DestroyWindow = Some(platform_cb);
        raw.Platform_GetWindowDpiScale = Some(platform_dpi_scale_cb);
        raw.Platform_OnChangedViewport = Some(platform_cb);
        raw.Renderer_CreateWindow = Some(renderer_cb);
        raw.Renderer_DestroyWindow = Some(renderer_cb);

        let pio = unsafe { PlatformIo::from_raw_mut((&mut raw) as *mut sys::ImGuiPlatformIO) };
        pio.clear_platform_handlers();
        pio.clear_renderer_handlers();

        assert!(raw.Platform_CreateWindow.is_none());
        assert!(raw.Platform_DestroyWindow.is_none());
        assert!(raw.Platform_GetWindowDpiScale.is_none());
        assert!(raw.Platform_OnChangedViewport.is_none());
        assert!(raw.Renderer_CreateWindow.is_none());
        assert!(raw.Renderer_DestroyWindow.is_none());
    }
}

// TODO: Add more viewport functionality:
// - Platform window handle access
// - Renderer data access
// - DPI scale information
// - Monitor information
