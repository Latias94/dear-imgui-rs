use crate::sys;

use super::super::{PlatformIo, Viewport, callback_abi};

impl PlatformIo {
    /// Set platform get window position callback (raw)
    ///
    /// The C++ shim stores this exact callback for the receiving `PlatformIo` and supplies the
    /// output pointer when Dear ImGui requests the value.
    ///
    /// # Safety
    ///
    /// See [`Self::set_platform_create_window_raw`]. The output pointer is non-null and valid
    /// for the duration of the callback. The callback must initialize it.
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_platform_get_window_pos_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut sys::ImVec2)>,
    ) {
        unsafe {
            sys::ImGuiPlatformIO_Set_Platform_GetWindowPos_OutParam(self.as_raw_mut(), callback)
        }
    }

    /// Set platform get window position callback (typed Viewport).
    ///
    /// # Safety
    ///
    /// See [`Self::set_platform_create_window`].
    ///
    /// See [`Self::set_platform_get_window_pos_raw`] for why this path must go through the
    /// out-parameter shim.
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_platform_get_window_pos(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport, *mut sys::ImVec2)>,
    ) {
        unsafe { self.set_platform_get_window_pos_raw(callback_abi::vec2_output(callback)) }
    }

    /// Set platform get window size callback (raw)
    ///
    /// The C++ shim stores this exact callback for the receiving `PlatformIo` and supplies the
    /// output pointer when Dear ImGui requests the value.
    ///
    /// # Safety
    ///
    /// See [`Self::set_platform_get_window_pos_raw`].
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_platform_get_window_size_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut sys::ImVec2)>,
    ) {
        unsafe {
            sys::ImGuiPlatformIO_Set_Platform_GetWindowSize_OutParam(self.as_raw_mut(), callback)
        }
    }

    /// Set platform get window size callback (typed Viewport).
    ///
    /// # Safety
    ///
    /// See [`Self::set_platform_create_window`].
    ///
    /// See [`Self::set_platform_get_window_size_raw`] for why this path must go through the
    /// out-parameter shim.
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_platform_get_window_size(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport, *mut sys::ImVec2)>,
    ) {
        unsafe { self.set_platform_get_window_size_raw(callback_abi::vec2_output(callback)) }
    }

    /// Set platform get window framebuffer scale callback (raw)
    ///
    /// The C++ shim stores this exact callback for the receiving `PlatformIo` and supplies the
    /// output pointer when Dear ImGui requests the value.
    ///
    /// # Safety
    ///
    /// See [`Self::set_platform_get_window_pos_raw`].
    /// The callback must write a finite, strictly positive scale on both axes,
    /// including when the viewport is minimized or has a zero-sized framebuffer.
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_platform_get_window_framebuffer_scale_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut sys::ImVec2)>,
    ) {
        unsafe {
            sys::ImGuiPlatformIO_Set_Platform_GetWindowFramebufferScale_OutParam(
                self.as_raw_mut(),
                callback,
            )
        }
    }

    /// Set platform get window framebuffer scale callback (typed Viewport).
    ///
    /// # Safety
    ///
    /// See [`Self::set_platform_create_window`].
    ///
    /// See [`Self::set_platform_get_window_framebuffer_scale_raw`] for why this path must go
    /// through the out-parameter shim.
    /// The callback must write a finite, strictly positive scale on both axes.
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_platform_get_window_framebuffer_scale(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport, *mut sys::ImVec2)>,
    ) {
        unsafe {
            self.set_platform_get_window_framebuffer_scale_raw(callback_abi::vec2_output(callback))
        }
    }

    /// Set platform get window work area insets callback (raw)
    ///
    /// The C++ shim stores this exact callback for the receiving `PlatformIo` and supplies the
    /// output pointer when Dear ImGui requests the value.
    ///
    /// # Safety
    ///
    /// See [`Self::set_platform_get_window_pos_raw`].
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_platform_get_window_work_area_insets_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut sys::ImVec4)>,
    ) {
        unsafe {
            sys::ImGuiPlatformIO_Set_Platform_GetWindowWorkAreaInsets_OutParam(
                self.as_raw_mut(),
                callback,
            )
        }
    }

    /// Set platform get window work area insets callback (typed Viewport).
    ///
    /// # Safety
    ///
    /// See [`Self::set_platform_create_window`].
    ///
    /// See [`Self::set_platform_get_window_work_area_insets_raw`] for why this path must go
    /// through the out-parameter shim.
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_platform_get_window_work_area_insets(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport, *mut sys::ImVec4)>,
    ) {
        unsafe {
            self.set_platform_get_window_work_area_insets_raw(callback_abi::vec4_output(callback))
        }
    }

    /// Release `Platform_GetWindowPos` only if `callback` is still the registered raw handler.
    ///
    /// # Safety
    ///
    /// Dear ImGui must no longer be able to invoke this callback, and all backend state it
    /// interprets must already have been released.
    #[cfg(feature = "multi-viewport")]
    #[doc(hidden)]
    pub unsafe fn clear_platform_get_window_pos_if_raw_callback(
        &mut self,
        callback: unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut sys::ImVec2),
    ) -> bool {
        unsafe {
            sys::ImGuiPlatformIO_ClearPlatformGetWindowPosIfOutParam(self.as_raw_mut(), callback)
        }
    }

    /// Release `Platform_GetWindowSize` only if `callback` is still the registered raw handler.
    ///
    /// # Safety
    ///
    /// See [`Self::clear_platform_get_window_pos_if_raw_callback`].
    #[cfg(feature = "multi-viewport")]
    #[doc(hidden)]
    pub unsafe fn clear_platform_get_window_size_if_raw_callback(
        &mut self,
        callback: unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut sys::ImVec2),
    ) -> bool {
        unsafe {
            sys::ImGuiPlatformIO_ClearPlatformGetWindowSizeIfOutParam(self.as_raw_mut(), callback)
        }
    }

    /// Release `Platform_GetWindowFramebufferScale` only if `callback` is still registered.
    ///
    /// # Safety
    ///
    /// See [`Self::clear_platform_get_window_pos_if_raw_callback`].
    #[cfg(feature = "multi-viewport")]
    #[doc(hidden)]
    pub unsafe fn clear_platform_get_window_framebuffer_scale_if_raw_callback(
        &mut self,
        callback: unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut sys::ImVec2),
    ) -> bool {
        unsafe {
            sys::ImGuiPlatformIO_ClearPlatformGetWindowFramebufferScaleIfOutParam(
                self.as_raw_mut(),
                callback,
            )
        }
    }

    /// Release `Platform_GetWindowWorkAreaInsets` only if `callback` is still registered.
    ///
    /// # Safety
    ///
    /// See [`Self::clear_platform_get_window_pos_if_raw_callback`].
    #[cfg(feature = "multi-viewport")]
    #[doc(hidden)]
    pub unsafe fn clear_platform_get_window_work_area_insets_if_raw_callback(
        &mut self,
        callback: unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut sys::ImVec4),
    ) -> bool {
        unsafe {
            sys::ImGuiPlatformIO_ClearPlatformGetWindowWorkAreaInsetsIfOutParam(
                self.as_raw_mut(),
                callback,
            )
        }
    }
}
