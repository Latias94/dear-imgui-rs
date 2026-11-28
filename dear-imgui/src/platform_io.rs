//! Platform IO functionality for Dear ImGui
//!
//! This module provides access to Dear ImGui's platform IO system, which handles
//! multi-viewport and platform-specific functionality.
//!
//! Main viewport identity is a property of the currently bound Context, not a stable numeric ID.
//! Use [`Viewport::is_main`] while inspecting a live viewport.
//!
//! ```compile_fail
//! use dear_imgui_rs::platform_io::MAIN_VIEWPORT_ID;
//! ```

use std::cell::UnsafeCell;
use std::num::NonZeroU64;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::context::ContextError;
use crate::{ContextId, Id, sys};

// Multi-viewport requires platform callbacks (C calling back into Rust). On the
// web (wasm32 import-style build), this is not supported yet, so we currently
// disable the `multi-viewport` feature for wasm at compile time.
#[cfg(all(target_arch = "wasm32", feature = "multi-viewport"))]
compile_error!("The `multi-viewport` feature is not supported on wasm32 targets yet.");

#[cfg(feature = "multi-viewport")]
mod platform_callbacks;
mod renderer_callbacks;
#[cfg(test)]
mod tests;
mod viewport;

pub use viewport::Viewport;

static NEXT_NATIVE_WINDOW_INSTANCE: AtomicU64 = AtomicU64::new(1);

/// Identity of one native window registration owned by a platform backend.
///
/// This identity is independent of both the operating system's reusable window identifier and
/// Dear ImGui's mutable viewport routing ID. Backends allocate it when publishing a native window,
/// preserve it across docking changes, and retire it when releasing that registration. Reattaching
/// even the same host window after retirement creates a new instance.
///
/// The value contains no native handle and can outlive the Context and window. It is metadata,
/// not a capability to access a live window; lookups must validate the current platform owner.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct NativeWindowInstanceId {
    context_id: ContextId,
    serial: NonZeroU64,
}

impl NativeWindowInstanceId {
    /// Allocates a process-unique registration identity for a platform backend.
    ///
    /// Allocation never wraps, including across backend shutdown and reattachment. An identity
    /// does not establish native ownership by itself. Custom platform backends must publish it
    /// transactionally with their own native window registration.
    ///
    /// ```
    /// use dear_imgui_rs::{Context, NativeWindowInstanceId};
    /// let context = Context::create();
    /// let first = NativeWindowInstanceId::allocate(context.id())?;
    /// let reopened = NativeWindowInstanceId::allocate(context.id())?;
    /// assert_ne!(first, reopened);
    /// assert_eq!(first.context_id(), context.id());
    /// # Ok::<(), dear_imgui_rs::ContextError>(())
    /// ```
    pub fn allocate(context_id: ContextId) -> Result<Self, ContextError> {
        let serial = allocate_serial(&NEXT_NATIVE_WINDOW_INSTANCE)?;
        Ok(Self { context_id, serial })
    }

    /// Returns the Context that owned this registration.
    pub fn context_id(self) -> ContextId {
        self.context_id
    }

    /// Returns the process-unique serial, without granting native handle access.
    pub fn get(self) -> NonZeroU64 {
        self.serial
    }
}

fn allocate_serial(counter: &AtomicU64) -> Result<NonZeroU64, ContextError> {
    let value = counter
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1)
        })
        .map_err(|_| ContextError::NativeWindowIdentityExhausted)?;
    NonZeroU64::new(value).ok_or(ContextError::NativeWindowIdentityExhausted)
}

/// Owned association between a native allocation and its viewport at observation time.
///
/// Copy this metadata into frame reports instead of resolving an old viewport ID against a newer
/// registry. The routing ID may change while `instance_id` remains the same. This snapshot retains
/// no window, Context, platform pointer, or operating-system handle.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct NativeWindowTarget {
    /// Stable identity of the native registration.
    pub instance_id: NativeWindowInstanceId,
    /// Dear ImGui routing ID when the association was observed.
    pub viewport_id: Id,
    /// Whether this was the Context's main viewport when observed.
    pub is_main: bool,
}

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
    raw: UnsafeCell<sys::ImGuiPlatformIO>,
}

// Ensure the wrapper stays layout-compatible with the sys bindings.
const _: [(); std::mem::size_of::<sys::ImGuiPlatformIO>()] =
    [(); std::mem::size_of::<PlatformIo>()];
const _: [(); std::mem::align_of::<sys::ImGuiPlatformIO>()] =
    [(); std::mem::align_of::<PlatformIo>()];

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

    /// Replace Dear ImGui's platform monitor list.
    ///
    /// Multi-viewport backends must keep at least one monitor in `PlatformIO.Monitors` before
    /// enabling `ConfigFlags::VIEWPORTS_ENABLE`. The vector storage is allocated with Dear ImGui's
    /// allocator so the context can safely own and release it.
    ///
    /// # Safety
    ///
    /// Every non-null `PlatformHandle` must use the representation expected by the installed
    /// platform callbacks and remain valid for as long as the monitor entry can be observed.
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_monitors(&mut self, monitors: &[sys::ImGuiPlatformMonitor]) {
        assert_monitor_contract(monitors, "PlatformIo::set_monitors()");
        let count = i32::try_from(monitors.len())
            .expect("PlatformIo::set_monitors() supports at most i32::MAX monitors");

        // Allocate the replacement before releasing the old vector so a failed allocation leaves
        // the previously valid monitor contract intact.
        let data = if monitors.is_empty() {
            std::ptr::null_mut()
        } else {
            let byte_len = std::mem::size_of_val(monitors);
            let data = unsafe { sys::igMemAlloc(byte_len) }.cast::<sys::ImGuiPlatformMonitor>();
            assert!(
                !data.is_null(),
                "PlatformIo::set_monitors() failed to allocate monitor storage"
            );
            unsafe {
                data.copy_from_nonoverlapping(monitors.as_ptr(), monitors.len());
            }
            data
        };

        let raw = &mut self.inner_mut().Monitors;
        if !raw.Data.is_null() {
            unsafe { sys::igMemFree(raw.Data.cast()) };
        }
        raw.Data = data;
        raw.Size = count;
        raw.Capacity = count;
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
}

