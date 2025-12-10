//! Reflection-based helpers for dear-imgui-rs.
//!
//! This crate provides traits and helpers to automatically generate Dear ImGui
//! widgets for your Rust types, similar to the C++ ImReflect library.

#![deny(rust_2018_idioms)]
#![deny(missing_docs)]
#![allow(clippy::needless_lifetimes)]

use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::hash::BuildHasher;
use std::rc::Rc;
use std::sync::{Arc, Mutex, OnceLock};

/// Re-export the dear-imgui-rs crate for convenience.
///
/// Users can write `use dear_imgui_reflect::imgui::*;` instead of depending
/// on `dear-imgui-rs` directly if they only need basic types.
pub use dear_imgui_rs as imgui;

/// Trait for values that can render themselves as a single ImGui input widget.
///
/// This is implemented for common primitive types and can be implemented
/// manually for your own types. Most users will interact with
/// [`ImGuiReflect`](trait.ImGuiReflect.html) instead of this trait directly.
pub trait ImGuiValue {
    /// Draw a widget for this value.
    ///
    /// Returns `true` if the value was modified.
    fn imgui_value(ui: &imgui::Ui, label: &str, value: &mut Self) -> bool;
}

/// Trait for complex types (structs/enums) that can generate ImGui controls
/// for all of their fields.
///
/// You can derive this trait with `#[derive(ImGuiReflect)]` from this crate.
pub trait ImGuiReflect {
    /// Draw an ImGui editor for this value with the given label.
    ///
    /// Returns `true` if any field was modified.
    fn imgui_reflect(&mut self, ui: &imgui::Ui, label: &str) -> bool;
}

/// Blanket implementation: any type that implements [`ImGuiReflect`] can also
/// be used wherever an [`ImGuiValue`] is expected.
impl<T: ImGuiReflect> ImGuiValue for T {
    fn imgui_value(ui: &imgui::Ui, label: &str, value: &mut Self) -> bool {
        value.imgui_reflect(ui, label)
    }
}

/// Transparent reflection for boxed values.
///
/// This allows `Box<T>` where `T: ImGuiReflect` to be edited like `T` itself,
/// matching ImReflect's behavior for smart pointers that simply forward to
/// the pointed-to value when engaged.
impl<T: ImGuiReflect> ImGuiReflect for Box<T> {
    fn imgui_reflect(&mut self, ui: &imgui::Ui, label: &str) -> bool {
        self.as_mut().imgui_reflect(ui, label)
    }
}

/// Transparent reflection for reference-counted values (`Rc<T>`).
///
/// When there is exactly one strong reference, this forwards editing to the
/// inner `T`. Otherwise, it renders a read-only marker indicating that the
/// value is shared and cannot be safely mutated.
impl<T: ImGuiReflect> ImGuiReflect for Rc<T> {
    fn imgui_reflect(&mut self, ui: &imgui::Ui, label: &str) -> bool {
        if let Some(inner) = Rc::get_mut(self) {
            inner.imgui_reflect(ui, label)
        } else {
            ui.text(label);
            ui.same_line();
            ui.text("<Rc shared (read-only)>");
            false
        }
    }
}

/// Transparent reflection for atomically reference-counted values (`Arc<T>`).
///
/// When there is exactly one strong reference, this forwards editing to the
/// inner `T`. Otherwise, it renders a read-only marker indicating that the
/// value is shared and cannot be safely mutated.
impl<T: ImGuiReflect> ImGuiReflect for Arc<T> {
    fn imgui_reflect(&mut self, ui: &imgui::Ui, label: &str) -> bool {
        if let Some(inner) = Arc::get_mut(self) {
            inner.imgui_reflect(ui, label)
        } else {
            ui.text(label);
            ui.same_line();
            ui.text("<Arc shared (read-only)>");
            false
        }
    }
}

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

/// Render ImGui controls for a value that implements [`ImGuiReflect`].
///
/// This is the main entry point mirroring the C++ `ImReflect::Input` API.
pub fn input<T: ImGuiReflect>(ui: &imgui::Ui, label: &str, value: &mut T) -> bool {
    value.imgui_reflect(ui, label)
}

/// Extension methods on `Ui` for reflection-based widgets.
pub trait ImGuiReflectExt {
    /// Render a reflected editor for a value.
    ///
    /// Returns `true` if any field changed.
    fn input_reflect<T: ImGuiReflect>(&self, label: &str, value: &mut T) -> bool;
}

impl ImGuiReflectExt for imgui::Ui {
    fn input_reflect<T: ImGuiReflect>(&self, label: &str, value: &mut T) -> bool {
        input(self, label, value)
    }
}

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
    /// removal, or reordering).
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

static GLOBAL_SETTINGS: OnceLock<Mutex<ReflectSettings>> = OnceLock::new();

fn settings_mutex() -> &'static Mutex<ReflectSettings> {
    GLOBAL_SETTINGS.get_or_init(|| Mutex::new(ReflectSettings::default()))
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

/// Per-popup temporary key buffers for map insertion popups, keyed by popup id.
///
/// This allows users to type a key for a new map entry across multiple frames
/// before confirming insertion, similar to ImReflect's `temp_key` storage.
static MAP_ADD_KEY_STATE: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();

fn map_add_state() -> &'static Mutex<HashMap<String, String>> {
    MAP_ADD_KEY_STATE.get_or_init(|| Mutex::new(HashMap::new()))
}

thread_local! {
    /// Per-popup temporary value buffers for map insertion popups, keyed by
    /// `(TypeId, popup_id)`. This allows users to edit the value for a new
    /// entry across multiple frames before confirming insertion.
    static MAP_ADD_VALUE_STATE: RefCell<HashMap<(TypeId, String), Box<dyn Any>>> =
        RefCell::new(HashMap::new());
}

fn with_map_add_value_state<F, R>(f: F) -> R
where
    F: FnOnce(&mut HashMap<(TypeId, String), Box<dyn Any>>) -> R,
{
    MAP_ADD_VALUE_STATE.with(|cell| {
        let mut map = cell.borrow_mut();
        f(&mut *map)
    })
}

