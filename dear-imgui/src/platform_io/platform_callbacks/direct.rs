use core::ffi::c_char;
use std::ffi::c_void;

use crate::sys;

use super::super::{PlatformIo, Viewport, callback_abi};

impl PlatformIo {
    /// Set the raw platform create-window callback.
    ///
    /// # Safety
    ///
    /// When present, the callback must remain callable until it is replaced or cleared, must not
    /// unwind across the C ABI, and must uphold Dear ImGui's callback contract for every viewport
    /// passed to it. Replacing or clearing a live backend callback is only valid after all state
    /// interpreted by that callback has been released.
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_platform_create_window_raw(
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
    /// Registration targets this `PlatformIo`. Invoke a saved callback with its owning Context
    /// bound and a live viewport belonging to that Context.
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_platform_create_window(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport)>,
    ) {
        unsafe { self.set_platform_create_window_raw(callback_abi::window(callback)) }
    }

    /// Set platform destroy window callback (raw).
    ///
    /// # Safety
    ///
    /// See [`Self::set_platform_create_window_raw`].
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_platform_destroy_window_raw(
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
        unsafe { self.set_platform_destroy_window_raw(callback_abi::window(callback)) }
    }

    /// Set platform show window callback (raw).
    ///
    /// # Safety
    ///
    /// See [`Self::set_platform_create_window_raw`].
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_platform_show_window_raw(
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
        unsafe { self.set_platform_show_window_raw(callback_abi::window(callback)) }
    }

