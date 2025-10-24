# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.5.0] - 2025-10-24

Upgrade to Dear ImGui v1.92.4 (docking branch) with new color styling option and bug fixes.

### Added

- dear-imgui-rs 0.5.0
  - New `StyleColor::UnsavedMarker` color for marking unsaved documents/windows
  - This color is used by Dear ImGui to indicate unsaved state in tabs and windows

- dear-imgui-sys 0.5.0
  - Updated to Dear ImGui v1.92.4 (docking branch)
  - Updated cimgui submodule to v1.92.4dock (commit 2d91c9d)
  - Regenerated FFI bindings with new ImGuiCol_UnsavedMarker constant

### Changed

- Updated all documentation references from v1.92.3 to v1.92.4
- ImGuiCol_COUNT increased from 60 to 61 due to new color addition
- dear-imgui-winit: Map extra mouse buttons
  - `winit::event::MouseButton::Back/Forward` and common `Other(3)/Other(4)` are now mapped to `ImGuiMouseButton::Extra1/Extra2`
  - Improves out-of-the-box support for side buttons on modern mice; no API changes

### Fixed

- Inherited all bug fixes from Dear ImGui v1.92.4:
  - InputText: Fixed single-line character clipping regression from v1.92.3
  - InputText: Fixed potential infinite loop in callback handling
  - Improved texture lifecycle management
  - Fixed multi-context ImFontAtlas sharing issues
- dear-imgui-winit: Stabilized tests that create an ImGui context by serializing them to avoid spurious `ContextAlreadyActive` panics (internal, no runtime impact)

### Version Updates

**All crates in the workspace have been upgraded to 0.5.0** due to the Dear ImGui v1.92.4 upgrade.

**Core:**
- `dear-imgui-sys` → 0.5.0
- `dear-imgui-rs` → 0.5.0

**Backends:**
- `dear-imgui-wgpu` → 0.5.0
- `dear-imgui-glow` → 0.5.0
- `dear-imgui-winit` → 0.5.0

**Application Framework:**
- `dear-app` → 0.5.0

**Extensions:**
- `dear-imnodes` → 0.5.0 (+ `dear-imnodes-sys` → 0.5.0)
- `dear-implot` → 0.5.0 (+ `dear-implot-sys` → 0.5.0)
- `dear-implot3d` → 0.5.0 (+ `dear-implot3d-sys` → 0.5.0)
- `dear-imguizmo` → 0.5.0 (+ `dear-imguizmo-sys` → 0.5.0)
- `dear-imguizmo-quat` → 0.5.0 (+ `dear-imguizmo-quat-sys` → 0.5.0)
- `dear-file-browser` → 0.5.0

## [0.4.1] - 2025-10-07

Small, focused improvements to enable real-time texture workflows (game view, atlas tools, image browsers) without frame delay.

### Added

- dear-imgui-wgpu 0.4.1
  - External texture APIs for real-time usage:
    - `WgpuRenderer::register_external_texture(&Texture, &TextureView) -> u64`
    - `WgpuRenderer::update_external_texture_view(id, &TextureView) -> bool`
    - `WgpuRenderer::unregister_texture(id)`
  - These allow displaying existing `wgpu::Texture` resources via legacy `TextureId` in the same frame (no reliance on TextureData state machine), ideal for game views/RTTs or dynamic atlases.

- dear-app 0.4.1
  - New `AddOns.gpu` API exposing:
    - `device()` / `queue()` passthroughs
    - `register_texture`, `update_texture_view`, `unregister_texture`
    - `update_texture_data(&mut TextureData)` that applies the backend result to set `TexID/Status` immediately (no white frame).
  - New example `examples/01-renderers/dear_app_wgpu_textures.rs` showcasing both managed `TextureData` updates and external WGPU textures in real time.

### Changed

- Examples now include a dear-app + wgpu textures demo exhibiting same-frame updates and game-view style external texture display.

### Version Updates

- `dear-imgui-wgpu` → 0.4.1
- `dear-app` → 0.4.1

## [0.4.0] - 2025-10-07

This is a major feature release that introduces several new extensions, improves the docking API, and adds a convenient application runner.

### 🎉 New Features

#### New Extensions

- **dear-app** - A convenient application runner built on Winit + WGPU
  - Provides easy-to-use application framework with docking support
  - Built-in theme support and add-ons integration
  - Simplifies the setup process for new projects
  - See examples: `dear_app_quickstart.rs`, `dear_app_docking.rs`

- **dear-implot3d** - 3D plotting extension
  - Full Rust bindings for ImPlot3D (cimplot3d C API)
  - Support for 3D scatter plots, line plots, surface plots, mesh plots
  - Triangle and quad rendering capabilities
  - 3D image display support
  - Comprehensive style customization
  - Example: `implot3d_basic.rs`

- **dear-imguizmo-quat** - Quaternion-based 3D gizmo
  - Full Rust bindings for ImGuIZMO.quat (cimguizmo_quat C API)
  - Quaternion manipulation widgets
  - 3D direction and rotation controls
  - Example: `imguizmo_quat_basic.rs`

