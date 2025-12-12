# Compatibility Matrix

This document tracks compatibility across the workspace crates, upstream Dear ImGui, and key external dependencies. The root README shows the latest recommendations; this file keeps history and upgrade guidance.

## Versioning Policy

- Unified release train: all `dear-*` crates in this workspace are versioned and released together under the same semver, so consumers can depend on a single minor across the board.
- Current train: unified `v0.6.0` (use `version = "0.6"`).
- Previous train: unified `v0.5.0` (use `version = "0.5"`).
- Internal dependency constraints in this repo also pin to the unified minor (e.g., `0.5`). Mixing different minors across our crates is unsupported.
- Exception: helper tooling like `tools/build-support` may follow an independent version and is not part of the unified train.

## Latest (Summary)

Core

| Crate           | Version | Upstream        | Notes                                     |
|-----------------|---------|-----------------|-------------------------------------------|
| dear-imgui-rs   | 0.6.x   | —               | Safe Rust API over dear-imgui-sys         |
| dear-imgui-sys  | 0.6.x   | ImGui v1.92.5   | Docking branch via cimgui                 |

Backends

| Crate             | Version | External deps           | Notes |
|-------------------|---------|-------------------------|-------|
| dear-imgui-wgpu   | 0.6.x   | wgpu = 27              | WebGPU renderer (experimental multi-viewport on native via winit/SDL3; disabled on wasm) |
| dear-imgui-glow   | 0.6.x   | glow = 0.16            | OpenGL renderer (winit/glutin) |
| dear-imgui-winit  | 0.6.x   | winit = 0.30.12        | Winit platform backend |
| dear-imgui-sdl3   | 0.6.x   | sdl3 = 0.16, sdl3-sys  | SDL3 platform backend (C++ imgui_impl_sdl3/GL3) |

Utilities

| Crate     | Version | External deps | Notes |
|-----------|---------|---------------|-------|
| dear-app  | 0.6.x   | winit, wgpu   | App runner (docking, themes, add-ons) |

Extensions

| Crate               | Version | Requires dear-imgui-rs | Sys crate                    | Notes                                  |
|---------------------|---------|------------------------|------------------------------|----------------------------------------|
| dear-implot         | 0.6.x   | 0.6.x                  | dear-implot-sys 0.6.x        | 2D plotting                            |
| dear-imnodes        | 0.6.x   | 0.6.x                  | dear-imnodes-sys 0.6.x       | Node editor                            |
| dear-imguizmo       | 0.6.x   | 0.6.x                  | dear-imguizmo-sys 0.6.x      | 3D gizmo                               |
| dear-file-browser   | 0.6.x   | 0.6.x                  | —                            | ImGui UI + native (rfd) backends       |
| dear-implot3d       | 0.6.x   | 0.6.x                  | dear-implot3d-sys 0.6.x      | 3D plotting                            |
| dear-imguizmo-quat  | 0.6.x   | 0.6.x                  | dear-imguizmo-quat-sys 0.6.x | Quaternion gizmo                       |
| dear-imgui-reflect  | 0.6.x   | 0.6.x                  | —                            | Reflection-based UI helpers (pure Rust)|

## History

Release Train 0.7 (upcoming)

- Planned changes (subject to adjustment before release):
  - Core + backends remain aligned with Dear ImGui v1.92.5 and the same wgpu/winit/glow/sdl3 baselines.
  - Import-style WASM support (via `imgui-sys-v0` provider) for selected extension crates:
    - `dear-implot` / `dear-implot-sys`: 2D plotting on wasm.
    - `dear-imnodes` / `dear-imnodes-sys`: node editor on wasm.
    - `dear-imguizmo` / `dear-imguizmo-sys`: 3D gizmo on wasm.
    - `dear-imguizmo-quat` / `dear-imguizmo-quat-sys`: quaternion gizmo on wasm.
    - `dear-implot3d` / `dear-implot3d-sys`: 3D plotting on wasm.
  - New/updated `xtask` flows for building the wasm demo and import-style provider:
    - `wasm-bindgen-*` commands for core + extensions (ImPlot, ImPlot3D, ImNodes, ImGuizmo, ImGuIZMO.quat).
    - `web-demo [features]` to toggle which extensions are compiled into the web demo.
    - `build-cimgui-provider` to build the shared `imgui-sys-v0` provider (Emscripten).

