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
