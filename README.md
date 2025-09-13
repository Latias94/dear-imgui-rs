# Dear ImGui Rust Bindings

Modern Rust bindings for [Dear ImGui](https://github.com/ocornut/imgui) with full Dear ImGui v1.92+ support, including dynamic font system, advanced docking, and modern texture management.

![Game Engine Docking Example](screenshots/game-engine-docking.png)
![Implot Example](screenshots/implot-basic.png)

## Overview

This project provides comprehensive Rust bindings for Dear ImGui v1.92+, featuring:

- **Dear ImGui v1.92+ Support**: Full support for the latest Dear ImGui features including dynamic font loading, `ImGuiBackendFlags_RendererHasTextures`, and modern texture management
- **Dynamic Font System**: On-demand glyph loading, runtime font size adjustment, and custom font loaders (FreeType support)
- **Advanced Docking**: Complete window docking and layout management with DockSpace API
- **Modern Backends**: WGPU v26, OpenGL via glow, with automatic texture lifecycle management
- **Type Safety**: Memory-safe Rust wrappers with zero-cost abstractions
- **Cross-Platform**: Windows, Linux, and macOS support

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
├── dear-imgui-sys/      # Low-level FFI bindings to Dear ImGui C++
├── backends/
│   ├── dear-imgui-wgpu/    # WGPU renderer backend
│   ├── dear-imgui-glow/    # OpenGL renderer backend
│   └── dear-imgui-winit/   # Winit platform integration
└── extensions/
    ├── dear-imguizmo/      # 3D gizmo manipulation (Work in progress)
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

Direct C++ FFI bindings with MSVC ABI compatibility fixes, handling complex return types and ensuring cross-platform stability.

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
- Direct C++ FFI without cimgui dependency
- Comprehensive MSVC ABI compatibility

**Current Status:**

- Production-ready for most use cases
- Active development with regular updates
- Growing ecosystem of extensions

Choose this library if you need the latest Dear ImGui features, docking support, or want to work with modern Rust graphics libraries.

## Technical Details

### MSVC ABI Compatibility

One of the key technical challenges in creating Rust bindings for C++ libraries is handling ABI (Application Binary Interface) compatibility issues. This is particularly problematic on MSVC where small C++ class return types can cause crashes.

Our solution, inspired by [easy-imgui-rs](https://github.com/rodrigorc/easy-imgui-rs/), includes:

1. **FFI-Safe Wrapper Types**: Convert C++ types like `ImVec2` to POD equivalents
2. **C Wrapper Functions**: Provide C-compatible interfaces for problematic functions
3. **Conditional Compilation**: Apply fixes only where needed (MSVC targets)
4. **Selective Function Blocking**: Block problematic functions during bindgen and provide manual implementations

See [`dear-imgui-sys/README.md`](dear-imgui-sys/README.md) for detailed technical information.
