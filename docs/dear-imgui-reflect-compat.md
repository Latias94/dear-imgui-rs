dear-imgui-reflect vs ImReflect Compatibility Checklist
======================================================

This document tracks how closely `dear-imgui-reflect` matches the behavior and
features of the C++ `ImReflect` library. It is meant as a living checklist to
guide incremental alignment.

Legend:

- `[x]` Fully matched: ImReflect supports this capability and
  `dear-imgui-reflect` matches its behavior/logic for the scope described by
  the bullet (after explicit code/demo comparison).
- `[~]` Implemented but not fully audited against ImReflect; expected to be
  close, but there may be subtle differences or missing edge cases.
- `[ ]` Not yet implemented or intentionally different.

Notes column can indicate partial coverage or subtle differences.

---

Core Architecture
-----------------

- [~] Reflection-based per-type UI generation (ImGuiInput-style)
  - `dear-imgui-reflect::ImGuiValue` and `ImGuiReflect` derive correspond to
    `ImInput`'s per-type customization points: `ImGuiValue` is the low-level
    "how to edit this type" hook, and `ImGuiReflect` derive generates struct/
    enum UIs by walking fields and dispatching to `ImGuiValue`. Compared to
    ImReflect, this keeps the customization surface focused on Rust traits and
    derive macros instead of a more general reflected type graph; behavior is
    equivalent for the covered Rust types, but does not attempt to model the
    full generality of ImReflect's type system.
- [~] Type- and member-level settings object (ImSettings)
  - ImReflect uses `ImSettings` with `push<T>()` / `push_member<&T::field>()`.
  - `dear-imgui-reflect` now includes a global `ReflectSettings` with
    type-level defaults for containers (`Vec<T>` insertable/removable/
    reorderable, map/tuple dropdown/line vs grid) and for primitive numerics
    (`NumericTypeSettings` controlling default widget/range/format/flags for
    `i32`, `u32`, `f32`, `f64`), plus per-member overrides via
    `ReflectSettings::for_member::<T>("field") -> MemberSettings`. Member
    settings can adjust bool style, numeric defaults, tuple/map/vec/array
    settings, and a `read_only` flag. Tuple members additionally support
    per-element overrides using paths like `"tuple_field[0]"`. Compared to
    ImReflect's `ImSettings`, this currently models a single global settings
    object (no full push/pop stack API) but covers the core type-level and
    member-level customization concepts for the supported Rust types. For
    scoped overrides, `dear-imgui-reflect` provides `with_settings_scope`,
    which snapshots the current global `ReflectSettings`, runs a closure, and
    restores the previous snapshot afterwards.

Primitive Numerics (Scalars)
----------------------------

Widget selection and basic behavior:

- [x] Default numeric input widgets
  - Integers and floats use `input_int` / `input_float` / `input_double` or
    `input_scalar` via `ImGuiValue` implementations.
  - When no per-field numeric attributes are present, types `i32`, `u32`,
    `f32`, and `f64` consult type-level numeric settings in
    `ReflectSettings` (`NumericTypeSettings`) to choose between input, drag,
    or slider widgets and to configure range/format/flags. These defaults are
    only applied when the field has no explicit numeric attributes; field
    attributes always take precedence. This mirrors ImReflect's use of type-
    level numeric settings (`ImSettings::type_settings<T>()`) to select the
    default widget and flags when no per-field overrides are present (exact
    default ranges/step values may differ; see type-level settings notes).
- [x] `as_input` selector
  - Attribute: `#[imgui(as_input)]` forces `input_scalar` for the field,
    regardless of the default `ImGuiValue` implementation.
  - Compatible with `step`, `step_fast`, and `format`. Other widget selectors
    (`as_drag`, `slider`, `slider_default_range`) on the same field are
    rejected at compile time. When active, behavior matches ImReflect's
    `as_input()` numeric mode using `InputScalar`.