fn with_temp_map_value<V, F>(popup_id: &str, f: F)
where
    V: Default + 'static,
    F: FnOnce(&mut V),
{
    let key = (TypeId::of::<V>(), popup_id.to_string());
    with_map_add_value_state(|values| {
        let entry = values
            .entry(key.clone())
            .or_insert_with(|| Box::<V>::new(V::default()) as Box<dyn Any>);
        let temp_value = entry
            .downcast_mut::<V>()
            .expect("map_add_value_state type mismatch");
        f(temp_value)
    })
}

// Primitive ImGuiValue implementations

/// ImGui editor for a `bool` using a checkbox.
impl ImGuiValue for bool {
    fn imgui_value(ui: &imgui::Ui, label: &str, value: &mut Self) -> bool {
        ui.checkbox(label, value)
    }
}

/// ImGui editor for a 32-bit signed integer.
impl ImGuiValue for i32 {
    fn imgui_value(ui: &imgui::Ui, label: &str, value: &mut Self) -> bool {
        ui.input_int(label, value)
    }
}

/// ImGui editor for a 32-bit float.
impl ImGuiValue for f32 {
    fn imgui_value(ui: &imgui::Ui, label: &str, value: &mut Self) -> bool {
        ui.input_float(label, value)
    }
}

/// ImGui editor for a 64-bit float.
impl ImGuiValue for f64 {
    fn imgui_value(ui: &imgui::Ui, label: &str, value: &mut Self) -> bool {
        ui.input_double(label, value)
    }
}

/// ImGui editor for an owned UTF-8 string.
impl ImGuiValue for String {
    fn imgui_value(ui: &imgui::Ui, label: &str, value: &mut Self) -> bool {
        ui.input_text(label, value).build()
    }
}

/// ImGui editor for an ImString buffer (zero-copy).
impl ImGuiValue for imgui::ImString {
    fn imgui_value(ui: &imgui::Ui, label: &str, value: &mut Self) -> bool {
        ui.input_text_imstr(label, value).build()
    }
}

// Integer scalar types via InputScalar

impl ImGuiValue for i8 {
    fn imgui_value(ui: &imgui::Ui, label: &str, value: &mut Self) -> bool {
        ui.input_scalar(label, value).build()
    }
}

impl ImGuiValue for u8 {
    fn imgui_value(ui: &imgui::Ui, label: &str, value: &mut Self) -> bool {
        ui.input_scalar(label, value).build()
    }
}

impl ImGuiValue for i16 {
    fn imgui_value(ui: &imgui::Ui, label: &str, value: &mut Self) -> bool {
        ui.input_scalar(label, value).build()
    }
}

impl ImGuiValue for u16 {
    fn imgui_value(ui: &imgui::Ui, label: &str, value: &mut Self) -> bool {
        ui.input_scalar(label, value).build()
    }
}

impl ImGuiValue for u32 {
    fn imgui_value(ui: &imgui::Ui, label: &str, value: &mut Self) -> bool {
        ui.input_scalar(label, value).build()
    }
}

impl ImGuiValue for i64 {
    fn imgui_value(ui: &imgui::Ui, label: &str, value: &mut Self) -> bool {
        ui.input_scalar(label, value).build()
    }
}

impl ImGuiValue for u64 {
    fn imgui_value(ui: &imgui::Ui, label: &str, value: &mut Self) -> bool {
        ui.input_scalar(label, value).build()
    }
}

impl ImGuiValue for isize {
    fn imgui_value(ui: &imgui::Ui, label: &str, value: &mut Self) -> bool {
        ui.input_scalar(label, value).build()
    }
}

impl ImGuiValue for usize {
    fn imgui_value(ui: &imgui::Ui, label: &str, value: &mut Self) -> bool {
        ui.input_scalar(label, value).build()
    }
}

// Small fixed-size arrays treated as containers (with optional reordering).

impl ImGuiValue for [f32; 2] {
    fn imgui_value(ui: &imgui::Ui, label: &str, value: &mut Self) -> bool {
        let settings = current_settings();
        let arr_settings = settings.arrays();
        imgui_array_with_settings(ui, label, value, arr_settings)
    }
}

impl ImGuiValue for [f32; 3] {
    fn imgui_value(ui: &imgui::Ui, label: &str, value: &mut Self) -> bool {
        let settings = current_settings();
        let arr_settings = settings.arrays();
        imgui_array_with_settings(ui, label, value, arr_settings)
    }
}

impl ImGuiValue for [f32; 4] {
    fn imgui_value(ui: &imgui::Ui, label: &str, value: &mut Self) -> bool {
        let settings = current_settings();
        let arr_settings = settings.arrays();
        imgui_array_with_settings(ui, label, value, arr_settings)
    }
}

impl ImGuiValue for [i32; 2] {
    fn imgui_value(ui: &imgui::Ui, label: &str, value: &mut Self) -> bool {
        let settings = current_settings();
        let arr_settings = settings.arrays();
        imgui_array_with_settings(ui, label, value, arr_settings)
    }
}

impl ImGuiValue for [i32; 3] {
    fn imgui_value(ui: &imgui::Ui, label: &str, value: &mut Self) -> bool {
        let settings = current_settings();
        let arr_settings = settings.arrays();
        imgui_array_with_settings(ui, label, value, arr_settings)
    }
}

impl ImGuiValue for [i32; 4] {
    fn imgui_value(ui: &imgui::Ui, label: &str, value: &mut Self) -> bool {
        let settings = current_settings();
        let arr_settings = settings.arrays();
        imgui_array_with_settings(ui, label, value, arr_settings)
    }
}

/// Public helper for rendering fixed-size arrays using explicit `ArraySettings`.
///
/// This mirrors the behavior of the built-in `ImGuiValue` implementations for
/// `[f32; N]` / `[i32; N]`, but allows callers (such as the derive macro) to
/// supply per-member settings layered on top of global defaults.
pub fn imgui_array_with_settings<T, const N: usize>(
    ui: &imgui::Ui,
    label: &str,
    value: &mut [T; N],
    arr_settings: &ArraySettings,
) -> bool
where
    T: ImGuiValue,
{
    let header_label = format!("{label} [{N}]");

    if arr_settings.dropdown {
        if let Some(_node) = ui.tree_node(&header_label) {
            imgui_array_body_inner(ui, label, value, arr_settings)
        } else {
            false
        }
    } else {
        ui.text(&header_label);
        imgui_array_body_inner(ui, label, value, arr_settings)
    }
}

