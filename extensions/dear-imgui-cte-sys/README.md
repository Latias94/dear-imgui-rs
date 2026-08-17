# dear-imgui-cte-sys

Preview low-level bindings for `cimCTE` and its pinned ImGuiColorTextEdit
source. The crate ships canonical native and WebAssembly bindings plus the small
`dear_imgui_cte_*` compatibility bridge used for userdata callbacks,
length-aware filters, transactions, and autocomplete configuration.

## Links

- [Workspace](https://github.com/Latias94/dear-imgui-rs)
- [cimCTE](https://github.com/cimgui/cimCTE)
- [ImGuiColorTextEdit](https://github.com/goossens/ImGuiColorTextEdit)

## Compatibility and Provenance

This crate is a Preview `-sys` crate in the `0.17` release train and must use
the same release train, workspace checkout, or Git revision as `dear-imgui-sys`
and `dear-imgui-cte`. It shares the Dear ImGui and cimgui core supplied by
`dear-imgui-sys`; upstream CMake files are not used because they compile a
second core.

The checked-in binding and source identity is:

| Component | Revision |
| --- | --- |
| `cimCTE` `main_goossens` | `b340b99748f9b13307a8e88b938c4c9f8d77df48` |
| nested ImGuiColorTextEdit | `3b46d759975dfd628ef20fd51b7e1c81ef635be5` |
| Dear ImGui baseline | v1.92.9b docking through the workspace cimgui pin |

Native and WASM snapshots are generated from one canonical C++20 specification
covering `cimCTE.h` and `shim/cte_bridge.h`. Ordinary builds use checked-in
bindings and do not require libclang. `cargo run -p xtask -- verify-bindings`
checks both profiles and their provenance.

Maintained source builds compile exactly these translation units:

- `third-party/cimCTE/cimCTE.cpp`
- `third-party/cimCTE/ImGuiColorTextEdit/TextEditor.cpp`
- `third-party/cimCTE/ImGuiColorTextEdit/TextDiff.cpp`
- `third-party/cimCTE/ImGuiColorTextEdit/example/dejavu.cpp`
- `third-party/cimCTE/ImGuiColorTextEdit/extras/TrieAutoComplete.cpp`
- `shim/cte_bridge.cpp`

## Build Modes

| Route | Contract |
| --- | --- |
| Native source, default | Compiles only cimCTE, TextEditor, TextDiff, DejaVu, Trie, and the repository bridge |
| `build-from-source` | Forces the native source route even if Cargo also enables `prebuilt` |
| `prebuilt` | Accepts only an archive whose target, CRT, source revisions, binding identity, feature profile, candidate SHA, and shared-core identity match |
| `wasm` | Uses import bindings for the single fixed `imgui-sys-v1` provider on `wasm32-unknown-unknown`; Cargo does not compile C++ for this target |
| docs.rs | Uses checked-in binding-only output and does not link a native archive |

The native static library is named `dear_imgui_cte`. Maintained source builds
use C++20 and disable C++ exceptions (`/EHs-c-` and `_HAS_EXCEPTIONS=0` on
MSVC, `-fno-exceptions` elsewhere). Bridge entry points are `noexcept`.
Allocation failure therefore terminates instead of unwinding into Rust.

## Environment Variables

| Variable | Purpose |
| --- | --- |
| `CTE_SYS_LIB_DIR` | Directory containing a matching packaged static library and manifest |
| `CTE_SYS_PREBUILT_URL` | Explicit packaged archive URL or local archive path; download/extraction requires `prebuilt` |
| `CTE_SYS_USE_PREBUILT=1` | Allow automatic release archive resolution; requires `prebuilt` |
| `CTE_SYS_SKIP_CC` | Skip C++ compilation and use pregenerated bindings; an external library route must satisfy final linking |
| `CTE_SYS_FORCE_BUILD=1` | Force the source build route |
| `CTE_SYS_PACKAGE_DIR` | Directory containing locally produced release archives |
| `CTE_SYS_CACHE_DIR` | Override the verified archive download/extraction cache root |

A library supplied through an override remains trusted foreign code and must
obey the same C++ runtime, ABI, shared-core, and no-unwind contract.

## API Families

Generated native and WASM bindings cover editor/document/configuration,
positions and selections, languages and palettes, glyph/iterator/codepoint
helpers, TextDiff, Notifications, Trie autocomplete, DejaVu, and every
`dear_imgui_cte_*` bridge family. Required representative symbols are checked
against both generated snapshots by the canonical binding specification. This
is an ABI-presence check, not proof that a safe Rust wrapper is sound.

The semantic safe/sys disposition is recorded in
[`docs/API_COVERAGE.md`](https://github.com/Latias94/dear-imgui-rs/blob/main/docs/API_COVERAGE.md#imguicolortextedit-api-family-ledger).

## Safety and Ownership

This crate exposes raw FFI. Callers must uphold the lifetime and Context
contracts of both cimCTE and Dear ImGui.

- Values returned by constructors such as `TextEditor_TextEditor`,
  `TextDiff_TextDiff`, `TrieAutoComplete_TrieAutoComplete`,
  `Notifications_Notifications`, and `Palette_Palette` are owned and must be
  passed to their matching `*_destroy` function exactly once. Language values
  and editor-owned or static palettes are borrowed and must not be destroyed.
- Editor ownership has two non-interchangeable families. Plain upstream editors
  returned by `TextEditor_TextEditor` must be destroyed with
  `TextEditor_destroy` and may only use upstream-compatible bridge helpers such
  as `dear_imgui_cte_set_change_callback`. Bridge-owned editors returned by
  `dear_imgui_cte_text_editor_create` must be destroyed with
  `dear_imgui_cte_text_editor_destroy`; only those pointers may be passed to
  `dear_imgui_cte_text_editor_set_change_callback`,
  `dear_imgui_cte_text_editor_reset_autocomplete`, and
  `dear_imgui_cte_text_editor_clear_callbacks`.
- Upstream string getters use temporary static storage. Copy bytes before the
  same getter is called again. Memory from `TextEditor_GetText_alloc` must be
  released only with `TextEditor_GetText_free`.
- Bridge callbacks retain userdata until replacement,
  `dear_imgui_cte_clear_callbacks`,
  `dear_imgui_cte_text_editor_clear_callbacks`, or editor destruction. Userdata
  must remain valid for that period and callbacks must not unwind through the C
  ABI. Filter output and event pointers are callback-scoped borrows.
- Applying bridge autocomplete configuration copies it into the editor. The
  configuration handle may be destroyed after a successful apply. State
  pointers received by autocomplete callbacks are callback-scoped.
- Trie `Connect` replaces the editor's autocomplete, change, and language-change
  slots. It must be disconnected before either the Trie or editor is destroyed.
- `TextEditor_SetImGuiContext` mutates Dear ImGui's process-current Context. It
  is a raw escape hatch, not a second Context owner.
- `SetDejavu` clears the application's font atlas and changes its font loader.
  Safe applications should use `dear_imgui_cte::dejavu_font_source` before
  renderer initialization instead.

## Third-party Notices

The selected cimCTE revision has no standalone top-level license file. Retained
source notices and the nested ImGuiColorTextEdit license are packaged as
described in [`third-party/README.md`](third-party/README.md).
