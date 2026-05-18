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

#[cfg(feature = "multi-viewport")]
pub(crate) fn clear_typed_callbacks_for_context(ctx: *mut sys::ImGuiContext) {
    trampolines::clear_callbacks_for_context(ctx);
}

#[cfg(not(feature = "multi-viewport"))]
pub(crate) fn clear_typed_callbacks_for_context(_ctx: *mut sys::ImGuiContext) {}

#[cfg(feature = "multi-viewport")]
pub(crate) unsafe fn clear_out_param_callbacks_for_current_context() {
    let pio = unsafe { sys::igGetPlatformIO_Nil() };
    if pio.is_null() {
        return;
    }
    unsafe {
        sys::ImGuiPlatformIO_Set_Platform_GetWindowPos_OutParam(pio, None);
        sys::ImGuiPlatformIO_Set_Platform_GetWindowSize_OutParam(pio, None);
        sys::ImGuiPlatformIO_Set_Platform_GetWindowFramebufferScale_OutParam(pio, None);
        sys::ImGuiPlatformIO_Set_Platform_GetWindowWorkAreaInsets_OutParam(pio, None);
    }
}

#[cfg(not(feature = "multi-viewport"))]
pub(crate) unsafe fn clear_out_param_callbacks_for_current_context() {}

#[cfg(feature = "multi-viewport")]
unsafe fn clear_out_param_callbacks_for_platform_io(pio: *mut sys::ImGuiPlatformIO) {
    if pio.is_null() {
        return;
    }
    unsafe {
        sys::ImGuiPlatformIO_Set_Platform_GetWindowPos_OutParam(pio, None);
        sys::ImGuiPlatformIO_Set_Platform_GetWindowSize_OutParam(pio, None);
        sys::ImGuiPlatformIO_Set_Platform_GetWindowFramebufferScale_OutParam(pio, None);
        sys::ImGuiPlatformIO_Set_Platform_GetWindowWorkAreaInsets_OutParam(pio, None);
    }
}

