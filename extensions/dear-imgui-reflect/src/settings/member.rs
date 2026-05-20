use super::*;

/// Per-member override settings layered on top of global [`ReflectSettings`].
///
/// This provides an ImSettings-style configuration surface for specific
/// fields (members) of a reflected type, analogous to
/// `ImSettings::push_member<&T::field>()` in ImReflect.
#[derive(Clone, Debug, Default)]
pub struct MemberSettings {
    /// Whether this member should be rendered in a read-only (disabled) state.
    pub read_only: bool,
    /// Optional override for boolean style settings on this member.
    pub bools: Option<BoolSettings>,
    /// Optional override for tuple rendering settings on this member.
    pub tuples: Option<TupleSettings>,
    /// Optional override for map rendering settings on this member.
    pub maps: Option<MapSettings>,
    /// Optional override for vector rendering settings on this member.
    pub vec: Option<VecSettings>,
    /// Optional override for fixed-size array rendering settings on this member.
    pub arrays: Option<ArraySettings>,
    /// Optional numeric settings override for `i32` members.
    pub numerics_i32: Option<NumericTypeSettings>,
    /// Optional numeric settings override for `f32` members.
    pub numerics_f32: Option<NumericTypeSettings>,
    /// Optional numeric settings override for `u32` members.
    pub numerics_u32: Option<NumericTypeSettings>,
    /// Optional numeric settings override for `f64` members.
    pub numerics_f64: Option<NumericTypeSettings>,
}

impl MemberSettings {
    /// Convenience helper: mark a `Vec<T>` member as "reorder-only" (no
    /// insertion/removal, but drag-to-reorder remains enabled).
    pub fn vec_reorder_only(&mut self) -> &mut Self {
        self.vec = Some(VecSettings::reorder_only());
        self
    }

    /// Convenience helper: mark a `Vec<T>` member as "fixed" (no insertion,
    /// removal, or reordering). The contents are still editable unless
    /// combined with `read_only`.
    pub fn vec_fixed(&mut self) -> &mut Self {
        self.vec = Some(VecSettings::fixed());
        self
    }

    /// Convenience helper: mark an array member as fixed-order (no reordering
    /// of elements, but still rendered inside an optional dropdown).
    pub fn arrays_fixed_order(&mut self) -> &mut Self {
        self.arrays = Some(ArraySettings::fixed_order());
        self
    }

    /// Convenience helper: mark a map member as "const-map" (no insertion or
    /// removal, keys/values still editable when not read-only).
    pub fn maps_const(&mut self) -> &mut Self {
        self.maps = Some(MapSettings::const_map());
        self
    }

    /// Convenience helper: explicitly mark a map member as fully editable
    /// using default dropdown/table behavior.
    pub fn maps_editable(&mut self) -> &mut Self {
        self.maps = Some(MapSettings::editable());
        self
    }

    /// Convenience helper: configure `f32` numerics on this member as a slider
    /// in the range [0, 1] with clamping and `%.Nf` formatting.
    pub fn numerics_f32_slider_0_to_1(&mut self, precision: u32) -> &mut Self {
        self.numerics_f32 = Some(NumericTypeSettings::default().slider_0_to_1(precision));
        self
    }

    /// Convenience helper: configure `f32` numerics on this member as a slider
    /// in the range [-1, 1] with clamping and `%.Nf` formatting.
    pub fn numerics_f32_slider_minus1_to_1(&mut self, precision: u32) -> &mut Self {
        self.numerics_f32 = Some(NumericTypeSettings::default().slider_minus1_to_1(precision));
        self
    }

    /// Convenience helper: configure `f32` numerics on this member as a drag
    /// widget with a given speed and `%.Nf` formatting.
    pub fn numerics_f32_drag_with_speed(&mut self, speed: f64, precision: u32) -> &mut Self {
        self.numerics_f32 = Some(NumericTypeSettings::default().drag_with_speed(speed, precision));
        self
    }

    /// Convenience helper: configure `f32` numerics on this member as a slider
    /// in [0, 1] displayed as a percentage `%.Nf%%` with clamping.
    pub fn numerics_f32_percentage_slider_0_to_1(&mut self, precision: u32) -> &mut Self {
        self.numerics_f32 =
            Some(NumericTypeSettings::default().percentage_slider_0_to_1(precision));
        self
    }

    /// Convenience helper: configure `f64` numerics on this member as a slider
    /// in the range [0, 1] with clamping and `%.Nf` formatting.
    pub fn numerics_f64_slider_0_to_1(&mut self, precision: u32) -> &mut Self {
        self.numerics_f64 = Some(NumericTypeSettings::default().slider_0_to_1(precision));
        self
    }

    /// Convenience helper: configure `f64` numerics on this member as a slider
    /// in the range [-1, 1] with clamping and `%.Nf` formatting.
    pub fn numerics_f64_slider_minus1_to_1(&mut self, precision: u32) -> &mut Self {
        self.numerics_f64 = Some(NumericTypeSettings::default().slider_minus1_to_1(precision));
        self
    }