#[cfg(feature = "multi-viewport")]
pub(crate) fn assert_monitor_contract(monitors: &[sys::ImGuiPlatformMonitor], caller: &str) {
    for (index, monitor) in monitors.iter().enumerate() {
        let values = [
            monitor.MainPos.x,
            monitor.MainPos.y,
            monitor.MainSize.x,
            monitor.MainSize.y,
            monitor.WorkPos.x,
            monitor.WorkPos.y,
            monitor.WorkSize.x,
            monitor.WorkSize.y,
            monitor.DpiScale,
        ];
        assert!(
            values.iter().all(|value| value.is_finite()),
            "{caller} rejected monitor {index}: geometry and DPI values must be finite"
        );
        assert!(
            monitor.MainSize.x > 0.0 && monitor.MainSize.y > 0.0,
            "{caller} rejected monitor {index}: MainSize must be positive"
        );
        assert!(
            monitor.WorkSize.x >= 0.0 && monitor.WorkSize.y >= 0.0,
            "{caller} rejected monitor {index}: WorkSize must not be negative"
        );

        let main_max = [
            monitor.MainPos.x + monitor.MainSize.x,
            monitor.MainPos.y + monitor.MainSize.y,
        ];
        let work_max = [
            monitor.WorkPos.x + monitor.WorkSize.x,
            monitor.WorkPos.y + monitor.WorkSize.y,
        ];
        assert!(
            main_max
                .iter()
                .chain(work_max.iter())
                .all(|value| value.is_finite()),
            "{caller} rejected monitor {index}: geometry bounds must not overflow"
        );
        assert!(
            monitor.WorkPos.x >= monitor.MainPos.x
                && monitor.WorkPos.y >= monitor.MainPos.y
                && work_max[0] <= main_max[0]
                && work_max[1] <= main_max[1],
            "{caller} rejected monitor {index}: WorkPos/WorkSize must be contained within MainPos/MainSize"
        );
        assert!(
            monitor.DpiScale > 0.0 && monitor.DpiScale < 99.0,
            "{caller} rejected monitor {index}: DpiScale must be greater than 0 and less than 99"
        );
    }
}

pub(crate) unsafe fn clear_aggregate_callbacks_for_current_context() {
    #[cfg(feature = "multi-viewport")]
    unsafe {
        let pio = sys::igGetPlatformIO_Nil();
        platform_callbacks::clear_platform_aggregate_callbacks_for_platform_io(pio);
        renderer_callbacks::clear_renderer_aggregate_callbacks_for_platform_io(pio);
    }
}

#[cfg(feature = "multi-viewport")]
mod callback_abi {
    //! Pointer ABI conversions for typed viewport callbacks. Native backends save and reinstall these
    //! function pointers, so each pointer must continue to call its original function after a slot
    //! is replaced. The C++ shim handles every by-value C++ aggregate.

    use super::Viewport;
    use crate::sys;
    use std::ffi::{c_char, c_void};

    macro_rules! viewport_callback {
        ($name:ident($($arg:ty),*) $(-> $result:ty)?) => {
            pub(super) fn $name(
                callback: Option<unsafe extern "C" fn(*mut Viewport $(, $arg)*) $(-> $result)?>,
            ) -> Option<unsafe extern "C" fn(*mut sys::ImGuiViewport $(, $arg)*) $(-> $result)?> {
                type Typed = unsafe extern "C" fn(*mut Viewport $(, $arg)*) $(-> $result)?;
                type Raw = unsafe extern "C" fn(*mut sys::ImGuiViewport $(, $arg)*) $(-> $result)?;
                // SAFETY: Only the pointee of the first thin pointer changes. Viewport is a
                // transparent view of ImGuiViewport. All other parameters, return type and ABI match.
                callback.map(|f| unsafe { std::mem::transmute::<Typed, Raw>(f) })
            }
        };
    }

    viewport_callback!(window());
    viewport_callback!(predicate() -> bool);
    viewport_callback!(dpi_scale() -> f32);
    viewport_callback!(title(*const c_char));
    viewport_callback!(alpha(f32));
    viewport_callback!(render(*mut c_void));
    viewport_callback!(vec2_output(*mut sys::ImVec2));
    viewport_callback!(vec4_output(*mut sys::ImVec4));

    pub(super) fn vec2_input(
        callback: Option<unsafe extern "C" fn(*mut Viewport, &sys::ImVec2)>,
    ) -> Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, *const sys::ImVec2)> {
        type Typed = unsafe extern "C" fn(*mut Viewport, &sys::ImVec2);
        type Raw = unsafe extern "C" fn(*mut sys::ImGuiViewport, *const sys::ImVec2);
        // SAFETY: The C++ thunk passes a pointer to its live, aligned local ImVec2 for the entire
        // call. The shared reference has the same ABI as that pointer, and the viewport is transparent.
        callback.map(|f| unsafe { std::mem::transmute::<Typed, Raw>(f) })
    }
}