Release Train 0.6 (current)

- All crates unified to 0.6.0 across the workspace
  - Core: dear-imgui-rs 0.6.0, dear-imgui-sys 0.6.0
  - Backends: dear-imgui-wgpu 0.6.0, dear-imgui-glow 0.6.0, dear-imgui-winit 0.6.0
  - Utilities: dear-app 0.6.0
  - Extensions: dear-implot 0.6.0, dear-imnodes 0.6.0, dear-imguizmo 0.6.0, dear-implot3d 0.6.0, dear-imguizmo-quat 0.6.0, dear-file-browser 0.6.0, dear-imgui-reflect 0.6.0
  - Sys crates: dear-implot-sys 0.6.0, dear-imnodes-sys 0.6.0, dear-imguizmo-sys 0.6.0, dear-implot3d-sys 0.6.0, dear-imguizmo-quat-sys 0.6.0
- dear-imgui-sys 0.6.x binds Dear ImGui v1.92.5 (docking) via cimgui
- New features:
  - New drag/drop flag and style color for improved drop target customization
  - Inherited all bug fixes and behavior changes from Dear ImGui v1.92.5
- External dependencies baseline: wgpu 27, winit 0.30.12, glow 0.16, sdl3 0.16
- Upgrade: change `version = "0.5"` to `version = "0.6"` in your Cargo.toml for all `dear-*` crates

### Backend & multi-viewport support notes (0.6.x)

- Winit + WGPU:
  - Multi-viewport support has experimental code paths in `dear-imgui-winit` + `dear-imgui-wgpu`, but is **not supported** in 0.6.x.
  - The `multi_viewport_wgpu` example is provided strictly as a **testbed** and is known to be unstable on some platforms (especially macOS/winit).
  - Do not rely on winit + WGPU multi-viewport for production use in this release train.
  - SDL3 + OpenGL3:
    - Supported via `dear-imgui-sdl3` (C++ `imgui_impl_sdl3.cpp` + `imgui_impl_opengl3.cpp`).
    - Multi-viewport: **supported** using the upstream SDL3 + OpenGL3 backend behaviour.
    - Example: `sdl3_opengl_multi_viewport` (`cargo run -p dear-imgui-examples --bin sdl3_opengl_multi_viewport --features multi-viewport,sdl3-opengl3`).
  - SDL3 + WGPU:
    - Supported via SDL3 platform backend (`dear-imgui-sdl3`) + Rust WGPU renderer (`dear-imgui-wgpu`).
    - A single-window example is provided: `sdl3_wgpu` (`cargo run -p dear-imgui-examples --bin sdl3_wgpu --features sdl3-platform`).
    - Multi-viewport for WebGPU remains **disabled** on this route, matching upstream `imgui_impl_wgpu` which currently does not implement multi-viewport.

Release Train 0.5 (previous)

- All crates unified to 0.5.0 across the workspace
  - Core: dear-imgui-rs 0.5.0, dear-imgui-sys 0.5.0
  - Backends: dear-imgui-wgpu 0.5.0, dear-imgui-glow 0.5.0, dear-imgui-winit 0.5.0
  - Utilities: dear-app 0.5.0
  - Extensions: dear-implot 0.5.0, dear-imnodes 0.5.0, dear-imguizmo 0.5.0, dear-implot3d 0.5.0, dear-imguizmo-quat 0.5.0, dear-file-browser 0.5.0
  - Sys crates: dear-implot-sys 0.5.0, dear-imnodes-sys 0.5.0, dear-imguizmo-sys 0.5.0, dear-implot3d-sys 0.5.0, dear-imguizmo-quat-sys 0.5.0
