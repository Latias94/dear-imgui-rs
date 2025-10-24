# Compatibility Matrix

This document tracks compatibility across the workspace crates, upstream Dear ImGui, and key external dependencies. The root README shows the latest recommendations; this file keeps history and upgrade guidance.

## Versioning Policy

- Unified release train: all `dear-*` crates in this workspace are versioned and released together under the same semver, so consumers can depend on a single minor across the board.
- Current train: unified `v0.5.0` (use `version = "0.5"`).
- Previous train: unified `v0.4.0` (use `version = "0.4"`).
- Internal dependency constraints in this repo also pin to the unified minor (e.g., `0.5`). Mixing different minors across our crates is unsupported.
- Exception: helper tooling like `tools/build-support` may follow an independent version and is not part of the unified train.

## Latest (Summary)

Core

| Crate           | Version | Upstream        | Notes                                     |
|-----------------|---------|-----------------|-------------------------------------------|
| dear-imgui-rs   | 0.5.x   | —               | Safe Rust API over dear-imgui-sys         |
| dear-imgui-sys  | 0.5.x   | ImGui v1.92.4   | Docking branch via cimgui                 |

Backends

| Crate            | Version | External deps   | Notes |
|------------------|---------|-----------------|-------|
| dear-imgui-wgpu  | 0.5.x   | wgpu = 27       |       |
| dear-imgui-glow  | 0.5.x   | glow = 0.16     |       |
| dear-imgui-winit | 0.5.x   | winit = 0.30.12 |       |

Utilities

| Crate     | Version | External deps | Notes |
|-----------|---------|---------------|-------|
| dear-app  | 0.5.x   | winit, wgpu   | App runner (docking, themes, add-ons) |

Extensions

| Crate               | Version | Requires dear-imgui-rs | Sys crate                  | Notes |
|---------------------|---------|------------------------|----------------------------|-------|
| dear-implot         | 0.5.x   | 0.5.x                  | dear-implot-sys 0.5.x      |       |
| dear-imnodes        | 0.5.x   | 0.5.x                  | dear-imnodes-sys 0.5.x     |       |
| dear-imguizmo       | 0.5.x   | 0.5.x                  | dear-imguizmo-sys 0.5.x    |       |
| dear-file-browser   | 0.5.x   | 0.5.x                  | —                          | ImGui UI + native (rfd) backends |
| dear-implot3d       | 0.5.x   | 0.5.x                  | dear-implot3d-sys 0.5.x    | 3D plotting |
| dear-imguizmo-quat  | 0.5.x   | 0.5.x                  | dear-imguizmo-quat-sys 0.5.x | Quaternion gizmo |

## History

Release Train 0.5 (current)

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
