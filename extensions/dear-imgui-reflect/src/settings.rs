//! Global and per-member settings for dear-imgui-reflect.
//!
//! This module defines [`ReflectSettings`] and [`MemberSettings`], along with
//! container and numeric configuration types that mirror many concepts from
//! ImReflect's `ImSettings` API.

use std::any::TypeId;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

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

/// Settings that control how certain types are rendered by `dear-imgui-reflect`.
///
/// This mirrors some of the concepts from ImReflect's `ImSettings` type, but is
/// intentionally smaller and focused on common container behaviors.
#[derive(Clone, Debug)]
pub struct ReflectSettings {
    vec: VecSettings,
    bools: BoolSettings,
    arrays: ArraySettings,
    maps: MapSettings,
    tuples: TupleSettings,
    numerics_i32: NumericTypeSettings,
    numerics_f32: NumericTypeSettings,
    numerics_u32: NumericTypeSettings,
    numerics_f64: NumericTypeSettings,
    member_overrides: HashMap<(TypeId, String), MemberSettings>,
}

impl ReflectSettings {
    /// Creates a new settings object with reasonable defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Settings that apply to all `Vec<T>` containers rendered via reflection.
    pub fn vec(&self) -> &VecSettings {
        &self.vec
    }

    /// Mutable access to settings that apply to all `Vec<T>` containers.
    pub fn vec_mut(&mut self) -> &mut VecSettings {
        &mut self.vec
    }

    /// Settings that apply to all `bool` fields rendered via reflection.
    pub fn bools(&self) -> &BoolSettings {
        &self.bools
    }

    /// Mutable access to settings that apply to all `bool` fields.
    pub fn bools_mut(&mut self) -> &mut BoolSettings {
        &mut self.bools
    }

    /// Settings that apply to fixed-size arrays rendered via reflection.
    pub fn arrays(&self) -> &ArraySettings {
        &self.arrays
    }

    /// Mutable access to settings that apply to fixed-size arrays.
    pub fn arrays_mut(&mut self) -> &mut ArraySettings {
        &mut self.arrays
    }

    /// Settings that apply to string-keyed maps rendered via reflection.
    pub fn maps(&self) -> &MapSettings {
        &self.maps
    }

    /// Mutable access to settings that apply to string-keyed maps.
    pub fn maps_mut(&mut self) -> &mut MapSettings {
        &mut self.maps
    }

    /// Settings that apply to tuple-like values rendered via reflection.
    pub fn tuples(&self) -> &TupleSettings {
        &self.tuples
    }

    /// Mutable access to settings that apply to tuple-like values.
    pub fn tuples_mut(&mut self) -> &mut TupleSettings {
        &mut self.tuples
    }

    /// Type-level numeric settings for `i32` values rendered via reflection.
    pub fn numerics_i32(&self) -> &NumericTypeSettings {
        &self.numerics_i32
    }

    /// Mutable access to type-level numeric settings for `i32` values.
    pub fn numerics_i32_mut(&mut self) -> &mut NumericTypeSettings {
        &mut self.numerics_i32
    }

    /// Type-level numeric settings for `f32` values rendered via reflection.
    pub fn numerics_f32(&self) -> &NumericTypeSettings {
        &self.numerics_f32
    }

    /// Mutable access to type-level numeric settings for `f32` values.
    pub fn numerics_f32_mut(&mut self) -> &mut NumericTypeSettings {
        &mut self.numerics_f32
    }

    /// Type-level numeric settings for `u32` values rendered via reflection.
    pub fn numerics_u32(&self) -> &NumericTypeSettings {
        &self.numerics_u32
    }

    /// Mutable access to type-level numeric settings for `u32` values.
    pub fn numerics_u32_mut(&mut self) -> &mut NumericTypeSettings {
        &mut self.numerics_u32
    }

    /// Type-level numeric settings for `f64` values rendered via reflection.
    pub fn numerics_f64(&self) -> &NumericTypeSettings {
        &self.numerics_f64
    }

    /// Mutable access to type-level numeric settings for `f64` values.
    pub fn numerics_f64_mut(&mut self) -> &mut NumericTypeSettings {
        &mut self.numerics_f64
    }

