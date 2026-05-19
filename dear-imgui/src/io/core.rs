use crate::io::assert_finite_f32;
use crate::sys;
use std::cell::UnsafeCell;

/// Settings and inputs/outputs for imgui-rs
/// This is a transparent wrapper around ImGuiIO
#[repr(transparent)]
pub struct Io(UnsafeCell<sys::ImGuiIO>);

// Ensure the wrapper stays layout-compatible with the sys bindings.
const _: [(); std::mem::size_of::<sys::ImGuiIO>()] = [(); std::mem::size_of::<Io>()];
const _: [(); std::mem::align_of::<sys::ImGuiIO>()] = [(); std::mem::align_of::<Io>()];

pub(crate) struct BoundContextGuard {
    previous: *mut sys::ImGuiContext,
    target: *mut sys::ImGuiContext,
}

impl BoundContextGuard {
    pub(crate) unsafe fn bind(target: *mut sys::ImGuiContext) -> Self {
        let previous = unsafe { sys::igGetCurrentContext() };
        if previous != target {
            unsafe { sys::igSetCurrentContext(target) };
        }
        Self { previous, target }
    }
}

impl Drop for BoundContextGuard {
    fn drop(&mut self) {
        if self.previous != self.target {
            unsafe {
                sys::igSetCurrentContext(self.previous);
            }
        }
    }
}

impl Io {
    #[inline]
    pub(crate) fn inner(&self) -> &sys::ImGuiIO {
        // Safety: `Io` is a transparent wrapper around the sys `ImGuiIO` value which is owned by
        // Dear ImGui. The value may be mutated by Dear ImGui even while Rust holds `&Io`, so we
        // store it behind `UnsafeCell` to make that interior mutability explicit.
        unsafe { &*self.0.get() }
    }

    #[inline]
    pub(crate) fn inner_mut(&mut self) -> &mut sys::ImGuiIO {
        // Safety: caller has `&mut Io`, so this is a unique Rust borrow for this wrapper.
        unsafe { &mut *self.0.get() }
    }

    pub(crate) fn context_ptr(&self, caller: &str) -> *mut sys::ImGuiContext {
        let ctx = self.inner().Ctx;
        assert!(!ctx.is_null(), "{caller} requires a valid ImGui context");
        ctx
    }

    fn frame_count(&self) -> i32 {
        let ctx = self.inner().Ctx;
        if ctx.is_null() {
            0
        } else {
            unsafe { (*ctx).FrameCount }
        }
    }

    pub(crate) fn assert_delta_time(&self, caller: &str, delta_time: f32) {
        assert_finite_f32(caller, "delta_time", delta_time);
        if self.frame_count() == 0 {
            assert!(
                delta_time >= 0.0,
                "{caller} delta_time must be non-negative before the first frame"
            );
        } else {
            assert!(
                delta_time > 0.0,
                "{caller} delta_time must be positive after the first frame"
            );
        }
    }
}