    /// Convenience helper: configure `f64` numerics on this member as a drag
    /// widget with a given speed and `%.Nf` formatting.
    pub fn numerics_f64_drag_with_speed(&mut self, speed: f64, precision: u32) -> &mut Self {
        self.numerics_f64 = Some(NumericTypeSettings::default().drag_with_speed(speed, precision));
        self
    }

    /// Convenience helper: configure `f64` numerics on this member as a slider
    /// in [0, 1] displayed as a percentage `%.Nf%%` with clamping.
    pub fn numerics_f64_percentage_slider_0_to_1(&mut self, precision: u32) -> &mut Self {
        self.numerics_f64 =
            Some(NumericTypeSettings::default().percentage_slider_0_to_1(precision));
        self
    }

    /// Convenience helper: configure `i32` numerics on this member as an
    /// input widget using decimal formatting (`%d`) and optional steps.
    ///
    /// Pass zero for `step` or `step_fast` to leave that parameter unset.
    pub fn numerics_i32_input_decimal(&mut self, step: i32, step_fast: i32) -> &mut Self {
        let mut numeric = NumericTypeSettings {
            widget: NumericWidgetKind::Input,
            ..NumericTypeSettings::default()
        };
        if step != 0 {
            numeric.step = Some(step as f64);
        }
        if step_fast != 0 {
            numeric.step_fast = Some(step_fast as f64);
        }
        numeric = numeric.with_decimal();
        self.numerics_i32 = Some(numeric);
        self
    }

    /// Convenience helper: configure `i32` numerics on this member as an
    /// input widget using lowercase hexadecimal formatting (`%x`).
    pub fn numerics_i32_input_hex(&mut self) -> &mut Self {
        let numeric = NumericTypeSettings {
            widget: NumericWidgetKind::Input,
            ..NumericTypeSettings::default()
        }
        .with_hex(false);
        self.numerics_i32 = Some(numeric);
        self
    }

    /// Convenience helper: configure `i32` numerics on this member as a slider
    /// over an explicit integer range, with optional clamping.
    pub fn numerics_i32_slider_range(&mut self, min: i32, max: i32, clamp: bool) -> &mut Self {
        let numeric = NumericTypeSettings {
            widget: NumericWidgetKind::Slider,
            range: NumericRange::Explicit {
                min: min as f64,
                max: max as f64,
            },
            clamp,
            always_clamp: clamp,
            ..NumericTypeSettings::default()
        }
        .with_decimal();
        self.numerics_i32 = Some(numeric);
        self
    }

    /// Convenience helper: configure `i32` numerics on this member as a slider
    /// in the range [0, 100] with clamping.
    pub fn numerics_i32_slider_0_to_100(&mut self) -> &mut Self {
        self.numerics_i32_slider_range(0, 100, true)
    }

    /// Convenience helper: configure `u32` numerics on this member as an
    /// input widget using unsigned decimal formatting (`%u`) and optional
    /// steps. Pass zero to leave a step unset.
    pub fn numerics_u32_input_decimal(&mut self, step: u32, step_fast: u32) -> &mut Self {
        let mut numeric = NumericTypeSettings {
            widget: NumericWidgetKind::Input,
            ..NumericTypeSettings::default()
        };
        if step != 0 {
            numeric.step = Some(step as f64);
        }
        if step_fast != 0 {
            numeric.step_fast = Some(step_fast as f64);
        }
        numeric = numeric.with_unsigned();
        self.numerics_u32 = Some(numeric);
        self
    }

    /// Convenience helper: configure `u32` numerics on this member as an
    /// input widget using lowercase hexadecimal formatting (`%x`).
    pub fn numerics_u32_input_hex(&mut self) -> &mut Self {
        let numeric = NumericTypeSettings {
            widget: NumericWidgetKind::Input,
            ..NumericTypeSettings::default()
        }
        .with_hex(false);
        self.numerics_u32 = Some(numeric);
        self
    }

    /// Convenience helper: configure `u32` numerics on this member as a slider
    /// over an explicit integer range, with optional clamping.
    pub fn numerics_u32_slider_range(&mut self, min: u32, max: u32, clamp: bool) -> &mut Self {
        let numeric = NumericTypeSettings {
            widget: NumericWidgetKind::Slider,
            range: NumericRange::Explicit {
                min: min as f64,
                max: max as f64,
            },
            clamp,
            always_clamp: clamp,
            ..NumericTypeSettings::default()
        }
        .with_unsigned();
        self.numerics_u32 = Some(numeric);
        self
    }

    /// Convenience helper: configure `u32` numerics on this member as a slider
    /// in the range [0, 100] with clamping.
    pub fn numerics_u32_slider_0_to_100(&mut self) -> &mut Self {
        self.numerics_u32_slider_range(0, 100, true)
    }
}
