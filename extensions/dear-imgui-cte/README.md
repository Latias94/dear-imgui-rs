# dear-imgui-cte

Preview safe Rust bindings for ImGuiColorTextEdit through `cimCTE`. The crate
provides context-bound text editors, text diffs, autocomplete, and callbacks on
native and WebAssembly targets.

## Links

- [Workspace](https://github.com/Latias94/dear-imgui-rs)
- [cimCTE](https://github.com/cimgui/cimCTE)
- [ImGuiColorTextEdit](https://github.com/goossens/ImGuiColorTextEdit)

## Compatibility

This Preview crate uses the `0.22` release train and requires Rust 1.95 or newer. Use it from the same release train as `dear-imgui-rs` and `dear-imgui-cte-sys`; mixing release trains is unsupported. The checked-in source identity is recorded in the sys crate README and Cargo metadata. See the [native build policy migration](../../docs/COMPATIBILITY.md#0220-native-build-policy).

```toml
[dependencies]
dear-imgui-rs = "0.22"
dear-imgui-cte = "0.22"
```

## Quick Start

Add bundled fonts during initialization, before attaching a managed-texture
renderer. Create persistent widget state while that Dear ImGui `Context` is
alive, then submit it through `CteUiExt` on each renderer-driven frame.

This API fragment assumes the application supplies platform input, display
size/delta time, renderer attachment, and normal frame reconciliation. The
commented integration point represents that setup; use `cte_minimal` below for
a complete runnable application:

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

// Attach/configure your platform and managed renderer here before starting a frame.
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
- Typed callbacks cover changes, transactions, decorators, carets, line numbers, popups,
  language changes, identifier iteration, and atomic selection/line filters.
- Custom autocomplete owns its callback and exposes only callback-scoped request
  state. The built-in Trie is attached to and destroyed with its editor.
- `TextDiff` supports integrated and side-by-side rendering with independent
  language, palette, and display configuration.
- `dejavu_font_source` and `noto_sans_sc_font_source` expose bundled fonts through the safe managed atlas
  path without clearing the application's other fonts.

The default occurrence and word-boundary methods retain upstream defaults.
Use their `_with_whole_word` variants to request complete-word occurrences or
word-only navigation that leaves whitespace and punctuation in place.

See the family-level disposition ledger in
[`docs/API_COVERAGE.md`](https://github.com/Latias94/dear-imgui-rs/blob/main/docs/API_COVERAGE.md#imguicolortextedit-api-family-ledger)
for the small set of raw-only capabilities.

## Migrating from 0.18

The 0.20 source pin includes upstream wide-glyph column handling. Remove the
`cte-wide-glyphs-v1` overlays and rebuild custom native archives and CTE-enabled
WASM providers together with the matching core. Raw `Glyph` mirrors now use
`columns: u8` and `squiggle: u32`; inspect the generated sys bindings before
updating raw callers. Both editor and diff tab sizes must be in `1..=255`.

`TextEditor::text()` now returns exact document content. Remove workarounds that
trimmed the former appended newline; append a newline explicitly when required
by your file format. Use `noto_sans_sc_font_source` for the bundled CJK font and
keep font installation within the managed atlas path. For the complete raw API
migration, see [the compatibility guide](../../docs/COMPATIBILITY.md#020-stable-release).

## Safety and Lifecycle

`TextEditor` and `TextDiff` are bound to the `Context` used at
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

String getters and palettes return owned Rust copies.
`TextEditor::text()` preserves the document's existing trailing newlines; it does
not add upstream `GetText`'s file-serialization newline. Editor and diff tab widths
must be in `1..=255`, matching upstream's per-glyph column storage.

Callback event views and autocomplete requests are valid only for their invocation. The unsafe `as_raw`
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

The current upstream removed its notification helper; use an application-owned
notification UI. `add_squiggle` now takes a `SquiggleStyle`, either `Wave` or
`Background`. `document_to_screen(ui, position)` returns coordinates only after
the editor renders visibly in that frame; document or layout changes invalidate
the cached coordinates. Current-line highlighting and the built-in find/replace
UI can be toggled independently.

## License

The Rust wrapper follows the workspace dual MIT/Apache-2.0 license. Packaged
third-party notices and exact revisions are documented by
`dear-imgui-cte-sys/third-party/README.md`.
