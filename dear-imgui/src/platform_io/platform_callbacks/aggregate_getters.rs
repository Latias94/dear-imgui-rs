use crate::sys;

use super::super::core::assert_platform_io_out_param_hooks_available;
use super::super::{PlatformIo, Viewport, trampolines};

impl PlatformIo {
    /// Set platform get window position callback (raw)
    ///
    /// This uses this crate's out-parameter shim internally instead of writing
    /// `ImGuiPlatformIO::Platform_GetWindowPos` directly. The shim stores a C-compatible
    /// out-parameter callback in `dear-imgui-sys` storage and installs a C++ thunk that returns
    /// `ImVec2` by value, which avoids exposing the fragile direct small-aggregate callback ABI
    /// on MSVC.
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_get_window_pos_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut sys::ImVec2)>,
    ) {
        use trampolines::*;

        self.assert_current_context_platform_io_for_callbacks();
        if callback.is_some() {
            assert_platform_io_out_param_hooks_available("Platform_GetWindowPos");
        }

        match callback {
            Some(cb) => {
                self.store_current_context_cb(&PLATFORM_GET_WINDOW_POS_RAW_CB, Some(cb));
                self.store_current_context_cb(&PLATFORM_GET_WINDOW_POS_CB, None);
            }
            None => {
                self.clear_current_context_cb(&PLATFORM_GET_WINDOW_POS_RAW_CB);
                self.clear_current_context_cb(&PLATFORM_GET_WINDOW_POS_CB);
            }
        }

        unsafe {
            match callback {
                Some(_) => sys::ImGuiPlatformIO_Set_Platform_GetWindowPos_OutParam(
                    self.as_raw_mut(),
                    Some(
                        trampolines::platform_get_window_pos_out
                            as unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut sys::ImVec2),
                    ),
                ),
                None => {
                    sys::ImGuiPlatformIO_Set_Platform_GetWindowPos_OutParam(self.as_raw_mut(), None)
                }
            }
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
        use trampolines::*;
        self.assert_current_context_platform_io_for_callbacks();
        if callback.is_some() {
            assert_platform_io_out_param_hooks_available("Platform_GetWindowPos");
        }

        match callback {
            Some(cb) => {
                self.store_current_context_cb(&PLATFORM_GET_WINDOW_POS_RAW_CB, None);
                self.store_current_context_cb(&PLATFORM_GET_WINDOW_POS_CB, Some(cb));
            }
            None => {
                self.clear_current_context_cb(&PLATFORM_GET_WINDOW_POS_RAW_CB);
                self.clear_current_context_cb(&PLATFORM_GET_WINDOW_POS_CB);
            }
        }

        unsafe {
            match callback {
                Some(_) => sys::ImGuiPlatformIO_Set_Platform_GetWindowPos_OutParam(
                    self.as_raw_mut(),
                    Some(
                        trampolines::platform_get_window_pos_out
                            as unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut sys::ImVec2),
                    ),
                ),
                None => {
                    sys::ImGuiPlatformIO_Set_Platform_GetWindowPos_OutParam(self.as_raw_mut(), None)
                }
            }
        }
    }

    /// Set platform get window size callback (raw)
    ///
    /// This uses this crate's out-parameter shim internally instead of writing
    /// `ImGuiPlatformIO::Platform_GetWindowSize` directly. The shim stores a C-compatible
    /// out-parameter callback in `dear-imgui-sys` storage and installs a C++ thunk that returns
    /// `ImVec2` by value, which avoids exposing the fragile direct small-aggregate callback ABI
    /// on MSVC.
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_get_window_size_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut sys::ImVec2)>,
    ) {
        use trampolines::*;

        self.assert_current_context_platform_io_for_callbacks();
        if callback.is_some() {
            assert_platform_io_out_param_hooks_available("Platform_GetWindowSize");
        }

        match callback {
            Some(cb) => {
                self.store_current_context_cb(&PLATFORM_GET_WINDOW_SIZE_RAW_CB, Some(cb));
                self.store_current_context_cb(&PLATFORM_GET_WINDOW_SIZE_CB, None);
            }
            None => {
                self.clear_current_context_cb(&PLATFORM_GET_WINDOW_SIZE_RAW_CB);
                self.clear_current_context_cb(&PLATFORM_GET_WINDOW_SIZE_CB);
            }
        }

        unsafe {
            match callback {
                Some(_) => sys::ImGuiPlatformIO_Set_Platform_GetWindowSize_OutParam(
                    self.as_raw_mut(),
                    Some(
                        trampolines::platform_get_window_size_out
                            as unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut sys::ImVec2),
                    ),
                ),
                None => sys::ImGuiPlatformIO_Set_Platform_GetWindowSize_OutParam(
                    self.as_raw_mut(),
                    None,
                ),
            }
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
        use trampolines::*;
        self.assert_current_context_platform_io_for_callbacks();
        if callback.is_some() {
            assert_platform_io_out_param_hooks_available("Platform_GetWindowSize");
        }

        match callback {
            Some(cb) => {
                self.store_current_context_cb(&PLATFORM_GET_WINDOW_SIZE_RAW_CB, None);
                self.store_current_context_cb(&PLATFORM_GET_WINDOW_SIZE_CB, Some(cb));
            }
            None => {
                self.clear_current_context_cb(&PLATFORM_GET_WINDOW_SIZE_RAW_CB);
                self.clear_current_context_cb(&PLATFORM_GET_WINDOW_SIZE_CB);
            }
        }

        unsafe {
            match callback {
                Some(_) => sys::ImGuiPlatformIO_Set_Platform_GetWindowSize_OutParam(
                    self.as_raw_mut(),
                    Some(
                        trampolines::platform_get_window_size_out
                            as unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut sys::ImVec2),
                    ),
                ),
                None => sys::ImGuiPlatformIO_Set_Platform_GetWindowSize_OutParam(
                    self.as_raw_mut(),
                    None,
                ),
            }
        }
    }

    /// Set platform get window framebuffer scale callback (raw)
    ///
    /// This uses this crate's out-parameter shim internally instead of writing
    /// `ImGuiPlatformIO::Platform_GetWindowFramebufferScale` directly. The shim stores a
    /// C-compatible out-parameter callback in `dear-imgui-sys` storage and installs a C++ thunk
    /// that returns `ImVec2` by value, which avoids exposing the fragile direct small-aggregate
    /// callback ABI on MSVC.
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_get_window_framebuffer_scale_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut sys::ImVec2)>,
    ) {
        use trampolines::*;

        self.assert_current_context_platform_io_for_callbacks();
        if callback.is_some() {
            assert_platform_io_out_param_hooks_available("Platform_GetWindowFramebufferScale");
        }

        match callback {
            Some(cb) => {
                self.store_current_context_cb(
                    &PLATFORM_GET_WINDOW_FRAMEBUFFER_SCALE_RAW_CB,
                    Some(cb),
                );
                self.store_current_context_cb(&PLATFORM_GET_WINDOW_FRAMEBUFFER_SCALE_CB, None);
            }
            None => {
                self.clear_current_context_cb(&PLATFORM_GET_WINDOW_FRAMEBUFFER_SCALE_RAW_CB);
                self.clear_current_context_cb(&PLATFORM_GET_WINDOW_FRAMEBUFFER_SCALE_CB);
            }
        }

        unsafe {
            match callback {
                Some(_) => sys::ImGuiPlatformIO_Set_Platform_GetWindowFramebufferScale_OutParam(
                    self.as_raw_mut(),
                    Some(
                        trampolines::platform_get_window_framebuffer_scale_out
                            as unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut sys::ImVec2),
                    ),
                ),
                None => sys::ImGuiPlatformIO_Set_Platform_GetWindowFramebufferScale_OutParam(
                    self.as_raw_mut(),
                    None,
                ),
            }
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
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_platform_get_window_framebuffer_scale(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport, *mut sys::ImVec2)>,
    ) {
        use trampolines::*;
        self.assert_current_context_platform_io_for_callbacks();
        if callback.is_some() {
            assert_platform_io_out_param_hooks_available("Platform_GetWindowFramebufferScale");
        }

        match callback {
            Some(cb) => {
                self.store_current_context_cb(&PLATFORM_GET_WINDOW_FRAMEBUFFER_SCALE_RAW_CB, None);
                self.store_current_context_cb(&PLATFORM_GET_WINDOW_FRAMEBUFFER_SCALE_CB, Some(cb));
            }
            None => {
                self.clear_current_context_cb(&PLATFORM_GET_WINDOW_FRAMEBUFFER_SCALE_RAW_CB);
                self.clear_current_context_cb(&PLATFORM_GET_WINDOW_FRAMEBUFFER_SCALE_CB);
            }
        }

        unsafe {
            match callback {
                Some(_) => sys::ImGuiPlatformIO_Set_Platform_GetWindowFramebufferScale_OutParam(
                    self.as_raw_mut(),
                    Some(
                        trampolines::platform_get_window_framebuffer_scale_out
                            as unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut sys::ImVec2),
                    ),
                ),
                None => sys::ImGuiPlatformIO_Set_Platform_GetWindowFramebufferScale_OutParam(
                    self.as_raw_mut(),
                    None,
                ),
            }
        }
    }

    /// Set platform get window work area insets callback (raw)
    ///
    /// This uses this crate's out-parameter shim internally instead of writing
    /// `ImGuiPlatformIO::Platform_GetWindowWorkAreaInsets` directly. The shim stores a
    /// C-compatible out-parameter callback in `dear-imgui-sys` storage and installs a C++ thunk
    /// that returns `ImVec4` by value, which avoids exposing the fragile direct aggregate callback
    /// ABI.
    #[cfg(feature = "multi-viewport")]
    pub fn set_platform_get_window_work_area_insets_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut sys::ImVec4)>,
    ) {
        use trampolines::*;

        self.assert_current_context_platform_io_for_callbacks();
        if callback.is_some() {
            assert_platform_io_out_param_hooks_available("Platform_GetWindowWorkAreaInsets");
        }

        match callback {
            Some(cb) => {
                self.store_current_context_cb(
                    &PLATFORM_GET_WINDOW_WORK_AREA_INSETS_RAW_CB,
                    Some(cb),
                );
                self.store_current_context_cb(&PLATFORM_GET_WINDOW_WORK_AREA_INSETS_CB, None);
            }
            None => {
                self.clear_current_context_cb(&PLATFORM_GET_WINDOW_WORK_AREA_INSETS_RAW_CB);
                self.clear_current_context_cb(&PLATFORM_GET_WINDOW_WORK_AREA_INSETS_CB);
            }
        }

        unsafe {
            match callback {
                Some(_) => sys::ImGuiPlatformIO_Set_Platform_GetWindowWorkAreaInsets_OutParam(
                    self.as_raw_mut(),
                    Some(
                        trampolines::platform_get_window_work_area_insets_out
                            as unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut sys::ImVec4),
                    ),
                ),
                None => sys::ImGuiPlatformIO_Set_Platform_GetWindowWorkAreaInsets_OutParam(
                    self.as_raw_mut(),
                    None,
                ),
            }
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
        use trampolines::*;
        self.assert_current_context_platform_io_for_callbacks();
        if callback.is_some() {
            assert_platform_io_out_param_hooks_available("Platform_GetWindowWorkAreaInsets");
        }

        match callback {
            Some(cb) => {
                self.store_current_context_cb(&PLATFORM_GET_WINDOW_WORK_AREA_INSETS_RAW_CB, None);
                self.store_current_context_cb(&PLATFORM_GET_WINDOW_WORK_AREA_INSETS_CB, Some(cb));
            }
            None => {
                self.clear_current_context_cb(&PLATFORM_GET_WINDOW_WORK_AREA_INSETS_RAW_CB);
                self.clear_current_context_cb(&PLATFORM_GET_WINDOW_WORK_AREA_INSETS_CB);
            }
        }

        unsafe {
            match callback {
                Some(_) => sys::ImGuiPlatformIO_Set_Platform_GetWindowWorkAreaInsets_OutParam(
                    self.as_raw_mut(),
                    Some(
                        trampolines::platform_get_window_work_area_insets_out
                            as unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut sys::ImVec4),
                    ),
                ),
                None => sys::ImGuiPlatformIO_Set_Platform_GetWindowWorkAreaInsets_OutParam(
                    self.as_raw_mut(),
                    None,
                ),
            }
        }
    }
}
