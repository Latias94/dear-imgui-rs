# Android Smoke Examples

This directory contains the repository-owned Android smoke templates.

They are intentionally:

- outside the main workspace build
- focused on proving integration shape
- not published as crates
- narrow enough to validate Android packaging and runtime glue without turning
  them into published runtime crates

## Current Example

| Example | Use when | Type |
|---|---|---|
| `dear-imgui-android-smoke` | you need the low-level Android route built from `dear-imgui-rs` + `dear-imgui-sys` backend shims | Android smoke template |

## How To Use This Template

- Treat this as a low-level Android integration template, not as a generic app
  runner.
- If your application already owns SDL3 on Android, prefer
  `dear-imgui-sdl3` as the higher-level platform integration direction.
- Keep Android lifecycle, EGL / GLES policy, packaging, signing, and store
  distribution owned by the consuming application.

## Included Template

### `dear-imgui-android-smoke`

Low-level Android route:

- `dear-imgui-rs`
- `dear-imgui-sys`
- `backend_shim::android`
- `backend_shim::opengl3`

Includes:

- a minimal Android `NativeActivity` smoke path
- a Python helper for per-target APK builds
- a checked-in README showing the ownership split and packaging expectations

See:

- `examples-android/dear-imgui-android-smoke/README.md`
