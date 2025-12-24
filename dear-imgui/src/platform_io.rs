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
use std::ffi::c_void;
#[cfg(feature = "multi-viewport")]
use std::sync::Mutex;

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
mod trampolines {
    use super::*;
    use core::ffi::c_char;
    use std::panic::{AssertUnwindSafe, catch_unwind};

    // Platform callbacks
    pub static PLATFORM_CREATE_WINDOW_CB: Mutex<Option<unsafe extern "C" fn(*mut Viewport)>> =
        Mutex::new(None);
    pub static PLATFORM_DESTROY_WINDOW_CB: Mutex<Option<unsafe extern "C" fn(*mut Viewport)>> =
        Mutex::new(None);
    pub static PLATFORM_SHOW_WINDOW_CB: Mutex<Option<unsafe extern "C" fn(*mut Viewport)>> =
        Mutex::new(None);
    pub static PLATFORM_SET_WINDOW_POS_CB: Mutex<
        Option<unsafe extern "C" fn(*mut Viewport, sys::ImVec2)>,
    > = Mutex::new(None);
    pub static PLATFORM_GET_WINDOW_POS_CB: Mutex<
        Option<unsafe extern "C" fn(*mut Viewport) -> sys::ImVec2>,
    > = Mutex::new(None);
    pub static PLATFORM_SET_WINDOW_SIZE_CB: Mutex<
        Option<unsafe extern "C" fn(*mut Viewport, sys::ImVec2)>,
    > = Mutex::new(None);
    pub static PLATFORM_GET_WINDOW_SIZE_CB: Mutex<
        Option<unsafe extern "C" fn(*mut Viewport) -> sys::ImVec2>,
    > = Mutex::new(None);
    pub static PLATFORM_SET_WINDOW_FOCUS_CB: Mutex<Option<unsafe extern "C" fn(*mut Viewport)>> =
        Mutex::new(None);
    pub static PLATFORM_GET_WINDOW_FOCUS_CB: Mutex<
        Option<unsafe extern "C" fn(*mut Viewport) -> bool>,
    > = Mutex::new(None);
    pub static PLATFORM_GET_WINDOW_MINIMIZED_CB: Mutex<
        Option<unsafe extern "C" fn(*mut Viewport) -> bool>,
    > = Mutex::new(None);
    pub static PLATFORM_SET_WINDOW_TITLE_CB: Mutex<
        Option<unsafe extern "C" fn(*mut Viewport, *const c_char)>,
    > = Mutex::new(None);
    pub static PLATFORM_SET_WINDOW_ALPHA_CB: Mutex<
        Option<unsafe extern "C" fn(*mut Viewport, f32)>,
    > = Mutex::new(None);
    pub static PLATFORM_UPDATE_WINDOW_CB: Mutex<Option<unsafe extern "C" fn(*mut Viewport)>> =
        Mutex::new(None);
    pub static PLATFORM_RENDER_WINDOW_CB: Mutex<
        Option<unsafe extern "C" fn(*mut Viewport, *mut c_void)>,
    > = Mutex::new(None);
    pub static PLATFORM_SWAP_BUFFERS_CB: Mutex<
        Option<unsafe extern "C" fn(*mut Viewport, *mut c_void)>,
    > = Mutex::new(None);

    // Renderer callbacks
    pub static RENDERER_CREATE_WINDOW_CB: Mutex<Option<unsafe extern "C" fn(*mut Viewport)>> =
        Mutex::new(None);
    pub static RENDERER_DESTROY_WINDOW_CB: Mutex<Option<unsafe extern "C" fn(*mut Viewport)>> =
        Mutex::new(None);
    pub static RENDERER_SET_WINDOW_SIZE_CB: Mutex<
        Option<unsafe extern "C" fn(*mut Viewport, sys::ImVec2)>,
    > = Mutex::new(None);
    pub static RENDERER_RENDER_WINDOW_CB: Mutex<
        Option<unsafe extern "C" fn(*mut Viewport, *mut c_void)>,
    > = Mutex::new(None);
    pub static RENDERER_SWAP_BUFFERS_CB: Mutex<
        Option<unsafe extern "C" fn(*mut Viewport, *mut c_void)>,
    > = Mutex::new(None);

    #[inline]
    fn load_cb<T: Copy>(m: &Mutex<T>) -> T {
        match m.lock() {
            Ok(g) => *g,
            Err(poison) => *poison.into_inner(),
        }
    }