fn imgui_array_body_inner<T, const N: usize>(
    ui: &imgui::Ui,
    label: &str,
    value: &mut [T; N],
    arr_settings: &ArraySettings,
) -> bool
where
    T: ImGuiValue,
{
    let mut changed = false;
    let mut move_op: Option<(usize, usize)> = None;

    for index in 0..N {
        if arr_settings.reorderable {
            let handle_label = format!("==##{label}_arr_handle_{index}");
            ui.text(&handle_label);

            if let Some(_source) = ui
                .drag_drop_source_config("IMGUI_REFLECT_ARRAY_ITEM")
                // Text() items do not have an ID, so allow a null ID here to
                // avoid Dear ImGui assertions when starting a drag from this
                // label.
                .flags(imgui::DragDropFlags::SOURCE_ALLOW_NULL_ID)
                .begin_payload(index as i32)
            {
                ui.text(&handle_label);
            }

            ui.same_line();
        }

        let elem_label = format!("{label}[{index}]");
        changed |= T::imgui_value(ui, &elem_label, &mut value[index]);

        if arr_settings.reorderable {
            if let Some(target) = ui.drag_drop_target() {
                if let Some(Ok(payload)) = target.accept_payload::<i32, _>(
                    "IMGUI_REFLECT_ARRAY_ITEM",
                    imgui::DragDropFlags::NONE,
                ) {
                    if payload.delivery {
                        let from = payload.data as usize;
                        let to = index;
                        move_op = Some((from, to));
                    }
                }
                target.pop();
            }
        }
    }

    if let Some((from, to)) = move_op {
        if from < N && to < N && from != to {
            value.swap(from, to);
            changed = true;
        }
    }

    changed
}

// Basic map views for string-keyed maps: edit values in-place, keys displayed
// as labels, with simple insertion/removal helpers controlled by MapSettings.

impl<V, S> ImGuiValue for HashMap<String, V, S>
where
    V: ImGuiValue + Default + Clone + 'static,
    S: BuildHasher,
{
    fn imgui_value(ui: &imgui::Ui, label: &str, map: &mut Self) -> bool {
        let settings = current_settings();
        let map_settings = settings.maps();
        imgui_hash_map_with_settings(ui, label, map, map_settings)
    }
}

impl<V> ImGuiValue for BTreeMap<String, V>
where
    V: ImGuiValue + Default + Clone + 'static,
{
    fn imgui_value(ui: &imgui::Ui, label: &str, map: &mut Self) -> bool {
        let settings = current_settings();
        let map_settings = settings.maps();
        imgui_btree_map_with_settings(ui, label, map, map_settings)
    }
}

/// Public helper for rendering `HashMap<String, V, S>` using explicit `MapSettings`.
///
/// This mirrors the behavior of the built-in `ImGuiValue` implementation but
/// allows callers (such as the derive macro) to supply per-member map settings.
pub fn imgui_hash_map_with_settings<V, S>(
    ui: &imgui::Ui,
    label: &str,
    map: &mut HashMap<String, V, S>,
    map_settings: &MapSettings,
) -> bool
where
    V: ImGuiValue + Default + Clone + 'static,
    S: BuildHasher,
{
    let mut changed = false;
    let header_label = format!("{label} [{}]", map.len());

    if map_settings.dropdown {
        if let Some(_node) = ui.tree_node(&header_label) {
            changed |= imgui_hash_map_body(ui, label, map, map_settings);
        }
    } else {
        ui.text(&header_label);
        changed |= imgui_hash_map_body(ui, label, map, map_settings);
    }

    changed
}

/// Public helper for rendering `BTreeMap<String, V>` using explicit `MapSettings`.
pub fn imgui_btree_map_with_settings<V>(
    ui: &imgui::Ui,
    label: &str,
    map: &mut BTreeMap<String, V>,
    map_settings: &MapSettings,
) -> bool
where
    V: ImGuiValue + Default + Clone + 'static,
{
    let mut changed = false;
    let header_label = format!("{label} [{}]", map.len());

    if map_settings.dropdown {
        if let Some(_node) = ui.tree_node(&header_label) {
            changed |= imgui_btree_map_body(ui, label, map, map_settings);
        }
    } else {
        ui.text(&header_label);
        changed |= imgui_btree_map_body(ui, label, map, map_settings);
    }

    changed
}