- [x] `as_drag` selector
  - Attribute: `#[imgui(as_drag)]` forces drag widgets via
    `Ui::drag_config(label)`.
  - Compatible with `speed`, optional `min`/`max` range, `format`, and slider
    flags. Conflicting selectors (`as_input`, `slider`, `slider_default_range`)
    are rejected at compile time. This corresponds to ImReflect's `as_drag()`
    mode using `DragScalar`.
- [x] `slider` selector and explicit `min`/`max`
  - Attributes: `#[imgui(slider, min = ..., max = ...)]` or `min/max` alone
    select slider widgets via `Ui::slider_config(label, min, max)`. For
    sliders, a range is mandatory and validated at compile time (either
    explicit `min`/`max` or `slider_default_range`), mirroring ImReflect's
    `as_slider()` with `min_max<T>` range.

Step and speed configuration:

- [x] Input step configuration
  - Attributes: `step = expr`, `step_fast = expr` (numeric fields).
  - Effective when using input-style widgets (`as_input` or implied); using
    these together with slider/drag selectors is rejected at compile time.
  - When specified, values are forwarded to `InputScalar::step` /
    `step_fast`, matching ImReflect `input_step<T>::step` / `step_fast`.
    (Default step values when attributes are omitted currently differ:
    ImReflect uses `1` / `10` while `dear-imgui-reflect` leaves steps
    disabled unless explicitly configured.)
- [x] Drag speed configuration
  - Attribute: `speed = expr` (numeric fields).
  - Effective when using drag-style widgets (`as_drag` or implied); using
    this together with input/slider-only selectors is rejected at compile time.
  - When specified, the value is forwarded to `Drag::speed(...)` and passed
    as the `v_speed` argument to `DragScalar`, matching ImReflect
    `drag_speed<T>::speed`. The default drag speed when the attribute is
    omitted is `1.0` in both libraries.

Range and clamping:

- [x] Explicit min/max range for sliders and drags
  - Attributes: `min = expr`, `max = expr` (both required when used).
  - For sliders, a numeric range is mandatory and enforced at compile time:
    either explicit `min`/`max` or `slider_default_range`. This mirrors
    ImReflect's use of `min_max<T>` to always provide a range (explicit or
    default half-range) for `SliderScalar`.
  - For drags, the range is optional: when both `min` and `max` are present
    we call `Drag::range(min, max)`, otherwise Dear ImGui's default
    unbounded drag behavior is used, matching ImReflect's `DragScalar`
    invocation with `min`/`max` taken from `min_max<T>`.
- [x] Default numeric range for sliders (no explicit min/max)
  - Attribute: `slider_default_range` selects a slider with a default "half-range"
    based on the numeric type:
    - Signed integers: `T::MIN / 2 ..= T::MAX / 2`
    - Unsigned integers: `0 ..= T::MAX / 2`
    - Floating-point: `T::MIN / 2.0 ..= T::MAX / 2.0`
  - Mirrors ImReflect's behavior when using sliders without custom `min`/`max`
    on numeric types.
- [x] Clamp value to range
  - Attribute: `clamp` performs a manual, post-widget clamp to `[min, max]` for
    slider widgets (using either explicit min/max or the default half-range when
    `slider_default_range` is set).
  - Attribute: `always_clamp` controls `SliderFlags::ALWAYS_CLAMP` and affects
    how ImGui treats CTRL+Click/text input.

Slider/Drag flags (ImGuiSliderFlags)
------------------------------------

Flags apply to both sliders and drags (where meaningful). All flags are
validated to be used only on numeric fields and with slider/drag widgets.

- [x] Logarithmic scale
  - Attribute: `log` → `SliderFlags::LOGARITHMIC`. Applied to both sliders and
    drags and forwarded directly to Dear ImGui, matching ImReflect
    `slider_flags<T>::logarithmic()`.
- [x] Wrap-around behavior
  - Attribute: `wrap_around` → `SliderFlags::WRAP_AROUND`. Applied to sliders
    and drags, mirroring ImReflect `slider_flags<T>::wrap_around()`.
