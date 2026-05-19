use super::flags::{
    ColorDataType, ColorDisplayMode, ColorEditOptions, ColorInputMode, ColorPickerMode,
};
use super::validation::{assert_finite_color3, assert_finite_color4};
use crate::sys;
use crate::ui::Ui;
use std::borrow::Cow;

/// Builder for a 3-component color edit widget
#[derive(Debug)]
#[must_use]
pub struct ColorEdit3<'ui, 'p> {
    ui: &'ui Ui,
    label: Cow<'ui, str>,
    color: &'p mut [f32; 3],
    flags: ColorEditOptions,
}

impl<'ui, 'p> ColorEdit3<'ui, 'p> {
    /// Creates a new color edit builder
    pub fn new(ui: &'ui Ui, label: impl Into<Cow<'ui, str>>, color: &'p mut [f32; 3]) -> Self {
        Self {
            ui,
            label: label.into(),
            color,
            flags: ColorEditOptions::new(),
        }
    }

    /// Sets the flags for the color edit
    pub fn flags(mut self, flags: impl Into<ColorEditOptions>) -> Self {
        self.flags = flags.into();
        self
    }

    /// Sets the display mode.
    pub fn display_mode(mut self, mode: ColorDisplayMode) -> Self {
        self.flags.display_mode = Some(mode);
        self
    }

    /// Sets the numeric data type.
    pub fn data_type(mut self, data_type: ColorDataType) -> Self {
        self.flags.data_type = Some(data_type);
        self
    }

    /// Sets the picker mode used by the popup picker.
    pub fn picker_mode(mut self, mode: ColorPickerMode) -> Self {
        self.flags.picker_mode = Some(mode);
        self
    }

    /// Sets the input/output color space.
    pub fn input_mode(mut self, mode: ColorInputMode) -> Self {
        self.flags.input_mode = Some(mode);
        self
    }

    /// Builds the color edit widget
    pub fn build(self) -> bool {
        self.flags.validate("ColorEdit3::build()");
        assert_finite_color3("ColorEdit3::build()", "color", &*self.color);
        let label_ptr = self.ui.scratch_txt(self.label.as_ref());
        unsafe { sys::igColorEdit3(label_ptr, self.color.as_mut_ptr(), self.flags.bits() as i32) }
    }
}

/// Builder for a 4-component color edit widget
#[derive(Debug)]
#[must_use]
pub struct ColorEdit4<'ui, 'p> {
    ui: &'ui Ui,
    label: Cow<'ui, str>,
    color: &'p mut [f32; 4],
    flags: ColorEditOptions,
}

impl<'ui, 'p> ColorEdit4<'ui, 'p> {
    /// Creates a new color edit builder
    pub fn new(ui: &'ui Ui, label: impl Into<Cow<'ui, str>>, color: &'p mut [f32; 4]) -> Self {
        Self {
            ui,
            label: label.into(),
            color,
            flags: ColorEditOptions::new(),
        }
    }

    /// Sets the flags for the color edit
    pub fn flags(mut self, flags: impl Into<ColorEditOptions>) -> Self {
        self.flags = flags.into();
        self
    }

    /// Sets the display mode.
    pub fn display_mode(mut self, mode: ColorDisplayMode) -> Self {
        self.flags.display_mode = Some(mode);
        self
    }

    /// Sets the numeric data type.
    pub fn data_type(mut self, data_type: ColorDataType) -> Self {
        self.flags.data_type = Some(data_type);
        self
    }

    /// Sets the picker mode used by the popup picker.
    pub fn picker_mode(mut self, mode: ColorPickerMode) -> Self {
        self.flags.picker_mode = Some(mode);
        self
    }

    /// Sets the input/output color space.
    pub fn input_mode(mut self, mode: ColorInputMode) -> Self {
        self.flags.input_mode = Some(mode);
        self
    }

    /// Builds the color edit widget
    pub fn build(self) -> bool {
        self.flags.validate("ColorEdit4::build()");
        assert_finite_color4("ColorEdit4::build()", "color", &*self.color);
        let label_ptr = self.ui.scratch_txt(self.label.as_ref());
        unsafe { sys::igColorEdit4(label_ptr, self.color.as_mut_ptr(), self.flags.bits() as i32) }
    }
}