fn imgui_hash_map_body<V, S>(
    ui: &imgui::Ui,
    label: &str,
    map: &mut HashMap<String, V, S>,
    map_settings: &MapSettings,
) -> bool
where
    V: ImGuiValue + Default + Clone + 'static,
    S: BuildHasher,
{
    let mut changed = false;
    let mut key_to_remove: Option<String> = None;
    let mut clear_all = false;
    let mut rename_ops: Vec<(String, String)> = Vec::new();

    // Popup id used for the "add new entry" dialog.
    let popup_id = format!("add_map_item_popup##{label}");

    // "+" button to open a popup where the user can type a key for the new entry.
    // When insertion is disabled via MapSettings, we still render a disabled button
    // with a tooltip to mirror ImReflect's behavior.
    ui.same_line();
    let add_label = format!("+##{label}_add");
    if map_settings.insertable {
        if ui.small_button(&add_label) {
            let mut state = map_add_state().lock().unwrap();
            let key_buf = state.entry(popup_id.clone()).or_insert_with(String::new);
            if key_buf.is_empty() {
                let mut idx = map.len();
                loop {
                    let candidate = format!("{label}_{}", idx);
                    if !map.contains_key(&candidate) {
                        *key_buf = candidate;
                        break;
                    }
                    idx += 1;
                }
            }
            ui.open_popup(&popup_id);
        }
    } else {
        let _disabled = ui.begin_disabled();
        let _ = ui.small_button(&add_label);
        drop(_disabled);
        if ui.is_item_hovered_with_flags(imgui::HoveredFlags::ALLOW_WHEN_DISABLED) {
            ui.set_item_tooltip("Insertion disabled in MapSettings");
        }
    }

    // Add-entry popup: let the user confirm insertion with a custom key and
    // a pre-edited value (optionally copied from an existing entry).
    if let Some(_popup) = ui.begin_popup(&popup_id) {
        let mut key_state = map_add_state().lock().unwrap();
        let key_buf = key_state
            .entry(popup_id.clone())
            .or_insert_with(String::new);

        ui.text("Add map entry");

        let key_label = format!("Key##{label}_new_key");
        let _ = String::imgui_value(ui, &key_label, key_buf);

        let value_label = format!("Value##{label}_new_value");
        with_temp_map_value::<V, _>(&popup_id, |temp_value| {
            changed |= V::imgui_value(ui, &value_label, temp_value);
        });

        if !map.is_empty() {
            ui.separator();
            ui.text("Copy value from existing entry:");
            let mut idx = 0usize;
            for (existing_key, existing_value) in map.iter() {
                let copy_label = format!("Copy from \"{existing_key}\"##{label}_copy_{idx}");
                if ui.small_button(&copy_label) {
                    with_temp_map_value::<V, _>(&popup_id, |temp_value| {
                        *temp_value = existing_value.clone();
                    });
                    changed = true;
                }
                idx += 1;
            }
        }

        if ui.button("Add") {
            if !key_buf.is_empty() && !map.contains_key(key_buf) {
                with_temp_map_value::<V, _>(&popup_id, |temp_value| {
                    let value = std::mem::take(temp_value);
                    map.insert(key_buf.clone(), value);
                });
                key_buf.clear();
                with_map_add_value_state(|values| {
                    values.remove(&(TypeId::of::<V>(), popup_id.clone()));
                });
                changed = true;
                ui.close_current_popup();
            }
        }

        ui.same_line();

        if ui.button("Cancel") {
            key_buf.clear();
            with_map_add_value_state(|values| {
                values.remove(&(TypeId::of::<V>(), popup_id.clone()));
            });
            ui.close_current_popup();
        }
    }
    let mut index = 0usize;

    // Optional table layout for better alignment between key and value columns.
    if map_settings.use_table {
        let columns = map_settings.columns.max(3);
        let table_id = format!("##map_table_{label}");

        if let Some(_table) = ui.begin_table(&table_id, columns) {
            // We always use the first three columns for handle, key, and value.
            for (key, value) in map.iter_mut() {
                ui.table_next_row();

                // Column 0: drag/context handle ("==").
                ui.table_next_column();
                let popup_id = format!("map_item_context_{index}##{label}");
                ui.text("==");
                if ui.is_item_clicked_with_button(imgui::MouseButton::Right) {
                    ui.open_popup(&popup_id);
                }

                // Column 1: editable key.
                ui.table_next_column();
                let mut key_buf = key.clone();
                let key_label = format!("##{label}_key_{index}");
                changed |= String::imgui_value(ui, &key_label, &mut key_buf);
                if key_buf != *key && !key_buf.is_empty() {
                    rename_ops.push((key.clone(), key_buf));
                }

                // Column 2: value editor.
                ui.table_next_column();
                let value_label = format!("##{label}_value_{index}");
                changed |= V::imgui_value(ui, &value_label, value);

                // Per-item context menu: remove this entry or clear all. When
                // removal is disabled, show disabled items with a tooltip.
                ui.popup(&popup_id, || {
                    if map_settings.removable {
                        if ui.menu_item("Remove item") {
                            key_to_remove = Some(key.clone());
                        }
                        if ui.menu_item("Clear all") {
                            clear_all = true;
                        }
                    } else {
                        let _disabled = ui.begin_disabled();
                        ui.menu_item("Remove item");
                        if ui.is_item_hovered_with_flags(imgui::HoveredFlags::ALLOW_WHEN_DISABLED) {
                            ui.set_item_tooltip("Removal disabled in MapSettings");
                        }
                        ui.menu_item("Clear all");
                        if ui.is_item_hovered_with_flags(imgui::HoveredFlags::ALLOW_WHEN_DISABLED) {
                            ui.set_item_tooltip("Removal disabled in MapSettings");
                        }
                        drop(_disabled);
                    }
                });

                index += 1;
            }
        }
    } else {
        for (key, value) in map.iter_mut() {
            // Drag/context handle, similar to ImReflect's "==" marker.
            let popup_id = format!("map_item_context_{index}##{label}");
            ui.text("==");
            if ui.is_item_clicked_with_button(imgui::MouseButton::Right) {
                ui.open_popup(&popup_id);
            }
            ui.same_line();

            // Editable key: start from the current key string.
            let mut key_buf = key.clone();
            let key_label = format!("##{label}_key_{index}");
            changed |= String::imgui_value(ui, &key_label, &mut key_buf);

            // Apply key rename after the loop to avoid mutating the map while iterating.
            if key_buf != *key && !key_buf.is_empty() {
                rename_ops.push((key.clone(), key_buf));
            }

            ui.same_line();

            // Value editor with a hidden label suffix to keep IDs unique.
            let value_label = format!("##{label}_value_{index}");
            changed |= V::imgui_value(ui, &value_label, value);

            // Per-item context menu: remove this entry or clear all.
            ui.popup(&popup_id, || {
                if map_settings.removable {
                    if ui.menu_item("Remove item") {
                        key_to_remove = Some(key.clone());
                    }
                    if ui.menu_item("Clear all") {
                        clear_all = true;
                    }
                } else {
                    let _disabled = ui.begin_disabled();
                    ui.menu_item("Remove item");
                    if ui.is_item_hovered_with_flags(imgui::HoveredFlags::ALLOW_WHEN_DISABLED) {
                        ui.set_item_tooltip("Removal disabled in MapSettings");
                    }
                    ui.menu_item("Clear all");
                    if ui.is_item_hovered_with_flags(imgui::HoveredFlags::ALLOW_WHEN_DISABLED) {
                        ui.set_item_tooltip("Removal disabled in MapSettings");
                    }
                    drop(_disabled);
                }
            });

            index += 1;
        }
    }

    if clear_all {
        map.clear();
        changed = true;
    } else {
        if let Some(k) = key_to_remove {
            map.remove(&k);
            changed = true;
        }

        // Apply any key renames that do not collide with existing entries.
        for (old_key, new_key) in rename_ops {
            if old_key == new_key {
                continue;
            }
            if map.contains_key(&new_key) {
                continue;
            }
            if let Some(value) = map.remove(&old_key) {
                map.insert(new_key, value);
                changed = true;
            }
        }
    }

    changed
}

