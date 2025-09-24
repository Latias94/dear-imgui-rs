use crate::Ui;
use crate::internal::DataTypeKind;
use crate::sys;
use std::ffi::c_void;

/// Builder for slider widgets
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

impl<'ui, Label, Data> Slider<'ui, Label, Data>
where
    Label: AsRef<str>,
    Data: DataTypeKind,
{
    /// Creates a new slider builder
    #[doc(alias = "SliderScalar", alias = "SliderScalarN")]
    #[deprecated(note = "Use `Ui::slider` or `Ui::slider_config`.", since = "0.1.0")]
    pub fn new(ui: &'ui Ui, label: Label, min: Data, max: Data) -> Self {
        Self {
            ui,
            label,
            min,
            max,
            display_format: None,
            flags: SliderFlags::NONE,
        }
    }
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
    /// # use dear_imgui::*;
    /// # let mut ctx = Context::create_or_panic();
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
    /// Note for f32 and f64 sliders, Dear ImGui limits the available
    /// range to half their full range (e.g `f32::MIN/2.0 .. f32::MAX/2.0`)
    /// Specifying a value above this will cause an abort.
    /// For large ranged values, consider using input widgets instead
    #[inline]
    pub fn range(mut self, min: Data, max: Data) -> Self {
        self.min = min;
        self.max = max;
        self
    }

    /// Sets the display format using *a C-style printf string*
    #[inline]
    pub fn display_format<Format2: AsRef<str>>(
        self,
        display_format: Format2,
    ) -> Slider<'ui, Label, Data, Format2> {
        Slider {
            ui: self.ui,
            label: self.label,
            min: self.min,
            max: self.max,
            display_format: Some(display_format),
            flags: self.flags,
        }
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
    pub fn build(self, value: &mut Data) -> bool {
        unsafe {
            let (label, display_format) = self
                .ui
                .scratch_txt_with_opt(self.label, self.display_format);

            sys::igSliderScalar(
                label,
                Data::KIND as i32,
                value as *mut Data as *mut c_void,
                &self.min as *const Data as *const c_void,
                &self.max as *const Data as *const c_void,
                display_format,
                self.flags.bits(),
            )
        }
    }

    /// Builds a horizontal array of multiple sliders attached to the given slice.
    ///
    /// Returns true if any slider value was changed.
    pub fn build_array(self, values: &mut [Data]) -> bool {
        unsafe {
            let (label, display_format) = self
                .ui
                .scratch_txt_with_opt(self.label, self.display_format);

            sys::igSliderScalarN(
                label,
                Data::KIND as i32,
                values.as_mut_ptr() as *mut c_void,
                values.len() as i32,
                &self.min as *const Data as *const c_void,
                &self.max as *const Data as *const c_void,
                display_format,
                self.flags.bits(),
            )
        }
    }
}

/// Builder for a vertical slider widget.
#[derive(Clone, Debug)]
#[must_use]
pub struct VerticalSlider<Label, Data, Format = &'static str> {
    label: Label,
    size: [f32; 2],
    min: Data,
    max: Data,
    display_format: Option<Format>,
    flags: SliderFlags,
}

impl<Label, Data> VerticalSlider<Label, Data>
where
    Label: AsRef<str>,
    Data: DataTypeKind,
{
    /// Constructs a new vertical slider builder with the given size and range.
    ///
    /// ```no_run
    /// # use dear_imgui::*;
    /// # let mut ctx = Context::create_or_panic();
    /// # let ui = ctx.frame();
    /// VerticalSlider::new("Example", [20.0, 20.0], i8::MIN, i8::MAX)
    ///     .range(4, 8)
    ///     // Remember to call .build(&ui)
    ///     ;
    /// ```
    ///
    /// It is safe, though up to C++ Dear ImGui, on how to handle when
    /// `min > max`.
    #[doc(alias = "VSliderScalar")]
    pub fn new(label: Label, size: impl Into<[f32; 2]>, min: Data, max: Data) -> Self {
        VerticalSlider {
            label,
            size: size.into(),
            min,
            max,
            display_format: None,
            flags: SliderFlags::NONE,
        }
    }
}

