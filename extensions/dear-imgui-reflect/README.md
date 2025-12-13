# dear-imgui-reflect

[![Crates.io](https://img.shields.io/crates/v/dear-imgui-reflect.svg)](https://crates.io/crates/dear-imgui-reflect)
[![Documentation](https://docs.rs/dear-imgui-reflect/badge.svg)](https://docs.rs/dear-imgui-reflect)

Reflection-based UI helpers for `dear-imgui-rs`: automatically generate Dear ImGui widgets for your Rust structs and enums, inspired by the C++ [ImReflect](https://github.com/Sven-vh/ImReflect) library.

This crate exposes:

- `ImGuiValue` – per-type editing widgets (low-level hook, similar to `ImInput<T>`).
- `ImGuiReflect` – derive-based struct/enum editor that walks fields and dispatches to `ImGuiValue`.
- `ReflectSettings` / `MemberSettings` – ImSettings-style configuration for numeric widgets, containers, tuples, maps, and more.

It is designed to integrate directly with `dear-imgui-rs` and the rest of this workspace.

## Links

- Core ImGui bindings: <https://crates.io/crates/dear-imgui-rs>
- Examples (native): `examples/reflect_demo.rs`
- ImReflect (C++ reference library): <https://github.com/Sven-vh/ImReflect>

## Compatibility

| Item                | Version |
|---------------------|---------|
| Crate               | 0.7.x   |
| dear-imgui-rs       | 0.7.x   |

The optional `glam` and `mint` features track the same workspace versions as other crates in this repository.

See also: [docs/COMPATIBILITY.md](https://github.com/Latias94/dear-imgui-rs/blob/main/docs/COMPATIBILITY.md) for the full workspace matrix.

## Cargo Integration

Add the crate alongside `dear-imgui-rs`:

```toml
[dependencies]
dear-imgui-rs = "0.7"
dear-imgui-reflect = "0.7"
```

Optional math interop:

```toml
[dependencies]
dear-imgui-rs = "0.7"
dear-imgui-reflect = { version = "0.7", features = ["glam", "mint"] }
glam = "0.29" # or workspace version
mint = "0.5"
```

By default the `derive` feature is enabled so you can use `#[derive(ImGuiReflect)]`.

## Basic Usage

Define a struct and derive `ImGuiReflect`:

```rust
use dear_imgui_reflect as reflect;
use reflect::ImGuiReflect;

#[derive(ImGuiReflect, Default)]
struct GameSettings {
    #[imgui(name = "Volume", slider, min = 0, max = 100)]
    volume: i32,
    sensitivity: f32,
    fullscreen: bool,
}
```

In your Dear ImGui frame, call `reflect::input` or the `Ui` extension:

```rust
use dear_imgui_reflect as reflect;
use reflect::ImGuiReflectExt;
use reflect::imgui::*; // re-export of dear-imgui-rs

fn frame(ui: &Ui, settings: &mut GameSettings) {
    ui.window("Settings").build(|| {
        // Free function (ImReflect-style)
        reflect::input(ui, "Game Settings", settings);

        // Or via Ui extension:
        // ui.input_reflect("Game Settings", settings);
    });
}
```

Enums (C-like, no payload) can also derive `ImGuiReflect` and are rendered as combos or radios:

```rust
use dear_imgui_reflect::ImGuiReflect;

#[derive(ImGuiReflect)]
enum Quality {
    #[imgui(name = "Low (Fast)")]
    Low,
    Medium,
    High,
}

#[derive(ImGuiReflect)]
#[imgui(enum_style = "radio")] // "combo" (default) or "radio"
enum RenderMode {
    Forward,
    Deferred,
    PathTracing,
}
```

## Features Overview

- **Derive-based struct/enum editing**
  - `#[derive(ImGuiReflect)]` for named-field structs and C-like enums.
  - Field attributes for labels, skipping, numeric behavior, text widgets, tuples, and more.
- **Numeric widgets (input / drag / slider)**
  - Per-field attributes: `as_input`, `as_drag`, `slider`, `slider_default_range`.
  - Range and steps: `min`, `max`, `step`, `step_fast`, `speed`.
  - Slider/drag flags: `log`, `wrap_around`, `clamp`, `always_clamp`, `no_input`, `no_round_to_format`, `clamp_on_input`, `clamp_zero_range`, `no_speed_tweaks`.
  - Formatting helpers: `format = "..."`, `hex`, `percentage`, `scientific`, `prefix = "..."`, `suffix = "..."`.
  - Type-level defaults via `ReflectSettings::numerics_*`.
  - Numeric presets via `NumericTypeSettings` helpers such as `with_float`, `with_hex`, `slider_0_to_1`, `slider_minus1_to_1`, `drag_with_speed`, and `percentage_slider_0_to_1`.
- **Booleans**
  - Styles: checkbox (default), button, radio, dropdown.
  - Per-field attributes: `bool_style = "checkbox|button|radio|dropdown"`, `true_text`, `false_text`.
- **Text**
  - Single-line `String` / `ImString` with `hint` and `min_width`.
  - Multiline with `multiline`, optional `lines`, and `auto_resize`.
  - `read_only` for non-editable text.
  - `display_only` for text labels without an input box (layout only).
- **Containers & optionals**
  - `Option<T>` – checkbox toggles presence; nested editor for `Some(T)`.
  - `Vec<T>` – insertable/removable/reorderable with tree-node dropdown.
  - Fixed arrays `[T; N]` (for `T: ImGuiValue`, currently tuned for small N).
  - Maps: `HashMap<String, V, S>` and `BTreeMap<String, V>` with inline key+value editors, add/remove, optional table layout.
- **Tuples**
  - Fixed tuples `(A, B)`, `(A, B, C)`, … up to higher arity (up to 8 elements in the current implementation).
  - Global and per-member layout control: line vs grid, columns, dropdown, min width.
  - Per-element overrides via `MemberSettings` paths like `"tuple_field[0]"`.
- **Pointers & math types**
  - `Box<T>` forwards to `T: ImGuiReflect`.
  - `Rc<T>` / `Arc<T>` editable only when unique; otherwise rendered read-only.
  - Optional `glam` and `mint` support (`Vec2/3/4`, `mint::Vector2/3/4<f32>`) via `input_float2/3/4`.
- **ImSettings-style configuration**
  - Global `ReflectSettings` with helpers:
    - `vec()`, `arrays()`, `maps()`, `tuples()`, `bools()`.
    - `numerics_i32()`, `numerics_f32()`, `numerics_u32()`, `numerics_f64()`.
  - Per-member overrides via `ReflectSettings::for_member::<T>("field_name")`.
  - Snapshot scope helper: `with_settings_scope(|| { ... })`.

## Field Attributes Cheat Sheet

These attributes are placed on struct fields inside a `#[derive(ImGuiReflect)]` type:

```rust
#[derive(ImGuiReflect)]
struct Example {
    // General
    #[imgui(skip)]
    ignored: i32,

    #[imgui(name = "Custom Label")]
    renamed: i32,

    #[imgui(read_only)]
    read_only_value: i32,

    // Numeric widgets
    #[imgui(as_input, step = 1, step_fast = 10)]
    counter: i32,

    #[imgui(as_drag, speed = 0.1, min = 0.0, max = 10.0, log, always_clamp)]
    drag_value: f32,

    #[imgui(slider, min = 0, max = 100, clamp, no_input)]
    slider_value: i32,

    #[imgui(slider, slider_default_range)]
    slider_half_range: f32,

    #[imgui(as_input, hex)]
    hex_display: i32,

    #[imgui(as_drag, percentage, speed = 0.5)]
    percent_display: f32,

    // Bool styles
    #[imgui(bool_style = "button", true_text = "On", false_text = "Off")]
    power: bool,

    #[imgui(bool_style = "dropdown", true_text = "Enabled", false_text = "Disabled")]
    dropdown_toggle: bool,

    // Text
    #[imgui(hint = "Window title", min_width = 200.0)]
    title: String,

    #[imgui(multiline, lines = 3, auto_resize)]
    description: String,

    #[imgui(display_only)]
    status: String,

    // Tuples
    #[imgui(tuple_render = "grid", tuple_columns = 4, tuple_min_width = 80.0)]
    color: (f32, f32, f32, f32),
}
```

## Containers & Maps

### Option, Vec, Arrays

```rust
use dear_imgui_reflect::ImGuiReflect;

#[derive(ImGuiReflect, Default)]
struct Containers {
    extra_gain: Option<f32>,
    samples: Vec<i32>,
    offset: [f32; 3],
}
```

- `Option<T>`: a checkbox toggles `Some(T)` / `None`; when set to `Some`, a default `T::default()` is created and edited inline.
- `Vec<T>`: supports insertion (`+`), removal (`-`), and drag-to-reorder handles by default.
- Arrays `[T; N]` (for supported `T`) use similar layout, with optional reordering.

You can change the default behavior globally:

```rust
use dear_imgui_reflect as reflect;

reflect::with_settings(|s| {
    // All Vec<T> become reorder-only by default
    *s.vec_mut() = reflect::VecSettings::reorder_only();

    // Arrays cannot be reordered
    *s.arrays_mut() = reflect::ArraySettings::fixed_order();
});
```

Or override on a single member using `MemberSettings`:

```rust
use dear_imgui_reflect as reflect;

#[derive(reflect::ImGuiReflect, Default)]
struct ContainerSettingsDemo {
    full_vec: Vec<i32>,
    reorder_only_vec: Vec<i32>,
    fixed_no_reorder: [i32; 3],
}

fn configure_member_settings() {
    reflect::with_settings(|settings| {
        // Make one vector reorder-only
        settings
            .for_member::<ContainerSettingsDemo>("reorder_only_vec")
            .vec_reorder_only();

        // Disable reordering for a single array
        settings
            .for_member::<ContainerSettingsDemo>("fixed_no_reorder")
            .arrays_fixed_order();
    });
}
```

### String-keyed Maps

`dear-imgui-reflect` supports:

- `HashMap<String, V, S>`
- `BTreeMap<String, V>`

Example:

```rust
use dear_imgui_reflect::ImGuiReflect;
use std::collections::{BTreeMap, HashMap};

#[derive(ImGuiReflect, Default)]
struct MapDemo {
    hash: HashMap<String, i32>,
    btree: BTreeMap<String, f32>,
}
```

Runtime behavior:

- The header shows `label [len]`.
- New entries are added via a `+` button, opening a popup to edit both key and value before insertion.
- Existing entries can be renamed by editing the key; the underlying map is updated.
- Right-clicking a row shows `Remove item` (when allowed); right-clicking the header shows `Clear all`.
- Optional table layout via `MapSettings::use_table`.

You can create const-maps (no insert/remove) globally or per member:

```rust
use dear_imgui_reflect as reflect;

#[derive(reflect::ImGuiReflect, Default)]
struct ConstMapDemo {
    items: std::collections::HashMap<String, i32>,
}

fn configure_maps() {
    reflect::with_settings(|settings| {
        // Treat all maps as const-maps by default
        *settings.maps_mut() = reflect::MapSettings::const_map();

        // Or just one field:
        settings
            .for_member::<ConstMapDemo>("items")
            .maps_const();
    });
}
```

## Tuples & Per-Element Settings

Tuples are rendered as small groups of widgets. You can choose line or grid layout and override per-element behavior using `MemberSettings`.

```rust
use dear_imgui_reflect as reflect;
use reflect::ImGuiReflect;

#[derive(ImGuiReflect, Default)]
struct TupleDemo {
    // Defaults to line layout, labels inferred from field name and index
    pair_int_float: (i32, f32),

    // Grid layout with 3 columns
    #[imgui(tuple_render = "grid", tuple_columns = 3)]
    triple_mixed: (bool, i32, f32),

    // Dropdown + line layout
    #[imgui(tuple_render = "line", tuple_dropdown)]
    quad_tuple: (i32, i32, i32, i32),
}
```

Per-element numeric overrides use member paths of the form `"field_name[index]"`:

```rust
use dear_imgui_reflect as reflect;
use reflect::{NumericRange, NumericTypeSettings, NumericWidgetKind};

#[derive(reflect::ImGuiReflect, Default)]
struct ColorConfig {
    #[imgui(name = "Color", tuple_render = "grid", tuple_columns = 4)]
    color: (f32, f32, f32, f32),
}

fn configure_color_tuple() {
    reflect::with_settings(|settings| {
        // color[0]: slider in [0, 1]
        let slider01 = NumericTypeSettings::default().slider_0_to_1(3);
        settings
            .for_member::<ColorConfig>("color[0]")
            .numerics_f32 = Some(slider01.clone());

        // color[3]: read-only copy of [0, 1] slider
        settings
            .for_member::<ColorConfig>("color[3]")
            .numerics_f32 = Some(slider01);
        settings.for_member::<ColorConfig>("color[3]").read_only = true;
    });
}
```

The same configuration can be expressed using `MemberSettings` helpers:

```rust
fn configure_color_tuple_with_helpers() {
    reflect::with_settings(|settings| {
        settings
            .for_member::<ColorConfig>("color[0]")
            .numerics_f32_slider_0_to_1(3);

        settings
            .for_member::<ColorConfig>("color[3]")
            .numerics_f32_slider_0_to_1(3)
            .read_only = true;
    });
}
```

Global tuple defaults live in `ReflectSettings::tuples()`:

```rust
use dear_imgui_reflect as reflect;
use reflect::TupleRenderMode;

reflect::with_settings(|s| {
    let tuples = s.tuples_mut();
    tuples.dropdown = false;
    tuples.render_mode = TupleRenderMode::Grid;
    tuples.columns = 4;
    tuples.same_line = true;
    tuples.min_width = Some(80.0);
});
```

## Numeric Presets (Type-level Helpers)

`NumericTypeSettings` provides small helper methods to quickly configure
common slider/drag patterns without writing out all fields:

```rust
use dear_imgui_reflect as reflect;
use reflect::NumericTypeSettings;

fn configure_numeric_presets() {
    reflect::with_settings(|s| {
        // f32 defaults: slider in [0, 1] with clamping and "%.3f" format
        *s.numerics_f32_mut() = NumericTypeSettings::default()
            .slider_0_to_1(3);

        // f64 defaults: drag widget with explicit speed and "%.4f" format
        *s.numerics_f64_mut() = NumericTypeSettings::default()
            .drag_with_speed(0.01, 4);

        // Per-member override: f32 tuple element as 0..1 percentage slider
        s.for_member::<ColorConfig>("color[1]").numerics_f32 =
            Some(NumericTypeSettings::default().percentage_slider_0_to_1(2));
    });
}
```

These presets are thin convenience wrappers over the `widget`, `range`,
`speed`, `clamp`, and `format` fields and can be mixed with manual
configuration when needed.

## glam / mint Integration

Enable the `glam` and/or `mint` features to get out-of-the-box `ImGuiValue` implementations for common math types:

```toml
[dependencies]
dear-imgui-reflect = { version = "0.7", features = ["glam", "mint"] }
glam = "0.29"
mint = "0.5"
```

```rust
use dear_imgui_reflect as reflect;
use reflect::ImGuiReflect;
use glam::{Vec2, Vec3, Vec4};
use mint::{Vector2, Vector3, Vector4};

#[derive(ImGuiReflect, Default)]
struct Transform {
    position: Vec3,
    scale: Vec3,
    dir: Vector3<f32>,
    uv: Vector2<f32>,
    color: Vector4<f32>,
}
```

These are rendered using `input_float2/3/4` behind the scenes and participate in `ImGuiReflect` just like other supported field types.

## Settings Scope Helper

`ReflectSettings` is stored globally (inside this crate) and can be modified at runtime. For localized overrides, use `with_settings_scope`, which snapshots settings, runs a closure, and then restores the previous state:

```rust
use dear_imgui_reflect as reflect;
use reflect::{NumericRange, NumericTypeSettings, NumericWidgetKind};

fn draw_debug_panel(ui: &reflect::imgui::Ui, state: &mut DebugState) {
    reflect::with_settings_scope(|| {
        // Temporarily change f32 default to drag slider in [0, 1]
        reflect::with_settings(|s| {
            *s.numerics_f32_mut() =
                NumericTypeSettings::default().slider_0_to_1(3);
        });

        ui.window("Debug").build(|| {
            ui.input_reflect("Debug State", state);
        });
    });
    // At this point, global settings are restored.
}
```

## Example Demo

The repository includes a full demo that exercises most features:

- File: `examples/reflect_demo.rs`
- Binary: `reflect_demo` (part of `dear-imgui-examples`)

Run it with:

```bash
cargo run -p dear-imgui-examples --bin reflect_demo --features reflect
```

## ImReflect Compatibility (Overview)

`dear-imgui-reflect` is designed to closely mirror the behavior of the C++ [ImReflect](https://github.com/Sven-vh/ImReflect) library for the covered Rust types.

A detailed checklist is maintained in `docs/dear-imgui-reflect-compat.md`. High-level status:

- **Matched ([x])**
  - Primitive numerics: input/drag/slider selection, `as_input` / `as_drag` / `slider`, `min`/`max`, `step`/`step_fast`, `speed`, log/clamp/wrap_around/flags, default half-range sliders.
  - Bool widgets: checkbox/button/radio/dropdown (including text labels).
  - Text: single-line, multiline, hints, `read_only`, display-only text.
  - Containers: `Option<T>`, `Vec<T>`, small fixed arrays, string-keyed maps (`HashMap<String, V>` / `BTreeMap<String, V>`), including insertable/removable/reorderable and const-map configurations.
  - Tuples: line/grid layouts, dropdown wrapping, global defaults, per-member overrides, and per-element numeric overrides.
  - Pointers: `Box<T>`, `Rc<T>`, `Arc<T>` with unique/editable vs shared/read-only behavior.
  - Math interop: `glam::Vec2/3/4` and `mint::Vector2/3/4<f32>` when features are enabled.
  - Global and member-level settings: ImSettings-style configuration for numerics, containers, tuples, bools, and read-only semantics.
- **Implemented but more general / partially audited ([~])**
  - Core architecture: Rust traits + derive macros instead of a fully generic reflected type graph.
  - Settings model: single global `ReflectSettings` plus `with_settings_scope` instead of a full push/pop stack of `ImSettings`.
- **Intentionally incomplete or TODO ([ ])**
  - Non-string map keys and more exotic STL/container shapes.
  - Additional math types (matrices, quaternions, etc.).
  - Some of ImReflect's richer numeric/text formatting helpers.

When in doubt, refer to `docs/dear-imgui-reflect-compat.md` for the exact status of each feature and behavioral notes.

## Limitations

- Derive macro currently supports:
  - Structs with named fields.
  - C-like enums without payloads.
- Map support is limited to `String` keys (for `HashMap`/`BTreeMap`); other key types need custom `ImGuiValue` + wrappers.
- Settings use a single global `ReflectSettings` instance with manual snapshot helpers; there is no full push/pop stack like ImReflect's `ImSettings`, but common use-cases are covered.
- For very deep or large object graphs, you may want to selectively `#[imgui(skip)]` certain fields or use custom editors for performance.

## License

MIT OR Apache-2.0