fn imgui_btree_map_body<V>(
    ui: &imgui::Ui,
    label: &str,
    map: &mut BTreeMap<String, V>,
    map_settings: &MapSettings,
) -> bool
where
    V: ImGuiValue + Default + Clone + 'static,
{
    let mut changed = false;
    let mut key_to_remove: Option<String> = None;
    let mut clear_all = false;
    let mut rename_ops: Vec<(String, String)> = Vec::new();

    // Popup id used for the "add new entry" dialog.
    let popup_id = format!("add_map_item_popup##{label}");

    ui.same_line();
    let add_label = format!("+##{label}_add");
    if map_settings.insertable {
        if ui.small_button(&add_label) {
            let mut state = map_add_state().lock().unwrap();
            let key_buf = state.entry(popup_id.clone()).or_insert_with(String::new);
            if key_buf.is_empty() {
                let mut idx = map.len();
                loop {
                    let candidate = format!("{label}_{}", idx);
                    if !map.contains_key(&candidate) {
                        *key_buf = candidate;
                        break;
                    }
                    idx += 1;
                }
            }
            ui.open_popup(&popup_id);
        }
    } else {
        let _disabled = ui.begin_disabled();
        let _ = ui.small_button(&add_label);
        drop(_disabled);
        if ui.is_item_hovered_with_flags(imgui::HoveredFlags::ALLOW_WHEN_DISABLED) {
            ui.set_item_tooltip("Insertion disabled in MapSettings");
        }
    }

    if let Some(_popup) = ui.begin_popup(&popup_id) {
        let mut key_state = map_add_state().lock().unwrap();
        let key_buf = key_state
            .entry(popup_id.clone())
            .or_insert_with(String::new);

        ui.text("Add map entry");

        let key_label = format!("Key##{label}_new_key");
        let _ = String::imgui_value(ui, &key_label, key_buf);

        let value_label = format!("Value##{label}_new_value");
        with_temp_map_value::<V, _>(&popup_id, |temp_value| {
            changed |= V::imgui_value(ui, &value_label, temp_value);
        });

        if !map.is_empty() {
            ui.separator();
            ui.text("Copy value from existing entry:");
            let mut idx = 0usize;
            for (existing_key, existing_value) in map.iter() {
                let copy_label = format!("Copy from \"{existing_key}\"##{label}_copy_{idx}");
                if ui.small_button(&copy_label) {
                    with_temp_map_value::<V, _>(&popup_id, |temp_value| {
                        *temp_value = existing_value.clone();
                    });
                    changed = true;
                }
                idx += 1;
            }
        }

        if ui.button("Add") {
            if !key_buf.is_empty() && !map.contains_key(key_buf) {
                with_temp_map_value::<V, _>(&popup_id, |temp_value| {
                    let value = std::mem::take(temp_value);
                    map.insert(key_buf.clone(), value);
                });
                key_buf.clear();
                with_map_add_value_state(|values| {
                    values.remove(&(TypeId::of::<V>(), popup_id.clone()));
                });
                changed = true;
                ui.close_current_popup();
            }
        }

        ui.same_line();

        if ui.button("Cancel") {
            key_buf.clear();
            with_map_add_value_state(|values| {
                values.remove(&(TypeId::of::<V>(), popup_id.clone()));
            });
            ui.close_current_popup();
        }
    }
    let mut index = 0usize;

    if map_settings.use_table {
        let columns = map_settings.columns.max(3);
        let table_id = format!("##map_table_{label}");

        if let Some(_table) = ui.begin_table(&table_id, columns) {
            for (key, value) in map.iter_mut() {
                ui.table_next_row();

                // Column 0: handle.
                ui.table_next_column();
                let popup_id = format!("map_item_context_{index}##{label}");
                ui.text("==");
                if ui.is_item_clicked_with_button(imgui::MouseButton::Right) {
                    ui.open_popup(&popup_id);
                }

                // Column 1: key.
                ui.table_next_column();
                let mut key_buf = key.clone();
                let key_label = format!("##{label}_key_{index}");
                changed |= String::imgui_value(ui, &key_label, &mut key_buf);
                if key_buf != *key && !key_buf.is_empty() {
                    rename_ops.push((key.clone(), key_buf));
                }

                // Column 2: value.
                ui.table_next_column();
                let value_label = format!("##{label}_value_{index}");
                changed |= V::imgui_value(ui, &value_label, value);

                ui.popup(&popup_id, || {
                    if map_settings.removable {
                        if ui.menu_item("Remove item") {
                            key_to_remove = Some(key.clone());
                        }
                        if ui.menu_item("Clear all") {
                            clear_all = true;
                        }
                    } else {
                        let _disabled = ui.begin_disabled();
                        ui.menu_item("Remove item");
                        if ui.is_item_hovered_with_flags(imgui::HoveredFlags::ALLOW_WHEN_DISABLED) {
                            ui.set_item_tooltip("Removal disabled in MapSettings");
                        }
                        ui.menu_item("Clear all");
                        if ui.is_item_hovered_with_flags(imgui::HoveredFlags::ALLOW_WHEN_DISABLED) {
                            ui.set_item_tooltip("Removal disabled in MapSettings");
                        }
                        drop(_disabled);
                    }
                });

                index += 1;
            }
        }
    } else {
        for (key, value) in map.iter_mut() {
            let popup_id = format!("map_item_context_{index}##{label}");
            ui.text("==");
            if ui.is_item_clicked_with_button(imgui::MouseButton::Right) {
                ui.open_popup(&popup_id);
            }
            ui.same_line();

            let mut key_buf = key.clone();
            let key_label = format!("##{label}_key_{index}");
            changed |= String::imgui_value(ui, &key_label, &mut key_buf);

            if key_buf != *key && !key_buf.is_empty() {
                rename_ops.push((key.clone(), key_buf));
            }

            ui.same_line();

            let value_label = format!("##{label}_value_{index}");
            changed |= V::imgui_value(ui, &value_label, value);

            ui.popup(&popup_id, || {
                if map_settings.removable {
                    if ui.menu_item("Remove item") {
                        key_to_remove = Some(key.clone());
                    }
                    if ui.menu_item("Clear all") {
                        clear_all = true;
                    }
                } else {
                    let _disabled = ui.begin_disabled();
                    ui.menu_item("Remove item");
                    if ui.is_item_hovered_with_flags(imgui::HoveredFlags::ALLOW_WHEN_DISABLED) {
                        ui.set_item_tooltip("Removal disabled in MapSettings");
                    }
                    ui.menu_item("Clear all");
                    if ui.is_item_hovered_with_flags(imgui::HoveredFlags::ALLOW_WHEN_DISABLED) {
                        ui.set_item_tooltip("Removal disabled in MapSettings");
                    }
                    drop(_disabled);
                }
            });

            index += 1;
        }
    }

    if clear_all {
        map.clear();
        changed = true;
    } else {
        if let Some(k) = key_to_remove {
            map.remove(&k);
            changed = true;
        }

        for (old_key, new_key) in rename_ops {
            if old_key == new_key {
                continue;
            }
            if map.contains_key(&new_key) {
                continue;
            }
            if let Some(value) = map.remove(&old_key) {
                map.insert(new_key, value);
                changed = true;
            }
        }
    }

    changed
}

