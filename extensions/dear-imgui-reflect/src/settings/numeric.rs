use super::*;

/// Trait providing a default numeric range for slider widgets when no explicit
/// `min`/`max` are given.
///
/// This mirrors the behavior of the C++ ImReflect library, which uses a
/// "half-range" of the underlying numeric limits to avoid Dear ImGui's
/// internal range restrictions for very large values.
pub trait NumericDefaultRange {
    /// Default minimum value for this numeric type.
    fn default_min() -> Self;
    /// Default maximum value for this numeric type.
    fn default_max() -> Self;
}

macro_rules! impl_default_range_signed {
    ($($ty:ty),* $(,)?) => {
        $(
            impl NumericDefaultRange for $ty {
                fn default_min() -> Self {
                    // Use half-range to match ImReflect's behavior and avoid
                    // hitting Dear ImGui's internal limits for large ranges.
                    Self::MIN / 2
                }

                fn default_max() -> Self {
                    Self::MAX / 2
                }
            }
        )*
    };
}

macro_rules! impl_default_range_unsigned {
    ($($ty:ty),* $(,)?) => {
        $(
            impl NumericDefaultRange for $ty {
                fn default_min() -> Self {
                    0
                }

                fn default_max() -> Self {
                    Self::MAX / 2
                }
            }
        )*
    };
}

macro_rules! impl_default_range_float {
    ($($ty:ty),* $(,)?) => {
        $(
            impl NumericDefaultRange for $ty {
                fn default_min() -> Self {
                    Self::MIN / 2.0
                }

                fn default_max() -> Self {
                    Self::MAX / 2.0
                }
            }
        )*
    };
}

impl_default_range_signed!(i8, i16, i32, i64, isize);
impl_default_range_unsigned!(u8, u16, u32, u64, usize);
impl_default_range_float!(f32, f64);

/// Preferred widget style for numeric fields of a given primitive type.
#[derive(Clone, Copy, Debug)]
pub enum NumericWidgetKind {
    /// Input-style widget (`InputScalar` / `input_int` / `input_float`).
    Input,
    /// Drag-style widget (`DragScalar`).
    Drag,
    /// Slider-style widget (`SliderScalar`).
    Slider,
}

/// Range configuration for numeric sliders and drags.
#[derive(Clone, Copy, Debug)]
pub enum NumericRange {
    /// No explicit range (only valid for input/drag widgets).
    None,
    /// Explicit minimum and maximum values (stored as `f64` and converted per type).
    Explicit {
        /// Minimum value in the range.
        min: f64,
        /// Maximum value in the range.
        max: f64,
    },
    /// Use the default half-range for the numeric type when a slider is selected.
    DefaultSlider,
}

/// Type-level settings controlling how a numeric primitive type is rendered.
///
/// `T` is also carried by the optional display format. This prevents a format
/// validated for one C variadic carrier type from being reused with another.
#[derive(Clone, Debug)]
pub struct NumericTypeSettings<T> {
    /// Default widget kind for this numeric type.
    pub widget: NumericWidgetKind,
    /// Default range behavior for this numeric type.
    pub range: NumericRange,
    /// Default drag speed (for drag widgets), stored as `f64`.
    pub speed: Option<f64>,
    /// Default step size (for input widgets), stored as `f64`.
    pub step: Option<f64>,
    /// Default fast step size (for input widgets), stored as `f64`.
    pub step_fast: Option<f64>,
    /// Validated display format for exactly `T`, if any.
    pub format: Option<dear_imgui_rs::NumericFormat<'static, T>>,
    /// Logarithmic scale flag for slider/drag widgets.
    pub log: bool,
    /// Post-edit manual clamp (our own helper, distinct from ImGui flags).
    pub clamp: bool,
    /// Always-clamp flag for slider/drag widgets.
    pub always_clamp: bool,
    /// Wrap-around flag for slider widgets.
    pub wrap_around: bool,
    /// Disable rounding to format for slider/drag widgets.
    pub no_round_to_format: bool,
    /// Disable direct text input on sliders.
    pub no_input: bool,
    /// Clamp when editing via text input.
    pub clamp_on_input: bool,
    /// Clamp zero-range behavior.
    pub clamp_zero_range: bool,
    /// Disable built-in speed tweaks for drag widgets.
    pub no_speed_tweaks: bool,
}

impl<T> Default for NumericTypeSettings<T> {
    fn default() -> Self {
        Self {
            widget: NumericWidgetKind::Input,
            range: NumericRange::None,
            speed: None,
            step: None,
            step_fast: None,
            format: None,
            log: false,
            clamp: false,
            always_clamp: false,
            wrap_around: false,
            no_round_to_format: false,
            no_input: false,
            clamp_on_input: false,
            clamp_zero_range: false,
            no_speed_tweaks: false,
        }
    }
}

