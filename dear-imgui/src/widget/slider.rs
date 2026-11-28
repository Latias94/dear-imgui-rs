//! Sliders
//!
//! Scalar and range sliders for numeric input with builder-based configuration
//! of speed, format and clamping.
//!
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::as_conversions
)]

use crate::internal::{DataType, DataTypeKind, component_count_i32};
use crate::validation::assert_finite_vec2;
use crate::{ContextError, NumericFormat, Ui, sys};
use std::borrow::Cow;
use std::ffi::c_void;

impl Ui {
    /// Creates a new slider widget. Returns true if the value has been edited.
    pub fn slider<T: AsRef<str>, K: DataTypeKind>(
        &self,
        label: T,
        value: &mut K,
        min: K,
        max: K,
    ) -> bool {
        self.slider_config(label, min, max).build(value)
    }

    /// Creates a new unbuilt Slider.
    pub fn slider_config<T: AsRef<str>, K: DataTypeKind>(
        &self,
        label: T,
        min: K,
        max: K,
    ) -> Slider<'_, T, K> {
        Slider {
            ui: self,
            label,
            min,
            max,
            display_format: Option::<&'static str>::None,
            flags: SliderFlags::NONE,
        }
    }

    /// Creates a float slider
    #[doc(alias = "SliderFloat")]
    pub fn slider_f32(&self, label: impl AsRef<str>, value: &mut f32, min: f32, max: f32) -> bool {
        self.slider(label, value, min, max)
    }

    /// Creates an integer slider
    #[doc(alias = "SliderInt")]
    pub fn slider_i32(&self, label: impl AsRef<str>, value: &mut i32, min: i32, max: i32) -> bool {
        self.slider(label, value, min, max)
    }

    /// Creates a float2 slider
    #[doc(alias = "SliderFloat2")]
    pub fn slider_float2(
        &self,
        label: impl AsRef<str>,
        value: &mut [f32; 2],
        min: f32,
        max: f32,
    ) -> bool {
        self.slider_config(label, min, max)
            .build_array(value.as_mut_slice())
    }

    /// Creates a float3 slider
    #[doc(alias = "SliderFloat3")]
    pub fn slider_float3(
        &self,
        label: impl AsRef<str>,
        value: &mut [f32; 3],
        min: f32,
        max: f32,
    ) -> bool {
        self.slider_config(label, min, max)
            .build_array(value.as_mut_slice())
    }

    /// Creates a float4 slider
    #[doc(alias = "SliderFloat4")]
    pub fn slider_float4(
        &self,
        label: impl AsRef<str>,
        value: &mut [f32; 4],
        min: f32,
        max: f32,
    ) -> bool {
        self.slider_config(label, min, max)
            .build_array(value.as_mut_slice())
    }

    /// Creates an int2 slider
    #[doc(alias = "SliderInt2")]
    pub fn slider_int2(
        &self,
        label: impl AsRef<str>,
        value: &mut [i32; 2],
        min: i32,
        max: i32,
    ) -> bool {
        self.slider_config(label, min, max)
            .build_array(value.as_mut_slice())
    }

    /// Creates an int3 slider
    #[doc(alias = "SliderInt3")]
    pub fn slider_int3(
        &self,
        label: impl AsRef<str>,
        value: &mut [i32; 3],
        min: i32,
        max: i32,
    ) -> bool {
        self.slider_config(label, min, max)
            .build_array(value.as_mut_slice())
    }

    /// Creates an int4 slider
    #[doc(alias = "SliderInt4")]
    pub fn slider_int4(
        &self,
        label: impl AsRef<str>,
        value: &mut [i32; 4],
        min: i32,
        max: i32,
    ) -> bool {
        self.slider_config(label, min, max)
            .build_array(value.as_mut_slice())
    }

    /// Creates a vertical slider
    #[doc(alias = "VSliderFloat")]
    pub fn v_slider_f32(
        &self,
        label: impl AsRef<str>,
        size: impl Into<[f32; 2]>,
        value: &mut f32,
        min: f32,
        max: f32,
    ) -> bool {
        self.v_slider_config(label, size, min, max).build(value)
    }

    /// Creates a vertical integer slider
    #[doc(alias = "VSliderInt")]
    pub fn v_slider_i32(
        &self,
        label: impl AsRef<str>,
        size: impl Into<[f32; 2]>,
        value: &mut i32,
        min: i32,
        max: i32,
    ) -> bool {
        self.v_slider_config(label, size, min, max).build(value)
    }

    /// Creates a vertical slider builder with the given size and range.
    #[doc(alias = "VSliderScalar")]
    pub fn v_slider_config<T: AsRef<str>, K: DataTypeKind>(
        &self,
        label: T,
        size: impl Into<[f32; 2]>,
        min: K,
        max: K,
    ) -> VerticalSlider<'_, T, K> {
        VerticalSlider::new(self, label, size, min, max)
    }

    /// Creates an angle slider (value in radians)
    #[doc(alias = "SliderAngle")]
    pub fn slider_angle(&self, label: impl AsRef<str>, value_rad: &mut f32) -> bool {
        self.slider_angle_config(label).build(value_rad)
    }

    /// Creates an angle slider builder with a default range of -360 to 360 degrees.
    pub fn slider_angle_config<T: AsRef<str>>(&self, label: T) -> AngleSlider<'_, T> {
        AngleSlider::new(self, label)
    }
}