// Optional values rendered as a checkbox plus nested editor when enabled.
impl<T> ImGuiValue for Option<T>
where
    T: ImGuiValue + Default,
{
    fn imgui_value(ui: &imgui::Ui, label: &str, value: &mut Self) -> bool {
        let mut enabled = value.is_some();
        let mut changed = ui.checkbox(label, &mut enabled);

        match (enabled, value.as_mut()) {
            (true, Some(inner)) => {
                ui.indent();
                let inner_label = format!("{label}##value");
                changed |= T::imgui_value(ui, &inner_label, inner);
                ui.unindent();
            }
            (true, None) => {
                *value = Some(T::default());
                changed = true;
                if let Some(inner) = value.as_mut() {
                    ui.indent();
                    let inner_label = format!("{label}##value");
                    changed |= T::imgui_value(ui, &inner_label, inner);
                    ui.unindent();
                }
            }
            (false, Some(_)) => {
                *value = None;
                changed = true;
            }
            (false, None) => {}
        }

        changed
    }
}

/// Tuple and pair-style values rendered in line or grid mode.
///
/// This helper is used both by the built-in `ImGuiValue` implementations for
/// small tuples and by the derive macro for struct fields that contain tuple
/// types, allowing consistent layout behavior across both paths.
pub fn imgui_tuple_body<F>(
    ui: &imgui::Ui,
    label: &str,
    element_count: usize,
    settings: &TupleSettings,
    mut render_element: F,
) -> bool
where
    F: FnMut(&imgui::Ui, usize) -> bool,
{
    let mut changed = false;

    let mut render_inner = |ui: &imgui::Ui, changed: &mut bool| match settings.render_mode {
        TupleRenderMode::Line => {
            let _id = ui.push_id(label);
            for index in 0..element_count {
                if index > 0 {
                    ui.same_line();
                }
                *changed |= render_element(ui, index);
            }
        }
        TupleRenderMode::Grid => {
            let columns = settings.columns.max(1).min(element_count.max(1));
            let table_id = format!("##tuple_table_{label}");

            if let Some(_table) = ui.begin_table(&table_id, columns) {
                if let Some(min_width) = settings.min_width {
                    for _ in 0..columns {
                        ui.table_setup_column(
                            "",
                            imgui::TableColumnFlags::WIDTH_FIXED,
                            min_width,
                            0,
                        );
                    }
                }

                for index in 0..element_count {
                    ui.table_next_column();
                    let _id = ui.push_id(index as i32);
                    *changed |= render_element(ui, index);
                }
            }
        }
    };

    if settings.dropdown {
        let _id = ui.push_id(label);
        if let Some(_node) = ui.tree_node(label) {
            render_inner(ui, &mut changed);
        }
    } else {
        // Outer label placement for non-dropdown tuples.
        ui.text(label);
        if settings.same_line {
            ui.same_line();
        }
        render_inner(ui, &mut changed);
    }

    changed
}

impl<A, B> ImGuiValue for (A, B)
where
    A: ImGuiValue,
    B: ImGuiValue,
{
    fn imgui_value(ui: &imgui::Ui, label: &str, value: &mut Self) -> bool {
        let settings = current_settings();
        let tuple_settings = settings.tuples();

        let (ref mut a, ref mut b) = *value;
        imgui_tuple_body(ui, label, 2, tuple_settings, |ui, index| match index {
            0 => A::imgui_value(ui, "##0", a),
            1 => B::imgui_value(ui, "##1", b),
            _ => false,
        })
    }
}

impl<A, B, C> ImGuiValue for (A, B, C)
where
    A: ImGuiValue,
    B: ImGuiValue,
    C: ImGuiValue,
{
    fn imgui_value(ui: &imgui::Ui, label: &str, value: &mut Self) -> bool {
        let settings = current_settings();
        let tuple_settings = settings.tuples();

        let (ref mut a, ref mut b, ref mut c) = *value;
        imgui_tuple_body(ui, label, 3, tuple_settings, |ui, index| match index {
            0 => A::imgui_value(ui, "##0", a),
            1 => B::imgui_value(ui, "##1", b),
            2 => C::imgui_value(ui, "##2", c),
            _ => false,
        })
    }
}

impl<A, B, C, D> ImGuiValue for (A, B, C, D)
where
    A: ImGuiValue,
    B: ImGuiValue,
    C: ImGuiValue,
    D: ImGuiValue,
{
    fn imgui_value(ui: &imgui::Ui, label: &str, value: &mut Self) -> bool {
        let settings = current_settings();
        let tuple_settings = settings.tuples();

        let (ref mut a, ref mut b, ref mut c, ref mut d) = *value;
        imgui_tuple_body(ui, label, 4, tuple_settings, |ui, index| match index {
            0 => A::imgui_value(ui, "##0", a),
            1 => B::imgui_value(ui, "##1", b),
            2 => C::imgui_value(ui, "##2", c),
            3 => D::imgui_value(ui, "##3", d),
            _ => false,
        })
    }
}