impl<T> NumericTypeSettings<T> {
    /// Build the Dear ImGui flags supported by slider widgets.
    ///
    /// `wrap_around` is intentionally excluded because Dear ImGui only
    /// supports it for drag widgets and asserts if it is passed to SliderXXX.
    pub fn slider_flags(&self) -> dear_imgui_rs::SliderFlags {
        let mut flags = dear_imgui_rs::SliderFlags::NONE;

        if self.log {
            flags |= dear_imgui_rs::SliderFlags::LOGARITHMIC;
        }
        if self.always_clamp {
            flags |= dear_imgui_rs::SliderFlags::ALWAYS_CLAMP;
        }
        if self.no_round_to_format {
            flags |= dear_imgui_rs::SliderFlags::NO_ROUND_TO_FORMAT;
        }
        if self.no_input {
            flags |= dear_imgui_rs::SliderFlags::NO_INPUT;
        }
        if self.clamp_on_input {
            flags |= dear_imgui_rs::SliderFlags::CLAMP_ON_INPUT;
        }
        if self.clamp_zero_range {
            flags |= dear_imgui_rs::SliderFlags::CLAMP_ZERO_RANGE;
        }
        if self.no_speed_tweaks {
            flags |= dear_imgui_rs::SliderFlags::NO_SPEED_TWEAKS;
        }

        flags
    }

    /// Build the Dear ImGui flags supported by drag widgets.
    pub fn drag_flags(&self) -> dear_imgui_rs::DragFlags {
        let mut flags = dear_imgui_rs::DragFlags::NONE;

        if self.log {
            flags |= dear_imgui_rs::DragFlags::LOGARITHMIC;
        }
        if self.always_clamp {
            flags |= dear_imgui_rs::DragFlags::ALWAYS_CLAMP;
        }
        if self.wrap_around {
            flags |= dear_imgui_rs::DragFlags::WRAP_AROUND;
        }
        if self.no_round_to_format {
            flags |= dear_imgui_rs::DragFlags::NO_ROUND_TO_FORMAT;
        }
        if self.no_input {
            flags |= dear_imgui_rs::DragFlags::NO_INPUT;
        }
        if self.clamp_on_input {
            flags |= dear_imgui_rs::DragFlags::CLAMP_ON_INPUT;
        }
        if self.clamp_zero_range {
            flags |= dear_imgui_rs::DragFlags::CLAMP_ZERO_RANGE;
        }
        if self.no_speed_tweaks {
            flags |= dear_imgui_rs::DragFlags::NO_SPEED_TWEAKS;
        }

        flags
    }

    /// Sets a validated display format for exactly `T`.
    pub fn with_format(mut self, format: dear_imgui_rs::NumericFormat<'static, T>) -> Self {
        self.format = Some(format);
        self
    }

    /// Clear any explicit format and fall back to Dear ImGui's defaults.
    pub fn without_format(mut self) -> Self {
        self.format = None;
        self
    }
}

macro_rules! validated_owned_format {
    ($ty:ty, $format:expr) => {{
        dear_imgui_rs::NumericFormat::<$ty>::new($format)
            .expect("internally generated numeric format must remain valid")
            .into_owned()
    }};
}

impl NumericTypeSettings<i32> {
    /// Validates and stores an owned signed 32-bit integer format.
    pub fn try_with_format(
        self,
        format: impl Into<String>,
    ) -> Result<Self, dear_imgui_rs::NumericFormatError> {
        Ok(self.with_format(dear_imgui_rs::NumericFormat::<i32>::new(format.into())?))
    }

    /// Signed decimal integer format (`%d`).
    pub fn with_decimal(self) -> Self {
        self.with_format(validated_owned_format!(i32, "%d"))
    }

    /// Validates and stores a zero-padded signed decimal format such as `%04d`.
    pub fn try_with_zero_padded_decimal(
        self,
        width: u32,
    ) -> Result<Self, dear_imgui_rs::NumericFormatError> {
        self.try_with_format(format!("%0{width}d"))
    }
}

impl NumericTypeSettings<u32> {
    /// Validates and stores an owned unsigned 32-bit integer format.
    pub fn try_with_format(
        self,
        format: impl Into<String>,
    ) -> Result<Self, dear_imgui_rs::NumericFormatError> {
        Ok(self.with_format(dear_imgui_rs::NumericFormat::<u32>::new(format.into())?))
    }

