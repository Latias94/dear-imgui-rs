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
```

This prints a representative list of public ImGui functions that are not referenced by the
high-level crate (either via `#[doc(alias = "...")]` or via a sys call).

Notes:
- A function being "uncovered" does not automatically mean it should be wrapped: many uncovered
  functions are intentionally sys-only (variadics, allocator hooks, debug helpers, etc.).
- Conversely, a function being "covered" does not guarantee perfect ergonomics; it only means we
  have a high-level reference point.

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

