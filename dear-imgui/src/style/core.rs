use crate::internal::RawWrapper;
use crate::sys;
use std::cell::UnsafeCell;

/// User interface style/colors
///
/// Note: This is a transparent wrapper over `sys::ImGuiStyle` (v1.92+ layout).
/// Do not assume field layout here; use accessors or `raw()/raw_mut()` if needed.
#[repr(transparent)]
#[derive(Debug)]
pub struct Style(pub(crate) UnsafeCell<sys::ImGuiStyle>);

// Ensure the wrapper stays layout-compatible with the sys bindings.
const _: [(); std::mem::size_of::<sys::ImGuiStyle>()] = [(); std::mem::size_of::<Style>()];
const _: [(); std::mem::align_of::<sys::ImGuiStyle>()] = [(); std::mem::align_of::<Style>()];

impl Style {
    /// Scale every size and spacing value using Dear ImGui's DPI-aware rules.
    ///
    /// This also updates Dear ImGui's internal main style scale and preserves
    /// sentinel values used by selected style fields.
    #[doc(alias = "ScaleAllSizes")]
    pub fn scale_all_sizes(&mut self, scale_factor: f32) {
        assert!(
            scale_factor.is_finite() && scale_factor > 0.0,
            "Style::scale_all_sizes() scale_factor must be finite and positive"
        );
        unsafe {
            sys::ImGuiStyle_ScaleAllSizes(self.inner_mut(), scale_factor);
        }
    }

    #[inline]
    pub(super) fn inner(&self) -> &sys::ImGuiStyle {
        // Safety: `Style` is a view into ImGui-owned style data. Dear ImGui can update style state
        // (e.g. via push/pop stacks or user code) while Rust holds `&Style`, so we store it behind
        // `UnsafeCell` to make that interior mutability explicit.
        unsafe { &*self.0.get() }
    }

    #[inline]
    pub(super) fn inner_mut(&mut self) -> &mut sys::ImGuiStyle {
        // Safety: caller has `&mut Style`, so this is a unique Rust borrow for this wrapper.
        unsafe { &mut *self.0.get() }
    }
}

impl Clone for Style {
    fn clone(&self) -> Self {
        Self(UnsafeCell::new(*self.inner()))
    }
}

impl PartialEq for Style {
    fn eq(&self, other: &Self) -> bool {
        *self.inner() == *other.inner()
    }
}

impl RawWrapper for Style {
    type Raw = sys::ImGuiStyle;

    unsafe fn raw(&self) -> &Self::Raw {
        self.inner()
    }

    unsafe fn raw_mut(&mut self) -> &mut Self::Raw {
        self.inner_mut()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn scale_all_sizes_uses_upstream_rounding_and_rejects_invalid_factors() {
        let mut ctx = crate::Context::create();
        let style = ctx.style_mut();
        style.set_window_padding([3.0, 5.0]);
        style.scale_all_sizes(2.0);
        assert_eq!(style.window_padding(), [6.0, 10.0]);

        for scale_factor in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    style.scale_all_sizes(scale_factor);
                }))
                .is_err()
            );
        }
    }
}
