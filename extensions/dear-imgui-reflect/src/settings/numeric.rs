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

/// Type-level settings controlling how a particular numeric primitive type is rendered.
#[derive(Clone, Debug)]
pub struct NumericTypeSettings {
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
    /// Default printf-style numeric format, if any.
    pub format: Option<String>,
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

impl Default for NumericTypeSettings {
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

impl NumericTypeSettings {
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

    /// Set an explicit printf-style format string for this numeric type.
    ///
    /// This is a convenience helper for configuring the underlying ImGui
    /// scalar/slider/drag widgets, equivalent to assigning `format` directly.
    pub fn with_format<S: Into<String>>(mut self, fmt: S) -> Self {
        self.format = Some(fmt.into());
        self
    }

    /// Clear any explicit format and fall back to Dear ImGui's defaults.
    pub fn without_format(mut self) -> Self {
        self.format = None;
        self
    }

    /// Decimal integer format (`%d`).
    ///
    /// Intended for signed integer types such as `i32`.
    pub fn with_decimal(self) -> Self {
        self.with_format("%d")
    }

    /// Unsigned decimal integer format (`%u`).
    ///
    /// Intended for unsigned integer types such as `u32`.
    pub fn with_unsigned(self) -> Self {
        self.with_format("%u")
    }

    /// Hexadecimal integer format (`%x` or `%X`).
    pub fn with_hex(self, uppercase: bool) -> Self {
        if uppercase {
            self.with_format("%X")
        } else {
            self.with_format("%x")
        }
    }

    /// Octal integer format (`%o`).
    pub fn with_octal(self) -> Self {
        self.with_format("%o")
    }

    /// Padded decimal integer format, e.g. width=4, pad_char='0' -> `%04d`.
    ///
    /// This is a small convenience for typical zero-padded integer displays;
    /// callers should ensure the format matches the underlying numeric type.
    pub fn with_int_padded(self, width: u32, pad_char: char) -> Self {
        // Only the first character of `pad_char` is used; multi-codepoint
        // characters will be truncated as in standard fmt! behavior.
        self.with_format(format!("%{}{}", pad_char, width) + "d")
    }

    /// Character format (`%c`), useful for small integer types interpreted as chars.
    pub fn with_char(self) -> Self {
        self.with_format("%c")
    }

    /// Floating-point format `%.Nf`, e.g. precision=3 -> `%.3f`.
    pub fn with_float(self, precision: u32) -> Self {
        self.with_format(format!("%.{}f", precision))
    }

    /// Double format `%.Nlf`, primarily for parity with ImReflect's helpers.
    pub fn with_double(self, precision: u32) -> Self {
        self.with_format(format!("%.{}lf", precision))
    }

    /// Scientific notation `%.Ne` / `%.NE`.
    pub fn with_scientific(self, precision: u32, uppercase: bool) -> Self {
        let spec = if uppercase { 'E' } else { 'e' };
        self.with_format(format!("%.{}{}", precision, spec))
    }

    /// Percentage format `%.Nf%%` (e.g. `12.3%`).
    pub fn with_percentage(self, precision: u32) -> Self {
        self.with_format(format!("%.{}f%%", precision))
    }

    /// Convenience preset: slider in the range [0, 1] with clamping and a
    /// floating-point display format `%.Nf`.
    ///
    /// This is primarily intended for floating-point types (`f32` / `f64`).
    pub fn slider_0_to_1(mut self, precision: u32) -> Self {
        self.widget = NumericWidgetKind::Slider;
        self.range = NumericRange::Explicit { min: 0.0, max: 1.0 };
        self.clamp = true;
        self.always_clamp = true;
        self.with_float(precision)
    }

    /// Convenience preset: slider in the range [-1, 1] with clamping and a
    /// floating-point display format `%.Nf`.
    ///
    /// This is primarily intended for floating-point types (`f32` / `f64`).
    pub fn slider_minus1_to_1(mut self, precision: u32) -> Self {
        self.widget = NumericWidgetKind::Slider;
        self.range = NumericRange::Explicit {
            min: -1.0,
            max: 1.0,
        };
        self.clamp = true;
        self.always_clamp = true;
        self.with_float(precision)
    }

    /// Convenience preset: drag widget with a given speed and floating-point
    /// display format `%.Nf`.
    ///
    /// This is primarily intended for floating-point types.
    pub fn drag_with_speed(mut self, speed: f64, precision: u32) -> Self {
        self.widget = NumericWidgetKind::Drag;
        self.range = NumericRange::None;
        self.speed = Some(speed);
        self.with_float(precision)
    }

    /// Convenience preset: slider in [0, 1] displayed as a percentage
    /// `%.Nf%%` with clamping.
    ///
    /// This expects the underlying numeric value to live in the 0..1 range.
    pub fn percentage_slider_0_to_1(mut self, precision: u32) -> Self {
        self.widget = NumericWidgetKind::Slider;
        self.range = NumericRange::Explicit { min: 0.0, max: 1.0 };
        self.clamp = true;
        self.always_clamp = true;
        self.with_percentage(precision)
    }
}
