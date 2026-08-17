# dear-imgui-cte

Preview safe Rust bindings for ImGuiColorTextEdit through `cimCTE`. The crate
provides context-bound text editors, text diffs, autocomplete, callbacks, and
notifications on native and WebAssembly targets.

## Links

- [Workspace](https://github.com/Latias94/dear-imgui-rs)
- [cimCTE](https://github.com/cimgui/cimCTE)
- [ImGuiColorTextEdit](https://github.com/goossens/ImGuiColorTextEdit)

## Compatibility

This crate is currently an Unreleased Preview in the workspace. Use it from the
same workspace checkout or Git revision as `dear-imgui-rs` and
`dear-imgui-cte-sys`; mixing revisions is unsupported. The checked-in source
identity is recorded in the sys crate README and Cargo metadata.

## Quick Start

Create persistent widget state while the Dear ImGui `Context` is alive, then
submit it through `CteUiExt` on every frame:

```rust,no_run
use dear_imgui_cte::{CteUiExt, Language, TextEditor, dejavu_font_source};
use dear_imgui_rs::Context;

# fn main() -> Result<(), dear_imgui_cte::CteError> {
let mut imgui = Context::create();
let font = dejavu_font_source(16.0)?;
imgui.font_atlas().add_font(&[font]);

let mut editor = TextEditor::try_create(&imgui)?;
editor.set_text("int main() { return 0; }\n")?;
editor.set_language(Some(Language::Cpp));

let ui = imgui.frame();
ui.text_editor(&mut editor, "Source")
    .size([640.0, 480.0])
    .build()?;
# Ok(())
# }
```

Add fonts before renderer initialization. Renderers used with Dear ImGui 1.92's
managed font atlas must consume its texture create/update/destroy requests. The
workspace WGPU, Glow, and Ash renderers implement that contract.

Copy-ready applications are available in the workspace:

```text
cargo run -p dear-imgui-examples --bin cte_minimal --features cte
cargo run -p dear-imgui-examples --bin cte_showcase --features cte
cargo run -p xtask -- web-demo cte
```

## Supported Surface

- `TextEditor` covers document ownership, editing, selection, navigation,
  diagnostics, languages, palettes, undo/redo, search, and render configuration.
- Typed callbacks cover changes, transactions, decorators, carets, popups,
  language changes, identifier iteration, and atomic selection/line filters.
- Custom autocomplete owns its callback and exposes only callback-scoped request
  state. The built-in Trie is attached to and destroyed with its editor.
- `TextDiff` supports integrated and side-by-side rendering with independent
  language, palette, and display configuration.
- `Notifications` owns a timed notification queue rendered through the same
  `Ui` context.
- `dejavu_font_source` exposes the bundled font through the safe managed atlas
  path without clearing the application's other fonts.

See the family-level disposition ledger in
[`docs/API_COVERAGE.md`](https://github.com/Latias94/dear-imgui-rs/blob/main/docs/API_COVERAGE.md#imguicolortextedit-api-family-ledger)
for the small set of raw-only capabilities.

## Safety and Lifecycle

`TextEditor`, `TextDiff`, and `Notifications` are bound to the `Context` used at
creation and are neither `Send` nor `Sync`. Rendering with a `Ui` from another
context is rejected before FFI. Drop temporarily binds the owner context and
restores the prior current context. Applications should destroy CTE state before
destroying its Context; after Context teardown, the wrapper deliberately leaks
the native handle rather than dereferencing dead native state.

Callbacks and their Rust userdata are owned by the editor. Replacing, clearing,
or dropping a callback unregisters the native slot before releasing its Rust
state. A callback panic is diagnosed and aborts the process instead of unwinding
through C++ frames. Reentering the same mutable callback is skipped; filter
callbacks validate a complete batch before native mutation. Trie autocomplete
uses upstream-exclusive change, language-change, and autocomplete slots, so the
safe API reports callback conflicts instead of silently replacing user state.

String getters and palettes return owned Rust copies. Callback event views and
autocomplete requests are valid only for their invocation. The unsafe `as_raw`
methods require callers to preserve every ownership, pointer-lifetime, Context,
and wrapper invariant before returning to safe methods.

## Features

| Feature | Effect |
| --- | --- |
| default | Native source build with checked-in bindings |
| `build-from-source` | Force the core and CTE native source routes |
| `prebuilt` | Allow verified matching prebuilt archives |
| `wasm` | Select fixed `imgui-sys-v1` import bindings for `wasm32-unknown-unknown` |

Source wins when Cargo unifies `build-from-source` and `prebuilt`. WASM requires
the explicit `wasm` feature on the highest safe crate in every dependency path.

## Raw Access

Use `dear-imgui-cte-sys` when an operation cannot be represented safely. In
particular, raw `SetDejavu` clears the complete font atlas and changes its loader
outside renderer texture management; prefer `dejavu_font_source`. Native
glyph/iterator pointer graphs, process-current Context mutation, and raw
line-break configuration also remain sys-only. Raw callbacks must never unwind
through the C ABI.

## License

The Rust wrapper follows the workspace dual MIT/Apache-2.0 license. Packaged
third-party notices and exact revisions are documented by
`dear-imgui-cte-sys/third-party/README.md`.
