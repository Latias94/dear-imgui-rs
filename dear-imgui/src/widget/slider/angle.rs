use std::borrow::Cow;

use crate::sys;
use crate::{NumericFormat, NumericFormatError, Ui};

use super::SliderFlags;
use super::validation::{validate_slider_flags, validate_slider_range};

/// Builder for an angle slider widget.
#[derive(Copy, Clone, Debug)]
#[must_use]
pub struct AngleSlider<'ui, Label, Format = &'static str> {
    ui: &'ui Ui,
    label: Label,
    min_degrees: f32,
    max_degrees: f32,
    display_format: Format,
    flags: SliderFlags,
}

impl<'ui, Label> AngleSlider<'ui, Label>
where
    Label: AsRef<str>,
{
    pub(super) fn new(ui: &'ui Ui, label: Label) -> Self {
        AngleSlider {
            ui,
            label,
            min_degrees: -360.0,
            max_degrees: 360.0,
            display_format: "%.0f deg",
            flags: SliderFlags::NONE,
        }
    }
}

impl<'ui, Label, Format> AngleSlider<'ui, Label, Format>
where
    Label: AsRef<str>,
    Format: AsRef<str>,
{
    /// Sets the range in degrees (inclusive)
    /// ```no_run
    /// # use dear_imgui_rs::*;
    /// # let mut ctx = Context::create();
    /// # let ui = ctx.frame();
    /// let mut angle = 0.0;
    /// ui.slider_angle_config("Example")
    ///     .range_degrees(-20.0, 20.0)
    ///     .build(&mut angle);
    /// ```
    ///
    /// It is safe, though up to C++ Dear ImGui, on how to handle when
    /// `min > max`.
    #[inline]
    pub fn range_degrees(mut self, min_degrees: f32, max_degrees: f32) -> Self {
        self.min_degrees = min_degrees;
        self.max_degrees = max_degrees;
        self
    }

    /// Sets the minimum value (in degrees)
    #[inline]
    pub fn min_degrees(mut self, min_degrees: f32) -> Self {
        self.min_degrees = min_degrees;
        self
    }

    /// Sets the maximum value (in degrees)
    #[inline]
    pub fn max_degrees(mut self, max_degrees: f32) -> Self {
        self.max_degrees = max_degrees;
        self
    }

    /// Sets the validated display format.
    #[inline]
    pub fn display_format<'fmt>(
        self,
        display_format: NumericFormat<'fmt, f32>,
    ) -> AngleSlider<'ui, Label, NumericFormat<'fmt, f32>> {
        AngleSlider {
            ui: self.ui,
            label: self.label,
            min_degrees: self.min_degrees,
            max_degrees: self.max_degrees,
            display_format,
            flags: self.flags,
        }
    }

    /// Validates and sets a C-style display format.
    pub fn try_display_format<'fmt>(
        self,
        display_format: impl Into<Cow<'fmt, str>>,
    ) -> Result<AngleSlider<'ui, Label, NumericFormat<'fmt, f32>>, NumericFormatError> {
        Ok(self.display_format(NumericFormat::new(display_format)?))
    }

    /// Replaces all current settings with the given flags
    #[inline]
    pub fn flags(mut self, flags: SliderFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Builds an angle slider that is bound to the given value (in radians).
    ///
    /// Returns true if the slider value was changed.
    pub fn build(self, value_rad: &mut f32) -> bool {
        validate_slider_flags("AngleSlider::build()", self.flags);
        validate_slider_range("AngleSlider::build()", &self.min_degrees, &self.max_degrees);
        self.ui.with_scratch_txt_two(
            self.label,
            self.display_format,
            |label, display_format| unsafe {
                sys::igSliderAngle(
                    label,
                    value_rad as *mut _,
                    self.min_degrees,
                    self.max_degrees,
                    display_format,
                    self.flags.bits(),
                )
            },
        )
    }
}