    /// Returns member-level settings for a given type and field name, if any.
    ///
    /// This provides an ImSettings-style per-member override analogous to
    /// `push_member<&T::field>()` in ImReflect.
    pub fn member<T: 'static>(&self, field: &str) -> Option<&MemberSettings> {
        let key = (TypeId::of::<T>(), field.to_string());
        self.member_overrides.get(&key)
    }

    /// Returns a mutable handle to member-level settings for a given type and
    /// field name, creating a default entry if it does not yet exist.
    pub fn for_member<T: 'static>(&mut self, field: &str) -> &mut MemberSettings {
        let key = (TypeId::of::<T>(), field.to_string());
        self.member_overrides
            .entry(key)
            .or_insert_with(MemberSettings::default)
    }
}

impl Default for ReflectSettings {
    fn default() -> Self {
        Self {
            vec: VecSettings::default(),
            bools: BoolSettings::default(),
            arrays: ArraySettings::default(),
            maps: MapSettings::default(),
            tuples: TupleSettings::default(),
            numerics_i32: NumericTypeSettings::default(),
            numerics_f32: NumericTypeSettings::default(),
            numerics_u32: NumericTypeSettings::default(),
            numerics_f64: NumericTypeSettings::default(),
            member_overrides: HashMap::new(),
        }
    }
}

/// Per-member override settings layered on top of global [`ReflectSettings`].
///
/// This provides an ImSettings-style configuration surface for specific
/// fields (members) of a reflected type, analogous to
/// `ImSettings::push_member<&T::field>()` in ImReflect.
#[derive(Clone, Debug)]
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

impl Default for MemberSettings {
    fn default() -> Self {
        Self {
            read_only: false,
            bools: None,
            tuples: None,
            maps: None,
            vec: None,
            arrays: None,
            numerics_i32: None,
            numerics_f32: None,
            numerics_u32: None,
            numerics_f64: None,
        }
    }
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
        let mut numeric = NumericTypeSettings::default();
        numeric.widget = NumericWidgetKind::Input;
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
        let mut numeric = NumericTypeSettings::default();
        numeric.widget = NumericWidgetKind::Input;
        numeric = numeric.with_hex(false);
        self.numerics_i32 = Some(numeric);
        self
    }

    /// Convenience helper: configure `i32` numerics on this member as a slider
    /// over an explicit integer range, with optional clamping.
    pub fn numerics_i32_slider_range(&mut self, min: i32, max: i32, clamp: bool) -> &mut Self {
        let mut numeric = NumericTypeSettings::default();
        numeric.widget = NumericWidgetKind::Slider;
        numeric.range = NumericRange::Explicit {
            min: min as f64,
            max: max as f64,
        };
        numeric.clamp = clamp;
        numeric.always_clamp = clamp;
        numeric = numeric.with_decimal();
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
        let mut numeric = NumericTypeSettings::default();
        numeric.widget = NumericWidgetKind::Input;
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
        let mut numeric = NumericTypeSettings::default();
        numeric.widget = NumericWidgetKind::Input;
        numeric = numeric.with_hex(false);
        self.numerics_u32 = Some(numeric);
        self
    }

    /// Convenience helper: configure `u32` numerics on this member as a slider
    /// over an explicit integer range, with optional clamping.
    pub fn numerics_u32_slider_range(&mut self, min: u32, max: u32, clamp: bool) -> &mut Self {
        let mut numeric = NumericTypeSettings::default();
        numeric.widget = NumericWidgetKind::Slider;
        numeric.range = NumericRange::Explicit {
            min: min as f64,
            max: max as f64,
        };
        numeric.clamp = clamp;
        numeric.always_clamp = clamp;
        numeric = numeric.with_unsigned();
        self.numerics_u32 = Some(numeric);
        self
    }

    /// Convenience helper: configure `u32` numerics on this member as a slider
    /// in the range [0, 100] with clamping.
    pub fn numerics_u32_slider_0_to_100(&mut self) -> &mut Self {
        self.numerics_u32_slider_range(0, 100, true)
    }
}

