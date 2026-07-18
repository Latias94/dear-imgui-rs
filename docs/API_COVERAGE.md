# High-level API coverage workflow

This repository contains a safe, ergonomic Rust API surface (`dear-imgui-rs`) built on top of
the raw sys bindings (`dear-imgui-sys`).

The goal of this document is to describe how we **track** and **incrementally improve** the
high-level coverage of Dear ImGui's *public* API without accidentally creating duplicated wrappers.

## What we cover (and what we don't)

We aim to provide high-level wrappers for:
- Common, stable, public Dear ImGui APIs.
- APIs that can be exposed ergonomically (typed flags, `Option<T>` for nullable, RAII for begin/end).
- APIs that are testable in a headless context (no real OS window required).

We intentionally avoid:
- `imgui_internal` APIs.
- C variadic formatting APIs (`*V` / `...`) where possible. Prefer non-variadic equivalents such as
  `TextUnformatted`-style helpers.

## Coverage report tool

Run:

```bash
python tools/api_surface_report.py --format plain --limit 40
python tools/api_surface_report.py --check
```

The tool has two deliberately different coverage layers.

### Generator contract snapshot: all namespaces

`tools/api_surface_snapshot.json` records every public function declaration emitted by cimgui's
generator across all namespaces. The canonical record includes the symbol, namespace, signature,
return type, arguments, defaults, generator traits, and the exact cimgui and Dear ImGui source
revisions. `--check` rejects added, removed, or changed declarations and source revision drift.

This layer detects raw API drift; it does **not** claim that every non-`ImGui` namespace function has
a safe Rust wrapper. After reviewing an intentional upstream change, refresh it explicitly with:

```bash
python tools/api_surface_report.py --update-snapshot
```

The snapshot currently covers function declarations. Enum values, flag bits, and public struct
fields still require their existing upgrade audit and are not independently snapshotted by this
tool.

### Safe semantic audit: top-level `ImGui` functions

For top-level `ImGui` functions, the report treats a matching `#[doc(alias = "...")]` on a public,
safe Rust item as direct safe API coverage. A builder, RAII token, context lifecycle, or typed
composition should carry the aliases of the upstream operations it replaces. Every remaining
function must have an explicit decision in `tools/api_surface_policy.json`:

- `intentional-sys-only`: wrapping it would expose variadic formatting, obsolete APIs, or unsafe
  global/raw ownership contracts.
- `deferred-design`: a safe wrapper is desirable but needs a documented lifetime or type design.

`--check` fails on generator drift, unclassified top-level functions, and stale policy entries. CI
runs it after checking the vendored source revisions, so a cimgui/Dear ImGui update cannot silently
add an unreviewed top-level safe API gap.

Notes:
- A policy decision is not permanent. Replace it with a rustdoc alias when a direct safe wrapper is
  added, or update its rationale when the high-level design changes.
- Direct `sys::ig*` usage is reported only as information. It is not proof of safe API coverage,
  because an internal call may expose only a small subset of the public operation.
- Namespaced APIs such as `ImFontAtlas`, `ImFontBaked`, `ImDrawList`, and `ImTextureData` are guarded
  against generator drift but still need a manual safe-layer audit during upgrades.

### Removed safe surface and FFI ownership

The same `--check` command discovers maintained Cargo packages from the repository manifest and
enforces the frozen 0.16 removal inventory. It tokenizes Rust source instead of searching raw text,
so comments, documentation, and string literals cannot hide or reintroduce an API. The contract
rejects:

- removed identifiers and call paths, including `Context::frame_with`, `Selectable::new`, the
  horizontal `Slider::new`, `TextureData::new`, `InputFlags`, `ArrowDirection`, and backend
  compatibility helpers;
- public re-exports of the removed `render::renderer` and `fonts::glyph_ranges` modules;
- a public ImPlot3D export of `validate_nonempty`, `validate_lengths`, or `validate_multiple` while
  allowing their crate-private implementation use;
- the removed `sdl3-backends` Cargo feature alias; and
- a foreign function declaration in a safe crate when that symbol is already supplied by a
  maintained `*-sys` crate, including the retired extension `compat_ffi` symbols.

