use super::flags::{
    ColorDataType, ColorInputMode, ColorPickerDisplayFlags, ColorPickerMode, ColorPickerOptions,
};
use super::validation::{assert_finite_color3, assert_finite_color4};
use crate::sys;
use crate::ui::Ui;
use std::borrow::Cow;

/// Builder for a 3-component color picker widget
#[derive(Debug)]
#[must_use]
pub struct ColorPicker3<'ui, 'p> {
    ui: &'ui Ui,
    label: Cow<'ui, str>,
    color: &'p mut [f32; 3],
    flags: ColorPickerOptions,
}

impl<'ui, 'p> ColorPicker3<'ui, 'p> {
    /// Creates a new color picker builder
    pub fn new(ui: &'ui Ui, label: impl Into<Cow<'ui, str>>, color: &'p mut [f32; 3]) -> Self {
        Self {
            ui,
            label: label.into(),
            color,
            flags: ColorPickerOptions::new(),
        }
    }

    /// Sets the flags for the color picker
    pub fn flags(mut self, flags: impl Into<ColorPickerOptions>) -> Self {
        self.flags = flags.into();
        self
    }

    /// Sets the display sub-editors shown inside the picker.
    pub fn display_flags(mut self, flags: ColorPickerDisplayFlags) -> Self {
        self.flags.display_flags = flags;
        self
    }

    /// Sets the numeric data type.
    pub fn data_type(mut self, data_type: ColorDataType) -> Self {
        self.flags.data_type = Some(data_type);
        self
    }

    /// Sets the picker mode.
    pub fn picker_mode(mut self, mode: ColorPickerMode) -> Self {
        self.flags.picker_mode = Some(mode);
        self
    }

    /// Sets the input/output color space.
    pub fn input_mode(mut self, mode: ColorInputMode) -> Self {
        self.flags.input_mode = Some(mode);
        self
    }

    /// Builds the color picker widget
    pub fn build(self) -> bool {
        self.flags.validate("ColorPicker3::build()");
        assert_finite_color3("ColorPicker3::build()", "color", &*self.color);
        let label_ptr = self.ui.scratch_txt(self.label.as_ref());
        unsafe { sys::igColorPicker3(label_ptr, self.color.as_mut_ptr(), self.flags.bits() as i32) }
    }
}

/// Builder for a 4-component color picker widget
#[derive(Debug)]
#[must_use]
pub struct ColorPicker4<'ui, 'p> {
    ui: &'ui Ui,
    label: Cow<'ui, str>,
    color: &'p mut [f32; 4],
    flags: ColorPickerOptions,
    ref_color: Option<[f32; 4]>,
}

impl<'ui, 'p> ColorPicker4<'ui, 'p> {
    /// Creates a new color picker builder
    pub fn new(ui: &'ui Ui, label: impl Into<Cow<'ui, str>>, color: &'p mut [f32; 4]) -> Self {
        Self {
            ui,
            label: label.into(),
            color,
            flags: ColorPickerOptions::new(),
            ref_color: None,
        }
    }

    /// Sets the flags for the color picker
    pub fn flags(mut self, flags: impl Into<ColorPickerOptions>) -> Self {
        self.flags = flags.into();
        self
    }

    /// Sets the display sub-editors shown inside the picker.
    pub fn display_flags(mut self, flags: ColorPickerDisplayFlags) -> Self {
        self.flags.display_flags = flags;
        self
    }

    /// Sets the numeric data type.
    pub fn data_type(mut self, data_type: ColorDataType) -> Self {
        self.flags.data_type = Some(data_type);
        self
    }

    /// Sets the picker mode.
    pub fn picker_mode(mut self, mode: ColorPickerMode) -> Self {
        self.flags.picker_mode = Some(mode);
        self
    }

    /// Sets the input/output color space.
    pub fn input_mode(mut self, mode: ColorInputMode) -> Self {
        self.flags.input_mode = Some(mode);
        self
    }

    /// Sets the reference color for comparison
    pub fn reference_color(mut self, ref_color: [f32; 4]) -> Self {
        self.ref_color = Some(ref_color);
        self
    }

    /// Builds the color picker widget
    pub fn build(self) -> bool {
        self.flags.validate("ColorPicker4::build()");
        assert_finite_color4("ColorPicker4::build()", "color", &*self.color);
        if let Some(ref_color) = &self.ref_color {
            assert_finite_color4("ColorPicker4::build()", "reference color", ref_color);
        }
        let label_ptr = self.ui.scratch_txt(self.label.as_ref());
        let ref_color_ptr = self
            .ref_color
            .as_ref()
            .map_or(std::ptr::null(), |c| c.as_ptr());

        unsafe {
            sys::igColorPicker4(
                label_ptr,
                self.color.as_mut_ptr(),
                self.flags.bits() as i32,
                ref_color_ptr,
            )
        }
    }
}
