use super::flags::{ColorButtonOptions, ColorInputMode};
use super::validation::{assert_finite_color4, assert_non_negative_finite_vec2};
use crate::sys;
use crate::ui::Ui;
use std::borrow::Cow;

/// Builder for a color button widget
#[derive(Debug)]
#[must_use]
pub struct ColorButton<'ui> {
    ui: &'ui Ui,
    desc_id: Cow<'ui, str>,
    color: [f32; 4],
    flags: ColorButtonOptions,
    size: [f32; 2],
}

impl<'ui> ColorButton<'ui> {
    /// Creates a new color button builder
    pub fn new(ui: &'ui Ui, desc_id: impl Into<Cow<'ui, str>>, color: [f32; 4]) -> Self {
        Self {
            ui,
            desc_id: desc_id.into(),
            color,
            flags: ColorButtonOptions::new(),
            size: [0.0, 0.0],
        }
    }

    /// Sets the flags for the color button
    pub fn flags(mut self, flags: impl Into<ColorButtonOptions>) -> Self {
        self.flags = flags.into();
        self
    }

    /// Sets the input color space.
    pub fn input_mode(mut self, mode: ColorInputMode) -> Self {
        self.flags.input_mode = Some(mode);
        self
    }

    /// Sets the size of the color button
    pub fn size(mut self, size: [f32; 2]) -> Self {
        self.size = size;
        self
    }

    /// Builds the color button widget
    pub fn build(self) -> bool {
        self.flags.validate("ColorButton::build()");
        assert_finite_color4("ColorButton::build()", "color", &self.color);
        assert_non_negative_finite_vec2("ColorButton::build()", "size", self.size);
        let desc_id_ptr = self.ui.scratch_txt(self.desc_id.as_ref());
        let size_vec: sys::ImVec2 = self.size.into();

        unsafe {
            sys::igColorButton(
                desc_id_ptr,
                sys::ImVec4 {
                    x: self.color[0],
                    y: self.color[1],
                    z: self.color[2],
                    w: self.color[3],
                },
                self.flags.bits() as i32,
                size_vec,
            )
        }
    }
}