Raw sys crates are deliberately outside the removed-safe-surface scan. They remain the documented
escape hatch for users who accept the native API's unsafe contract. Safe crates may call
`sys::function(...)` directly and may define C-ABI callbacks or declarations for crate-owned native
shims; only duplicate ownership of a generated sys declaration is rejected.

## Completed lifetime-sensitive designs

The safe layer models the previously deferred namespaced capabilities through lifetime- and
state-aware APIs:

- `Ui::current_font()` returns an atlas-validated `FontId`, and metadata methods copy owned values
  instead of exposing a borrowed `ImFont` view. `Ui::{current_baked_font,baked_font,
  baked_font_with_density}` returns `BakedFont<'ui>`, which revalidates the font and resolves native
  baked storage on each access. Glyph queries return owned `Glyph` metric copies. The safe glyph API
  intentionally omits UVs because another lazy glyph load may repack the atlas within the same frame.
- `FontAtlas::tex_data()` returns a read-only `FontAtlasTexture<'_>` lease. Atlas rebuilds, clears,
  custom-rectangle pixel writes, and context frame advancement reject a live lease rather than
  invalidating borrowed texture memory.
- `FontSource` is opaque, and constructors for external TTF/OTF, compressed, Base85, and file inputs
  are unsafe because the native parsers do not consistently enforce their input bounds. Direct
  include ranges use structured `(start, end)` pairs stored for the native source lifetime.
- `FontAtlas::{add_custom_rect,write_custom_rect,remove_custom_rect,custom_rect}` uses an
  atlas-validated `CustomRectId`, strict `CustomRectData`, and copy-out `CustomRectSnapshot` values.
  Pixel writes queue the exact managed texture region for renderer upload; `Ui::image_custom_rect`
  resolves current texture and UV data at submission time.
- `Ui::{show_demo_window,show_metrics_window,show_style_editor,show_default_style_editor}` remains an
  explicit unsafe boundary because upstream Fonts panels can perform destructive atlas operations
  during the call; the safe layer does not claim control over later user interaction.
- `ListClipper::unknown_count()` returns a distinct token whose `next_range()` protocol is finalized
  by consuming `finish(final_items_count)`. Known counts reject the native `INT_MAX` sentinel.
  Clipper tokens enforce native LIFO plus their exact frame, window `Begin`, and table instance. An
  out-of-order drop defers cleanup until the stack recovers, wrong-scope cleanup suppresses cursor
  seeking while letting native code restore its temporary stack, and a forgotten token rejects and
  recovers only the current frame.

Obsolete functions such as `ImFontAtlas::ClearInputData` remain intentionally sys-only, while
low-level font rendering already has safe draw-list equivalents. `ImFontLoader` remains an unsafe
native extension boundary because upstream still declares its callback table as internal and
evolving.

## Avoiding duplicate wrappers (required)

Before adding a wrapper for an upstream ImGui function `FooBar`:

1. Search by doc alias:
   - `rg -n "alias = \\\"FooBar\\\"" dear-imgui/src`
2. Search by Rust naming convention:
   - `rg -n "foo_bar\\(|FooBar" dear-imgui/src`
3. Search by sys usage:
   - `rg -n "sys::igFooBar" dear-imgui/src`

If an equivalent exists, prefer adding `#[doc(alias = "...")]` and/or a convenience overload
instead of creating a second wrapper with a different name.

## Implementation checklist

When you add a new wrapper:
- Put it in the most relevant module (`ui`, `input`, `widget/*`, `window`, `platform_io`, ...).
- Add `#[doc(alias = "...")]` to match the upstream C++ API name.
- Prefer typed flags and safe Rust signatures.
- Add a focused headless test when feasible (see `dear-imgui/tests/*`).
- Update `CHANGELOG.md` for user-visible additions.

Some APIs require cross-crate coordination (backends/extensions/examples). In those cases, update
the relevant crate(s) or examples in the same PR to keep the repository consistent.

## Local TODO tracking (optional)

For local, non-committed tracking, keep notes under `repo-ref/` (this folder is ignored by git in
this repo). This is useful for scratch work and prioritization, but user-facing changes should be
captured in `CHANGELOG.md`.