    /// Set platform set window position callback through the aggregate ABI shim.
    ///
    /// The callback receives a pointer because Dear ImGui's C++ slot accepts `ImVec2` by value.
    ///
    /// # Safety
    ///
    /// See [`Self::set_platform_create_window_raw`]. The pointed-to `ImVec2` is valid only for
    /// the duration of the callback.
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_platform_set_window_pos_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, *const sys::ImVec2)>,
    ) {
        unsafe {
            sys::ImGuiPlatformIO_Set_Platform_SetWindowPos_PointerParam(
                self.as_raw_mut(),
                callback,
            );
        }
    }

    /// Clear the aggregate position setter only if `callback` still owns it.
    ///
    /// # Safety
    ///
    /// Dear ImGui must no longer be able to invoke this callback, and all backend state it
    /// interprets must already have been released.
    #[doc(hidden)]
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn clear_platform_set_window_pos_if_pointer_callback(
        &mut self,
        callback: unsafe extern "C" fn(*mut sys::ImGuiViewport, *const sys::ImVec2),
    ) -> bool {
        unsafe {
            sys::ImGuiPlatformIO_ClearPlatformSetWindowPosIfPointerParam(
                self.as_raw_mut(),
                callback,
            )
        }
    }

    /// Set the platform window position callback. The vector is borrowed for the call.
    ///
    /// # Safety
    ///
    /// See [`Self::set_platform_create_window`].
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_platform_set_window_pos(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport, &sys::ImVec2)>,
    ) {
        unsafe { self.set_platform_set_window_pos_raw(callback_abi::vec2_input(callback)) }
    }

    /// Set platform set window size callback through the aggregate ABI shim.
    ///
    /// The callback receives a pointer because Dear ImGui's C++ slot accepts `ImVec2` by value.
    ///
    /// # Safety
    ///
    /// See [`Self::set_platform_create_window_raw`]. The pointed-to `ImVec2` is valid only for
    /// the duration of the callback.
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_platform_set_window_size_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, *const sys::ImVec2)>,
    ) {
        unsafe {
            sys::ImGuiPlatformIO_Set_Platform_SetWindowSize_PointerParam(
                self.as_raw_mut(),
                callback,
            );
        }
    }

    /// Clear the aggregate size setter only if `callback` still owns it.
    ///
    /// # Safety
    ///
    /// Dear ImGui must no longer be able to invoke this callback, and all backend state it
    /// interprets must already have been released.
    #[doc(hidden)]
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn clear_platform_set_window_size_if_pointer_callback(
        &mut self,
        callback: unsafe extern "C" fn(*mut sys::ImGuiViewport, *const sys::ImVec2),
    ) -> bool {
        unsafe {
            sys::ImGuiPlatformIO_ClearPlatformSetWindowSizeIfPointerParam(
                self.as_raw_mut(),
                callback,
            )
        }
    }

    /// Set the platform window size callback. The vector is borrowed for the call.
    ///
    /// # Safety
    ///
    /// See [`Self::set_platform_create_window`].
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_platform_set_window_size(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport, &sys::ImVec2)>,
    ) {
        unsafe { self.set_platform_set_window_size_raw(callback_abi::vec2_input(callback)) }
    }

    /// Set platform set window focus callback (raw).
    ///
    /// # Safety
    ///
    /// See [`Self::set_platform_create_window_raw`].
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_platform_set_window_focus_raw(
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
        unsafe { self.set_platform_set_window_focus_raw(callback_abi::window(callback)) }
    }

    /// Set platform get window DPI scale callback (raw).
    ///
    /// # Safety
    ///
    /// See [`Self::set_platform_create_window_raw`].
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_platform_get_window_dpi_scale_raw(
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
        unsafe { self.set_platform_get_window_dpi_scale_raw(callback_abi::dpi_scale(callback)) }
    }

    /// Set platform get window focus callback (raw).
    ///
    /// # Safety
    ///
    /// See [`Self::set_platform_create_window_raw`].
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_platform_get_window_focus_raw(
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
        unsafe { self.set_platform_get_window_focus_raw(callback_abi::predicate(callback)) }
    }

    /// Set platform get window minimized callback (raw).
    ///
    /// # Safety
    ///
    /// See [`Self::set_platform_create_window_raw`].
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_platform_get_window_minimized_raw(
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
        unsafe { self.set_platform_get_window_minimized_raw(callback_abi::predicate(callback)) }
    }

    /// Set platform on changed viewport callback (raw).
    ///
    /// # Safety
    ///
    /// See [`Self::set_platform_create_window_raw`].
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_platform_on_changed_viewport_raw(
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
        unsafe { self.set_platform_on_changed_viewport_raw(callback_abi::window(callback)) }
    }

    /// Set platform set window title callback (raw).
    ///
    /// # Safety
    ///
    /// See [`Self::set_platform_create_window_raw`]. The title pointer is valid only for the
    /// duration of the callback.
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_platform_set_window_title_raw(
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
        unsafe { self.set_platform_set_window_title_raw(callback_abi::title(callback)) }
    }

    /// Set platform set window alpha callback (raw).
    ///
    /// # Safety
    ///
    /// See [`Self::set_platform_create_window_raw`].
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_platform_set_window_alpha_raw(
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
        unsafe { self.set_platform_set_window_alpha_raw(callback_abi::alpha(callback)) }
    }

    /// Set platform update window callback (raw).
    ///
    /// # Safety
    ///
    /// See [`Self::set_platform_create_window_raw`].
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_platform_update_window_raw(
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
        unsafe { self.set_platform_update_window_raw(callback_abi::window(callback)) }
    }

    /// Set platform render window callback (raw).
    ///
    /// # Safety
    ///
    /// See [`Self::set_platform_create_window_raw`]. `render_arg` must satisfy the backend's
    /// contract whenever Dear ImGui supplies it.
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_platform_render_window_raw(
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
        unsafe { self.set_platform_render_window_raw(callback_abi::render(callback)) }
    }

    /// Set platform swap buffers callback (raw).
    ///
    /// # Safety
    ///
    /// See [`Self::set_platform_create_window_raw`]. `render_arg` must satisfy the backend's
    /// contract whenever Dear ImGui supplies it.
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_platform_swap_buffers_raw(
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
        unsafe { self.set_platform_swap_buffers_raw(callback_abi::render(callback)) }
    }
}