    /// Unsigned decimal integer format (`%u`).
    pub fn with_unsigned_decimal(self) -> Self {
        self.with_format(validated_owned_format!(u32, "%u"))
    }

    /// Hexadecimal integer format (`%x` or `%X`).
    pub fn with_hex(self, uppercase: bool) -> Self {
        let format = if uppercase { "%X" } else { "%x" };
        self.with_format(validated_owned_format!(u32, format))
    }

    /// Octal integer format (`%o`).
    pub fn with_octal(self) -> Self {
        self.with_format(validated_owned_format!(u32, "%o"))
    }

    /// Validates and stores a zero-padded unsigned decimal format such as `%04u`.
    pub fn try_with_zero_padded_decimal(
        self,
        width: u32,
    ) -> Result<Self, dear_imgui_rs::NumericFormatError> {
        self.try_with_format(format!("%0{width}u"))
    }
}

macro_rules! impl_float_numeric_settings {
    ($ty:ty) => {
        impl NumericTypeSettings<$ty> {
            /// Validates and stores an owned floating-point format.
            pub fn try_with_format(
                self,
                format: impl Into<String>,
            ) -> Result<Self, dear_imgui_rs::NumericFormatError> {
                Ok(self.with_format(dear_imgui_rs::NumericFormat::<$ty>::new(format.into())?))
            }

            /// Validates and stores a fixed-point format `%.Nf`.
            pub fn try_with_fixed(
                self,
                precision: u32,
            ) -> Result<Self, dear_imgui_rs::NumericFormatError> {
                self.try_with_format(format!("%.{precision}f"))
            }

            /// Validates and stores scientific notation `%.Ne` or `%.NE`.
            pub fn try_with_scientific(
                self,
                precision: u32,
                uppercase: bool,
            ) -> Result<Self, dear_imgui_rs::NumericFormatError> {
                let conversion = if uppercase { 'E' } else { 'e' };
                self.try_with_format(format!("%.{precision}{conversion}"))
            }

            /// Validates and stores percentage format `%.Nf%%` (for example, `12.3%`).
            pub fn try_with_percentage(
                self,
                precision: u32,
            ) -> Result<Self, dear_imgui_rs::NumericFormatError> {
                self.try_with_format(format!("%.{precision}f%%"))
            }

            /// Configures a clamped slider in the range [0, 1].
            pub fn try_slider_0_to_1(
                mut self,
                precision: u32,
            ) -> Result<Self, dear_imgui_rs::NumericFormatError> {
                self.widget = NumericWidgetKind::Slider;
                self.range = NumericRange::Explicit { min: 0.0, max: 1.0 };
                self.clamp = true;
                self.always_clamp = true;
                self.try_with_fixed(precision)
            }

            /// Configures a clamped slider in the range [-1, 1].
            pub fn try_slider_minus1_to_1(
                mut self,
                precision: u32,
            ) -> Result<Self, dear_imgui_rs::NumericFormatError> {
                self.widget = NumericWidgetKind::Slider;
                self.range = NumericRange::Explicit {
                    min: -1.0,
                    max: 1.0,
                };
                self.clamp = true;
                self.always_clamp = true;
                self.try_with_fixed(precision)
            }

            /// Configures a drag widget with fixed-point display formatting.
            pub fn try_drag_with_speed(
                mut self,
                speed: f64,
                precision: u32,
            ) -> Result<Self, dear_imgui_rs::NumericFormatError> {
                self.widget = NumericWidgetKind::Drag;
                self.range = NumericRange::None;
                self.speed = Some(speed);
                self.try_with_fixed(precision)
            }

            /// Configures a clamped [0, 1] slider displayed as a percentage.
            pub fn try_percentage_slider_0_to_1(
                mut self,
                precision: u32,
            ) -> Result<Self, dear_imgui_rs::NumericFormatError> {
                self.widget = NumericWidgetKind::Slider;
                self.range = NumericRange::Explicit { min: 0.0, max: 1.0 };
                self.clamp = true;
                self.always_clamp = true;
                self.try_with_percentage(precision)
            }
        }
    };
}

impl_float_numeric_settings!(f32);
impl_float_numeric_settings!(f64);

/// Numeric settings for signed 32-bit integer fields.
pub type I32NumericSettings = NumericTypeSettings<i32>;
/// Numeric settings for unsigned 32-bit integer fields.
pub type U32NumericSettings = NumericTypeSettings<u32>;
/// Numeric settings for 32-bit floating-point fields.
pub type F32NumericSettings = NumericTypeSettings<f32>;
/// Numeric settings for 64-bit floating-point fields.
pub type F64NumericSettings = NumericTypeSettings<f64>;
