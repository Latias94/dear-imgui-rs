use crate::sys;
use std::cell::UnsafeCell;

/// Platform IO structure for multi-viewport support
///
/// This is a transparent wrapper around `ImGuiPlatformIO` that provides
/// safe access to platform-specific functionality.
///
/// Callback setters and clear operations act on this table, including while another Context is
/// bound. Invoke installed callbacks with the owning Context current and its live viewport. The
/// C++ aggregate hooks find the receiver's callback storage through that current Context.
#[repr(transparent)]
pub struct PlatformIo {
    pub(super) raw: UnsafeCell<sys::ImGuiPlatformIO>,
}

// Ensure the wrapper stays layout-compatible with the sys bindings.
const _: [(); std::mem::size_of::<sys::ImGuiPlatformIO>()] =
    [(); std::mem::size_of::<PlatformIo>()];
const _: [(); std::mem::align_of::<sys::ImGuiPlatformIO>()] =
    [(); std::mem::align_of::<PlatformIo>()];

#[cfg(feature = "multi-viewport")]
pub(super) unsafe fn clear_platform_aggregate_callbacks_for_platform_io(
    pio: *mut sys::ImGuiPlatformIO,
) {
    unsafe {
        sys::ImGuiPlatformIO_Set_Platform_SetWindowPos_PointerParam(pio, None);
        sys::ImGuiPlatformIO_Set_Platform_GetWindowPos_OutParam(pio, None);
        sys::ImGuiPlatformIO_Set_Platform_SetWindowSize_PointerParam(pio, None);
        sys::ImGuiPlatformIO_Set_Platform_GetWindowSize_OutParam(pio, None);
        sys::ImGuiPlatformIO_Set_Platform_GetWindowFramebufferScale_OutParam(pio, None);
        sys::ImGuiPlatformIO_Set_Platform_GetWindowWorkAreaInsets_OutParam(pio, None);
    }
}

#[cfg(feature = "multi-viewport")]
pub(super) unsafe fn clear_renderer_aggregate_callbacks_for_platform_io(
    pio: *mut sys::ImGuiPlatformIO,
) {
    unsafe { sys::ImGuiPlatformIO_Set_Renderer_SetWindowSize_PointerParam(pio, None) }
}

#[cfg(feature = "multi-viewport")]
pub(crate) unsafe fn clear_aggregate_callbacks_for_current_context() {
    unsafe {
        let pio = sys::igGetPlatformIO_Nil();
        clear_platform_aggregate_callbacks_for_platform_io(pio);
        clear_renderer_aggregate_callbacks_for_platform_io(pio);
    }
}

#[cfg(not(feature = "multi-viewport"))]
pub(crate) unsafe fn clear_aggregate_callbacks_for_current_context() {}

impl PlatformIo {
    #[inline]
    pub(super) fn inner(&self) -> &sys::ImGuiPlatformIO {
        // Safety: `PlatformIo` is a view into ImGui-owned platform state which may be mutated by
        // Dear ImGui while Rust holds `&PlatformIo`, so we store it behind `UnsafeCell` to make
        // that interior mutability explicit.
        unsafe { &*self.raw.get() }
    }

    #[inline]
    pub(super) fn inner_mut(&mut self) -> &mut sys::ImGuiPlatformIO {
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
    /// - If the table is managed through raw FFI, uninstall any aggregate callbacks installed
    ///   through this wrapper before freeing it. [`crate::Context`] handles its own teardown.
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
}
