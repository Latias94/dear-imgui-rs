use std::borrow::Cow;
use std::ffi::c_void;

use crate::internal::DataTypeKind;
use crate::sys;
use crate::validation::assert_finite_vec2;
use crate::{NumericFormat, NumericFormatError, Ui};

use super::SliderFlags;
use super::validation::validate_slider_preconditions;

/// Builder for a vertical slider widget.
#[derive(Clone, Debug)]
#[must_use]
pub struct VerticalSlider<'ui, Label, Data, Format = &'static str> {
    ui: &'ui Ui,
    label: Label,
    size: [f32; 2],
    min: Data,
    max: Data,
    display_format: Option<Format>,
    flags: SliderFlags,
}

impl<'ui, Label, Data> VerticalSlider<'ui, Label, Data>
where
    Label: AsRef<str>,
    Data: DataTypeKind,
{
    pub(super) fn new(
        ui: &'ui Ui,
        label: Label,
        size: impl Into<[f32; 2]>,
        min: Data,
        max: Data,
    ) -> Self {
        VerticalSlider {
            ui,
            label,
            size: size.into(),
            min,
            max,
            display_format: None,
            flags: SliderFlags::NONE,
        }
    }
}

impl<'ui, Label, Data, Format> VerticalSlider<'ui, Label, Data, Format>
where
    Label: AsRef<str>,
    Data: DataTypeKind,
    Format: AsRef<str>,
{
    /// Sets the range for the vertical slider.
    ///
    /// ```no_run
    /// # use dear_imgui_rs::*;
    /// # let mut ctx = Context::create();
    /// # let ui = ctx.frame();
    /// let mut value = 4i8;
    /// ui.v_slider_config("Example", [20.0, 20.0], i8::MIN, i8::MAX)
    ///     .range(4, 8)
    ///     .build(&mut value);
    /// ```
    ///
    /// It is safe, though up to C++ Dear ImGui, on how to handle when
    /// `min > max`.
    #[inline]
    pub fn range(mut self, min: Data, max: Data) -> Self {
        self.min = min;
        self.max = max;
        self
    }

    /// Sets the validated display format.
    #[inline]
    pub fn display_format<'fmt>(
        self,
        display_format: NumericFormat<'fmt, Data>,
    ) -> VerticalSlider<'ui, Label, Data, NumericFormat<'fmt, Data>> {
        VerticalSlider {
            ui: self.ui,
            label: self.label,
            size: self.size,
            min: self.min,
            max: self.max,
            display_format: Some(display_format),
            flags: self.flags,
        }
    }

    /// Validates and sets a C-style display format.
    pub fn try_display_format<'fmt>(
        self,
        display_format: impl Into<Cow<'fmt, str>>,
    ) -> Result<VerticalSlider<'ui, Label, Data, NumericFormat<'fmt, Data>>, NumericFormatError>
    {
        Ok(self.display_format(NumericFormat::new(display_format)?))
    }

    /// Replaces all current settings with the given flags
    #[inline]
    pub fn flags(mut self, flags: SliderFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Builds a vertical slider that is bound to the given value.
    ///
    /// Returns true if the slider value was changed.
    pub fn build(self, value: &mut Data) -> bool {
        validate_slider_preconditions("VerticalSlider::build()", &self.min, &self.max, self.flags);
        assert_finite_vec2("VerticalSlider::build()", "size", self.size);

        self.ui.with_scratch_txt_with_opt(
            self.label,
            self.display_format,
            |label, display_format| unsafe {
                sys::igVSliderScalar(
                    label,
                    self.size.into(),
                    Data::KIND as i32,
                    value as *mut Data as *mut c_void,
                    &self.min as *const Data as *const c_void,
                    &self.max as *const Data as *const c_void,
                    display_format,
                    self.flags.bits(),
                )
            },
        )
    }
}