- [x] Always clamp
  - Attribute: `always_clamp` → `SliderFlags::ALWAYS_CLAMP`. This controls
    ImGui's `CTRL+Click`/text input clamping behavior for sliders and drags and
    corresponds to ImReflect `slider_flags<T>::always_clamp()`.
- [x] No round-to-format
  - Attribute: `no_round_to_format` → `SliderFlags::NO_ROUND_TO_FORMAT`.
    Mapped directly for sliders and drags, matching ImReflect
    `slider_flags<T>::no_round_to_format()`.
- [x] No direct text input
  - Attribute: `no_input` → `SliderFlags::NO_INPUT`. Disables direct text
    input, mirroring ImReflect `slider_flags<T>::no_input()`.
- [x] Clamp-on-input
  - Attribute: `clamp_on_input` → `SliderFlags::CLAMP_ON_INPUT`. Forwarded to
    Dear ImGui for sliders and drags, matching ImReflect
    `slider_flags<T>::clamp_on_input()`.
- [x] Clamp-zero-range
  - Attribute: `clamp_zero_range` → `SliderFlags::CLAMP_ZERO_RANGE`. Prevents
    zero-sized ranges for sliders and drags, corresponding to ImReflect
    `slider_flags<T>::clamp_zero_range()`.
- [x] No speed tweaks
  - Attribute: `no_speed_tweaks` → `SliderFlags::NO_SPEED_TWEAKS`. Disables
    ImGui's automatic speed adjustments, mirroring ImReflect
    `slider_flags<T>::no_speed_tweaks()`.

Formatting and display
----------------------

- [x] Basic format string and helpers for numeric widgets
  - Attribute: `format = "..."` on numeric fields.
  - Applied to sliders and drags via `display_format`, and to input scalars
    via `display_format`.
  - Additional helpers on numeric fields:
    - `hex` on integral types uses a hexadecimal format (e.g. `%#x`).
    - `percentage` on floating-point types uses a percentage format
      (e.g. `\"%.2f%%\"`); values are not scaled.
    - `scientific` on floating-point types uses scientific notation (e.g. `%e`).
    - `prefix = \"...\"` / `suffix = \"...\"` wrap the core format string with
      a prefix/suffix. These are compiled into a single printf-style format
      string at derive-time, similar to ImReflect's `format_settings<T>` prefix
      and suffix helpers.
- [~] Advanced format settings (prefix/suffix, as_hex, as_percentage, etc.)
  - `dear-imgui-reflect` supports `hex`, `percentage`, `scientific`, and
    `prefix`/`suffix` attributes on numeric fields and composes them into a
    single printf-style format string at derive-time. This covers the most
    common ImReflect `format_settings<T>` use cases.
  - More advanced formatting controls from ImReflect (e.g. width/padding,
    alignment) are not modeled yet.

Booleans
--------

- [x] Default checkbox style
  - `bool` fields without attributes use `Ui::checkbox`, matching ImReflect's
    default `Checkbox` widget for bools.
- [x] Button style
  - Attribute: `#[imgui(bool_style = "button")]`.
  - Optional `true_text` / `false_text`, defaulting to `"On"` / `"Off"`.
    The button toggles the value on click and displays the current state text,
    mirroring ImReflect's `as_button()` mode (layout differs slightly: we
    include the field label in the button text instead of rendering it
    separately).
- [x] Radio style
  - Attribute: `#[imgui(bool_style = "radio")]`.
  - Two radio buttons rendered with `true_text` / `false_text` labels that
    select `false` or `true`, corresponding to ImReflect's `as_radio()`
    behavior (we omit the extra text label ImReflect renders after the
    buttons).
- [x] Dropdown style
  - Attribute: `#[imgui(bool_style = "dropdown")]`.
  - Renders a two-item combo box using `combo_simple_string`, with items
    `[false_text, true_text]`, matching ImReflect's dropdown mode for bools.
