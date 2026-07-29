use crate::sys;
use std::cell::UnsafeCell;

#[cfg(feature = "multi-viewport")]
use super::trampolines;

/// Platform IO structure for multi-viewport support
///
/// This is a transparent wrapper around `ImGuiPlatformIO` that provides
/// safe access to platform-specific functionality.
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
pub(crate) fn clear_typed_callbacks_for_context(ctx: *mut sys::ImGuiContext) {
    trampolines::clear_callbacks_for_context(ctx);
}

#[cfg(not(feature = "multi-viewport"))]
pub(crate) fn clear_typed_callbacks_for_context(_ctx: *mut sys::ImGuiContext) {}

#[cfg(feature = "multi-viewport")]
pub(crate) unsafe fn clear_platform_aggregate_callbacks_for_current_context() {
    let pio = unsafe { sys::igGetPlatformIO_Nil() };
    if pio.is_null() {
        return;
    }
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
pub(super) unsafe fn clear_platform_aggregate_callbacks_for_platform_io(
    pio: *mut sys::ImGuiPlatformIO,
) {
    if pio.is_null() {
        return;
    }
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
pub(crate) unsafe fn clear_renderer_aggregate_callbacks_for_current_context() {
    let pio = unsafe { sys::igGetPlatformIO_Nil() };
    if !pio.is_null() {
        unsafe { sys::ImGuiPlatformIO_Set_Renderer_SetWindowSize_PointerParam(pio, None) }
    }
}

#[cfg(feature = "multi-viewport")]
pub(super) unsafe fn clear_renderer_aggregate_callbacks_for_platform_io(
    pio: *mut sys::ImGuiPlatformIO,
) {
    if !pio.is_null() {
        unsafe { sys::ImGuiPlatformIO_Set_Renderer_SetWindowSize_PointerParam(pio, None) }
    }
}

#[cfg(feature = "multi-viewport")]
pub(crate) unsafe fn clear_aggregate_callbacks_for_current_context() {
    unsafe {
        clear_platform_aggregate_callbacks_for_current_context();
        clear_renderer_aggregate_callbacks_for_current_context();
    }
}

#[cfg(not(feature = "multi-viewport"))]
pub(crate) unsafe fn clear_aggregate_callbacks_for_current_context() {}

#[cfg(feature = "multi-viewport")]
pub(super) fn assert_platform_io_aggregate_hooks_available(callback_name: &str) {
    assert!(
        sys::HAS_PLATFORM_IO_AGGREGATE_HOOKS,
        "dear-imgui-sys was built without PlatformIO aggregate ABI hooks; \
         rebuild without IMGUI_SYS_SKIP_CC to install {callback_name} callbacks"
    );
}

fn validate_session_date(caller: &str, date: u32) {
    let year = date / 10_000;
    let month = (date / 100) % 100;
    let day = date % 100;
    assert!(
        (1000..=9999).contains(&year),
        "{caller} date must contain a four-digit year"
    );
    let days_in_month = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if year.is_multiple_of(400) || (year.is_multiple_of(4) && !year.is_multiple_of(100)) => {
            29
        }
        2 => 28,
        _ => panic!("{caller} date contains an invalid month"),
    };
    assert!(
        (1..=days_in_month).contains(&day),
        "{caller} date contains an invalid day"
    );
}

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

    /// Returns the application session date as `YYYYMMDD`, when available.
    ///
    /// Dear ImGui initializes this from the platform clock and uses it for dated `.ini` entries.
    #[doc(alias = "Platform_SessionDate")]
    pub fn session_date(&self) -> Option<u32> {
        let raw = self.inner().Platform_SessionDate;
        let date = u32::try_from(raw)
            .expect("PlatformIo::session_date() found a negative raw session date");
        if date == 0 {
            return None;
        }
        validate_session_date("PlatformIo::session_date()", date);
        Some(date)
    }

    /// Overrides the application session date stored as `YYYYMMDD`.
    ///
    /// Custom platform integrations can set this when Dear ImGui is built without its time
    /// functions. `None` disables date-based settings retention for the session.
    #[doc(alias = "Platform_SessionDate")]
    pub fn set_session_date(&mut self, date: Option<u32>) {
        self.inner_mut().Platform_SessionDate = match date {
            Some(date) => {
                validate_session_date("PlatformIo::set_session_date()", date);
                i32::try_from(date).expect("PlatformIo::set_session_date() date must fit in an i32")
            }
            None => 0,
        };
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
    pub(super) fn assert_current_context_platform_io_for_callbacks(&self) {
        assert!(
            self.is_current_context_platform_io(),
            "PlatformIo typed/out-parameter callback setters must be called on the active ImGui context's PlatformIo"
        );
    }

    #[cfg(feature = "multi-viewport")]
    pub(super) fn store_current_context_cb<T: Copy>(
        &self,
        slot: &trampolines::CallbackSlot<T>,
        callback: Option<T>,
    ) {
        self.assert_current_context_platform_io_for_callbacks();
        trampolines::store_cb(slot, callback);
    }

    #[cfg(feature = "multi-viewport")]
    pub(super) fn clear_current_context_cb<T: Copy>(&self, slot: &trampolines::CallbackSlot<T>) {
        self.assert_current_context_platform_io_for_callbacks();
        trampolines::clear_cb_for_current_context(slot);
    }

    #[cfg(feature = "multi-viewport")]
    pub(super) fn clear_platform_io_cb<T: Copy>(&self, slot: &trampolines::CallbackSlot<T>) {
        trampolines::clear_cb_for_platform_io(self.as_raw(), slot);
    }
}
