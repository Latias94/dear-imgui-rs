use super::super::validation::validate_input_scalar_flags;
use crate::ui::Ui;
use crate::{InputScalarFlags, sys};

/// Builder for a 2-component int input widget.
#[must_use]
pub struct InputInt2<'ui, 'p, L> {
    label: L,
    value: &'p mut [i32; 2],
    flags: InputScalarFlags,
    ui: &'ui Ui,
}

impl<'ui, 'p, L: AsRef<str>> InputInt2<'ui, 'p, L> {
    /// Constructs a new input int2 builder.
    #[doc(alias = "InputInt2")]
    pub fn new(ui: &'ui Ui, label: L, value: &'p mut [i32; 2]) -> Self {
        InputInt2 {
            label,
            value,
            flags: InputScalarFlags::empty(),
            ui,
        }
    }

    /// Sets the input text flags
    #[inline]
    pub fn flags(mut self, flags: InputScalarFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Builds the input int2 widget.
    ///
    /// Returns true if any value was changed.
    pub fn build(self) -> bool {
        validate_input_scalar_flags("InputInt2::build()", self.flags);
        unsafe {
            let label_cstr = self.ui.scratch_txt(self.label);

            sys::igInputInt2(label_cstr, self.value.as_mut_ptr(), self.flags.raw())
        }
    }
}

/// Builder for a 3-component int input widget.
#[must_use]
pub struct InputInt3<'ui, 'p, L> {
    label: L,
    value: &'p mut [i32; 3],
    flags: InputScalarFlags,
    ui: &'ui Ui,
}

impl<'ui, 'p, L: AsRef<str>> InputInt3<'ui, 'p, L> {
    /// Constructs a new input int3 builder.
    #[doc(alias = "InputInt3")]
    pub fn new(ui: &'ui Ui, label: L, value: &'p mut [i32; 3]) -> Self {
        InputInt3 {
            label,
            value,
            flags: InputScalarFlags::empty(),
            ui,
        }
    }

    /// Sets the input text flags
    #[inline]
    pub fn flags(mut self, flags: InputScalarFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Builds the input int3 widget.
    ///
    /// Returns true if any value was changed.
    pub fn build(self) -> bool {
        validate_input_scalar_flags("InputInt3::build()", self.flags);
        unsafe {
            let label_cstr = self.ui.scratch_txt(self.label);

            sys::igInputInt3(label_cstr, self.value.as_mut_ptr(), self.flags.raw())
        }
    }
}

/// Builder for a 4-component int input widget.
#[must_use]
pub struct InputInt4<'ui, 'p, L> {
    label: L,
    value: &'p mut [i32; 4],
    flags: InputScalarFlags,
    ui: &'ui Ui,
}

impl<'ui, 'p, L: AsRef<str>> InputInt4<'ui, 'p, L> {
    /// Constructs a new input int4 builder.
    #[doc(alias = "InputInt4")]
    pub fn new(ui: &'ui Ui, label: L, value: &'p mut [i32; 4]) -> Self {
        InputInt4 {
            label,
            value,
            flags: InputScalarFlags::empty(),
            ui,
        }
    }

    /// Sets the input text flags
    #[inline]
    pub fn flags(mut self, flags: InputScalarFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Builds the input int4 widget.
    ///
    /// Returns true if any value was changed.
    pub fn build(self) -> bool {
        validate_input_scalar_flags("InputInt4::build()", self.flags);
        unsafe {
            let label_cstr = self.ui.scratch_txt(self.label);

            sys::igInputInt4(label_cstr, self.value.as_mut_ptr(), self.flags.raw())
        }
    }
}