- [x] True/false text customization
  - Attributes: `true_text = "..."`, `false_text = "..."` override the labels
    used in button/radio/dropdown styles, analogous to ImReflect's
    `true_false_text<T>` settings.
- [x] Type-level default bool style
  - ImReflect can configure default bool style via `ImSettings`.
  - `dear-imgui-reflect` exposes a global `ReflectSettings::bools()` which
    controls the default widget style (`Checkbox`/`Button`/`Radio`/`Dropdown`)
    for `bool` fields when no per-field attributes are present; attributes
    (`bool_style`, `true_text`, `false_text`) still override these type-level
    defaults.

Enums
-----

- [x] C-like enums as dropdown (combo)
  - `#[derive(ImGuiReflect)]` on enums without payloads.
  - Default style uses `combo_simple_string` with one item per variant. The
    current value is mapped to an index via `core::mem::discriminant`, and
    selecting a new index maps back to the corresponding variant. This
    mirrors ImReflect's default enum dropdown behavior. Variant labels can be
    overridden with `#[imgui(name = "...")]`.
- [x] Enum radio style
  - Type attribute: `#[imgui(enum_style = "radio")]`.
  - Renders one radio button per variant, using `radio_button_bool` with a
    boolean `active` state per button and mapping the chosen index back to
    the enum variant. This corresponds to ImReflect's `as_radio()` mode for
    enums (we do not currently support enum sliders or drags).
- [x] Enums with payloads (sum types)
  - `#[derive(ImGuiReflect)]` supports enums with unit, tuple, or struct-like
    payload variants.
  - Variant switching constructs the new payload via `Default`, so payload
    field types must implement `Default` to allow switching (mirrors the
    "default construct on switch" behavior in ImReflect, adapted to Rust).

Text (Strings and ImString)
---------------------------

Single-line text:

- [x] Basic editable text
  - Default behavior uses `ui.input_text(label, &mut String)` or
    `ui.input_text_imstr(label, &mut ImString)` when no text-specific
    attributes are present, matching ImReflect's default single-line string
    editing via `InputText`.
- [x] Hint text
  - Attribute: `hint = "..."` applies to single-line inputs and is forwarded
    to the underlying builder as a `String` (`builder.hint(String::from(...))`).
    Hints are rejected at compile time when combined with `multiline`, which
    is consistent with Dear ImGui's API and ImReflect's focus on single-line
    hint usage.
- [x] Read-only mode
  - Attribute: `read_only` sets the input to read-only via the builder
    (`builder.read_only(true)`) and, more generally, all fields (including
    strings) also honor field-level `#[imgui(read_only)]` and
    `MemberSettings::read_only` by wrapping the UI in `ui.begin_disabled()`.
    This achieves the same "const string" editing semantics as ImReflect's
    read-only string handling (layout may differ slightly; we do not yet
    implement a dedicated `TextWrapped`+tooltip mode for const strings).

Multi-line text:

- [x] Multiline editing
  - Attribute: `multiline` chooses `input_text_multiline` or
    `input_text_multiline_imstr`.
- [x] Fixed line count (height)
  - Attribute: `lines = N` with `multiline`.
  - Height computed as `ui.text_line_height_with_spacing() * (N as f32)`.
  - Width is controlled via ImGui item width (`size.x = 0.0`).
- [x] Auto-resize height based on content
  - Attribute: `auto_resize` with `multiline`.
  - Counts `'\n'` in the current value and uses the number of lines to compute
    height.
- [x] Minimum width / auto width
  - ImReflect's `std::string` handling does not expose explicit min-width or
    auto-width settings for text inputs. `dear-imgui-reflect` provides
    additional convenience attributes: `min_width = expr` uses
    `ui.push_item_width(expr as f32)` and `auto_resize` on single-line text
    uses `ui.push_item_width_text(&self.field)` to match the current content
    width. These are extra capabilities, not present in ImReflect.
