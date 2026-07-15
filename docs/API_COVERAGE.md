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

## Known deferred namespaced designs

The current audit intentionally leaves these raw capabilities without a public safe wrapper:

- `ImGui::GetFontBaked`, `ImFont::GetFontBaked`, and the `ImFontBaked` query methods need a
  frame-bound `BakedFont` view. Upstream states that these pointers are valid only for the current
  frame, and atlas mutation or density changes can replace the underlying cache.
- `ImFontAtlas::{AddCustomRect,GetCustomRect,RemoveCustomRect}` need a typed rectangle ID, copy-out
  snapshots for UV data that upstream may invalidate at any time, and an API that marks the affected
  managed texture region for renderer upload after pixel writes.
- `ImGuiListClipper::SeekCursorForItem` is useful only with the `INT_MAX` unknown-item-count mode. A
  safe wrapper should model known versus unknown counts explicitly and permit the final seek only in
  the correct post-step state.

These are design tasks, not aliases to add mechanically. Obsolete functions such as
`ImFontAtlas::ClearInputData` remain intentionally sys-only, while low-level font rendering already
has safe draw-list equivalents.

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
