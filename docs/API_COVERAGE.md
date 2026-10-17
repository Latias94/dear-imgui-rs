# High-level API coverage

`dear-imgui-rs` provides safe, ergonomic Rust APIs over the raw `dear-imgui-sys` bindings.
Coverage is a semantic review problem: finding the same native symbol in Rust source does not prove
that the wrapper has correct ownership, lifetime, callback, or ABI behavior.

## Coverage policy

Add a safe wrapper when the native operation can be expressed with enforceable Rust invariants.
Prefer typed flags, owned copies, validated IDs, scoped tokens, and borrows tied to the owning
`Context`, frame, atlas, or renderer epoch.

Keep an operation sys-only when a safe contract would be misleading. Current examples include:

- C variadic formatting entry points;
- obsolete Columns APIs, which are superseded by tables;
- process-global allocator and raw hook configuration;
- raw `MemAlloc` and `MemFree` ownership;
- internal or evolving font-loader callbacks; and
- APIs whose native parser accepts an unbounded external buffer.

The sys crates remain the explicit unsafe escape hatch. A wrapper may be added later when its
ownership and lifetime design is clear.

Every user-relevant native or provisional high-level surface must have one recorded disposition:

- `keep`: the public concept and its invariants are already accurate;
- `rename`: the contract remains, but the current name teaches the wrong lifecycle state;
- `replace`: the existing contract is removed in favor of a named ownership or validation model;
- `unsafe`: the operation remains public only with an explicit native prerequisite that Rust cannot
  prove; or
- `delete`: no high-level replacement is justified, so callers use a safer composition or the sys
  escape hatch.

