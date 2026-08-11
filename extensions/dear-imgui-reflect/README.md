# dear-imgui-reflect

[![Crates.io](https://img.shields.io/crates/v/dear-imgui-reflect.svg)](https://crates.io/crates/dear-imgui-reflect)
[![Documentation](https://docs.rs/dear-imgui-reflect/badge.svg)](https://docs.rs/dear-imgui-reflect)

Reflection-driven Dear ImGui editors for Rust structs and enums, inspired by
the C++ [ImReflect](https://github.com/Sven-vh/ImReflect) library.

The public model has five parts:

- `ReflectSession` owns persistent settings and map-popup drafts for one UI owner.
- `Inspector` owns response and path state for one render pass.
- `ImGuiReflectExt` starts each pass through `ui.inspector(&session)`.
- `ImGuiValue` renders one value through an explicit `&mut Inspector`.
- `ImGuiReflect` walks a struct or enum; the default `derive` feature generates it.

There is no global or thread-local reflection state. Keep a session beside the
Dear ImGui context or panel whose settings and drafts it should retain.

## Cargo Integration

`0.16.0-alpha.3` is not published yet. Test the candidate from `main`:

```toml
[dependencies]
dear-imgui-rs = { git = "https://github.com/Latias94/dear-imgui-rs", branch = "main" }
dear-imgui-reflect = { git = "https://github.com/Latias94/dear-imgui-rs", branch = "main" }
```

After publication, use the exact prerelease requirements:

```toml
[dependencies]
dear-imgui-rs = "=0.16.0-alpha.3"
dear-imgui-reflect = "=0.16.0-alpha.3"
```

Optional math support:

```toml
dear-imgui-reflect = { version = "=0.16.0-alpha.3", features = ["glam", "mint"] }
glam = "0.32"
mint = "0.5"
```

## Basic Usage

```rust
use dear_imgui_reflect as reflect;
use reflect::{ImGuiReflect, ImGuiReflectExt};

#[derive(ImGuiReflect, Default)]
struct GameSettings {
    #[imgui(name = "Volume", slider, min = 0, max = 100)]
    volume: i32,
    sensitivity: f32,
    fullscreen: bool,
}

struct Editor {
    reflect: reflect::ReflectSession,
    settings: GameSettings,
}

impl Editor {
    fn frame(&mut self, ui: &reflect::imgui::Ui) {
        let mut inspector = ui.inspector(&self.reflect);
        ui.window("Settings").build(|| {
            inspector.input("Game Settings", &mut self.settings);
        });

        for event in inspector.response().events() {
            eprintln!("structural edit: {event:?}");
        }
    }
}
```

`Inspector::input` returns `true` when a field changes. Container insert,
remove, clear, reorder, and map rename operations are also recorded in the
inspector's `ReflectResponse`.

### Migrating from 0.15

Global reflection settings and the no-session `ui.input_reflect(...)` entry
were removed. Own a `ReflectSession`, import `ImGuiReflectExt`, and create the
one-frame inspector from the `Ui` instead:

```rust
use dear_imgui_reflect::ImGuiReflectExt;

let mut inspector = ui.inspector(&reflect_session);
inspector.input("Game Settings", &mut settings);
```

The equivalent `reflect_session.inspector(ui)` form remains available, but the
`Ui` extension is the canonical per-frame entry point.

## Session Settings

Configure a session before creating an inspector:

```rust
use dear_imgui_reflect as reflect;

fn configure(
    session: &mut reflect::ReflectSession,
) -> Result<(), reflect::imgui::NumericFormatError> {
    let settings = session.settings_mut();
    *settings.vec_mut() = reflect::VecSettings::reorder_only();
    *settings.maps_mut() = reflect::MapSettings::const_map();
    *settings.numerics_f32_mut() = reflect::F32NumericSettings::default()
        .try_slider_0_to_1(3)?;
    Ok(())
}
```

Use separate sessions when panels need different defaults. An inspector
borrows its session, so settings cannot be changed during a render pass.

Per-member settings are keyed by reflected type and generated member path:

```rust
# use dear_imgui_reflect as reflect;
# #[derive(reflect::ImGuiReflect)]
# struct Material { color: (f32, f32, f32, f32), layers: Vec<i32> }
fn configure_material(
    session: &mut reflect::ReflectSession,
) -> Result<(), reflect::imgui::NumericFormatError> {
    let settings = session.settings_mut();
    settings
        .for_member::<Material>("layers")
        .vec_reorder_only();
    settings
        .for_member::<Material>("color[0]")
        .try_numerics_f32_slider_0_to_1(3)?;
    settings
        .for_member::<Material>("color[3]")
        .read_only = true;
    Ok(())
}
```

## Manual Values

Custom value implementations receive the same inspector as generated code:

```rust
use dear_imgui_reflect::{ImGuiValue, Inspector};

struct Angle(f32);

impl ImGuiValue for Angle {
    fn imgui_value(
        inspector: &mut Inspector<'_, '_>,
        label: &str,
        value: &mut Self,
    ) -> bool {
        inspector.ui().input_float(label, &mut value.0)
    }
}
```

Nested implementations must pass `inspector` onward. Derive-generated field
and container paths use scoped guards whose `Drop` restores the previous path,
including during panic unwind.

## Supported Types

- Primitive scalar and string inputs.
- `Option<T>`, `Vec<T>`, and fixed arrays.
- `HashMap<String, V>` and `BTreeMap<String, V>`.
- Tuples from two through eight elements, plus derive-generated tuple fields.
- `Box<T>`, `Rc<T>`, and `Arc<T>` for reflected values.
- `glam` vectors, quaternions, and matrices with the `glam` feature.
- `mint::Vector2/3/4<f32>` with the `mint` feature.

Map insertion drafts are retained by `ReflectSession`. Their identity includes
the value `TypeId`, the active Dear ImGui ID stack, and the owning
`ImGuiContext`, so equal text labels in different scopes or contexts do not
share drafts. Own the session beside its context; rebuild both together after
destroying and recreating the context.

## Field Attributes

Common derive attributes include:

- `skip`, `name`, and `read_only`.
- `as_input`, `as_drag`, `slider`, `min`, `max`, `step`, and `speed`.
- `format`, unsigned-integer `hex`, floating-point `percentage`, `prefix`, and `suffix`.

Numeric formats are validated against the exact field type. Runtime settings
store `NumericFormat<'static, T>` rather than raw strings; use
`try_with_format` when a format comes from configuration or user input.
Custom derive formats intentionally require fixed-width numeric fields because
one `isize`/`usize` format cannot be correct for every target pointer width.
Wide integer literals using MSVC `%I64*` syntax are normalized before code is
generated, and the runtime `NumericFormat` validator applies the target form.
- `bool_style = "checkbox|button|radio|dropdown"`.
- `multiline`, `lines`, `hint`, `auto_resize`, and `display_only`.
- `tuple_render`, `tuple_dropdown`, `tuple_columns`, and `tuple_min_width`.

See `examples/03-features/reflect_demo.rs` for a complete inspector and
`docs/dear-imgui-reflect-compat.md` for the ImReflect compatibility matrix.

## Compatibility

| Item | Version |
|---|---|
| Crate | 0.16.0-alpha.3 |
| dear-imgui-rs | 0.16.0-alpha.3 |

## License

MIT OR Apache-2.0, matching the workspace.