impl<Label, Data, Format> VerticalSlider<Label, Data, Format>
where
    Label: AsRef<str>,
    Data: DataTypeKind,
    Format: AsRef<str>,
{
    /// Sets the range for the vertical slider.
    ///
    /// ```no_run
    /// # use dear_imgui::*;
    /// # let mut ctx = Context::create_or_panic();
    /// # let ui = ctx.frame();
    /// VerticalSlider::new("Example", [20.0, 20.0], i8::MIN, i8::MAX)
    ///     .range(4, 8)
    ///     // Remember to call .build(&ui)
    ///     ;
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

    /// Sets the display format using *a C-style printf string*
    #[inline]
    pub fn display_format<Format2: AsRef<str>>(
        self,
        display_format: Format2,
    ) -> VerticalSlider<Label, Data, Format2> {
        VerticalSlider {
            label: self.label,
            size: self.size,
            min: self.min,
            max: self.max,
            display_format: Some(display_format),
            flags: self.flags,
        }
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
    pub fn build(self, ui: &Ui, value: &mut Data) -> bool {
        unsafe {
            let (label, display_format) = ui.scratch_txt_with_opt(self.label, self.display_format);
            let size = sys::ImVec2::new(self.size[0], self.size[1]);

            sys::igVSliderScalar(
                label,
                size,
                Data::KIND as i32,
                value as *mut Data as *mut c_void,
                &self.min as *const Data as *const c_void,
                &self.max as *const Data as *const c_void,
                display_format,
                self.flags.bits(),
            )
        }
    }
}

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
    /// # use dear_imgui::*;
    /// # let mut ctx = Context::create_or_panic();
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
        unsafe {
            let (label, display_format) = ui.scratch_txt_two(self.label, self.display_format);

            sys::igSliderAngle(
                label,
                value_rad as *mut _,
                self.min_degrees,
                self.max_degrees,
                display_format,
                self.flags.bits(),
            )
        }
    }
}

bitflags::bitflags! {
    /// Flags for slider widgets
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct SliderFlags: i32 {
        /// No flags
        const NONE = 0;
        /// Clamp value to min/max bounds when input manually with CTRL+Click. By default CTRL+Click allows going out of bounds.
        const ALWAYS_CLAMP = sys::ImGuiSliderFlags_AlwaysClamp as i32;
        /// Make the widget logarithmic (linear otherwise). Consider using ImGuiSliderFlags_NoRoundToFormat with this if using a format-string with small amount of digits.
        const LOGARITHMIC = sys::ImGuiSliderFlags_Logarithmic as i32;
        /// Disable rounding underlying value to match precision of the display format string (e.g. %.3f values are rounded to those 3 digits)
        const NO_ROUND_TO_FORMAT = sys::ImGuiSliderFlags_NoRoundToFormat as i32;
        /// Disable CTRL+Click or Enter key allowing to input text directly into the widget
        const NO_INPUT = sys::ImGuiSliderFlags_NoInput as i32;
    }
}

impl Ui {
    /// Creates a new slider widget. Returns true if the value has been edited.
    pub fn slider<T: AsRef<str>, K: DataTypeKind>(
        &self,
        label: T,
        min: K,
        max: K,
        value: &mut K,
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
        self.slider_config(label, min, max).build(value)
    }

    /// Creates an integer slider
    #[doc(alias = "SliderInt")]
    pub fn slider_i32(&self, label: impl AsRef<str>, value: &mut i32, min: i32, max: i32) -> bool {
        self.slider_config(label, min, max).build(value)
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
        VerticalSlider::new(label, size, min, max).build(self, value)
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
        VerticalSlider::new(label, size, min, max).build(self, value)
    }

    /// Creates an angle slider (value in radians)
    #[doc(alias = "SliderAngle")]
    pub fn slider_angle(&self, label: impl AsRef<str>, value_rad: &mut f32) -> bool {
        AngleSlider::new(label).build(self, value_rad)
    }
}
