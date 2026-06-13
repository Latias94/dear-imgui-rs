use crate::Ui;
use crate::sys;

use super::SliderFlags;
use super::validation::{validate_slider_flags, validate_slider_range};

/// Builder for an angle slider widget.
#[derive(Copy, Clone, Debug)]
#[must_use]
pub struct AngleSlider<Label, Format = &'static str> {
    label: Label,
    min_degrees: f32,
    max_degrees: f32,
    display_format: Format,
    flags: SliderFlags,
}

impl<Label> AngleSlider<Label>
where
    Label: AsRef<str>,
{
    /// Constructs a new angle slider builder, where its minimum defaults to -360.0 and
    /// maximum defaults to 360.0
    #[doc(alias = "SliderAngle")]
    pub fn new(label: Label) -> Self {
        AngleSlider {
            label,
            min_degrees: -360.0,
            max_degrees: 360.0,
            display_format: "%.0f deg",
            flags: SliderFlags::NONE,
        }
    }
}

impl<Label, Format> AngleSlider<Label, Format>
where
    Label: AsRef<str>,
    Format: AsRef<str>,
{
    /// Sets the range in degrees (inclusive)
    /// ```no_run
    /// # use dear_imgui_rs::*;
    /// # let mut ctx = Context::create();
    /// # let ui = ctx.frame();
    /// AngleSlider::new("Example")
    ///     .range_degrees(-20.0, 20.0)
    ///     // Remember to call .build(&ui)
    ///     ;
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

    /// Sets the display format using *a C-style printf string*
    #[inline]
    pub fn display_format<Format2: AsRef<str>>(
        self,
        display_format: Format2,
    ) -> AngleSlider<Label, Format2> {
        AngleSlider {
            label: self.label,
            min_degrees: self.min_degrees,
            max_degrees: self.max_degrees,
            display_format,
            flags: self.flags,
        }
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
    pub fn build(self, ui: &Ui, value_rad: &mut f32) -> bool {
        validate_slider_flags("AngleSlider::build()", self.flags);
        validate_slider_range("AngleSlider::build()", &self.min_degrees, &self.max_degrees);
        let (label, display_format) = ui.scratch_txt_two(self.label, self.display_format);

        ui.run_with_bound_context(|| unsafe {
            sys::igSliderAngle(
                label,
                value_rad as *mut _,
                self.min_degrees,
                self.max_degrees,
                display_format,
                self.flags.bits(),
            )
        })
    }
}
