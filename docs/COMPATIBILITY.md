# Compatibility Matrix

This document tracks compatibility across the workspace crates, upstream Dear ImGui, and key external dependencies. The root README shows the latest recommendations; this file keeps history.

## Latest (Summary)

Core

| Crate          | Version | Upstream        | Notes                                     |
|----------------|---------|-----------------|-------------------------------------------|
| dear-imgui     | 0.2.x   | —               | Safe Rust API over dear-imgui-sys         |
| dear-imgui-sys | 0.2.x   | ImGui v1.92.3   | Docking branch via cimgui                 |

Backends

| Crate            | Version | External deps      | Notes |
|------------------|---------|--------------------|-------|
| dear-imgui-wgpu  | 0.2.x   | wgpu = 26          |       |
| dear-imgui-glow  | 0.2.x   | glow = 0.16        |       |
| dear-imgui-winit | 0.2.x   | winit = 0.30.12    |       |

Extensions

| Crate         | Version | Requires dear-imgui | Sys crate            | Notes |
|---------------|---------|---------------------|----------------------|-------|
| dear-implot   | 0.2.x   | 0.2.x               | dear-implot-sys 0.2.x   |     |
| dear-imnodes  | 0.1.x   | 0.2.x               | dear-imnodes-sys 0.1.x   |     |
| dear-imguizmo | 0.1.x   | 0.2.x               | dear-imguizmo-sys 0.1.x  |     |

## History

Release Train 0.2 (current)

- dear-imgui-sys 0.2.x binds Dear ImGui v1.92.3 (docking) via cimgui.
- dear-imgui 0.2.x layers a safe API over the 0.2.x sys crate.
- Backends: wgpu (26), winit (0.30.12), glow (0.16).
- Extensions: dear-implot 0.2.x (with -sys 0.2.x), dear-imnodes 0.1.x, dear-imguizmo 0.1.x — all depend on dear-imgui/dear-imgui-sys 0.2.x.

## Upgrade Guidelines

- When bumping dear-imgui-sys to a new upstream ImGui, bump all -sys extensions (implot/imnodes/imguizmo) in lockstep and verify bindgen output/ABI.
- When bumping dear-imgui, check backends/extensions for API surface changes and update versions accordingly.
- Backend external deps (wgpu/winit/glow) often introduce breaking changes; track and bump backend crates even if core didn’t change.