#[cfg(feature = "multi-viewport")]
fn assert_platform_io_out_param_hooks_available(callback_name: &str) {
    assert!(
        sys::HAS_PLATFORM_IO_OUT_PARAM_HOOKS,
        "dear-imgui-sys was built without PlatformIO out-parameter hooks; \
         rebuild without IMGUI_SYS_SKIP_CC to install {callback_name} callbacks"
    );
}

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

    #[cfg(feature = "multi-viewport")]
    fn is_current_context_platform_io(&self) -> bool {
        unsafe {
            if sys::igGetCurrentContext().is_null() {
                return false;
            }

            let current = sys::igGetPlatformIO_Nil();
            !current.is_null() && std::ptr::addr_eq(current.cast_const(), self.as_raw())
        }
    }

    #[cfg(feature = "multi-viewport")]
    fn assert_current_context_platform_io_for_callbacks(&self) {
        assert!(
            self.is_current_context_platform_io(),
            "PlatformIo typed/out-parameter callback setters must be called on the active ImGui context's PlatformIo"
        );
    }

    #[cfg(feature = "multi-viewport")]
    fn store_current_context_cb<T: Copy>(
        &self,
        slot: &trampolines::CallbackSlot<T>,
        callback: Option<T>,
    ) {
        self.assert_current_context_platform_io_for_callbacks();
        trampolines::store_cb(slot, callback);
    }

    #[cfg(feature = "multi-viewport")]
    fn clear_current_context_cb<T: Copy>(&self, slot: &trampolines::CallbackSlot<T>) {
        self.assert_current_context_platform_io_for_callbacks();
        trampolines::clear_cb_for_current_context(slot);
    }

    #[cfg(feature = "multi-viewport")]
    fn clear_platform_io_cb<T: Copy>(&self, slot: &trampolines::CallbackSlot<T>) {
        trampolines::clear_cb_for_platform_io(self.as_raw(), slot);
    }

    /// Clear all platform backend handlers.
    ///
    /// This resets the `Platform_*` callback table stored in `ImGuiPlatformIO`.
    /// This also clears Rust typed callback storage for this `PlatformIo`'s context and the
    /// out-parameter callback shim used by aggregate-return platform getters.
    #[cfg(feature = "multi-viewport")]
    pub fn clear_platform_handlers(&mut self) {
        unsafe { sys::ImGuiPlatformIO_ClearPlatformHandlers(self.as_raw_mut()) }

        trampolines::clear_platform_callbacks_for_platform_io(self.as_raw());
        unsafe {
            clear_out_param_callbacks_for_platform_io(self.as_raw_mut());
        }
    }

    /// Clear all renderer backend handlers.
    ///
    /// This resets the `Renderer_*` callback table stored in `ImGuiPlatformIO`.
    /// This also clears Rust typed renderer callback storage for this `PlatformIo`'s context.
    #[cfg(feature = "multi-viewport")]
    pub fn clear_renderer_handlers(&mut self) {
        unsafe { sys::ImGuiPlatformIO_ClearRendererHandlers(self.as_raw_mut()) }

        trampolines::clear_renderer_callbacks_for_platform_io(self.as_raw());
    }

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
        self.clear_platform_io_cb(&trampolines::RENDERER_CREATE_WINDOW_CB);
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
        self.assert_current_context_platform_io_for_callbacks();
        use trampolines::*;
        self.set_renderer_create_window_raw(callback.map(|_| {
            trampolines::renderer_create_window as unsafe extern "C" fn(*mut sys::ImGuiViewport)
        }));
        self.store_current_context_cb(&RENDERER_CREATE_WINDOW_CB, callback);
    }

    /// Set renderer destroy window callback (raw)
    #[cfg(feature = "multi-viewport")]
    pub fn set_renderer_destroy_window_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport)>,
    ) {
        self.inner_mut().Renderer_DestroyWindow = callback;
        self.clear_platform_io_cb(&trampolines::RENDERER_DESTROY_WINDOW_CB);
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
        self.assert_current_context_platform_io_for_callbacks();
        use trampolines::*;
        self.set_renderer_destroy_window_raw(callback.map(|_| {
            trampolines::renderer_destroy_window as unsafe extern "C" fn(*mut sys::ImGuiViewport)
        }));
        self.store_current_context_cb(&RENDERER_DESTROY_WINDOW_CB, callback);
    }

    /// Set renderer set window size callback (raw)
    #[cfg(feature = "multi-viewport")]
    pub fn set_renderer_set_window_size_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, sys::ImVec2)>,
    ) {
        self.inner_mut().Renderer_SetWindowSize = callback;
        self.clear_platform_io_cb(&trampolines::RENDERER_SET_WINDOW_SIZE_CB);
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
        self.assert_current_context_platform_io_for_callbacks();
        use trampolines::*;
        self.set_renderer_set_window_size_raw(callback.map(|_| {
            trampolines::renderer_set_window_size
                as unsafe extern "C" fn(*mut sys::ImGuiViewport, sys::ImVec2)
        }));
        self.store_current_context_cb(&RENDERER_SET_WINDOW_SIZE_CB, callback);
    }

    /// Set renderer render window callback (raw)
    #[cfg(feature = "multi-viewport")]
    pub fn set_renderer_render_window_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void)>,
    ) {
        self.inner_mut().Renderer_RenderWindow = callback;
        self.clear_platform_io_cb(&trampolines::RENDERER_RENDER_WINDOW_CB);
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
        self.assert_current_context_platform_io_for_callbacks();
        use trampolines::*;
        self.set_renderer_render_window_raw(callback.map(|_| {
            trampolines::renderer_render_window
                as unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void)
        }));
        self.store_current_context_cb(&RENDERER_RENDER_WINDOW_CB, callback);
    }

    /// Set renderer swap buffers callback (raw)
    #[cfg(feature = "multi-viewport")]
    pub fn set_renderer_swap_buffers_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void)>,
    ) {
        self.inner_mut().Renderer_SwapBuffers = callback;
        self.clear_platform_io_cb(&trampolines::RENDERER_SWAP_BUFFERS_CB);
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
        self.assert_current_context_platform_io_for_callbacks();
        use trampolines::*;
        self.set_renderer_swap_buffers_raw(callback.map(|_| {
            trampolines::renderer_swap_buffers
                as unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void)
        }));
        self.store_current_context_cb(&RENDERER_SWAP_BUFFERS_CB, callback);
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

    /// Get a shared iterator over all textures managed by the platform.
    ///
    /// Use this for inspection. Renderer backends or feedback application code that need to write
    /// texture status or backend IDs must use [`Self::textures_mut`].
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

    /// Get a mutable cursor over all textures managed by the platform.
    ///
    /// This is used on the UI thread for applying renderer feedback and during shutdown paths that
    /// need to mutate backend texture fields.
    pub fn textures_mut(&mut self) -> crate::render::draw_data::TextureMutCursor<'_> {
        unsafe {
            let vector = &mut self.inner_mut().Textures;
            let size = match usize::try_from(vector.Size) {
                Ok(size) => size,
                Err(_) => 0,
            };
            if size == 0 || vector.Data.is_null() {
                crate::render::draw_data::TextureMutCursor::new(
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                )
            } else {
                crate::render::draw_data::TextureMutCursor::new(vector.Data, vector.Data.add(size))
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

        let mut by_id: std::collections::HashMap<
            crate::texture::ManagedTextureId,
            crate::render::snapshot::TextureFeedback,
        > = std::collections::HashMap::with_capacity(feedback.len());
        for &fb in feedback {
            by_id.insert(fb.id, fb);
        }

        let mut applied = 0usize;
        let mut textures = self.textures_mut();
        while let Some(mut tex) = textures.next() {
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

    /// Set the standard draw callback used to request renderer-state reset.
    ///
    /// Renderer backends may install a backend-specific function pointer here. Higher-level
    /// draw iteration recognizes this callback as [`crate::render::DrawCmd::ResetRenderState`]
    /// instead of treating it as an arbitrary raw callback.
    #[doc(alias = "DrawCallback_ResetRenderState")]
    pub fn set_draw_callback_reset_render_state_raw(&mut self, callback: sys::ImDrawCallback) {
        self.inner_mut().DrawCallback_ResetRenderState = callback;
    }

    /// Get the standard draw callback used to request renderer-state reset.
    #[doc(alias = "DrawCallback_ResetRenderState")]
    pub fn draw_callback_reset_render_state_raw(&self) -> sys::ImDrawCallback {
        self.inner().DrawCallback_ResetRenderState
    }

    /// Set the standard draw callback used to request linear texture sampling.
    #[doc(alias = "DrawCallback_SetSamplerLinear")]
    pub fn set_draw_callback_set_sampler_linear_raw(&mut self, callback: sys::ImDrawCallback) {
        self.inner_mut().DrawCallback_SetSamplerLinear = callback;
    }

    /// Get the standard draw callback used to request linear texture sampling.
    #[doc(alias = "DrawCallback_SetSamplerLinear")]
    pub fn draw_callback_set_sampler_linear_raw(&self) -> sys::ImDrawCallback {
        self.inner().DrawCallback_SetSamplerLinear
    }

    /// Set the standard draw callback used to request nearest/point texture sampling.
    #[doc(alias = "DrawCallback_SetSamplerNearest")]
    pub fn set_draw_callback_set_sampler_nearest_raw(&mut self, callback: sys::ImDrawCallback) {
        self.inner_mut().DrawCallback_SetSamplerNearest = callback;
    }

    /// Get the standard draw callback used to request nearest/point texture sampling.
    #[doc(alias = "DrawCallback_SetSamplerNearest")]
    pub fn draw_callback_set_sampler_nearest_raw(&self) -> sys::ImDrawCallback {
        self.inner().DrawCallback_SetSamplerNearest
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

    unsafe extern "C" fn draw_callback_marker(
        _parent_list: *const sys::ImDrawList,
        _cmd: *const sys::ImDrawCmd,
    ) {
    }

    #[test]
    fn platform_io_textures_empty_is_safe() {
        let mut raw: sys::ImGuiPlatformIO = new_platform_io();

        raw.Textures.Size = 0;
        raw.Textures.Data = std::ptr::null_mut();
        let mut pio = PlatformIo {
            raw: UnsafeCell::new(raw),
        };
        assert_eq!(pio.textures().count(), 0);
        assert!(pio.textures_mut().next().is_none());
        assert_eq!(pio.textures_count(), 0);

        let mut raw: sys::ImGuiPlatformIO = new_platform_io();
        raw.Textures.Size = 1;
        raw.Textures.Data = std::ptr::null_mut();
        let mut pio = PlatformIo {
            raw: UnsafeCell::new(raw),
        };
        assert_eq!(pio.textures().count(), 0);
        assert!(pio.textures_mut().next().is_none());
        assert_eq!(pio.textures_count(), 0);
        assert!(pio.texture(0).is_none());
    }

    #[test]
    fn apply_texture_feedback_matches_typed_managed_texture_ids() {
        let mut texture = crate::texture::TextureData::new();
        unsafe {
            (*texture.as_raw_mut()).UniqueID = 314;
        }
        texture.set_status(crate::texture::TextureStatus::WantCreate);
        let id = texture.unique_id();

        let mut texture_ptr = texture.as_mut().as_raw_mut();
        let mut raw: sys::ImGuiPlatformIO = new_platform_io();
        raw.Textures.Size = 1;
        raw.Textures.Capacity = 1;
        raw.Textures.Data = &mut texture_ptr;

        let mut pio = PlatformIo {
            raw: UnsafeCell::new(raw),
        };
        let applied = pio.apply_texture_feedback(&[crate::render::snapshot::TextureFeedback {
            id,
            status: crate::texture::TextureStatus::OK,
            tex_id: Some(crate::texture::TextureId::new(99)),
        }]);

        assert_eq!(applied, 1);
        assert_eq!(texture.status(), crate::texture::TextureStatus::OK);
        assert_eq!(texture.tex_id(), crate::texture::TextureId::new(99));
    }

    #[test]
    fn platform_io_from_raw_matches_mut_wrapper() {
        let mut raw: sys::ImGuiPlatformIO = new_platform_io();
        let raw_ptr = (&mut raw) as *mut sys::ImGuiPlatformIO;

        let shared = unsafe { PlatformIo::from_raw(raw_ptr.cast_const()) };
        let mutable = unsafe { PlatformIo::from_raw_mut(raw_ptr) };

        assert_eq!(shared.as_raw(), mutable.as_raw());
    }

    #[test]
    fn platform_io_standard_draw_callback_accessors_roundtrip() {
        let mut raw: sys::ImGuiPlatformIO = new_platform_io();
        let pio = unsafe { PlatformIo::from_raw_mut((&mut raw) as *mut sys::ImGuiPlatformIO) };

        pio.set_draw_callback_reset_render_state_raw(Some(draw_callback_marker));
        pio.set_draw_callback_set_sampler_linear_raw(Some(draw_callback_marker));
        pio.set_draw_callback_set_sampler_nearest_raw(Some(draw_callback_marker));

        assert_eq!(
            pio.draw_callback_reset_render_state_raw()
                .map(|f| f as usize),
            Some(draw_callback_marker as usize)
        );
        assert_eq!(
            pio.draw_callback_set_sampler_linear_raw()
                .map(|f| f as usize),
            Some(draw_callback_marker as usize)
        );
        assert_eq!(
            pio.draw_callback_set_sampler_nearest_raw()
                .map(|f| f as usize),
            Some(draw_callback_marker as usize)
        );
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

    #[cfg(feature = "multi-viewport")]
    #[test]
    fn clear_platform_handlers_clears_typed_get_window_callbacks() {
        unsafe extern "C" fn get_pos(_viewport: *mut sys::ImGuiViewport, out: *mut sys::ImVec2) {
            if let Some(out) = unsafe { out.as_mut() } {
                *out = sys::ImVec2 { x: 41.0, y: 42.0 };
            }
        }
        unsafe extern "C" fn get_size(_viewport: *mut sys::ImGuiViewport, out: *mut sys::ImVec2) {
            if let Some(out) = unsafe { out.as_mut() } {
                *out = sys::ImVec2 { x: 43.0, y: 44.0 };
            }
        }
        unsafe extern "C" fn get_scale(_viewport: *mut sys::ImGuiViewport, out: *mut sys::ImVec2) {
            if let Some(out) = unsafe { out.as_mut() } {
                *out = sys::ImVec2 { x: 45.0, y: 46.0 };
            }
        }
        unsafe extern "C" fn get_insets(_viewport: *mut sys::ImGuiViewport, out: *mut sys::ImVec4) {
            if let Some(out) = unsafe { out.as_mut() } {
                *out = sys::ImVec4::new(1.0, 2.0, 3.0, 4.0);
            }
        }

        let mut ctx = crate::Context::create();
        let pio = ctx.platform_io_mut();
        pio.set_platform_get_window_pos_raw(Some(get_pos));
        pio.set_platform_get_window_size_raw(Some(get_size));
        pio.set_platform_get_window_framebuffer_scale_raw(Some(get_scale));
        pio.set_platform_get_window_work_area_insets_raw(Some(get_insets));
        pio.clear_platform_handlers();

        let raw = unsafe { &*pio.as_raw() };
        assert!(raw.Platform_GetWindowPos.is_none());
        assert!(raw.Platform_GetWindowSize.is_none());
        assert!(raw.Platform_GetWindowFramebufferScale.is_none());
        assert!(raw.Platform_GetWindowWorkAreaInsets.is_none());

        let viewport = std::ptr::NonNull::<sys::ImGuiViewport>::dangling().as_ptr();
        let mut pos = sys::ImVec2 { x: 1.0, y: 1.0 };
        let mut size = sys::ImVec2 { x: 1.0, y: 1.0 };
        let mut scale = sys::ImVec2 { x: 2.0, y: 2.0 };
        let mut insets = sys::ImVec4::new(9.0, 9.0, 9.0, 9.0);
        unsafe {
            trampolines::platform_get_window_pos_out(viewport, &mut pos);
            trampolines::platform_get_window_size_out(viewport, &mut size);
            trampolines::platform_get_window_framebuffer_scale_out(viewport, &mut scale);
            trampolines::platform_get_window_work_area_insets_out(viewport, &mut insets);
        }

        assert_eq!((pos.x, pos.y), (0.0, 0.0));
        assert_eq!((size.x, size.y), (0.0, 0.0));
        assert_eq!((scale.x, scale.y), (1.0, 1.0));
        assert_eq!(
            (insets.x, insets.y, insets.z, insets.w),
            (0.0, 0.0, 0.0, 0.0)
        );
    }

    #[cfg(feature = "multi-viewport")]
    #[test]
    fn get_window_pos_and_size_callbacks_are_context_local() {
        unsafe extern "C" fn get_pos_a(_viewport: *mut sys::ImGuiViewport, out: *mut sys::ImVec2) {
            if let Some(out) = unsafe { out.as_mut() } {
                *out = sys::ImVec2 { x: 11.0, y: 12.0 };
            }
        }
        unsafe extern "C" fn get_size_a(_viewport: *mut sys::ImGuiViewport, out: *mut sys::ImVec2) {
            if let Some(out) = unsafe { out.as_mut() } {
                *out = sys::ImVec2 { x: 13.0, y: 14.0 };
            }
        }
        unsafe extern "C" fn get_pos_b(_viewport: *mut sys::ImGuiViewport, out: *mut sys::ImVec2) {
            if let Some(out) = unsafe { out.as_mut() } {
                *out = sys::ImVec2 { x: 21.0, y: 22.0 };
            }
        }
        unsafe extern "C" fn get_size_b(_viewport: *mut sys::ImGuiViewport, out: *mut sys::ImVec2) {
            if let Some(out) = unsafe { out.as_mut() } {
                *out = sys::ImVec2 { x: 23.0, y: 24.0 };
            }
        }
        unsafe extern "C" fn get_scale_a(
            _viewport: *mut sys::ImGuiViewport,
            out: *mut sys::ImVec2,
        ) {
            if let Some(out) = unsafe { out.as_mut() } {
                *out = sys::ImVec2 { x: 1.0, y: 2.0 };
            }
        }
        unsafe extern "C" fn get_scale_b(
            _viewport: *mut sys::ImGuiViewport,
            out: *mut sys::ImVec2,
        ) {
            if let Some(out) = unsafe { out.as_mut() } {
                *out = sys::ImVec2 { x: 3.0, y: 4.0 };
            }
        }
        unsafe extern "C" fn get_insets_a(
            _viewport: *mut sys::ImGuiViewport,
            out: *mut sys::ImVec4,
        ) {
            if let Some(out) = unsafe { out.as_mut() } {
                *out = sys::ImVec4::new(1.0, 2.0, 3.0, 4.0);
            }
        }
        unsafe extern "C" fn get_insets_b(
            _viewport: *mut sys::ImGuiViewport,
            out: *mut sys::ImVec4,
        ) {
            if let Some(out) = unsafe { out.as_mut() } {
                *out = sys::ImVec4::new(5.0, 6.0, 7.0, 8.0);
            }
        }

        let mut ctx_a = crate::Context::create();
        let language_user_data_a = std::ptr::NonNull::<u8>::dangling().as_ptr().cast();
        ctx_a
            .io_mut()
            .set_backend_language_user_data(language_user_data_a);
        ctx_a
            .platform_io_mut()
            .set_platform_get_window_pos_raw(Some(get_pos_a));
        ctx_a
            .platform_io_mut()
            .set_platform_get_window_size_raw(Some(get_size_a));
        ctx_a
            .platform_io_mut()
            .set_platform_get_window_framebuffer_scale_raw(Some(get_scale_a));
        ctx_a
            .platform_io_mut()
            .set_platform_get_window_work_area_insets_raw(Some(get_insets_a));
        assert_eq!(
            ctx_a.io().backend_language_user_data(),
            language_user_data_a
        );

        let suspended_a = ctx_a.suspend();

        let mut ctx_b = crate::Context::create();
        let language_user_data_b = std::ptr::NonNull::<u16>::dangling().as_ptr().cast();
        ctx_b
            .io_mut()
            .set_backend_language_user_data(language_user_data_b);
        ctx_b
            .platform_io_mut()
            .set_platform_get_window_pos_raw(Some(get_pos_b));
        ctx_b
            .platform_io_mut()
            .set_platform_get_window_size_raw(Some(get_size_b));
        ctx_b
            .platform_io_mut()
            .set_platform_get_window_framebuffer_scale_raw(Some(get_scale_b));
        ctx_b
            .platform_io_mut()
            .set_platform_get_window_work_area_insets_raw(Some(get_insets_b));
        assert_eq!(
            ctx_b.io().backend_language_user_data(),
            language_user_data_b
        );

        let mut b_pos = sys::ImVec2 { x: 0.0, y: 0.0 };
        let mut b_size = sys::ImVec2 { x: 0.0, y: 0.0 };
        let mut b_scale = sys::ImVec2 { x: 0.0, y: 0.0 };
        let mut b_insets = sys::ImVec4::new(9.0, 9.0, 9.0, 9.0);
        unsafe {
            trampolines::platform_get_window_pos_out(std::ptr::null_mut(), &mut b_pos);
            trampolines::platform_get_window_size_out(std::ptr::null_mut(), &mut b_size);
            trampolines::platform_get_window_framebuffer_scale_out(
                std::ptr::null_mut(),
                &mut b_scale,
            );
            trampolines::platform_get_window_work_area_insets_out(
                std::ptr::null_mut(),
                &mut b_insets,
            );
        }
        assert_eq!((b_pos.x, b_pos.y), (0.0, 0.0));
        assert_eq!((b_size.x, b_size.y), (0.0, 0.0));
        assert_eq!((b_scale.x, b_scale.y), (1.0, 1.0));
        assert_eq!(
            (b_insets.x, b_insets.y, b_insets.z, b_insets.w),
            (0.0, 0.0, 0.0, 0.0)
        );

        let viewport_b = std::ptr::NonNull::<sys::ImGuiViewport>::dangling().as_ptr();
        unsafe {
            trampolines::platform_get_window_pos_out(viewport_b, &mut b_pos);
            trampolines::platform_get_window_size_out(viewport_b, &mut b_size);
            trampolines::platform_get_window_framebuffer_scale_out(viewport_b, &mut b_scale);
            trampolines::platform_get_window_work_area_insets_out(viewport_b, &mut b_insets);
        }
        assert_eq!((b_pos.x, b_pos.y), (21.0, 22.0));
        assert_eq!((b_size.x, b_size.y), (23.0, 24.0));
        assert_eq!((b_scale.x, b_scale.y), (3.0, 4.0));
        assert_eq!(
            (b_insets.x, b_insets.y, b_insets.z, b_insets.w),
            (5.0, 6.0, 7.0, 8.0)
        );

        let suspended_b = ctx_b.suspend();
        let ctx_a = suspended_a.activate().expect("ctx_a should activate");

        let mut a_pos = sys::ImVec2 { x: 0.0, y: 0.0 };
        let mut a_size = sys::ImVec2 { x: 0.0, y: 0.0 };
        let mut a_scale = sys::ImVec2 { x: 0.0, y: 0.0 };
        let mut a_insets = sys::ImVec4::zero();
        let viewport_a = std::ptr::NonNull::<sys::ImGuiViewport>::dangling().as_ptr();
        unsafe {
            trampolines::platform_get_window_pos_out(viewport_a, &mut a_pos);
            trampolines::platform_get_window_size_out(viewport_a, &mut a_size);
            trampolines::platform_get_window_framebuffer_scale_out(viewport_a, &mut a_scale);
            trampolines::platform_get_window_work_area_insets_out(viewport_a, &mut a_insets);
        }
        assert_eq!((a_pos.x, a_pos.y), (11.0, 12.0));
        assert_eq!((a_size.x, a_size.y), (13.0, 14.0));
        assert_eq!((a_scale.x, a_scale.y), (1.0, 2.0));
        assert_eq!(
            (a_insets.x, a_insets.y, a_insets.z, a_insets.w),
            (1.0, 2.0, 3.0, 4.0)
        );

        drop(ctx_a);
        drop(suspended_b);
    }

    #[cfg(feature = "multi-viewport")]
    #[test]
    fn typed_callback_setters_reject_non_current_platform_io() {
        unsafe extern "C" fn create_window(_viewport: *mut Viewport) {}

        let mut ctx_a = crate::Context::create();
        let pio_a = ctx_a.platform_io_mut().as_raw_mut();
        let suspended_a = ctx_a.suspend();

        let ctx_b = crate::Context::create();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
            PlatformIo::from_raw_mut(pio_a).set_platform_create_window(Some(create_window));
        }));

        assert!(result.is_err());

        drop(ctx_b);
        drop(suspended_a);
    }

    #[cfg(feature = "multi-viewport")]
    #[test]
    fn out_param_callback_setters_reject_non_current_platform_io() {
        unsafe extern "C" fn get_pos(_viewport: *mut sys::ImGuiViewport, out: *mut sys::ImVec2) {
            if let Some(out) = unsafe { out.as_mut() } {
                *out = sys::ImVec2 { x: 41.0, y: 42.0 };
            }
        }

        let mut ctx_a = crate::Context::create();
        let pio_a = ctx_a.platform_io_mut().as_raw_mut();
        let suspended_a = ctx_a.suspend();

        let ctx_b = crate::Context::create();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
            PlatformIo::from_raw_mut(pio_a).set_platform_get_window_pos_raw(Some(get_pos));
        }));

        assert!(result.is_err());

        drop(ctx_b);
        drop(suspended_a);
    }

    #[cfg(feature = "multi-viewport")]
    #[test]
    fn clear_handlers_target_receiver_platform_io_not_current_context() {
        unsafe extern "C" fn get_pos(_viewport: *mut sys::ImGuiViewport, out: *mut sys::ImVec2) {
            if let Some(out) = unsafe { out.as_mut() } {
                *out = sys::ImVec2 { x: 41.0, y: 42.0 };
            }
        }
        unsafe extern "C" fn create_window(_viewport: *mut Viewport) {}
        unsafe extern "C" fn renderer_window(_viewport: *mut Viewport) {}

        let mut ctx_a = crate::Context::create();
        let raw_a = ctx_a.as_raw();
        let pio_a = ctx_a.platform_io_mut().as_raw_mut();
        ctx_a
            .platform_io_mut()
            .set_platform_get_window_pos_raw(Some(get_pos));
        unsafe {
            ctx_a
                .platform_io_mut()
                .set_platform_create_window(Some(create_window));
            ctx_a
                .platform_io_mut()
                .set_renderer_create_window(Some(renderer_window));
        }
        let suspended_a = ctx_a.suspend();

        let ctx_b = crate::Context::create();
        let raw_b = ctx_b.as_raw();

        unsafe {
            PlatformIo::from_raw_mut(pio_a).clear_platform_handlers();
            PlatformIo::from_raw_mut(pio_a).clear_renderer_handlers();
        }

        unsafe {
            assert_eq!(sys::igGetCurrentContext(), raw_b);

            let raw = &*pio_a;
            assert!(raw.Platform_GetWindowPos.is_none());
            assert!(raw.Platform_CreateWindow.is_none());
            assert!(raw.Renderer_CreateWindow.is_none());

            sys::igSetCurrentContext(raw_a);
        }

        let viewport = std::ptr::NonNull::<sys::ImGuiViewport>::dangling().as_ptr();
        let mut pos = sys::ImVec2 { x: 1.0, y: 1.0 };
        unsafe {
            trampolines::platform_get_window_pos_out(viewport, &mut pos);
            trampolines::platform_create_window(viewport);
            trampolines::renderer_create_window(viewport);
        }

        assert_eq!((pos.x, pos.y), (0.0, 0.0));

        unsafe {
            sys::igSetCurrentContext(raw_b);
        }
        drop(ctx_b);
        drop(suspended_a);
    }

    #[cfg(feature = "multi-viewport")]
    #[test]
    fn raw_setters_clear_receiver_typed_callback_slots() {
        unsafe extern "C" fn create_window(_viewport: *mut Viewport) {
            CREATE_WINDOW_CALLS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
        unsafe extern "C" fn renderer_window(_viewport: *mut Viewport) {
            RENDERER_WINDOW_CALLS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }

        static CREATE_WINDOW_CALLS: std::sync::atomic::AtomicUsize =
            std::sync::atomic::AtomicUsize::new(0);
        static RENDERER_WINDOW_CALLS: std::sync::atomic::AtomicUsize =
            std::sync::atomic::AtomicUsize::new(0);

        CREATE_WINDOW_CALLS.store(0, std::sync::atomic::Ordering::SeqCst);
        RENDERER_WINDOW_CALLS.store(0, std::sync::atomic::Ordering::SeqCst);

        let mut ctx_a = crate::Context::create();
        let raw_a = ctx_a.as_raw();
        let pio_a = ctx_a.platform_io_mut().as_raw_mut();
        unsafe {
            ctx_a
                .platform_io_mut()
                .set_platform_create_window(Some(create_window));
            ctx_a
                .platform_io_mut()
                .set_renderer_create_window(Some(renderer_window));
        }

        let viewport = std::ptr::NonNull::<sys::ImGuiViewport>::dangling().as_ptr();
        unsafe {
            trampolines::platform_create_window(viewport);
            trampolines::renderer_create_window(viewport);
        }
        assert_eq!(
            CREATE_WINDOW_CALLS.load(std::sync::atomic::Ordering::SeqCst),
            1
        );
        assert_eq!(
            RENDERER_WINDOW_CALLS.load(std::sync::atomic::Ordering::SeqCst),
            1
        );

        let suspended_a = ctx_a.suspend();
        let ctx_b = crate::Context::create();
        let raw_b = ctx_b.as_raw();

        unsafe {
            PlatformIo::from_raw_mut(pio_a).set_platform_create_window_raw(None);
            PlatformIo::from_raw_mut(pio_a).set_renderer_create_window_raw(None);
            assert_eq!(sys::igGetCurrentContext(), raw_b);
            sys::igSetCurrentContext(raw_a);
        }

        unsafe {
            trampolines::platform_create_window(viewport);
            trampolines::renderer_create_window(viewport);
        }
        assert_eq!(
            CREATE_WINDOW_CALLS.load(std::sync::atomic::Ordering::SeqCst),
            1
        );
        assert_eq!(
            RENDERER_WINDOW_CALLS.load(std::sync::atomic::Ordering::SeqCst),
            1
        );

        unsafe {
            sys::igSetCurrentContext(raw_b);
        }
        drop(ctx_b);
        drop(suspended_a);
    }
}

// TODO: Add more viewport functionality:
// - Platform window handle access
// - Renderer data access
// - DPI scale information
// - Monitor information