The [0.16 alpha.2 public API disposition](COMPATIBILITY.md#016-alpha2-public-api-disposition) records the ownership migration baseline. Later release sections in [the compatibility guide](COMPATIBILITY.md) and `CHANGELOG.md` record subsequent additions and migrations.
Do not duplicate these decisions in generated coverage reports or infer a `keep` decision merely because a Rust
method and native symbol share a name. Every removal must identify one replacement or explicitly
state that the sys API is the intended unsafe route.

## ImGuiColorTextEdit API family ledger

This is a family-level semantic ledger over the checked-in native and WASM cimCTE binding
snapshots at wrapper revision `3333a9bd3b7b881a0c34d8abbd97c8ab2c60ea3f` and nested
ImGuiColorTextEdit revision `b12d713e77e8edddff1e69973f67085ac9cfaf74`. It is not a second
declaration inventory, does not infer safety from symbol names, and does not parse Rust source.

The canonical binding specification owns the generated family allowlist. Its static required-symbol
sentinels are checked against both native and WASM snapshots, so a regenerated artifact cannot
silently lose one of these families. New or changed declarations still require a generated diff and
human ownership review; a symbol-presence check is not proof of safe coverage.

| Generated family | High-level disposition |
| --- | --- |
| `TextEditor_*` document, configuration, render, navigation, search, and diagnostics | Safe ergonomic subset on context-bound `TextEditor` and `TextEditorRenderer`; strings are owned copies and indices are validated. Process-current Context mutation and raw line-break configuration remain sys-only. |
| `DocPos_*`, `DocSelection_*`, `VisPos_*` | Copied Rust `Position`, `Selection`, and `VisualPosition` values; native heap constructors are unnecessary in safe code. |
| `Language_*`, `Palette_*` | Closed `Language` values plus copied, bounds-checked `Palette`; raw language pointers remain borrowed static values. |
| `dear_imgui_cte_*` callback, transaction, iteration, and filter bridge | Editor-owned typed closures with length-aware UTF-8, callback-scoped views, complete-batch validation, explicit clear/replace ordering, reentry policy, and abort-on-panic no-unwind behavior. |
| `TrieAutoComplete_*` and `dear_imgui_cte_*autocomplete*` | Editor-owned Trie attachment or custom autocomplete with callback-scoped `AutocompleteRequest`; exclusive upstream callback slots are represented as conflicts. |
| `TextDiff_*` | Context-bound `TextDiff` with copied configuration and a `Ui`-checked render builder. |
| `Glyph_*`, `Iterator_*`, `CodePoint_*` | Sys-only. Their native pointer/iterator graph has no stable ownership benefit over the copied editor values exposed by the safe layer. |
| `CustomLineNumber` and line-number callback configuration | Editor-owned typed callbacks receive copied `LineNumberEvent` geometry and the active `Ui`; the native draw-list pointer remains sys-only. |
| `GetDejavu`, `Getnotosans`, `SetDejavu` | `dejavu_font_source` and `noto_sans_sc_font_source` return static compressed sources for the managed atlas. `SetDejavu` remains sys-only because it clears the full atlas and changes the global loader outside renderer texture management. |

Unsafe `as_raw` escape hatches on safe owners require the caller to preserve every native
ownership, pointer-lifetime, Context, and wrapper invariant before returning to safe methods.

## Dear ImGui 1.93 decisions

- Unified `DrawFlags`, per-primitive stroke options, and `with_draw_flags` model the new drawing API. Flag scopes enforce the native stack capacity and unwind safely. Circle, ellipse, and ngon anti-aliasing must be configured through the scope because upstream does not forward their per-primitive AA bits.
- `ItemFlags::MIXED_VALUE`, line-end anti-aliasing, pixel-based curve error, `FontAtlasFlags`, and `TextFilter::draw_with_hint` are safe wrappers. Atlas options must be set before building starts. Do not re-enable line AA through drawing flags when `NO_BAKED_LINES` is set.
- Standard texture-sampler callbacks survive borrowed draw iteration and detached snapshots. Glow supports them; WGPU and Ash reject unsupported commands before drawing. Bevy abandons the affected Context snapshot before upload and reports a public preparation diagnostic.
- `ImFont::AddRemapCodepoint` and `AddRemapCodepointToGlyphIndex` remain sys-only: source/glyph indices and already-baked glyph invalidation require a separate validated mapping lifecycle. Custom font-loader tables remain unsafe native interop and must use the new glyph-index callback ABI.
- Internal pixel-density setters, window-flag mutation helpers, tessellation routines, and native allocation helpers remain sys-only. Backends obtain density through viewport framebuffer scale.
- CTE document-to-screen conversion returns a copied coordinate only after a visible render in the same frame with unchanged layout. Node background draw lists return `Option` after node submission completes.
- CTE's shared native/WASM source transforms preserve bounded Unicode range lookups and adapt TextDiff's three scrollbar corner flags; native artifact identities include both corrections.

## Reviewing an upstream update

The vendored source and regenerated bindings are the canonical diff. Do not infer Rust safety from
symbol names or maintain a second generated snapshot of the same declarations.

1. Read the upstream release notes and inspect every updated submodule commit range.
2. Regenerate all maintained bindings with the canonical libclang version:

   ```console
   python3 tools/update_submodule_and_bindings.py --crates all --submodules skip --wasm
   ```

3. Review generated binding changes, especially callback signatures, aggregate parameters and
   returns, field layout, enum representation, ownership comments, and newly exposed functions.
4. Search the high-level crate for an existing semantic equivalent:

   ```console
   rg -n 'alias = "FooBar"|foo_bar\(|sys::igFooBar' dear-imgui/src
   ```

5. For each user-relevant addition, choose one explicit outcome: add or extend a safe wrapper, keep
   it sys-only with a documented reason, or defer it because a separate lifetime design is needed.
6. Prove the chosen contract with the narrowest appropriate mechanism: a Rust unit test,
   compile-fail doctest, native C++/Rust ABI probe, or backend runtime test.

`#[doc(alias = "...")]` improves discovery and prevents duplicate wrappers. It is not itself proof
of safe coverage.

## Lifetime-sensitive designs

The high-level layer already models the major ownership-sensitive surfaces:

- `BakedFont<'ui>` resolves native baked storage through a live frame borrow, while glyph queries
  return owned metric copies.
- `FontAtlasTexture<'_>` is a read-only atlas lease; rebuild, clear, and pixel mutation reject a
  live lease.
- `CustomRectId` is atlas-validated, and custom-rectangle queries return owned snapshots.
- external font sources that rely on native parsers remain explicitly unsafe constructors.
- curated demo and diagnostics windows are safe; the destructive font-atlas panel is isolated.
- clipper tokens enforce their exact frame, window, table, and native stack order.
- rendered frames and snapshots are move-only leases tied to one renderer consumer and epoch.

## Context and callback rules

Only the core Context binding implementation may switch native `GImGui`. Backends and extensions
must use the scoped Context APIs instead of calling `igSetCurrentContext` in production code.

Dear ImGui platform callbacks that pass `ImVec2` or `ImVec4` by value are compiler-ABI-sensitive.
Repository-owned C++ shims translate those slots to pointer or out-parameter callbacks. Changes to
these callbacks require the existing native ABI probes on MSVC and the affected backend runtime
tests; source-text scanning is not an ABI test.

## Adding a wrapper

- Put the API in the module that owns its state and lifetime.
- Add the upstream C++ name as a rustdoc alias.
- Avoid exposing raw pointers, process-global state, or unvalidated native IDs from safe methods.
- Add a headless test when possible and a runtime or ABI test when native behavior matters.
- Update `CHANGELOG.md` for user-visible additions or migration requirements.

Use `repo-ref/` for local comparison checkouts and scratch notes. User-facing decisions belong in
the API documentation, tests, and changelog.
