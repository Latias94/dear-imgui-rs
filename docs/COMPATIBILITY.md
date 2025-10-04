# Compatibility Matrix

This document tracks compatibility across the workspace crates, upstream Dear ImGui, and key external dependencies. The root README shows the latest recommendations; this file keeps history.

## Latest (Summary)

Core

| Crate           | Version | Upstream        | Notes                                     |
|-----------------|---------|-----------------|-------------------------------------------|
| dear-imgui-rs   | 0.3.x   | —               | Safe Rust API over dear-imgui-sys         |
| dear-imgui-sys  | 0.3.x   | ImGui v1.92.3   | Docking branch via cimgui                 |

Backends

| Crate            | Version | External deps      | Notes |
|------------------|---------|--------------------|-------|
| dear-imgui-wgpu  | 0.3.x   | wgpu = 26          |       |
| dear-imgui-glow  | 0.3.x   | glow = 0.16        |       |
| dear-imgui-winit | 0.3.x   | winit = 0.30.12    |       |

Extensions

| Crate         | Version | Requires dear-imgui-rs | Sys crate               | Notes |
|---------------|---------|------------------------|-------------------------|-------|
| dear-implot   | 0.3.x   | 0.3.x                  | dear-implot-sys 0.3.x   |       |
| dear-imnodes  | 0.3.x   | 0.3.x                  | dear-imnodes-sys 0.3.x  |       |
| dear-imguizmo | 0.3.x   | 0.3.x                  | dear-imguizmo-sys 0.3.x |       |
| dear-file-browser | 0.3.x | 0.3.x               | — (pure Rust + rfd)     | ImGui UI + native (rfd) backends |

## History

Release Train 0.3 (current)

- **BREAKING**: Main crate renamed from `dear-imgui` to `dear-imgui-rs` (v0.3.0)
- **All crates unified to 0.3.0**: Complete version alignment across the entire workspace
  - Core: dear-imgui-rs 0.3.0, dear-imgui-sys 0.3.0
  - Backends: dear-imgui-wgpu 0.3.0, dear-imgui-glow 0.3.0, dear-imgui-winit 0.3.0
  - Extensions: dear-implot 0.3.0, dear-imnodes 0.3.0, dear-imguizmo 0.3.0
  - Sys crates: dear-implot-sys 0.3.0, dear-imnodes-sys 0.3.0, dear-imguizmo-sys 0.3.0
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

