use super::super::validation::validate_input_scalar_flags;
use crate::internal::{DataTypeKind, component_count_i32};
use crate::ui::Ui;
use crate::{InputScalarFlags, sys};
use std::ffi::c_void;
use std::ptr;

/// Builder for an input scalar widget.
#[must_use]
pub struct InputScalar<'ui, 'p, T, L, F = &'static str> {
    value: &'p mut T,
    label: L,
    step: Option<T>,
    step_fast: Option<T>,
    display_format: Option<F>,
    flags: InputScalarFlags,
    ui: &'ui Ui,
}

impl<'ui, 'p, L: AsRef<str>, T: DataTypeKind> InputScalar<'ui, 'p, T, L> {
    /// Constructs a new input scalar builder.
    #[doc(alias = "InputScalar")]
    pub fn new(ui: &'ui Ui, label: L, value: &'p mut T) -> Self {
        InputScalar {
            value,
            label,
            step: None,
            step_fast: None,
            display_format: None,
            flags: InputScalarFlags::empty(),
            ui,
        }
    }
}

impl<'ui, 'p, L: AsRef<str>, T: DataTypeKind, F: AsRef<str>> InputScalar<'ui, 'p, T, L, F> {
    /// Sets the display format using *a C-style printf string*
    pub fn display_format<F2: AsRef<str>>(
        self,
        display_format: F2,
    ) -> InputScalar<'ui, 'p, T, L, F2> {
        InputScalar {
            value: self.value,
            label: self.label,
            step: self.step,
            step_fast: self.step_fast,
            display_format: Some(display_format),
            flags: self.flags,
            ui: self.ui,
        }
    }

    /// Sets the step value for the input
    #[inline]
    pub fn step(mut self, value: T) -> Self {
        self.step = Some(value);
        self
    }

    /// Sets the fast step value for the input
    #[inline]
    pub fn step_fast(mut self, value: T) -> Self {
        self.step_fast = Some(value);
        self
    }

    /// Sets the input text flags
    #[inline]
    pub fn flags(mut self, flags: InputScalarFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Builds an input scalar that is bound to the given value.
    ///
    /// Returns true if the value was changed.
    pub fn build(self) -> bool {
        validate_input_scalar_flags("InputScalar::build()", self.flags);
        let (one, two) = self
            .ui
            .scratch_txt_with_opt(self.label, self.display_format);

        self.ui.run_with_bound_context(|| unsafe {
            sys::igInputScalar(
                one,
                T::KIND as i32,
                self.value as *mut T as *mut c_void,
                self.step
                    .as_ref()
                    .map(|step| step as *const T)
                    .unwrap_or(ptr::null()) as *const c_void,
                self.step_fast
                    .as_ref()
                    .map(|step| step as *const T)
                    .unwrap_or(ptr::null()) as *const c_void,
                two,
                self.flags.raw(),
            )
        })
    }
}

/// Builder for an input scalar array widget.
#[must_use]
pub struct InputScalarN<'ui, 'p, T, L, F = &'static str> {
    values: &'p mut [T],
    label: L,
    step: Option<T>,
    step_fast: Option<T>,
    display_format: Option<F>,
    flags: InputScalarFlags,
    ui: &'ui Ui,
}

impl<'ui, 'p, L: AsRef<str>, T: DataTypeKind> InputScalarN<'ui, 'p, T, L> {
    /// Constructs a new input scalar array builder.
    #[doc(alias = "InputScalarN")]
    pub fn new(ui: &'ui Ui, label: L, values: &'p mut [T]) -> Self {
        InputScalarN {
            values,
            label,
            step: None,
            step_fast: None,
            display_format: None,
            flags: InputScalarFlags::empty(),
            ui,
        }
    }
}

impl<'ui, 'p, L: AsRef<str>, T: DataTypeKind, F: AsRef<str>> InputScalarN<'ui, 'p, T, L, F> {
    /// Sets the display format using *a C-style printf string*
    pub fn display_format<F2: AsRef<str>>(
        self,
        display_format: F2,
    ) -> InputScalarN<'ui, 'p, T, L, F2> {
        InputScalarN {
            values: self.values,
            label: self.label,
            step: self.step,
            step_fast: self.step_fast,
            display_format: Some(display_format),
            flags: self.flags,
            ui: self.ui,
        }
    }

    /// Sets the step value for the input
    #[inline]
    pub fn step(mut self, value: T) -> Self {
        self.step = Some(value);
        self
    }

    /// Sets the fast step value for the input
    #[inline]
    pub fn step_fast(mut self, value: T) -> Self {
        self.step_fast = Some(value);
        self
    }

    /// Sets the input text flags
    #[inline]
    pub fn flags(mut self, flags: InputScalarFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Builds a horizontal array of multiple input scalars attached to the given slice.
    ///
    /// Returns true if any value was changed.
    pub fn build(self) -> bool {
        validate_input_scalar_flags("InputScalarN::build()", self.flags);
        let count = component_count_i32("InputScalarN::build()", self.values.len());
        let (one, two) = self
            .ui
            .scratch_txt_with_opt(self.label, self.display_format);

        self.ui.run_with_bound_context(|| unsafe {
            sys::igInputScalarN(
                one,
                T::KIND as i32,
                self.values.as_mut_ptr() as *mut c_void,
                count,
                self.step
                    .as_ref()
                    .map(|step| step as *const T)
                    .unwrap_or(ptr::null()) as *const c_void,
                self.step_fast
                    .as_ref()
                    .map(|step| step as *const T)
                    .unwrap_or(ptr::null()) as *const c_void,
                two,
                self.flags.raw(),
            )
        })
    }
}
