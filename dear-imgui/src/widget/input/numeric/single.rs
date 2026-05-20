use super::super::validation::validate_input_scalar_flags;
use crate::ui::Ui;
use crate::{InputScalarFlags, sys};
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
        unsafe {
            sys::igInputInt(
                label_ptr,
                value as *mut i32,
                self.step,
                self.step_fast,
                self.flags.raw(),
            )
        }
    }
}

/// Builder for float input widget
#[derive(Debug)]
#[must_use]
pub struct InputFloat<'ui> {
    ui: &'ui Ui,
    label: Cow<'ui, str>,
    step: f32,
    step_fast: f32,
    format: Option<Cow<'ui, str>>,
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
            format: None,
            flags: InputScalarFlags::NONE,
        }
    }

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

    /// Sets the display format
    pub fn format(mut self, format: impl Into<Cow<'ui, str>>) -> Self {
        self.format = Some(format.into());
        self
    }

    /// Sets the flags for the input
    pub fn flags(mut self, flags: InputScalarFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Builds the float input widget
    pub fn build(self, value: &mut f32) -> bool {
        validate_input_scalar_flags("InputFloat::build()", self.flags);
        let format = self.format.as_deref().unwrap_or("%.3f");
        let (label_ptr, format_ptr) = self.ui.scratch_txt_two(self.label.as_ref(), format);

        unsafe {
            sys::igInputFloat(
                label_ptr,
                value as *mut f32,
                self.step,
                self.step_fast,
                format_ptr,
                self.flags.raw(),
            )
        }
    }
}

/// Builder for double input widget
#[derive(Debug)]
#[must_use]
pub struct InputDouble<'ui> {
    ui: &'ui Ui,
    label: Cow<'ui, str>,
    step: f64,
    step_fast: f64,
    format: Option<Cow<'ui, str>>,
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
            format: None,
            flags: InputScalarFlags::NONE,
        }
    }

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

    /// Sets the display format
    pub fn format(mut self, format: impl Into<Cow<'ui, str>>) -> Self {
        self.format = Some(format.into());
        self
    }

    /// Sets the flags for the input
    pub fn flags(mut self, flags: InputScalarFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Builds the double input widget
    pub fn build(self, value: &mut f64) -> bool {
        validate_input_scalar_flags("InputDouble::build()", self.flags);
        let format = self.format.as_deref().unwrap_or("%.6f");
        let (label_ptr, format_ptr) = self.ui.scratch_txt_two(self.label.as_ref(), format);

        unsafe {
            sys::igInputDouble(
                label_ptr,
                value as *mut f64,
                self.step,
                self.step_fast,
                format_ptr,
                self.flags.raw(),
            )
        }
    }
}
