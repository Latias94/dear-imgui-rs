use crate::{
    ImGuiValue, Inspector, MemberSettings, NumericDefaultRange, NumericRange, NumericTypeSettings,
    NumericWidgetKind, ReflectSettings, imgui,
};
use imgui::{DragFlags, NumericFormat, SliderFlags, internal::DataTypeKind};

#[doc(hidden)]
pub enum NumericWidget<T> {
    Input {
        step: Option<T>,
        step_fast: Option<T>,
    },
    Slider {
        range: Option<(T, T)>,
        flags: SliderFlags,
        clamp: bool,
    },
    Drag {
        range: Option<(T, T)>,
        speed: Option<f32>,
        flags: DragFlags,
        clamp: bool,
    },
}

#[doc(hidden)]
pub fn render_numeric<T: DataTypeKind + NumericDefaultRange + PartialOrd>(
    inspector: &Inspector<'_, '_>,
    label: &str,
    value: &mut T,
    widget: NumericWidget<T>,
    format: Option<NumericFormat<'_, T>>,
) -> bool {
    let ui = inspector.ui();
    match widget {
        NumericWidget::Input { step, step_fast } => {
            let mut input = ui.input_scalar(label, value);
            if let Some(step) = step {
                input = input.step(step);
            }
            if let Some(step_fast) = step_fast {
                input = input.step_fast(step_fast);
            }
            if let Some(format) = format {
                input.display_format(format).build()
            } else {
                input.build()
            }
        }
        NumericWidget::Slider {
            range,
            flags,
            clamp,
        } => {
            let (min, max) = range.unwrap_or_else(|| (T::default_min(), T::default_max()));
            let slider = ui.slider_config(label, min, max).flags(flags);
            let changed = if let Some(format) = format {
                slider.display_format(format).build(value)
            } else {
                slider.build(value)
            };
            changed | (!inspector.is_read_only() && clamp && clamp_in_range(value, min, max))
        }
        NumericWidget::Drag {
            range,
            speed,
            flags,
            clamp,
        } => {
            let mut drag = ui.drag_config(label).flags(flags);
            if let Some(speed) = speed {
                drag = drag.speed(speed);
            }
            if let Some((min, max)) = range {
                drag = drag.range(min, max);
            }
            let changed = if let Some(format) = format {
                drag.display_format(format).build(value)
            } else {
                drag.build(value)
            };
            changed
                | (!inspector.is_read_only()
                    && clamp
                    && range.is_some_and(|(min, max)| clamp_in_range(value, min, max)))
        }
    }
}

fn clamp_in_range<T: Copy + PartialOrd>(value: &mut T, min: T, max: T) -> bool {
    if min <= max {
        if *value < min {
            *value = min;
            return true;
        }
        if *value > max {
            *value = max;
            return true;
        }
    }
    false
}

trait ReflectNumeric: DataTypeKind + NumericDefaultRange + PartialOrd {
    fn from_setting(value: f64) -> Self;

    fn settings<'a>(
        settings: &'a ReflectSettings,
        member: Option<&'a MemberSettings>,
    ) -> &'a NumericTypeSettings<Self>;
}

fn render_with_settings<T: ReflectNumeric>(
    inspector: &Inspector<'_, '_>,
    label: &str,
    value: &mut T,
    member: Option<&MemberSettings>,
) -> bool {
    let settings = T::settings(inspector.settings(), member);
    let range = match settings.range {
        NumericRange::Explicit { min, max } => Some((T::from_setting(min), T::from_setting(max))),
        NumericRange::DefaultSlider | NumericRange::None => None,
    };
    let widget = match settings.widget {
        NumericWidgetKind::Input => NumericWidget::Input {
            step: settings.step.map(T::from_setting),
            step_fast: settings.step_fast.map(T::from_setting),
        },
        NumericWidgetKind::Slider => NumericWidget::Slider {
            range,
            flags: settings.slider_flags(),
            clamp: settings.clamp,
        },
        NumericWidgetKind::Drag => NumericWidget::Drag {
            range,
            speed: settings.speed.map(|speed| speed as f32),
            flags: settings.drag_flags(),
            clamp: settings.clamp,
        },
    };
    render_numeric(
        inspector,
        label,
        value,
        widget,
        settings.format.as_ref().map(NumericFormat::borrowed),
    )
}

macro_rules! impl_numeric_settings {
    ($($ty:ty => $settings:ident),* $(,)?) => {
        $(
            impl ReflectNumeric for $ty {
                fn from_setting(value: f64) -> Self {
                    value as Self
                }

                fn settings<'a>(
                    settings: &'a ReflectSettings,
                    member: Option<&'a MemberSettings>,
                ) -> &'a NumericTypeSettings<Self> {
                    member.and_then(|member| member.$settings.as_ref())
                        .unwrap_or_else(|| settings.$settings())
                }
            }

            impl ImGuiValue for $ty {
                fn imgui_value(inspector: &mut Inspector<'_, '_>, label: &str, value: &mut Self) -> bool {
                    Self::imgui_value_with_member(inspector, label, value, None)
                }

                fn imgui_value_with_member(
                    inspector: &mut Inspector<'_, '_>,
                    label: &str,
                    value: &mut Self,
                    member: Option<&MemberSettings>,
                ) -> bool {
                    render_with_settings(inspector, label, value, member)
                }
            }
        )*
    };
}

impl_numeric_settings!(
    i32 => numerics_i32,
    u32 => numerics_u32,
    f32 => numerics_f32,
    f64 => numerics_f64,
);

macro_rules! impl_input_scalar {
    ($($ty:ty),* $(,)?) => {
        $(
            impl ImGuiValue for $ty {
                fn imgui_value(inspector: &mut Inspector<'_, '_>, label: &str, value: &mut Self) -> bool {
                    inspector.ui().input_scalar(label, value).build()
                }
            }
        )*
    };
}

impl_input_scalar!(i8, i16, i64, isize, u8, u16, u64, usize);
