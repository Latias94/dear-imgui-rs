# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
# Changelog



### Chore

- update image url (5b2ebfc…)


### Features

- add dear-file-browser (72a7a06…)

- safe DockBuilder node API + viewport/id helpers; update docking demo [imgui] (14d96cf…)

- add imguizmo.quat, cargo fmt (b9b1dc2…)

- add dear-app, optimize cargo exclude, add package bin for dear-imguizmo-quat-sys (16b1ee5…)

- fix implot3d bugs and fix examples, add readme (2aa7a25…)


### Fixes

- cargo clippy and add ci (a69ff25…)

- fix docking split node function [imgui] (5e4f303…)

- fix multi viewport example feature (7eef1bc…)

- fix example features (082581e…)


### Doc

- add readme (8515569…)

- modify readme (e9d36ed…)



## [0.3.0] - 2025-09-30

### Changed

- **BREAKING**: Renamed main crate from `dear-imgui` to `dear-imgui-rs` following feedback from Dear ImGui maintainer
  - Update your `Cargo.toml` dependencies from `dear-imgui = "0.2"` to `dear-imgui-rs = "0.3"`
  - Update all `use dear_imgui::*` imports to `use dear_imgui_rs::*`
  - The old `dear-imgui` crate (v0.2.0) has been yanked on crates.io
- Updated all backend crates to version 0.3.0 to match the new naming
  - `dear-imgui-wgpu` 0.3.0
  - `dear-imgui-glow` 0.3.0
  - `dear-imgui-winit` 0.3.0
- Updated extension crates to depend on `dear-imgui-rs` 0.3
  - `dear-implot` 0.3.0
  - `dear-imnodes` 0.2.0
  - `dear-imguizmo` 0.2.0

### Migration Guide

To migrate from `dear-imgui` 0.2.x to `dear-imgui-rs` 0.3.x:

1. Update your `Cargo.toml`:
   ```toml
   # Before
   dear-imgui = "0.2"

   # After
   dear-imgui-rs = "0.3"
   ```

2. Update your imports:
   ```rust
   // Before
   use dear_imgui::*;

   // After
   use dear_imgui_rs::*;
   ```

3. Update backend dependencies if you use them:
   ```toml
   dear-imgui-wgpu = "0.3"
   dear-imgui-glow = "0.3"
   dear-imgui-winit = "0.3"
   ```

No API changes were made - only the crate name changed.

## [0.1.0] - 2025-09-13

### Added
- Initial release of dear-imgui Rust bindings with docking support
- Support for Dear ImGui v1.92 features
- Backend support for winit, wgpu, and glow
- Extension support for implot

### Features
- Core dear-imgui bindings with safe Rust API
- Docking support (enabled by default)
- Comprehensive backend ecosystem

### Crates
- `dear-imgui-sys`: Low-level FFI bindings
- `dear-imgui`: High-level safe Rust API
- `dear-imgui-winit`: Winit backend integration
- `dear-imgui-wgpu`: WGPU renderer backend
- `dear-imgui-glow`: OpenGL/GLOW renderer backend
- `dear-implot-sys`: ImPlot FFI bindings
- `dear-implot`: ImPlot Rust API