/// Settings controlling how `Vec<T>` containers are edited.
///
/// These correspond conceptually to ImReflect's `insertable` / `removable` /
/// `reorderable` mixins for `std::vector<T>`.
#[derive(Clone, Debug)]
pub struct VecSettings {
    /// Whether insertion of new elements is allowed (via `+` button).
    pub insertable: bool,
    /// Whether removal of elements is allowed (via `-` button).
    pub removable: bool,
    /// Whether elements can be reordered using drag-and-drop handles.
    pub reorderable: bool,
    /// Whether the vector contents are wrapped in a collapsible tree node.
    pub dropdown: bool,
}

impl Default for VecSettings {
    fn default() -> Self {
        Self::editable()
    }
}

impl VecSettings {
    /// Fully editable vector: insertion/removal and reordering enabled, wrapped
    /// in a dropdown. This corresponds to the default ImReflect behavior for
    /// `std::vector<T>`.
    pub fn editable() -> Self {
        Self {
            insertable: true,
            removable: true,
            reorderable: true,
            dropdown: true,
        }
    }

    /// Reorder-only vector: disable insertion/removal, keep drag-to-reorder
    /// handles enabled. This mirrors an ImReflect-style "reorderable only"
    /// configuration.
    pub fn reorder_only() -> Self {
        Self {
            insertable: false,
            removable: false,
            reorderable: true,
            dropdown: true,
        }
    }

    /// Fixed vector: no insertion, removal, or reordering. The contents are
    /// still editable unless combined with `read_only`.
    pub fn fixed() -> Self {
        Self {
            insertable: false,
            removable: false,
            reorderable: false,
            dropdown: true,
        }
    }
}

/// Preferred widget style for boolean fields.
#[derive(Clone, Copy, Debug)]
pub enum BoolStyle {
    /// Render using a standard ImGui checkbox.
    Checkbox,
    /// Render using a toggle button with text for true/false.
    Button,
    /// Render using two radio buttons (true/false).
    Radio,
    /// Render using a two-item dropdown (false/true).
    Dropdown,
}

/// Settings controlling how `bool` fields are edited when no per-field
/// attributes are provided.
#[derive(Clone, Debug)]
pub struct BoolSettings {
    /// Default widget style for `bool` fields.
    pub style: BoolStyle,
}

impl Default for BoolSettings {
    fn default() -> Self {
        Self {
            style: BoolStyle::Checkbox,
        }
    }
}

/// Settings controlling how fixed-size arrays like `[T; N]` are edited.
#[derive(Clone, Debug)]
pub struct ArraySettings {
    /// Whether the array contents are wrapped in a collapsible tree node.
    pub dropdown: bool,
    /// Whether elements can be reordered within the array.
    pub reorderable: bool,
}

impl Default for ArraySettings {
    fn default() -> Self {
        Self {
            dropdown: true,
            reorderable: true,
        }
    }
}

impl ArraySettings {
    /// Fully editable array: elements can be reordered via drag handles.
    pub fn editable() -> Self {
        Self {
            dropdown: true,
            reorderable: true,
        }
    }

    /// Fixed-order array: reordering disabled, but still rendered in a
    /// dropdown. This mirrors an ImReflect-style "no reorder" array.
    pub fn fixed_order() -> Self {
        Self {
            dropdown: true,
            reorderable: false,
        }
    }
}

/// Settings controlling how string-keyed maps like `HashMap<String, V>` and
/// `BTreeMap<String, V>` are edited.
#[derive(Clone, Debug)]
pub struct MapSettings {
    /// Whether the map contents are wrapped in a collapsible tree node.
    pub dropdown: bool,
    /// Whether insertion of new entries is allowed (via `+` button).
    pub insertable: bool,
    /// Whether removal of entries is allowed (via `-` button next to each row).
    pub removable: bool,
    /// Whether entries are rendered inside an ImGui table for better alignment.
    pub use_table: bool,
    /// Number of columns to use when `use_table` is true (at least 3).
    ///
    /// The first column is reserved for the row handle/context menu, the
    /// second for the key, and the third for the value. Larger values widen
    /// the table but currently do not change semantics.
    pub columns: usize,
}

impl Default for MapSettings {
    fn default() -> Self {
        Self::editable()
    }
}

impl MapSettings {
    /// Fully editable map: insertion/removal enabled, optional table layout.
    pub fn editable() -> Self {
        Self {
            dropdown: true,
            insertable: true,
            removable: true,
            use_table: false,
            columns: 3,
        }
    }

