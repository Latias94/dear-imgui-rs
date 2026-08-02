use super::super::validation::validate_input_scalar_flags;
use crate::ui::Ui;
use crate::{InputScalarFlags, NumericFormat, NumericFormatError, sys};
use std::borrow::Cow;

/// Builder for integer input widget
#[derive(Debug)]
#[must_use]
pub struct InputInt<'ui> {
    ui: &'ui Ui,
    label: Cow<'ui, str>,
    step: i32,
    step_fast: i32,
    flags: InputScalarFlags,
}

impl<'ui> InputInt<'ui> {
    /// Creates a new integer input builder
    pub fn new(ui: &'ui Ui, label: impl Into<Cow<'ui, str>>) -> Self {
        Self {
            ui,
            label: label.into(),
            step: 1,
            step_fast: 100,
            flags: InputScalarFlags::NONE,
        }
    }

    /// Sets the step value
    pub fn step(mut self, step: i32) -> Self {
        self.step = step;
        self
    }

    /// Sets the fast step value
    pub fn step_fast(mut self, step_fast: i32) -> Self {
        self.step_fast = step_fast;
        self
    }

    /// Sets the flags for the input
    pub fn flags(mut self, flags: InputScalarFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Builds the integer input widget
    pub fn build(self, value: &mut i32) -> bool {
        validate_input_scalar_flags("InputInt::build()", self.flags);
        let label_ptr = self.ui.scratch_txt(self.label.as_ref());
        self.ui.run_with_bound_context(|| unsafe {
            sys::igInputInt(
                label_ptr,
                value as *mut i32,
                self.step,
                self.step_fast,
                self.flags.raw(),
            )
        })
    }
}

/// Builder for float input widget
#[derive(Debug)]
#[must_use]
pub struct InputFloat<'ui, F = &'static str> {
    ui: &'ui Ui,
    label: Cow<'ui, str>,
    step: f32,
    step_fast: f32,
    display_format: Option<F>,
    flags: InputScalarFlags,
}

impl<'ui> InputFloat<'ui> {
    /// Creates a new float input builder
    pub fn new(ui: &'ui Ui, label: impl Into<Cow<'ui, str>>) -> Self {
        Self {
            ui,
            label: label.into(),
            step: 0.0,
            step_fast: 0.0,
            display_format: None,
            flags: InputScalarFlags::NONE,
        }
    }
}

impl<'ui, F: AsRef<str>> InputFloat<'ui, F> {
    /// Sets the step value
    pub fn step(mut self, step: f32) -> Self {
        self.step = step;
        self
    }

    /// Sets the fast step value
    pub fn step_fast(mut self, step_fast: f32) -> Self {
        self.step_fast = step_fast;
        self
    }

    /// Sets the validated display format.
    pub fn display_format<'fmt>(
        self,
        display_format: NumericFormat<'fmt, f32>,
    ) -> InputFloat<'ui, NumericFormat<'fmt, f32>> {
        InputFloat {
            ui: self.ui,
            label: self.label,
            step: self.step,
            step_fast: self.step_fast,
            display_format: Some(display_format),
            flags: self.flags,
        }
    }

    /// Validates and sets a C-style display format.
    pub fn try_display_format<'fmt>(
        self,
        display_format: impl Into<Cow<'fmt, str>>,
    ) -> Result<InputFloat<'ui, NumericFormat<'fmt, f32>>, NumericFormatError> {
        Ok(self.display_format(NumericFormat::new(display_format)?))
    }

    /// Sets the flags for the input
    pub fn flags(mut self, flags: InputScalarFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Builds the float input widget
    pub fn build(self, value: &mut f32) -> bool {
        validate_input_scalar_flags("InputFloat::build()", self.flags);
        let format = self
            .display_format
            .as_ref()
            .map(AsRef::as_ref)
            .unwrap_or("%.3f");
        let (label_ptr, format_ptr) = self.ui.scratch_txt_two(self.label.as_ref(), format);

        self.ui.run_with_bound_context(|| unsafe {
            sys::igInputFloat(
                label_ptr,
                value as *mut f32,
                self.step,
                self.step_fast,
                format_ptr,
                self.flags.raw(),
            )
        })
    }
}

/// Builder for double input widget
#[derive(Debug)]
#[must_use]
pub struct InputDouble<'ui, F = &'static str> {
    ui: &'ui Ui,
    label: Cow<'ui, str>,
    step: f64,
    step_fast: f64,
    display_format: Option<F>,
    flags: InputScalarFlags,
}

impl<'ui> InputDouble<'ui> {
    /// Creates a new double input builder
    pub fn new(ui: &'ui Ui, label: impl Into<Cow<'ui, str>>) -> Self {
        Self {
            ui,
            label: label.into(),
            step: 0.0,
            step_fast: 0.0,
            display_format: None,
            flags: InputScalarFlags::NONE,
        }
    }
}

impl<'ui, F: AsRef<str>> InputDouble<'ui, F> {
    /// Sets the step value
    pub fn step(mut self, step: f64) -> Self {
        self.step = step;
        self
    }

    /// Sets the fast step value
    pub fn step_fast(mut self, step_fast: f64) -> Self {
        self.step_fast = step_fast;
        self
    }

    /// Sets the validated display format.
    pub fn display_format<'fmt>(
        self,
        display_format: NumericFormat<'fmt, f64>,
    ) -> InputDouble<'ui, NumericFormat<'fmt, f64>> {
        InputDouble {
            ui: self.ui,
            label: self.label,
            step: self.step,
            step_fast: self.step_fast,
            display_format: Some(display_format),
            flags: self.flags,
        }
    }

    /// Validates and sets a C-style display format.
    pub fn try_display_format<'fmt>(
        self,
        display_format: impl Into<Cow<'fmt, str>>,
    ) -> Result<InputDouble<'ui, NumericFormat<'fmt, f64>>, NumericFormatError> {
        Ok(self.display_format(NumericFormat::new(display_format)?))
    }

    /// Sets the flags for the input
    pub fn flags(mut self, flags: InputScalarFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Builds the double input widget
    pub fn build(self, value: &mut f64) -> bool {
        validate_input_scalar_flags("InputDouble::build()", self.flags);
        let format = self
            .display_format
            .as_ref()
            .map(AsRef::as_ref)
            .unwrap_or("%.6f");
        let (label_ptr, format_ptr) = self.ui.scratch_txt_two(self.label.as_ref(), format);

        self.ui.run_with_bound_context(|| unsafe {
            sys::igInputDouble(
                label_ptr,
                value as *mut f64,
                self.step,
                self.step_fast,
                format_ptr,
                self.flags.raw(),
            )
        })
    }
}