- dear-imgui-sys 0.5.x binds Dear ImGui v1.92.4 (docking) via cimgui
- New features:
  - Added `StyleColor::UnsavedMarker` for marking unsaved documents/windows
  - Inherited all bug fixes from Dear ImGui v1.92.4
- External dependencies baseline: wgpu 27, winit 0.30.12, glow 0.16
- Upgrade: change `version = "0.4"` to `version = "0.5"` in your Cargo.toml for all `dear-*` crates

Release Train 0.4 (previous)

- All crates unified to 0.4.0 across the workspace
  - Core: dear-imgui-rs 0.4.0, dear-imgui-sys 0.4.0
  - Backends: dear-imgui-wgpu 0.4.0, dear-imgui-glow 0.4.0, dear-imgui-winit 0.4.0
  - Extensions: dear-implot 0.4.0, dear-imnodes 0.4.0, dear-imguizmo 0.4.0, dear-implot3d 0.4.0, dear-imguizmo-quat 0.4.0
  - Sys crates: dear-implot-sys 0.4.0, dear-imnodes-sys 0.4.0, dear-imguizmo-sys 0.4.0, dear-implot3d-sys 0.4.0, dear-imguizmo-quat-sys 0.4.0
- dear-imgui-sys 0.4.x binds Dear ImGui v1.92.3 (docking) via cimgui
- External dependencies baseline: wgpu 27, winit 0.30.12, glow 0.16
- Upgrade: change `version = "0.3"` to `version = "0.4"` in your Cargo.toml for all `dear-*` crates

Release Train 0.3 (previous)

- BREAKING: Main crate renamed from `dear-imgui` to `dear-imgui-rs` (v0.3.0)
- All crates unified to 0.3.0 across the workspace
  - Core: dear-imgui-rs 0.3.0, dear-imgui-sys 0.3.0
  - Backends: dear-imgui-wgpu 0.3.0, dear-imgui-glow 0.3.0, dear-imgui-winit 0.3.0
  - Extensions: dear-implot 0.3.0, dear-imnodes 0.3.0, dear-imguizmo 0.3.0, dear-implot3d 0.3.0, dear-imguizmo-quat 0.3.0
  - Sys crates: dear-implot-sys 0.3.0, dear-imnodes-sys 0.3.0, dear-imguizmo-sys 0.3.0, dear-implot3d-sys 0.3.0, dear-imguizmo-quat-sys 0.3.0
- dear-imgui-sys 0.3.x binds Dear ImGui v1.92.3 (docking) via cimgui.
- dear-imgui-rs 0.3.x layers a safe API over the 0.3.x sys crate.
- External dependencies: wgpu 26, winit 0.30.12, glow 0.16
  - dear-file-browser (preview): optional features `imgui` (pure UI) and `native-rfd` (rfd backend) enabled by default. On wasm32, prefer `native-rfd` (Web File Picker). The ImGui UI enumerates the filesystem via `std::fs` and cannot list local files in the browser environment.

Release Train 0.2 (deprecated, yanked)

- dear-imgui-sys 0.2.x binds Dear ImGui v1.92.3 (docking) via cimgui.
- dear-imgui 0.2.x layers a safe API over the 0.2.x sys crate.
- Backends: wgpu (26), winit (0.30.12), glow (0.16).
- Extensions: dear-implot 0.2.x (with -sys 0.2.x), dear-imnodes 0.1.x, dear-imguizmo 0.1.x — all depend on dear-imgui/dear-imgui-sys 0.2.x.

## Upgrade Guidelines

- When bumping dear-imgui-sys to a new upstream ImGui, bump all -sys extensions (implot/imnodes/imguizmo) in lockstep and verify bindgen output/ABI.
- When bumping dear-imgui, check backends/extensions for API surface changes and update versions accordingly.
- Backend external deps (wgpu/winit/glow) often introduce breaking changes; track and bump backend crates even if core didn’t change.