- [x] Const string display mode
  - ImReflect uses a special display mode for `const std::string` (disabled
    `TextWrapped` plus hover tooltip).
  - `dear-imgui-reflect` provides a similar display-only mode for string
    fields via `#[imgui(display_only)]` on `String`/`ImString` fields. These
    fields are rendered using `text_wrapped` with the label and value
    combined, and a hover tooltip shows the full string content when hovered.
    Unlike simple `read_only`, this avoids drawing an input box entirely and
    more closely matches ImReflect's const-string display intent.

Containers and Option Types
---------------------------

- [x] `Option<T>`
  - Rendered as a checkbox (present/absent) plus a nested editor when enabled.
  - When toggled from `None` to `Some`, a `T::default()` is allocated and then
    edited via `ImGuiValue` for `T`, mirroring ImReflect's `std::optional<T>`
    behavior of default-constructing `T` when the optional becomes engaged and
    resetting it when disengaged. Layout differs slightly (we indent the nested
    value instead of showing `<nullopt>` text for the empty case).
- [x] `Vec<T>`
  - Rendered inside a tree node with a header showing the current length
    (`label [len]`). Each element is edited as `label[index]` using
    `ImGuiValue` for `T`, and requires `T: Default`, similar to ImReflect's
    reliance on default-constructible types for insertion.
  - A `+` button appends `T::default()` at the end and a `-` button removes
    the last element, mirroring ImReflect's generic container insertion and
    removal behavior for `std::vector<T>`.
  - Elements can be reordered via a small drag handle per row, using ImGui's
    drag-and-drop API to move items within the vector, analogous to ImReflect's
    `reorderable_mixin` support and `container_input` drag-to-reorder UI. A
    type-level `VecSettings` controls global defaults (insertable/removable/
    reorderable/dropdown).
- [x] Fixed-size arrays `[f32; 2/3/4]`, `[i32; 2/3/4]`
  - Rendered similarly to small containers: a header `label [N]` plus one
    scalar control per element (`label[index]` using `ImGuiValue` for the
    element type). In contrast to ImReflect's `std::array`, insertion/removal
    are disabled, but elements can be reordered via drag handles when enabled.
    Global `ArraySettings` control whether arrays are wrapped in a dropdown and
    whether reordering is allowed.
- [x] Smart pointers (`Box<T>`)
  - `Box<T>` where `T: ImGuiReflect` is treated transparently by implementing
    `ImGuiReflect` for `Box<T>` and delegating to the inner value. This
    corresponds to ImReflect's smart-pointer handling for engaged pointers.
    Other smart pointers (`Rc<T>`, `Arc<T>`) are partially supported by
    forwarding editing only when there is a unique strong reference (otherwise
    a read-only marker is shown), which is conceptually similar to ImReflect's
    handling of shared pointers but constrained by Rust's aliasing rules.
- [~] Tuples `(T, U, ...)`
  - `dear-imgui-reflect` provides `ImGuiValue` implementations for Rust tuples
    up to arity 8: `(A, B)`, `(A, B, C)`, `(A, B, C, D)`, …,
    `(A, B, C, D, E, F, G, H)` where each element implements `ImGuiValue`.
    Tuples can be rendered either on a single line with the outer label
    followed by one widget per element (line mode), or inside an ImGui table
    (grid mode) controlled by a type-level `TupleSettings`
    (dropdown/line vs grid/columns/same_line/min_width), roughly
    corresponding to ImReflect's tuple/`std::pair` line+dropdown/grid modes.
  - Struct fields whose type is a tuple of any length (including arities
    greater than 8) are rendered via a shared helper (`imgui_tuple_body`)
    that applies `TupleSettings` layered as:
    global `ReflectSettings::tuples()`
    → optional `MemberSettings::tuples` (via `for_member::<T>("field")`)
    → optional field attributes (`tuple_render`, `tuple_dropdown`,
    `tuple_columns`, `tuple_min_width`). For tuples with more than 8
    elements, the struct field path is supported even if there is no direct
    `ImGuiValue` for the tuple type itself.
  - Tuple elements support per-element read-only semantics via member paths
    like `"tuple_field[0]"`, which toggle `MemberSettings::read_only` for
    that element only. Per-element numeric customization for primitive
    scalars is also supported: paths like `"tuple_field[2]"` combined with
    `MemberSettings::numerics_i32` / `numerics_u32` / `numerics_f32` /
    `numerics_f64` apply `NumericTypeSettings` to that element only, reusing
    the same widget/range/format/flags model as struct fields. This roughly
    corresponds to ImReflect's ability to configure tuple elements via
    `ImSettings::push_member<&T::field>()` with nested numeric settings. The
    current design focuses on positional tuple elements and does not attempt
    to expose additional tuple metadata (such as per-element labels) beyond
    what ImReflect offers for small `std::pair`/tuple types.