bitflags::bitflags! {
    /// Flags for slider widgets.
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct SliderFlags: i32 {
        /// No flags
        const NONE = 0;
        /// Clamp on input when editing via CTRL+Click or direct text input.
        ///
        /// This is one of the two bits combined by `ALWAYS_CLAMP`.
        const CLAMP_ON_INPUT = sys::ImGuiSliderFlags_ClampOnInput;
        /// Clamp zero-range sliders to avoid a zero-sized range.
        ///
        /// This is one of the two bits combined by `ALWAYS_CLAMP`.
        const CLAMP_ZERO_RANGE = sys::ImGuiSliderFlags_ClampZeroRange;
        /// Disable small "smart" speed tweaks for very small/large ranges.
        ///
        /// Useful when precise, linear dragging behavior is desired.
        const NO_SPEED_TWEAKS = sys::ImGuiSliderFlags_NoSpeedTweaks;
        /// Clamp value to min/max bounds when input manually with CTRL+Click. By default CTRL+Click allows going out of bounds.
        const ALWAYS_CLAMP = sys::ImGuiSliderFlags_AlwaysClamp;
        /// Make the widget logarithmic (linear otherwise). Consider using ImGuiSliderFlags_NoRoundToFormat with this if using a format-string with small amount of digits.
        const LOGARITHMIC = sys::ImGuiSliderFlags_Logarithmic;
        /// Disable rounding underlying value to match precision of the display format string (e.g. %.3f values are rounded to those 3 digits)
        const NO_ROUND_TO_FORMAT = sys::ImGuiSliderFlags_NoRoundToFormat;
        /// Disable CTRL+Click or Enter key allowing to input text directly into the widget
        const NO_INPUT = sys::ImGuiSliderFlags_NoInput;
        /// Draw R/G/B/A color markers on each component.
        ///
        /// Dear ImGui only defines four default component colors.
        const COLOR_MARKERS = sys::ImGuiSliderFlags_ColorMarkers;
    }
}

/// Builder for slider widgets.
///
/// Create this builder through [`Ui::slider_config`]. The former direct constructor is
/// intentionally unavailable:
///
/// ```compile_fail
/// # use dear_imgui_rs::{Context, Slider};
/// # let mut context = Context::create();
/// # let ui = context.frame();
/// let _ = Slider::new(ui, "value", 0_i32, 10_i32);
/// ```
#[derive(Clone, Debug)]
#[must_use]
pub struct Slider<'ui, Label, Data, Format = &'static str> {
    ui: &'ui Ui,
    label: Label,
    min: Data,
    max: Data,
    display_format: Option<Format>,
    flags: SliderFlags,
}