    #[inline]
    pub(super) fn store_cb<T: Copy>(m: &Mutex<Option<T>>, cb: Option<T>) {
        let mut g = match m.lock() {
            Ok(g) => g,
            Err(poison) => poison.into_inner(),
        };
        *g = cb;
    }

    #[inline]
    fn abort_if_panicked<T>(ctx: &str, res: Result<T, Box<dyn std::any::Any + Send>>) -> T {
        match res {
            Ok(v) => v,
            Err(_) => {
                eprintln!("dear-imgui-rs: panic in PlatformIO callback ({ctx})");
                std::process::abort();
            }
        }
    }

    // Trampolines for platform callbacks
    pub unsafe extern "C" fn platform_create_window(vp: *mut sys::ImGuiViewport) {
        if vp.is_null() {
            return;
        }
        if let Some(cb) = load_cb(&PLATFORM_CREATE_WINDOW_CB) {
            abort_if_panicked(
                "Platform_CreateWindow",
                catch_unwind(AssertUnwindSafe(|| unsafe { cb(vp as *mut Viewport) })),
            );
        }
    }
    pub unsafe extern "C" fn platform_destroy_window(vp: *mut sys::ImGuiViewport) {
        if vp.is_null() {
            return;
        }
        if let Some(cb) = load_cb(&PLATFORM_DESTROY_WINDOW_CB) {
            abort_if_panicked(
                "Platform_DestroyWindow",
                catch_unwind(AssertUnwindSafe(|| unsafe { cb(vp as *mut Viewport) })),
            );
        }
    }
    pub unsafe extern "C" fn platform_show_window(vp: *mut sys::ImGuiViewport) {
        if vp.is_null() {
            return;
        }
        if let Some(cb) = load_cb(&PLATFORM_SHOW_WINDOW_CB) {
            abort_if_panicked(
                "Platform_ShowWindow",
                catch_unwind(AssertUnwindSafe(|| unsafe { cb(vp as *mut Viewport) })),
            );
        }
    }
    pub unsafe extern "C" fn platform_set_window_pos(vp: *mut sys::ImGuiViewport, p: sys::ImVec2) {
        if vp.is_null() {
            return;
        }
        if let Some(cb) = load_cb(&PLATFORM_SET_WINDOW_POS_CB) {
            abort_if_panicked(
                "Platform_SetWindowPos",
                catch_unwind(AssertUnwindSafe(|| unsafe { cb(vp as *mut Viewport, p) })),
            );
        }
    }
    pub unsafe extern "C" fn platform_get_window_pos(vp: *mut sys::ImGuiViewport) -> sys::ImVec2 {
        if vp.is_null() {
            return sys::ImVec2 { x: 0.0, y: 0.0 };
        }
        if let Some(cb) = load_cb(&PLATFORM_GET_WINDOW_POS_CB) {
            return abort_if_panicked(
                "Platform_GetWindowPos",
                catch_unwind(AssertUnwindSafe(|| unsafe { cb(vp as *mut Viewport) })),
            );
        }
        sys::ImVec2 { x: 0.0, y: 0.0 }
    }
    pub unsafe extern "C" fn platform_set_window_size(vp: *mut sys::ImGuiViewport, s: sys::ImVec2) {
        if vp.is_null() {
            return;
        }
        if let Some(cb) = load_cb(&PLATFORM_SET_WINDOW_SIZE_CB) {
            abort_if_panicked(
                "Platform_SetWindowSize",
                catch_unwind(AssertUnwindSafe(|| unsafe { cb(vp as *mut Viewport, s) })),
            );
        }
    }
    pub unsafe extern "C" fn platform_get_window_size(vp: *mut sys::ImGuiViewport) -> sys::ImVec2 {
        if vp.is_null() {
            return sys::ImVec2 { x: 0.0, y: 0.0 };
        }
        if let Some(cb) = load_cb(&PLATFORM_GET_WINDOW_SIZE_CB) {
            return abort_if_panicked(
                "Platform_GetWindowSize",
                catch_unwind(AssertUnwindSafe(|| unsafe { cb(vp as *mut Viewport) })),
            );
        }
        sys::ImVec2 { x: 0.0, y: 0.0 }
    }
    pub unsafe extern "C" fn platform_set_window_focus(vp: *mut sys::ImGuiViewport) {
        if vp.is_null() {
            return;
        }
        if let Some(cb) = load_cb(&PLATFORM_SET_WINDOW_FOCUS_CB) {
            abort_if_panicked(
                "Platform_SetWindowFocus",
                catch_unwind(AssertUnwindSafe(|| unsafe { cb(vp as *mut Viewport) })),
            );
        }
    }
    pub unsafe extern "C" fn platform_get_window_focus(vp: *mut sys::ImGuiViewport) -> bool {
        if vp.is_null() {
            return false;
        }
        if let Some(cb) = load_cb(&PLATFORM_GET_WINDOW_FOCUS_CB) {
            return abort_if_panicked(
                "Platform_GetWindowFocus",
                catch_unwind(AssertUnwindSafe(|| unsafe { cb(vp as *mut Viewport) })),
            );
        }
        false
    }
    pub unsafe extern "C" fn platform_get_window_minimized(vp: *mut sys::ImGuiViewport) -> bool {
        if vp.is_null() {
            return false;
        }
        if let Some(cb) = load_cb(&PLATFORM_GET_WINDOW_MINIMIZED_CB) {
            return abort_if_panicked(
                "Platform_GetWindowMinimized",
                catch_unwind(AssertUnwindSafe(|| unsafe { cb(vp as *mut Viewport) })),
            );
        }
        false
    }
    pub unsafe extern "C" fn platform_set_window_title(
        vp: *mut sys::ImGuiViewport,
        title: *const c_char,
    ) {
        if vp.is_null() {
            return;
        }
        if let Some(cb) = load_cb(&PLATFORM_SET_WINDOW_TITLE_CB) {
            abort_if_panicked(
                "Platform_SetWindowTitle",
                catch_unwind(AssertUnwindSafe(|| unsafe {
                    cb(vp as *mut Viewport, title)
                })),
            );
        }
    }
    pub unsafe extern "C" fn platform_set_window_alpha(vp: *mut sys::ImGuiViewport, a: f32) {
        if vp.is_null() {
            return;
        }
        if let Some(cb) = load_cb(&PLATFORM_SET_WINDOW_ALPHA_CB) {
            abort_if_panicked(
                "Platform_SetWindowAlpha",
                catch_unwind(AssertUnwindSafe(|| unsafe { cb(vp as *mut Viewport, a) })),
            );
        }
    }
    pub unsafe extern "C" fn platform_update_window(vp: *mut sys::ImGuiViewport) {
        if vp.is_null() {
            return;
        }
        if let Some(cb) = load_cb(&PLATFORM_UPDATE_WINDOW_CB) {
            abort_if_panicked(
                "Platform_UpdateWindow",
                catch_unwind(AssertUnwindSafe(|| unsafe { cb(vp as *mut Viewport) })),
            );
        }
    }
    pub unsafe extern "C" fn platform_render_window(vp: *mut sys::ImGuiViewport, r: *mut c_void) {
        if vp.is_null() {
            return;
        }
        if let Some(cb) = load_cb(&PLATFORM_RENDER_WINDOW_CB) {
            abort_if_panicked(
                "Platform_RenderWindow",
                catch_unwind(AssertUnwindSafe(|| unsafe { cb(vp as *mut Viewport, r) })),
            );
        }
    }
    pub unsafe extern "C" fn platform_swap_buffers(vp: *mut sys::ImGuiViewport, r: *mut c_void) {
        if vp.is_null() {
            return;
        }
        if let Some(cb) = load_cb(&PLATFORM_SWAP_BUFFERS_CB) {
            abort_if_panicked(
                "Platform_SwapBuffers",
                catch_unwind(AssertUnwindSafe(|| unsafe { cb(vp as *mut Viewport, r) })),
            );
        }
    }