- **dear-file-browser** - File browser and dialog extension
  - Native OS file dialogs via `rfd` backend
  - Pure ImGui in-UI file browser implementation
  - Support for file/folder selection, save dialogs
  - Customizable file filters and multi-selection
  - Examples: `file_dialog_native.rs`, `file_browser_imgui.rs`

#### Core Improvements

- **Safe DockBuilder API** ([#14d96cf](https://github.com/Latias94/dear-imgui-rs/commit/14d96cf2f527d978a23c793e84d34d80cd8c6a5f))
  - Added `Ui::set_next_window_viewport()` and `Ui::get_id()` helper methods
  - Introduced `DockNode<'ui>` with read-only queries and `NodeRect` type
  - Added safe methods: `DockBuilder::node()`, `central_node()`, `node_exists()`
  - Removed unsafe methods: `DockBuilder::get_node()`, `get_central_node()`
  - Updated docking examples to use safe APIs

- **Enhanced Docking Support**
  - Fixed docking split node function for more reliable layout management
  - Improved game engine docking example with better UI organization
  - Updated dockspace minimal example with safe API usage

### 🔧 Improvements

#### Dependencies

- **Updated wgpu to v27** - Latest WGPU version with improved performance and features
- Updated workspace to use Rust edition 2024

#### Build System

- Added prebuilt binary packaging support for `dear-imguizmo-quat-sys`
- Improved CI workflow for prebuilt binaries
- Added cargo clippy checks to CI pipeline
- Optimized cargo exclude patterns for smaller package sizes

#### Documentation

- Comprehensive README updates for all new extensions
- Updated compatibility matrix in `docs/COMPATIBILITY.md`
- Added detailed usage examples for new features
- Improved build instructions and feature flag documentation

### 📦 Version Updates

#### Core Packages (0.4.0)
- `dear-imgui-rs` → 0.4.0
- `dear-imgui-sys` → 0.4.0
- `dear-imgui-wgpu` → 0.4.0
- `dear-imgui-glow` → 0.4.0
- `dear-imgui-winit` → 0.4.0

#### Application Runner (0.4.0)
- `dear-app` → 0.4.0 (new)

#### Extensions (0.4.0)
- `dear-implot` → 0.4.0
- `dear-implot-sys` → 0.4.0
- `dear-imnodes` → 0.4.0
- `dear-imnodes-sys` → 0.4.0
- `dear-imguizmo` → 0.4.0
- `dear-imguizmo-sys` → 0.4.0
- `dear-implot3d` → 0.4.0 (new)
- `dear-implot3d-sys` → 0.4.0 (new)
- `dear-imguizmo-quat` → 0.4.0 (new)
- `dear-imguizmo-quat-sys` → 0.4.0 (new)
- `dear-file-browser` → 0.4.0 (new)

### 📚 Examples

New examples added:
- `dear_app_quickstart.rs` - Quick start guide using dear-app
- `dear_app_docking.rs` - Docking example using dear-app
- `implot3d_basic.rs` - Comprehensive 3D plotting demo
- `imguizmo_quat_basic.rs` - Quaternion gizmo demonstration
- `file_dialog_native.rs` - Native file dialog usage
- `file_browser_imgui.rs` - ImGui file browser UI

Updated examples:
- `game_engine_docking.rs` - Significantly improved with better layout and features
- `dockspace_minimal.rs` - Rewritten to use safe DockBuilder APIs
- `tables_minimal.rs` - Minor improvements

### ⚠️ Breaking Changes

- **DockBuilder API**: Removed unsafe methods `get_node()` and `get_central_node()`. Use the new safe alternatives: `node()` and `central_node()`
- **Docking Split API**: Updated signature for split node functions to be more type-safe

### 🔮 Experimental

- Multi-viewport support is still work-in-progress and not production-ready
  - Test example available: `cargo run --bin multi_viewport_wgpu --features multi-viewport`
  - This feature is excluded from this release as it's not yet complete

### 📖 Migration Guide

#### Updating DockBuilder Usage

**Before (v0.3.0):**
```rust
unsafe {
    let node = DockBuilder::get_node(dock_id);
    let central = DockBuilder::get_central_node(dock_id);
}
```

**After (v0.4.0):**
```rust
if let Some(node) = DockBuilder::node(ui, dock_id) {
    // Use node safely
}
if let Some(central) = DockBuilder::central_node(ui, dock_id) {
    // Use central node safely
}
```

#### Using the New dear-app Runner

**Before (manual setup):**
```rust
// Manual Winit + WGPU setup code...
```

**After (with dear-app):**
```rust
use dear_app::*;

fn main() {
    App::new("My App")
        .run(|ui| {
            ui.window("Hello").build(|| {
                ui.text("Hello, world!");
            });
        });
}
```

### 🙏 Acknowledgments

Special thanks to all contributors and the upstream projects:
- Dear ImGui by Omar Cornut
- ImPlot3D for 3D plotting capabilities
- ImGuIZMO.quat for quaternion manipulation
- rfd for native file dialogs

**Full Changelog**: https://github.com/Latias94/dear-imgui-rs/compare/v0.3.0...v0.4.0

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