impl<'ui, Label, Data, Format> Slider<'ui, Label, Data, Format>
where
    Label: AsRef<str>,
    Data: DataTypeKind,
    Format: AsRef<str>,
{
    /// Sets the range inclusively, such that both values given
    /// are valid values which the slider can be dragged to.
    ///
    /// ```no_run
    /// # use dear_imgui_rs::*;
    /// # let mut ctx = Context::create();
    /// # let ui = ctx.frame();
    /// ui.slider_config("Example", i8::MIN, i8::MAX)
    ///     .range(4, 8)
    ///     // Remember to call .build()
    ///     ;
    /// ```
    ///
    /// It is safe, though up to C++ Dear ImGui, on how to handle when
    /// `min > max`.
    ///
    /// Note for f32 and f64 sliders, Dear ImGui limits each range endpoint to
    /// finite values within half their full range (e.g.
    /// `-f32::MAX/2.0..=f32::MAX/2.0`). Specifying a value outside this range
    /// will panic before calling into Dear ImGui.
    /// For large ranged values, consider using input widgets instead
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
    ) -> Slider<'ui, Label, Data, NumericFormat<'fmt, Data>> {
        Slider {
            ui: self.ui,
            label: self.label,
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
    ) -> Result<Slider<'ui, Label, Data, NumericFormat<'fmt, Data>>, ContextError> {
        Ok(self.display_format(NumericFormat::new(display_format)?))
    }

    /// Replaces all current settings with the given flags
    #[inline]
    pub fn flags(mut self, flags: SliderFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Builds a slider that is bound to the given value.
    ///
    /// Returns true if the slider value was changed.
    #[doc(alias = "SliderScalar")]
    pub fn build(self, value: &mut Data) -> bool {
        validate_slider_preconditions("Slider::build()", &self.min, &self.max, self.flags);
        self.ui.with_scratch_txt_with_opt(
            self.label,
            self.display_format,
            |label, display_format| unsafe {
                sys::igSliderScalar(
                    label,
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

    /// Builds a horizontal array of multiple sliders attached to the given slice.
    ///
    /// Returns true if any slider value was changed.
    #[doc(alias = "SliderScalarN")]
    pub fn build_array(self, values: &mut [Data]) -> bool {
        validate_slider_preconditions("Slider::build_array()", &self.min, &self.max, self.flags);
        let count = component_count_i32("Slider::build_array()", values.len());
        if self.flags.contains(SliderFlags::COLOR_MARKERS) {
            assert!(
                count <= 4,
                "Slider::build_array() supports at most 4 components with COLOR_MARKERS"
            );
        }
        self.ui.with_scratch_txt_with_opt(
            self.label,
            self.display_format,
            |label, display_format| unsafe {
                sys::igSliderScalarN(
                    label,
                    Data::KIND as i32,
                    values.as_mut_ptr() as *mut c_void,
                    count,
                    &self.min as *const Data as *const c_void,
                    &self.max as *const Data as *const c_void,
                    display_format,
                    self.flags.bits(),
                )
            },
        )
    }
}

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
    fn new(ui: &'ui Ui, label: Label, size: impl Into<[f32; 2]>, min: Data, max: Data) -> Self {
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
    ) -> Result<VerticalSlider<'ui, Label, Data, NumericFormat<'fmt, Data>>, ContextError> {
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
    fn new(ui: &'ui Ui, label: Label) -> Self {
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
    ) -> Result<AngleSlider<'ui, Label, NumericFormat<'fmt, f32>>, ContextError> {
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

fn validate_slider_flags(caller: &str, flags: SliderFlags) {
    let bits = flags.bits();
    assert!(
        bits & sys::ImGuiSliderFlags_WrapAround == 0,
        "{caller} does not support ImGuiSliderFlags_WrapAround; use DragFlags::WRAP_AROUND with drag widgets"
    );
    let unsupported = bits & !SliderFlags::all().bits();
    assert!(
        unsupported == 0,
        "{caller} received unsupported ImGuiSliderFlags bits: 0x{unsupported:X}"
    );
}

fn validate_slider_range<Data: DataTypeKind>(caller: &str, min: &Data, max: &Data) {
    match Data::KIND {
        DataType::I8 | DataType::U8 | DataType::I16 | DataType::U16 => {}
        DataType::I32 => {
            let min = unsafe { *(min as *const Data as *const i32) };
            let max = unsafe { *(max as *const Data as *const i32) };
            let lower = i32::MIN / 2;
            let upper = i32::MAX / 2;
            assert!(
                (lower..=upper).contains(&min) && (lower..=upper).contains(&max),
                "{caller} i32/isize range endpoints must stay within i32::MIN/2..=i32::MAX/2"
            );
        }
        DataType::U32 => {
            let min = unsafe { *(min as *const Data as *const u32) };
            let max = unsafe { *(max as *const Data as *const u32) };
            let upper = u32::MAX / 2;
            assert!(
                min <= upper && max <= upper,
                "{caller} u32/usize range endpoints must be <= u32::MAX/2"
            );
        }
        DataType::I64 => {
            let min = unsafe { *(min as *const Data as *const i64) };
            let max = unsafe { *(max as *const Data as *const i64) };
            let lower = i64::MIN / 2;
            let upper = i64::MAX / 2;
            assert!(
                (lower..=upper).contains(&min) && (lower..=upper).contains(&max),
                "{caller} i64/isize range endpoints must stay within i64::MIN/2..=i64::MAX/2"
            );
        }
        DataType::U64 => {
            let min = unsafe { *(min as *const Data as *const u64) };
            let max = unsafe { *(max as *const Data as *const u64) };
            let upper = u64::MAX / 2;
            assert!(
                min <= upper && max <= upper,
                "{caller} u64/usize range endpoints must be <= u64::MAX/2"
            );
        }
        DataType::F32 => {
            let min = unsafe { *(min as *const Data as *const f32) };
            let max = unsafe { *(max as *const Data as *const f32) };
            assert!(
                min.is_finite()
                    && max.is_finite()
                    && (-f32::MAX / 2.0..=f32::MAX / 2.0).contains(&min)
                    && (-f32::MAX / 2.0..=f32::MAX / 2.0).contains(&max),
                "{caller} f32 range endpoints must be finite and stay within -f32::MAX/2..=f32::MAX/2"
            );
        }
        DataType::F64 => {
            let min = unsafe { *(min as *const Data as *const f64) };
            let max = unsafe { *(max as *const Data as *const f64) };
            assert!(
                min.is_finite()
                    && max.is_finite()
                    && (-f64::MAX / 2.0..=f64::MAX / 2.0).contains(&min)
                    && (-f64::MAX / 2.0..=f64::MAX / 2.0).contains(&max),
                "{caller} f64 range endpoints must be finite and stay within -f64::MAX/2..=f64::MAX/2"
            );
        }
    }
}

fn validate_slider_preconditions<Data: DataTypeKind>(
    caller: &str,
    min: &Data,
    max: &Data,
    flags: SliderFlags,
) {
    validate_slider_flags(caller, flags);
    validate_slider_range(caller, min, max);
}
