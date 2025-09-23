# Dear ImGui Rust Bindings

Modern Rust bindings for [Dear ImGui](https://github.com/ocornut/imgui) with full Dear ImGui v1.92+ support, including dynamic font system, advanced docking, and modern texture management.

![Game Engine Docking Example](screenshots/game-engine-docking.png)
![Implot Example](screenshots/implot-basic.png)
![Imguizmo Example](screenshots/imguizmo-basic.png)

## Overview

This project provides comprehensive Rust bindings for Dear ImGui v1.92+, featuring:

- **Dear ImGui v1.92+ Support**: Full support for the latest Dear ImGui features including dynamic font loading, `ImGuiBackendFlags_RendererHasTextures`, and modern texture management
- **Dynamic Font System**: On-demand glyph loading, runtime font size adjustment, and custom font loaders (FreeType support)
- **Advanced Docking**: Complete window docking and layout management with DockSpace API
- **Modern Backends**: WGPU v26, OpenGL via glow, with automatic texture lifecycle management
- **Type Safety**: Memory-safe Rust wrappers with zero-cost abstractions
- **Cross-Platform**: Windows, Linux, macOS support

**Note**: Multi-viewport support is not currently implemented but may be added in future releases.

## Key Features

- **Complete Widget Set**: All Dear ImGui widgets with type-safe Rust APIs
- **Advanced Tables**: Sorting, filtering, resizing, and custom rendering
- **Custom Drawing**: Full DrawList API for custom graphics
- **Comprehensive Styling**: Theme system with runtime style modifications
- **Input Handling**: Keyboard, mouse, gamepad with callback support

## Crates

```text
dear-imgui/
├── dear-imgui/          # High-level safe Rust bindings
├── dear-imgui-sys/      # Low-level FFI bindings via cimgui (C API)
├── backends/
│   ├── dear-imgui-wgpu/    # WGPU renderer backend
│   ├── dear-imgui-glow/    # OpenGL renderer backend
│   └── dear-imgui-winit/   # Winit platform integration
└── extensions/
    ├── dear-imguizmo/      # 3D gizmo manipulation 
    └── dear-implot/        # Advanced plotting library
```

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
dear-imgui = "0.1"
dear-imgui-wgpu = "0.1"
dear-imgui-winit = "0.1"
```

Basic WGPU example:

```rust
use dear_imgui::*;
use dear_imgui_wgpu::{WgpuRenderer, WgpuInitInfo};

let mut imgui = Context::create()?;
let mut renderer = WgpuRenderer::new(init_info, &mut imgui)?;

// Main loop
let ui = imgui.frame();
ui.window("Hello World")
    .size([300.0, 100.0], Condition::FirstUseEver)
    .build(|| {
        ui.text("Hello, world!");
        if ui.button("Click me") {
            println!("Button clicked!");
        }
    });

let draw_data = imgui.render();
renderer.render(&draw_data, &render_pass);
```

## Examples

Run the included examples to see features in action:

```bash
cargo run --bin wgpu_basic           # Basic WGPU integration
cargo run --bin glow_basic           # Basic Glow integration
cargo run --bin game_engine_docking  # Advanced docking layout
cargo run --bin implot_basic         # Plotting with dear-implot
```

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
dear-imgui = "0.1"
dear-imgui-wgpu = "0.1"  # For WGPU backend
dear-imgui-winit = "0.1" # For Winit integration
```

Or for OpenGL support:

```toml
[dependencies]
dear-imgui = "0.1"
dear-imgui-glow = "0.1"  # For OpenGL backend
dear-imgui-winit = "0.1" # For Winit integration
```

## Architecture

### Modern Texture Management

This library implements Dear ImGui v1.92+'s `ImGuiBackendFlags_RendererHasTextures` system, enabling:

- Automatic texture lifecycle management
- Dynamic font atlas resizing
- Backend-agnostic texture operations
- Memory-efficient texture streaming

### FFI Layer (`dear-imgui-sys`)

- Based on cimgui (C API) for Dear ImGui (docking branch)
- Bindings are generated with bindgen from vendored headers
- Cross-platform friendly（no C++ ABI/MSVC quirks）
- Windows native builds prefer CMake (auto-detects VS/SDK); prebuilt static libraries are supported

### Safe Layer (`dear-imgui`)

Type-safe Rust API featuring:

- Builder patterns for complex widgets
- RAII resource management
- Zero-cost abstractions over C++ types
- Comprehensive error handling

## Acknowledgments

This project builds upon the excellent work of several other projects:

- **[Dear ImGui](https://github.com/ocornut/imgui)** by Omar Cornut - The original C++ immediate mode GUI library
- **[imgui-rs](https://github.com/imgui-rs/imgui-rs)** - Provided the API design patterns and inspiration for the Rust binding approach
- **[easy-imgui-rs](https://github.com/rodrigorc/easy-imgui-rs/)** by rodrigorc - Demonstrated solutions for C++ ABI compatibility issues and MSVC fixes that we adapted for our FFI layer
- **[imgui-wgpu-rs](https://github.com/Yatekii/imgui-wgpu-rs/)** - Provided reference implementation for WGPU backend integration

## License

This project is licensed under MIT OR Apache-2.0.

- Apache License, Version 2.0 (<http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license (<http://opensource.org/licenses/MIT>)

### Development Setup

```bash
git clone https://github.com/Latias94/dear-imgui
cd dear-imgui
git submodule update --init --recursive
cargo build
cargo test
cargo run --example wgpu_basic
```

## Comparison with imgui-rs

**Advantages:**

- Dear ImGui v1.92+ support with dynamic fonts and modern texture management
- Built-in docking support
- Modern Rust ecosystem integration (wgpu v26, winit v0.30+)
- Uses cimgui C API (avoids C++ ABI issues), simpler cross-platform builds

**Current Status:**

- Production-ready for most use cases
- Active development with regular updates
- Growing ecosystem of extensions

Choose this library if you need the latest Dear ImGui features, docking support, or want to work with modern Rust graphics libraries.

## Build & Packaging (sys)

`dear-imgui-sys` uses cimgui + bindgen and supports multiple link strategies:

- Prebuilt static library（recommended）
  - Set `IMGUI_SYS_LIB_DIR=...` to a folder containing the static lib
    - Windows: `dear_imgui.lib`
    - Linux/macOS: `libdear_imgui.a`
  - Or set `IMGUI_SYS_PREBUILT_URL=...` to a direct URL of the static lib
  - We publish platform archives in Releases (include + static lib)

- Native build from source
  - Windows prefers CMake (auto-detects VS/SDK); set `IMGUI_SYS_USE_CMAKE=1` to force CMake elsewhere
  - Otherwise falls back to cc crate

- Fast Rust-only checks
  - Set `IMGUI_SYS_SKIP_CC=1` to skip native C/C++ compilation while iterating

Docs.rs
- On docs.rs we generate bindings offline from vendored headers or use `src/bindings_pregenerated.rs` if present.

Examples
- `cargo run -p dear-imgui-examples --bin wgpu_basic`
- `cargo run -p dear-imgui-examples --bin game_engine_docking`

Prebuilt packages
- Naming: `dear-imgui-prebuilt-{version}-{target}-static[-{md|mt}].tar.gz`
  - Example (Windows MD): `dear-imgui-prebuilt-0.1.0-x86_64-pc-windows-msvc-static-md.tar.gz`
  - Example (Linux): `dear-imgui-prebuilt-0.1.0-x86_64-unknown-linux-gnu-static.tar.gz`
- Contents:
  - `include/imgui/*` (Dear ImGui headers)
  - `include/cimgui/cimgui.h`
  - `lib/dear_imgui.lib` (Windows) or `lib/libdear_imgui.a` (Linux/macOS)
- Use:
  - Extract somewhere and set `IMGUI_SYS_LIB_DIR` to the folder containing the static library
  - Or host the static library and set `IMGUI_SYS_PREBUILT_URL` to its direct URL