    /// Const-map: insertion and removal disabled, values are still editable
    /// unless combined with `read_only`. Uses a table layout by default for
    /// better alignment.
    pub fn const_map() -> Self {
        Self {
            dropdown: true,
            insertable: false,
            removable: false,
            use_table: true,
            columns: 3,
        }
    }
}

/// Preferred render mode for tuple-like values.
#[derive(Clone, Copy, Debug)]
pub enum TupleRenderMode {
    /// Render all elements on a single line.
    Line,
    /// Render elements inside an ImGui table with multiple columns.
    Grid,
}

/// Settings controlling how tuple-like values such as `(A, B)` and `(A, B, C)`
/// are rendered.
#[derive(Clone, Debug)]
pub struct TupleSettings {
    /// Whether the tuple contents are wrapped in a collapsible tree node.
    pub dropdown: bool,
    /// How tuple elements are laid out: line or grid.
    pub render_mode: TupleRenderMode,
    /// Number of columns to use in grid mode (clamped to at least 1 and at
    /// most the number of tuple elements).
    pub columns: usize,
    /// Whether the outer label is rendered on the same line as the tuple
    /// contents (line mode) or above them.
    pub same_line: bool,
    /// Optional minimum width for each element when rendered in grid mode.
    pub min_width: Option<f32>,
}

impl Default for TupleSettings {
    fn default() -> Self {
        Self {
            dropdown: false,
            render_mode: TupleRenderMode::Line,
            columns: 3,
            same_line: true,
            min_width: None,
        }
    }
}

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

static GLOBAL_SETTINGS: OnceLock<Mutex<ReflectSettings>> = OnceLock::new();

fn settings_mutex() -> &'static Mutex<ReflectSettings> {
    GLOBAL_SETTINGS.get_or_init(|| Mutex::new(ReflectSettings::default()))
}

/// Runs `f` with a shared reference to the current global `ReflectSettings`.
///
/// This helper avoids cloning the settings object on read-only access paths
/// (such as container `ImGuiValue` implementations) while preserving the
/// existing `current_settings()` API for callers that need an owned copy.
pub(crate) fn with_settings_read<F, R>(f: F) -> R
where
    F: FnOnce(&ReflectSettings) -> R,
{
    let guard = settings_mutex().lock().unwrap();
    f(&*guard)
}

/// Returns a clone of the current global `ReflectSettings`.
///
/// This is used internally by container editors to honor type-level defaults.
pub fn current_settings() -> ReflectSettings {
    settings_mutex().lock().unwrap().clone()
}

/// Mutates the global `ReflectSettings` in place.
///
/// This provides an `ImSettings`-style entry point to configure type-level
/// defaults for container editing behavior.
pub fn with_settings<F, R>(f: F) -> R
where
    F: FnOnce(&mut ReflectSettings) -> R,
{
    let mut guard = settings_mutex().lock().unwrap();
    f(&mut *guard)
}

/// Executes a closure with a temporary modification of the global
/// [`ReflectSettings`], restoring the previous settings afterwards.
///
/// This provides a lightweight, ImSettings-style "scope" mechanism for
/// experiments or per-panel overrides without introducing a full push/pop
/// stack API. A typical usage pattern is:
///
/// ```no_run
/// # use dear_imgui_reflect as reflect;
/// # use reflect::ImGuiReflectExt;
/// #
/// # #[derive(reflect::ImGuiReflect, Default)]
/// # struct MyType {
/// #     items: Vec<i32>,
/// # }
/// #
/// # fn ui_frame(ui: &reflect::imgui::Ui, value: &mut MyType) {
/// reflect::with_settings_scope(|| {
///     reflect::with_settings(|s| {
///         s.for_member::<MyType>("items").vec_reorder_only();
///     });
///     ui.input_reflect("Debug Items", value);
/// });
/// # }
/// ```
///
/// Inside the scope, `with_settings` can be used freely; once the closure
/// returns, the previous `ReflectSettings` are restored.
pub fn with_settings_scope<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
{
    // Snapshot current settings.
    let saved = current_settings();
    // Run user code, allowing it to call `with_settings` as needed.
    let result = f();
    // Restore previous settings.
    let mut guard = settings_mutex().lock().unwrap();
    *guard = saved;
    result
}