impl<A, B, C, D, E> ImGuiValue for (A, B, C, D, E)
where
    A: ImGuiValue,
    B: ImGuiValue,
    C: ImGuiValue,
    D: ImGuiValue,
    E: ImGuiValue,
{
    fn imgui_value(ui: &imgui::Ui, label: &str, value: &mut Self) -> bool {
        let settings = current_settings();
        let tuple_settings = settings.tuples();

        let (ref mut a, ref mut b, ref mut c, ref mut d, ref mut e) = *value;
        imgui_tuple_body(ui, label, 5, tuple_settings, |ui, index| match index {
            0 => A::imgui_value(ui, "##0", a),
            1 => B::imgui_value(ui, "##1", b),
            2 => C::imgui_value(ui, "##2", c),
            3 => D::imgui_value(ui, "##3", d),
            4 => E::imgui_value(ui, "##4", e),
            _ => false,
        })
    }
}

impl<A, B, C, D, E, F> ImGuiValue for (A, B, C, D, E, F)
where
    A: ImGuiValue,
    B: ImGuiValue,
    C: ImGuiValue,
    D: ImGuiValue,
    E: ImGuiValue,
    F: ImGuiValue,
{
    fn imgui_value(ui: &imgui::Ui, label: &str, value: &mut Self) -> bool {
        let settings = current_settings();
        let tuple_settings = settings.tuples();

        let (ref mut a, ref mut b, ref mut c, ref mut d, ref mut e, ref mut f) = *value;
        imgui_tuple_body(ui, label, 6, tuple_settings, |ui, index| match index {
            0 => A::imgui_value(ui, "##0", a),
            1 => B::imgui_value(ui, "##1", b),
            2 => C::imgui_value(ui, "##2", c),
            3 => D::imgui_value(ui, "##3", d),
            4 => E::imgui_value(ui, "##4", e),
            5 => F::imgui_value(ui, "##5", f),
            _ => false,
        })
    }
}

impl<A, B, C, D, E, F, G> ImGuiValue for (A, B, C, D, E, F, G)
where
    A: ImGuiValue,
    B: ImGuiValue,
    C: ImGuiValue,
    D: ImGuiValue,
    E: ImGuiValue,
    F: ImGuiValue,
    G: ImGuiValue,
{
    fn imgui_value(ui: &imgui::Ui, label: &str, value: &mut Self) -> bool {
        let settings = current_settings();
        let tuple_settings = settings.tuples();

        let (ref mut a, ref mut b, ref mut c, ref mut d, ref mut e, ref mut f, ref mut g) = *value;
        imgui_tuple_body(ui, label, 7, tuple_settings, |ui, index| match index {
            0 => A::imgui_value(ui, "##0", a),
            1 => B::imgui_value(ui, "##1", b),
            2 => C::imgui_value(ui, "##2", c),
            3 => D::imgui_value(ui, "##3", d),
            4 => E::imgui_value(ui, "##4", e),
            5 => F::imgui_value(ui, "##5", f),
            6 => G::imgui_value(ui, "##6", g),
            _ => false,
        })
    }
}

impl<A, B, C, D, E, F, G, H> ImGuiValue for (A, B, C, D, E, F, G, H)
where
    A: ImGuiValue,
    B: ImGuiValue,
    C: ImGuiValue,
    D: ImGuiValue,
    E: ImGuiValue,
    F: ImGuiValue,
    G: ImGuiValue,
    H: ImGuiValue,
{
    fn imgui_value(ui: &imgui::Ui, label: &str, value: &mut Self) -> bool {
        let settings = current_settings();
        let tuple_settings = settings.tuples();

        let (
            ref mut a,
            ref mut b,
            ref mut c,
            ref mut d,
            ref mut e,
            ref mut f,
            ref mut g,
            ref mut h,
        ) = *value;
        imgui_tuple_body(ui, label, 8, tuple_settings, |ui, index| match index {
            0 => A::imgui_value(ui, "##0", a),
            1 => B::imgui_value(ui, "##1", b),
            2 => C::imgui_value(ui, "##2", c),
            3 => D::imgui_value(ui, "##3", d),
            4 => E::imgui_value(ui, "##4", e),
            5 => F::imgui_value(ui, "##5", f),
            6 => G::imgui_value(ui, "##6", g),
            7 => H::imgui_value(ui, "##7", h),
            _ => false,
        })
    }
}

// Editable vectors with basic insertion/removal and drag-to-reorder support.
impl<T> ImGuiValue for Vec<T>
where
    T: ImGuiValue + Default,
{
    fn imgui_value(ui: &imgui::Ui, label: &str, value: &mut Self) -> bool {
        let settings = current_settings();
        let vec_settings = settings.vec();
        imgui_vec_with_settings(ui, label, value, vec_settings)
    }
}

/// Public helper for rendering `Vec<T>` using explicit `VecSettings`.
///
/// This mirrors the behavior of the built-in `ImGuiValue` implementation but
/// allows callers (such as the derive macro) to supply per-member vector
/// settings layered on top of global defaults.
pub fn imgui_vec_with_settings<T>(
    ui: &imgui::Ui,
    label: &str,
    value: &mut Vec<T>,
    vec_settings: &VecSettings,
) -> bool
where
    T: ImGuiValue + Default,
{
    // Show element count in the header label.
    let header_label = format!("{label} [{}]", value.len());

    if vec_settings.dropdown {
        if let Some(_node) = ui.tree_node(&header_label) {
            imgui_vec_body(ui, label, value, vec_settings)
        } else {
            false
        }
    } else {
        ui.text(&header_label);
        imgui_vec_body(ui, label, value, vec_settings)
    }
}