- [x] Maps and associative containers
  - ImReflect includes rich support for `std::map`, `std::unordered_map`, etc.
  - `dear-imgui-reflect` currently supports string-keyed maps:
    `HashMap<String, V, S>` and `BTreeMap<String, V>` where `V: ImGuiValue +
    Default + Clone + 'static`. Map entries are shown as `label [len]` with an
    optional dropdown, controlled by a type-level `MapSettings`
    (insertable/removable/dropdown/use_table/columns) inside `ReflectSettings`,
    plus per-member overrides via `MemberSettings::maps` (accessible through
    `ReflectSettings::for_member::<T>("field")`) for const-map and
    field-specific behavior. Convenience helpers such as
    `MemberSettings::maps_const()` mirror ImReflect-style "const map"
    configurations where insertion/removal are disabled but values remain
    editable.
  - A `+` button opens a small popup where a new key can be entered. The key
    field is pre-filled with a suggested unique name (`"{label}_{idx}"`), and
    the value is edited inline using `ImGuiValue` for `V`. The popup also
    includes convenience buttons to copy the value from any existing entry via
    `Clone`. Confirming inserts a new entry with the typed key and the
    current popup value. Each entry then renders key and value on a single
    line (tuple-style): the key is an editable `String` field and the value
    uses `ImGuiValue` for `V`. Right-clicking the row handle opens a context
    menu with "Remove item" (removes that entry) and "Clear all" (clears the
    entire map) when removal is enabled. When insertion/removal are disabled
    via `MapSettings`, disabled `+` buttons and disabled menu items are still
    rendered with explanatory tooltips, mirroring ImReflect's feedback for
    non-insertable/non-removable containers. Renaming the key performs a
    remove+insert as long as the new key does not collide with an existing
    entry. Overall behavior is still a subset of ImReflect's richer
    `std::map` handling (no key-type generics), but adds some extra ergonomics
    for string-keyed maps compared to the C++ library.

Math Types (glam/mint)
----------------------
 
- [x] `glam::Vec2`, `Vec3`, `Vec4` (feature `glam`)
  - Implemented as `ImGuiValue` using `input_float2/3/4` and simple
    to_array/from_array conversions, matching the typical ImGui integration
    pattern for math vector types. These types can be used directly in
    reflected structs (e.g. `GlamDemo` in `examples/reflect_demo.rs`) and in
    tests (`glam_support.rs`) under the `glam` feature.
- [x] `mint::Vector2/3/4<f32>` (feature `mint`)
  - Implemented as `ImGuiValue` using `input_float2/3/4` by reading/writing
    the `x/y/z/w` fields, allowing mint vector types to be edited in the same
    way as glam vectors within reflected types when the `mint` feature is
    enabled.
- [x] `glam::Quat` and `glam::Mat4` (feature `glam`)
  - `Quat` is edited as `(x, y, z, w)` via `input_float4`, then normalized on
    write-back (falling back to identity on zero-length input).
  - `Mat4` is edited as four `input_float4` rows (debug/inspection oriented).

Examples and Tests
------------------
 
