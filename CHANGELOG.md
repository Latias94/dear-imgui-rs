# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Breaking
- Image APIs now accept `impl Into<TextureRef>` instead of `TextureId` only:
  - `Ui::image`, `Ui::image_button`, `Ui::image_config`, `Ui::image_button_config`
  - DrawList: added `push_texture/pop_texture` and `add_image*` which accept `Into<TextureRef>`
  - This aligns with Dear ImGui 1.92 `ImTextureRef` design and simplifies using managed textures.

### Added
- `TextureRef` wrapper for `ImTextureRef` with conversions from `TextureId` and `&mut TextureData`.
- `Ui::image_with_bg()` convenience for 1.92 `ImageWithBg`.
- `DrawListMut::{push_texture, pop_texture, add_image, add_image_quad, add_image_rounded}`.
- Rich style accessors on `Style` for 1.92 fields (font scales, paddings, tab metrics, hover delays, etc.).

### Changed
- `Io::{font_global_scale, set_font_global_scale}` now map to `style.FontScaleMain` (1.92).

### Fixed
- `TextureData::set_data()` now allocates/copies safely, sets `UpdateRect` and `WantUpdates` (no dangling pointers).

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