fn imgui_vec_body<T>(
    ui: &imgui::Ui,
    label: &str,
    value: &mut Vec<T>,
    vec_settings: &VecSettings,
) -> bool
where
    T: ImGuiValue + Default,
{
    let mut changed = false;

    // Inline "+" / "-" controls for inserting/removing elements.
    if vec_settings.insertable {
        ui.same_line();
        if ui.small_button("+") {
            value.push(T::default());
            changed = true;
        }
    }

    if vec_settings.removable && !value.is_empty() {
        ui.same_line();
        if ui.small_button("-") {
            value.pop();
            changed = true;
        }
    }

    // Optional drag-and-drop reordering state captured for this frame.
    let mut move_op: Option<(usize, usize)> = None;

    // Render each element as "label[index]" with an optional drag handle.
    for index in 0..value.len() {
        if vec_settings.reorderable {
            let handle_label = format!("==##{label}_handle_{index}");
            ui.text(&handle_label);

            if let Some(_source) = ui
                .drag_drop_source_config("IMGUI_REFLECT_VEC_ITEM")
                // Text() items do not have an ID, so we must allow a null ID
                // here to avoid Dear ImGui's internal assertion when starting
                // a drag from this label.
                .flags(imgui::DragDropFlags::SOURCE_ALLOW_NULL_ID)
                .begin_payload(index as i32)
            {
                ui.text(&handle_label);
            }

            ui.same_line();
        }

        let elem_label = format!("{label}[{index}]");
        changed |= T::imgui_value(ui, &elem_label, &mut value[index]);

        if vec_settings.reorderable {
            if let Some(target) = ui.drag_drop_target() {
                if let Some(Ok(payload)) = target
                    .accept_payload::<i32, _>("IMGUI_REFLECT_VEC_ITEM", imgui::DragDropFlags::NONE)
                {
                    if payload.delivery {
                        let from = payload.data as usize;
                        let to = index;
                        move_op = Some((from, to));
                    }
                }
                target.pop();
            }
        }
    }

    if let Some((from, to)) = move_op {
        let len = value.len();
        if from < len && to < len && from != to {
            let item = value.remove(from);
            let insert_index = if from < to { to.saturating_sub(1) } else { to };
            value.insert(insert_index, item);
            changed = true;
        }
    }

    changed
}

// Optional math crate integrations

/// ImGui editors for `glam` vector types when the `glam` feature is enabled.
#[cfg(feature = "glam")]
mod glam_impls {
    use super::{ImGuiValue, imgui};
    use glam::{Mat4, Quat, Vec2, Vec3, Vec4};

    impl ImGuiValue for Vec2 {
        fn imgui_value(ui: &imgui::Ui, label: &str, value: &mut Self) -> bool {
            let mut arr = value.to_array();
            let changed = ui.input_float2(label, &mut arr).build();
            if changed {
                *value = Vec2::from_array(arr);
            }
            changed
        }
    }

    impl ImGuiValue for Vec3 {
        fn imgui_value(ui: &imgui::Ui, label: &str, value: &mut Self) -> bool {
            let mut arr = value.to_array();
            let changed = ui.input_float3(label, &mut arr).build();
            if changed {
                *value = Vec3::from_array(arr);
            }
            changed
        }
    }

    impl ImGuiValue for Vec4 {
        fn imgui_value(ui: &imgui::Ui, label: &str, value: &mut Self) -> bool {
            let mut arr = value.to_array();
            let changed = ui.input_float4(label, &mut arr).build();
            if changed {
                *value = Vec4::from_array(arr);
            }
            changed
        }
    }

    impl ImGuiValue for Quat {
        fn imgui_value(ui: &imgui::Ui, label: &str, value: &mut Self) -> bool {
            // Represent the quaternion as (x, y, z, w) and allow direct editing.
            // After editing, renormalize to keep it as a unit quaternion.
            let mut arr = value.to_array();
            let changed = ui.input_float4(label, &mut arr).build();
            if changed {
                let mut q = Quat::from_xyzw(arr[0], arr[1], arr[2], arr[3]);
                // Avoid NaNs from zero-length quaternions; fall back to identity.
                if q.length_squared() > 0.0 {
                    q = q.normalize();
                } else {
                    q = Quat::IDENTITY;
                }
                *value = q;
            }
            changed
        }
    }

    impl ImGuiValue for Mat4 {
        fn imgui_value(ui: &imgui::Ui, label: &str, value: &mut Self) -> bool {
            // Render the 4x4 matrix as four rows of input_float4 widgets.
            // This is primarily intended for debugging/inspection.
            let mut cols = value.to_cols_array();
            let mut changed = false;

            // ImGui uses row-major visual layout; convert 16-element column-major
            // storage into four row slices for editing.
            for row in 0..4 {
                let mut row_vals = [
                    cols[0 * 4 + row],
                    cols[1 * 4 + row],
                    cols[2 * 4 + row],
                    cols[3 * 4 + row],
                ];
                let row_label = format!("{label} [{row}]");
                let row_changed = ui.input_float4(&row_label, &mut row_vals).build();
                if row_changed {
                    cols[0 * 4 + row] = row_vals[0];
                    cols[1 * 4 + row] = row_vals[1];
                    cols[2 * 4 + row] = row_vals[2];
                    cols[3 * 4 + row] = row_vals[3];
                }
                changed |= row_changed;
            }

            if changed {
                *value = Mat4::from_cols_array(&cols);
            }
            changed
        }
    }
}

/// ImGui editors for `mint` vector types when the `mint` feature is enabled.
#[cfg(feature = "mint")]
mod mint_impls {
    use super::{ImGuiValue, imgui};
    use mint::{Vector2, Vector3, Vector4};

    impl ImGuiValue for Vector2<f32> {
        fn imgui_value(ui: &imgui::Ui, label: &str, value: &mut Self) -> bool {
            let mut arr = [value.x, value.y];
            let changed = ui.input_float2(label, &mut arr).build();
            if changed {
                value.x = arr[0];
                value.y = arr[1];
            }
            changed
        }
    }

    impl ImGuiValue for Vector3<f32> {
        fn imgui_value(ui: &imgui::Ui, label: &str, value: &mut Self) -> bool {
            let mut arr = [value.x, value.y, value.z];
            let changed = ui.input_float3(label, &mut arr).build();
            if changed {
                value.x = arr[0];
                value.y = arr[1];
                value.z = arr[2];
            }
            changed
        }
    }

    impl ImGuiValue for Vector4<f32> {
        fn imgui_value(ui: &imgui::Ui, label: &str, value: &mut Self) -> bool {
            let mut arr = [value.x, value.y, value.z, value.w];
            let changed = ui.input_float4(label, &mut arr).build();
            if changed {
                value.x = arr[0];
                value.y = arr[1];
                value.z = arr[2];
                value.w = arr[3];
            }
            changed
        }
    }
}

// Re-export the derive macro when the "derive" feature is enabled so users can
// simply depend on `dear-imgui-reflect` and write `#[derive(ImGuiReflect)]`.
#[cfg(feature = "derive")]
pub use dear_imgui_reflect_derive::ImGuiReflect;