- [x] Primitive numeric parity demo
  - `examples/reflect_demo.rs` demonstrates:
    - Input vs slider vs drag.
    - Step/step_fast/speed.
    - Log/clamp/wrap_around flags.
    - Type-level numeric defaults for `i32`/`f32` via `ReflectSettings`.
- [x] Bool styles demo
  - Checkbox, button, radio, dropdown with custom true/false labels.
- [x] Text and containers demo
  - Single-line with hints/min_width.
  - Multiline with read-only and auto-resize height.
  - Display-only status text rendered without an input box using
    `#[imgui(display_only)]`.
  - `Option<T>`, `Vec<T>`, fixed-size arrays.
- [x] Maps, tuples, pointers, and type-level defaults
  - `examples/reflect_demo.rs` "Maps & Tuples" section:
    - String-keyed `HashMap<String, i32>` / `BTreeMap<String, f32>` with
      inline editing, add-entry popup, and per-row context menu.
    - Small tuples `(i32, f32)`, `(bool, i32, f32)`, `(i32, i32, i32, i32)`
      rendered in grid mode, driven by global `TupleSettings`.
  - "Pointers" section:
    - `Box<T>`, `Rc<T>`, and `Arc<T>` wrappers around `PrimitivesDemo`.
  - "Type-level Numerics" section:
    - A small struct whose `i32`/`f32` fields have no per-field attributes and
      are rendered purely according to type-level `NumericTypeSettings`.
- [x] glam vectors demo
  - `Vec2`, `Vec3`, `Vec4` reflection.
- [x] Basic no-panic tests
  - `extensions/dear-imgui-reflect/tests/basic_reflect.rs`:
    - Struct/enum derivation, numeric attributes, text attributes, containers.
  - `extensions/dear-imgui-reflect/tests/glam_support.rs`:
    - glam vector reflection under `glam` feature.
- [x] Complex container graph and nested scenarios
  - ImReflect includes deep tests such as `complex_object_test` and nested
    deques/tuples/maps.
  - `dear-imgui-reflect` exercises similar patterns via nested `Option<T>`,
    `Vec<T>`, string-keyed maps, and tuples, including combinations such as
    `Option<Box<GameSettings>>`, `Vec<GameSettings>`, `HashMap<String,
    Vec<i32>>`, `Vec<HashMap<String, f32>>`, and tuple mixes
    `(BTreeMap<String, f32>, Vec<i32>)` in
    `ComplexContainerDemo` (`extensions/dear-imgui-reflect/tests/basic_reflect.rs`).
    While the exact container shapes differ from ImReflect's demo, the test
    validates that the derive macro and container editors handle non-trivial
    nested graphs without panics or obvious behavioral issues.

---

Planned Next Steps (High-Level)
------------------------------

This section is intentionally informal and can be adjusted as the project
evolves. It only lists items that are **not** yet covered by the checklist
above (or are still intentionally partial):

- Strongly-typed member-level settings API (ImSettings-style
  `push_member<&T::field>()`):
  - Today member overrides are keyed by `(TypeId, String)` via
    `ReflectSettings::for_member::<T>("field")`, with tuple elements using
    paths like `"field[0]"`. This mirrors ImReflect's behavior but lacks
    compile-time checking when fields are renamed.
  - A future API could add pointer-based member selection to regain stronger
    typing while keeping the current string-based paths available for
    macros and dynamic configuration.
- Additional formatting controls for numerics and text:
  - Width/padding/alignment knobs similar to ImReflect's richer
    `format_settings<T>` surface.
- Generic-key map editors:
  - Support for key types beyond `String` where feasible, while preserving
    good UX and safety.
- Extended math support:
  - Matrices, quaternions, and additional `glam` / `mint` types.
- Additional "complex object" demos and stress tests:
  - A demo even closer to ImReflect's `complex_object_test`, exercising deep
    nesting, maps, optional fields, tuples, and shared-pointer graphs beyond
    what `ComplexContainerDemo` covers today.
