# dear-imgui-cte-sys

Preview low-level binding crate for `cimCTE` and its pinned
`ImGuiColorTextEdit` source. It includes canonical native and WebAssembly
bindings plus the small `dear_imgui_cte_*` compatibility bridge used for
userdata callbacks, length-aware filters, and autocomplete configuration.

This crate shares the Dear ImGui and cimgui core supplied by `dear-imgui-sys`.
The upstream CMake files are not used because they also compile their own copy
of cimgui and Dear ImGui.

The native library name is `dear_imgui_cte`. Native configuration variables use
the `CTE_SYS_*` prefix.

## Safety and ownership

This crate exposes raw FFI. Callers must uphold the lifetime and context
contracts of both cimCTE and Dear ImGui.

- Values returned by constructor functions such as `TextEditor_TextEditor`,
  `TextDiff_TextDiff`, `TrieAutoComplete_TrieAutoComplete`,
  `Notifications_Notifications`, and `Palette_Palette` are owned and must be
  passed to their matching `*_destroy` function exactly once. Language values
  and editor-owned or static palettes are borrowed and must not be destroyed.
- String getters backed by upstream static storage are temporary. Copy their
  bytes before calling the same getter again. Memory returned by
  `TextEditor_GetText_alloc` must be released only with
  `TextEditor_GetText_free`.
- Bridge callbacks retain the supplied userdata pointer until that callback is
  replaced, `dear_imgui_cte_clear_callbacks` is called, or the editor is
  destroyed. The userdata must remain valid for that entire period. A callback
  must not unwind through the C ABI; filter output is borrowed only for the
  duration of the callback and is copied by the bridge before returning.
- Applying a bridge autocomplete configuration copies it into the editor. The
  configuration handle may be destroyed after a successful apply. State
  pointers received by autocomplete callbacks are callback-scoped borrows.
- `TextEditor_SetImGuiContext` mutates Dear ImGui's process-local current
  context. It is a raw escape hatch, not a second context owner.

## Build contract

Maintained source builds use C++20 and compile every CTE translation unit with
C++ exceptions disabled (`/EHs-c-` and `_HAS_EXCEPTIONS=0` on MSVC,
`-fno-exceptions` elsewhere). Bridge entry points are also `noexcept`.
Allocation failure therefore terminates instead of unwinding into Rust.

Prebuilt artifacts are accepted only when their target, CRT, source revisions,
binding identity, feature profile, candidate SHA, and shared
`dear-imgui-sys` artifact identity match. A library supplied through a
`CTE_SYS_*` override remains trusted foreign code and must obey the same ABI and
no-unwind contract.

See `third-party/README.md` for exact upstream revisions and license locations.