    // Trampolines for renderer callbacks
    pub unsafe extern "C" fn renderer_create_window(vp: *mut sys::ImGuiViewport) {
        if vp.is_null() {
            return;
        }
        if let Some(cb) = load_cb(&RENDERER_CREATE_WINDOW_CB) {
            abort_if_panicked(
                "Renderer_CreateWindow",
                catch_unwind(AssertUnwindSafe(|| unsafe { cb(vp as *mut Viewport) })),
            );
        }
    }
    pub unsafe extern "C" fn renderer_destroy_window(vp: *mut sys::ImGuiViewport) {
        if vp.is_null() {
            return;
        }
        if let Some(cb) = load_cb(&RENDERER_DESTROY_WINDOW_CB) {
            abort_if_panicked(
                "Renderer_DestroyWindow",
                catch_unwind(AssertUnwindSafe(|| unsafe { cb(vp as *mut Viewport) })),
            );
        }
    }
    pub unsafe extern "C" fn renderer_set_window_size(vp: *mut sys::ImGuiViewport, s: sys::ImVec2) {
        if vp.is_null() {
            return;
        }
        if let Some(cb) = load_cb(&RENDERER_SET_WINDOW_SIZE_CB) {
            abort_if_panicked(
                "Renderer_SetWindowSize",
                catch_unwind(AssertUnwindSafe(|| unsafe { cb(vp as *mut Viewport, s) })),
            );
        }
    }
    pub unsafe extern "C" fn renderer_render_window(vp: *mut sys::ImGuiViewport, r: *mut c_void) {
        if vp.is_null() {
            return;
        }
        if let Some(cb) = load_cb(&RENDERER_RENDER_WINDOW_CB) {
            abort_if_panicked(
                "Renderer_RenderWindow",
                catch_unwind(AssertUnwindSafe(|| unsafe { cb(vp as *mut Viewport, r) })),
            );
        }
    }
    pub unsafe extern "C" fn renderer_swap_buffers(vp: *mut sys::ImGuiViewport, r: *mut c_void) {
        if vp.is_null() {
            return;
        }
        if let Some(cb) = load_cb(&RENDERER_SWAP_BUFFERS_CB) {
            abort_if_panicked(
                "Renderer_SwapBuffers",
                catch_unwind(AssertUnwindSafe(|| unsafe { cb(vp as *mut Viewport, r) })),
            );
        }
    }
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
    pub(crate) unsafe fn from_raw<'a>(raw: *const sys::ImGuiPlatformIO) -> &'a Self {
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
        self.inner_mut().Platform_GetWindowPos = callback;
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
        store_cb(&PLATFORM_GET_WINDOW_POS_CB, callback);
        self.set_platform_get_window_pos_raw(callback.map(|_| {
            trampolines::platform_get_window_pos
                as unsafe extern "C" fn(*mut sys::ImGuiViewport) -> sys::ImVec2
        }));
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
        self.inner_mut().Platform_GetWindowSize = callback;
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
        store_cb(&PLATFORM_GET_WINDOW_SIZE_CB, callback);
        self.set_platform_get_window_size_raw(callback.map(|_| {
            trampolines::platform_get_window_size
                as unsafe extern "C" fn(*mut sys::ImGuiViewport) -> sys::ImVec2
        }));
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
    pub fn viewports(&self) -> &crate::internal::ImVector<*mut sys::ImGuiViewport> {
        unsafe {
            crate::internal::imvector_cast_ref::<
                *mut sys::ImGuiViewport,
                sys::ImVector_ImGuiViewportPtr,
            >(&self.inner().Viewports)
        }
    }

    /// Get mutable access to the viewports vector
    #[cfg(feature = "multi-viewport")]
    pub fn viewports_mut(&mut self) -> &mut crate::internal::ImVector<*mut sys::ImGuiViewport> {
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

/// Viewport structure for multi-viewport support
///
/// This is a transparent wrapper around `ImGuiViewport` that provides
/// safe access to viewport functionality.
#[repr(transparent)]
pub struct Viewport {
    raw: UnsafeCell<sys::ImGuiViewport>,
}

// Ensure the wrapper stays layout-compatible with the sys bindings.
const _: [(); std::mem::size_of::<sys::ImGuiViewport>()] = [(); std::mem::size_of::<Viewport>()];
const _: [(); std::mem::align_of::<sys::ImGuiViewport>()] = [(); std::mem::align_of::<Viewport>()];

impl Viewport {
    #[inline]
    fn inner(&self) -> &sys::ImGuiViewport {
        // Safety: `Viewport` is a view into ImGui-owned viewport state which may be mutated by
        // Dear ImGui and platform/renderer backends while Rust holds `&Viewport`.
        unsafe { &*self.raw.get() }
    }

    #[inline]
    fn inner_mut(&mut self) -> &mut sys::ImGuiViewport {
        unsafe { &mut *self.raw.get() }
    }

    /// Get a reference to the viewport from a raw pointer
    ///
    /// # Safety
    ///
    /// The caller must ensure that:
    /// - `raw` is non-null and points to a valid `ImGuiViewport`.
    /// - The viewport outlives the returned reference (e.g. it belongs to the
    ///   currently active ImGui context).
    pub(crate) unsafe fn from_raw<'a>(raw: *const sys::ImGuiViewport) -> &'a Self {
        unsafe { &*(raw as *const Self) }
    }

    /// Get a mutable reference to the viewport from a raw pointer
    ///
    /// # Safety
    ///
    /// The caller must ensure that:
    /// - `raw` is non-null and points to a valid `ImGuiViewport`.
    /// - The viewport outlives the returned reference (e.g. it belongs to the
    ///   currently active ImGui context).
    /// - No other references (shared or mutable) to the same viewport are alive.
    pub unsafe fn from_raw_mut<'a>(raw: *mut sys::ImGuiViewport) -> &'a mut Self {
        unsafe { &mut *(raw as *mut Self) }
    }

    /// Get the raw pointer to the underlying `ImGuiViewport`
    pub fn as_raw(&self) -> *const sys::ImGuiViewport {
        self.raw.get().cast_const()
    }

    /// Get the raw mutable pointer to the underlying `ImGuiViewport`
    pub fn as_raw_mut(&mut self) -> *mut sys::ImGuiViewport {
        self.raw.get()
    }

    /// Get the viewport ID
    pub fn id(&self) -> sys::ImGuiID {
        self.inner().ID
    }

    /// Set the viewport position
    pub fn set_pos(&mut self, pos: [f32; 2]) {
        self.inner_mut().Pos.x = pos[0];
        self.inner_mut().Pos.y = pos[1];
    }

    /// Get the viewport position
    pub fn pos(&self) -> [f32; 2] {
        [self.inner().Pos.x, self.inner().Pos.y]
    }

    /// Set the viewport size
    pub fn set_size(&mut self, size: [f32; 2]) {
        self.inner_mut().Size.x = size[0];
        self.inner_mut().Size.y = size[1];
    }

    /// Get the viewport size
    pub fn size(&self) -> [f32; 2] {
        [self.inner().Size.x, self.inner().Size.y]
    }

    /// Get the viewport work position (excluding menu bars, task bars, etc.)
    pub fn work_pos(&self) -> [f32; 2] {
        [self.inner().WorkPos.x, self.inner().WorkPos.y]
    }

    /// Get the viewport work size (excluding menu bars, task bars, etc.)
    pub fn work_size(&self) -> [f32; 2] {
        [self.inner().WorkSize.x, self.inner().WorkSize.y]
    }

    /// Check if this is the main viewport
    ///
    /// Note: Main viewport is typically identified by ID == 0 or by checking if it's not a platform window
    #[cfg(feature = "multi-viewport")]
    pub fn is_main(&self) -> bool {
        self.inner().ID == 0
            || (self.inner().Flags & (crate::ViewportFlags::IS_PLATFORM_WINDOW.bits())) == 0
    }

    #[cfg(not(feature = "multi-viewport"))]
    pub fn is_main(&self) -> bool {
        self.inner().ID == 0
    }

    /// Check if this is a platform window (not the main viewport)
    #[cfg(feature = "multi-viewport")]
    pub fn is_platform_window(&self) -> bool {
        (self.inner().Flags & (crate::ViewportFlags::IS_PLATFORM_WINDOW.bits())) != 0
    }

    #[cfg(not(feature = "multi-viewport"))]
    pub fn is_platform_window(&self) -> bool {
        false
    }

    /// Check if this is a platform monitor
    #[cfg(feature = "multi-viewport")]
    pub fn is_platform_monitor(&self) -> bool {
        (self.inner().Flags & (crate::ViewportFlags::IS_PLATFORM_MONITOR.bits())) != 0
    }

    #[cfg(not(feature = "multi-viewport"))]
    pub fn is_platform_monitor(&self) -> bool {
        false
    }

    /// Check if this viewport is owned by the application
    #[cfg(feature = "multi-viewport")]
    pub fn is_owned_by_app(&self) -> bool {
        (self.inner().Flags & (crate::ViewportFlags::OWNED_BY_APP.bits())) != 0
    }

    #[cfg(not(feature = "multi-viewport"))]
    pub fn is_owned_by_app(&self) -> bool {
        false
    }

    /// Get the platform user data
    pub fn platform_user_data(&self) -> *mut c_void {
        self.inner().PlatformUserData
    }

    /// Set the platform user data
    pub fn set_platform_user_data(&mut self, data: *mut c_void) {
        self.inner_mut().PlatformUserData = data;
    }

    /// Get the renderer user data
    pub fn renderer_user_data(&self) -> *mut c_void {
        self.inner().RendererUserData
    }

    /// Set the renderer user data
    pub fn set_renderer_user_data(&mut self, data: *mut c_void) {
        self.inner_mut().RendererUserData = data;
    }

    /// Get the platform handle
    pub fn platform_handle(&self) -> *mut c_void {
        self.inner().PlatformHandle
    }

    /// Set the platform handle
    pub fn set_platform_handle(&mut self, handle: *mut c_void) {
        self.inner_mut().PlatformHandle = handle;
    }

    /// Check if the platform window was created
    pub fn platform_window_created(&self) -> bool {
        self.inner().PlatformWindowCreated
    }

    /// Set whether the platform window was created
    pub fn set_platform_window_created(&mut self, created: bool) {
        self.inner_mut().PlatformWindowCreated = created;
    }

    /// Check if the platform requested move
    pub fn platform_request_move(&self) -> bool {
        self.inner().PlatformRequestMove
    }

    /// Set whether the platform requested move
    pub fn set_platform_request_move(&mut self, request: bool) {
        self.inner_mut().PlatformRequestMove = request;
    }

    /// Check if the platform requested resize
    pub fn platform_request_resize(&self) -> bool {
        self.inner().PlatformRequestResize
    }

    /// Set whether the platform requested resize
    pub fn set_platform_request_resize(&mut self, request: bool) {
        self.inner_mut().PlatformRequestResize = request;
    }

    /// Check if the platform requested close
    pub fn platform_request_close(&self) -> bool {
        self.inner().PlatformRequestClose
    }

    /// Set whether the platform requested close
    pub fn set_platform_request_close(&mut self, request: bool) {
        self.inner_mut().PlatformRequestClose = request;
    }

    /// Get the viewport flags
    pub fn flags(&self) -> sys::ImGuiViewportFlags {
        self.inner().Flags
    }

    /// Set the viewport flags
    pub fn set_flags(&mut self, flags: sys::ImGuiViewportFlags) {
        self.inner_mut().Flags = flags;
    }

    /// Get the DPI scale factor
    #[cfg(feature = "multi-viewport")]
    pub fn dpi_scale(&self) -> f32 {
        self.inner().DpiScale
    }

    /// Set the DPI scale factor
    #[cfg(feature = "multi-viewport")]
    pub fn set_dpi_scale(&mut self, scale: f32) {
        self.inner_mut().DpiScale = scale;
    }

    /// Get the parent viewport ID
    #[cfg(feature = "multi-viewport")]
    pub fn parent_viewport_id(&self) -> sys::ImGuiID {
        self.inner().ParentViewportId
    }

    /// Set the parent viewport ID
    #[cfg(feature = "multi-viewport")]
    pub fn set_parent_viewport_id(&mut self, id: sys::ImGuiID) {
        self.inner_mut().ParentViewportId = id;
    }

    /// Get the draw data pointer
    #[cfg(feature = "multi-viewport")]
    pub fn draw_data(&self) -> *mut sys::ImDrawData {
        self.inner().DrawData
    }

    /// Get the draw data as a reference (if available)
    #[cfg(feature = "multi-viewport")]
    pub fn draw_data_ref(&self) -> Option<&sys::ImDrawData> {
        if self.inner().DrawData.is_null() {
            None
        } else {
            Some(unsafe { &*self.inner().DrawData })
        }
    }

    /// Get the framebuffer scale
    #[cfg(feature = "multi-viewport")]
    pub fn framebuffer_scale(&self) -> [f32; 2] {
        [
            self.inner().FramebufferScale.x,
            self.inner().FramebufferScale.y,
        ]
    }

    /// Set the framebuffer scale
    #[cfg(feature = "multi-viewport")]
    pub fn set_framebuffer_scale(&mut self, scale: [f32; 2]) {
        self.inner_mut().FramebufferScale.x = scale[0];
        self.inner_mut().FramebufferScale.y = scale[1];
    }
}

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
}

// TODO: Add more viewport functionality:
// - Platform window handle access
// - Renderer data access
// - DPI scale information
// - Monitor information
