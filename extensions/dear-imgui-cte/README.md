# dear-imgui-cte

Preview safe Rust bindings for ImGuiColorTextEdit through `cimCTE`. The crate
provides context-bound text editors, text diffs, autocomplete, and callbacks on
native and WebAssembly targets.

## Links

- [Workspace](https://github.com/Latias94/dear-imgui-rs)
- [cimCTE](https://github.com/cimgui/cimCTE)
- [ImGuiColorTextEdit](https://github.com/goossens/ImGuiColorTextEdit)

## Compatibility

This Preview crate requires Rust 1.95 or newer. Use `dear-imgui-rs` and `dear-imgui-cte-sys` from the same `0.23` release train. Rebuild custom archives and browser providers with the [pinned CTE sources](../dear-imgui-cte-sys/README.md#compatibility-and-provenance). For build settings carried forward from 0.21, see the [0.22 native build policy migration](../../docs/COMPATIBILITY.md#0220-native-build-policy).

```toml
[dependencies]
dear-imgui-rs = "0.23"
dear-imgui-cte = "0.23"
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
  state. Configure extra trigger and word characters for completions such as
  `\alpha`. A request can supply display labels and choose the glyph range to
  replace. Submit asynchronous results on the editor's render thread. The
  built-in Trie is attached to and destroyed with its editor.
- `TextDiff` supports integrated and side-by-side rendering with independent
  language, palette, and display configuration.
- `dejavu_font_source` and `noto_sans_sc_font_source` expose bundled fonts through the safe managed atlas
  path without clearing the application's other fonts.

The default occurrence and word-boundary methods retain upstream defaults.
Use their `_with_whole_word` variants to request complete-word occurrences or
word-only navigation that leaves whitespace and punctuation in place.

`Language::from_extension` accepts case-sensitive extensions with a leading dot,
including `.jl` (Julia) and `.wgsl` (WGSL). Palettes have 33 slots, including
`CustomColor1` through `CustomColor9`. Use `set_left_margin` for the editor margin
in glyph columns. It also moves the text when line numbers are hidden. Smooth
scrolling is opt-in through `set_smooth_scrolling_enabled`.

For operations that require the raw API, see the [CTE API coverage table](../../docs/API_COVERAGE.md#imguicolortextedit-api-family-ledger).

## Breaking changes

The `cimCTE` revision shipped in 0.23 replaces the removed `Language_*` raw getters with
registry lookup. Safe code should use the `Language` enum, `Language::from_name`,
or `Language::from_extension`. The built-in set now contains Julia and WGSL.
`PaletteColor::COUNT` is 33 and includes `CustomColor1` through `CustomColor9`.
The raw line-number margin functions were replaced by `TextEditor_SetLeftMargin`
and `TextEditor_GetLeftMargin`. The safe wrapper keeps deprecated forwarding
methods while callers migrate to `set_left_margin` and `left_margin`.

Autocomplete configuration now supports extra trigger and word characters.
Callbacks can inspect and change the replacement range and add display labels.
Asynchronous labeled suggestions are copied during the render-thread call, so
the input strings only need to remain valid for that call.

## Migrating from 0.18

Upstream already handled wide-glyph columns in the 0.22 source pin. Remove the
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
context is rejected before FFI. Drop temporarily binds the owner while it is live.
After Context teardown, the editor, its optional connected Trie, and the diff can
still release their native allocations on the owning thread. Drop the last such
owner before creating a Dear ImGui context on a different thread.

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
Rebuild the Rust application and the `imgui-sys-v1` provider together when upgrading a CTE-enabled web application. See the [browser runtime setup](../../docs/WASM.md#runtime-model).

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
