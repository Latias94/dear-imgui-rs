use core::ffi::c_char;
use std::ffi::c_void;

use crate::sys;

use super::super::{PlatformIo, Viewport, trampolines};

impl PlatformIo {
    /// Set platform create window callback (raw sys pointer)
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_create_window_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport)>,
    ) {
        self.inner_mut().Platform_CreateWindow = callback;
        self.clear_platform_io_cb(&trampolines::PLATFORM_CREATE_WINDOW_CB);
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
    /// Note: the typed callback is stored for the current ImGui context, so this must be called
    /// on the active context's `PlatformIo`.
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_platform_create_window(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport)>,
    ) {
        self.assert_current_context_platform_io_for_callbacks();
        use trampolines::*;
        self.set_platform_create_window_raw(callback.map(|_| {
            trampolines::platform_create_window as unsafe extern "C" fn(*mut sys::ImGuiViewport)
        }));
        self.store_current_context_cb(&PLATFORM_CREATE_WINDOW_CB, callback);
    }

    /// Set platform destroy window callback (raw)
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_destroy_window_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport)>,
    ) {
        self.inner_mut().Platform_DestroyWindow = callback;
        self.clear_platform_io_cb(&trampolines::PLATFORM_DESTROY_WINDOW_CB);
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
        self.assert_current_context_platform_io_for_callbacks();
        use trampolines::*;
        self.set_platform_destroy_window_raw(callback.map(|_| {
            trampolines::platform_destroy_window as unsafe extern "C" fn(*mut sys::ImGuiViewport)
        }));
        self.store_current_context_cb(&PLATFORM_DESTROY_WINDOW_CB, callback);
    }

    /// Set platform show window callback (raw)
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_show_window_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport)>,
    ) {
        self.inner_mut().Platform_ShowWindow = callback;
        self.clear_platform_io_cb(&trampolines::PLATFORM_SHOW_WINDOW_CB);
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
        self.assert_current_context_platform_io_for_callbacks();
        use trampolines::*;
        self.set_platform_show_window_raw(callback.map(|_| {
            trampolines::platform_show_window as unsafe extern "C" fn(*mut sys::ImGuiViewport)
        }));
        self.store_current_context_cb(&PLATFORM_SHOW_WINDOW_CB, callback);
    }

    /// Set platform set window position callback (raw)
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_set_window_pos_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, sys::ImVec2)>,
    ) {
        self.inner_mut().Platform_SetWindowPos = callback;
        self.clear_platform_io_cb(&trampolines::PLATFORM_SET_WINDOW_POS_CB);
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
        self.assert_current_context_platform_io_for_callbacks();
        use trampolines::*;
        self.set_platform_set_window_pos_raw(callback.map(|_| {
            trampolines::platform_set_window_pos
                as unsafe extern "C" fn(*mut sys::ImGuiViewport, sys::ImVec2)
        }));
        self.store_current_context_cb(&PLATFORM_SET_WINDOW_POS_CB, callback);
    }

    /// Set platform set window size callback (raw)
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_set_window_size_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, sys::ImVec2)>,
    ) {
        self.inner_mut().Platform_SetWindowSize = callback;
        self.clear_platform_io_cb(&trampolines::PLATFORM_SET_WINDOW_SIZE_CB);
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
        self.assert_current_context_platform_io_for_callbacks();
        use trampolines::*;
        self.set_platform_set_window_size_raw(callback.map(|_| {
            trampolines::platform_set_window_size
                as unsafe extern "C" fn(*mut sys::ImGuiViewport, sys::ImVec2)
        }));
        self.store_current_context_cb(&PLATFORM_SET_WINDOW_SIZE_CB, callback);
    }

    /// Set platform set window focus callback (raw)
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_set_window_focus_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport)>,
    ) {
        self.inner_mut().Platform_SetWindowFocus = callback;
        self.clear_platform_io_cb(&trampolines::PLATFORM_SET_WINDOW_FOCUS_CB);
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
        self.assert_current_context_platform_io_for_callbacks();
        use trampolines::*;
        self.set_platform_set_window_focus_raw(callback.map(|_| {
            trampolines::platform_set_window_focus as unsafe extern "C" fn(*mut sys::ImGuiViewport)
        }));
        self.store_current_context_cb(&PLATFORM_SET_WINDOW_FOCUS_CB, callback);
    }

    /// Set platform get window DPI scale callback (raw)
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_get_window_dpi_scale_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport) -> f32>,
    ) {
        self.inner_mut().Platform_GetWindowDpiScale = callback;
        self.clear_platform_io_cb(&trampolines::PLATFORM_GET_WINDOW_DPI_SCALE_CB);
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
        self.assert_current_context_platform_io_for_callbacks();
        use trampolines::*;
        self.set_platform_get_window_dpi_scale_raw(callback.map(|_| {
            trampolines::platform_get_window_dpi_scale
                as unsafe extern "C" fn(*mut sys::ImGuiViewport) -> f32
        }));
        self.store_current_context_cb(&PLATFORM_GET_WINDOW_DPI_SCALE_CB, callback);
    }

    /// Set platform get window focus callback (raw)
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_get_window_focus_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport) -> bool>,
    ) {
        self.inner_mut().Platform_GetWindowFocus = callback;
        self.clear_platform_io_cb(&trampolines::PLATFORM_GET_WINDOW_FOCUS_CB);
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
        self.assert_current_context_platform_io_for_callbacks();
        use trampolines::*;
        self.set_platform_get_window_focus_raw(callback.map(|_| {
            trampolines::platform_get_window_focus
                as unsafe extern "C" fn(*mut sys::ImGuiViewport) -> bool
        }));
        self.store_current_context_cb(&PLATFORM_GET_WINDOW_FOCUS_CB, callback);
    }

    /// Set platform get window minimized callback (raw)
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_get_window_minimized_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport) -> bool>,
    ) {
        self.inner_mut().Platform_GetWindowMinimized = callback;
        self.clear_platform_io_cb(&trampolines::PLATFORM_GET_WINDOW_MINIMIZED_CB);
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
        self.assert_current_context_platform_io_for_callbacks();
        use trampolines::*;
        self.set_platform_get_window_minimized_raw(callback.map(|_| {
            trampolines::platform_get_window_minimized
                as unsafe extern "C" fn(*mut sys::ImGuiViewport) -> bool
        }));
        self.store_current_context_cb(&PLATFORM_GET_WINDOW_MINIMIZED_CB, callback);
    }

    /// Set platform on changed viewport callback (raw)
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_on_changed_viewport_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport)>,
    ) {
        self.inner_mut().Platform_OnChangedViewport = callback;
        self.clear_platform_io_cb(&trampolines::PLATFORM_ON_CHANGED_VIEWPORT_CB);
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
        self.assert_current_context_platform_io_for_callbacks();
        use trampolines::*;
        self.set_platform_on_changed_viewport_raw(callback.map(|_| {
            trampolines::platform_on_changed_viewport
                as unsafe extern "C" fn(*mut sys::ImGuiViewport)
        }));
        self.store_current_context_cb(&PLATFORM_ON_CHANGED_VIEWPORT_CB, callback);
    }

    /// Set platform set window title callback (raw)
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_set_window_title_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, *const c_char)>,
    ) {
        self.inner_mut().Platform_SetWindowTitle = callback;
        self.clear_platform_io_cb(&trampolines::PLATFORM_SET_WINDOW_TITLE_CB);
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
        self.assert_current_context_platform_io_for_callbacks();
        use trampolines::*;
        self.set_platform_set_window_title_raw(callback.map(|_| {
            trampolines::platform_set_window_title
                as unsafe extern "C" fn(*mut sys::ImGuiViewport, *const c_char)
        }));
        self.store_current_context_cb(&PLATFORM_SET_WINDOW_TITLE_CB, callback);
    }

    /// Set platform set window alpha callback (raw)
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_set_window_alpha_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, f32)>,
    ) {
        self.inner_mut().Platform_SetWindowAlpha = callback;
        self.clear_platform_io_cb(&trampolines::PLATFORM_SET_WINDOW_ALPHA_CB);
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
        self.assert_current_context_platform_io_for_callbacks();
        use trampolines::*;
        self.set_platform_set_window_alpha_raw(callback.map(|_| {
            trampolines::platform_set_window_alpha
                as unsafe extern "C" fn(*mut sys::ImGuiViewport, f32)
        }));
        self.store_current_context_cb(&PLATFORM_SET_WINDOW_ALPHA_CB, callback);
    }

    /// Set platform update window callback (raw)
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_update_window_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport)>,
    ) {
        self.inner_mut().Platform_UpdateWindow = callback;
        self.clear_platform_io_cb(&trampolines::PLATFORM_UPDATE_WINDOW_CB);
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
        self.assert_current_context_platform_io_for_callbacks();
        use trampolines::*;
        self.set_platform_update_window_raw(callback.map(|_| {
            trampolines::platform_update_window as unsafe extern "C" fn(*mut sys::ImGuiViewport)
        }));
        self.store_current_context_cb(&PLATFORM_UPDATE_WINDOW_CB, callback);
    }

    /// Set platform render window callback (raw)
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_render_window_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void)>,
    ) {
        self.inner_mut().Platform_RenderWindow = callback;
        self.clear_platform_io_cb(&trampolines::PLATFORM_RENDER_WINDOW_CB);
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
        self.assert_current_context_platform_io_for_callbacks();
        use trampolines::*;
        self.set_platform_render_window_raw(callback.map(|_| {
            trampolines::platform_render_window
                as unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void)
        }));
        self.store_current_context_cb(&PLATFORM_RENDER_WINDOW_CB, callback);
    }

    /// Set platform swap buffers callback (raw)
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_swap_buffers_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void)>,
    ) {
        self.inner_mut().Platform_SwapBuffers = callback;
        self.clear_platform_io_cb(&trampolines::PLATFORM_SWAP_BUFFERS_CB);
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
        self.assert_current_context_platform_io_for_callbacks();
        use trampolines::*;
        self.set_platform_swap_buffers_raw(callback.map(|_| {
            trampolines::platform_swap_buffers
                as unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void)
        }));
        self.store_current_context_cb(&PLATFORM_SWAP_BUFFERS_CB, callback);
    }
}
