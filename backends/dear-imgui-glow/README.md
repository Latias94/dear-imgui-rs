# dear-imgui-glow

Glow (OpenGL) renderer for Dear ImGui.

This crate provides a Glow-based renderer for Dear ImGui, allowing you to render Dear ImGui interfaces using the Glow OpenGL abstraction.

## Compatibility

| Item       | Version |
|------------|---------|
| Crate      | 0.2.x   |
| dear-imgui | 0.2.x   |
| glow       | 0.16    |

See also: [docs/COMPATIBILITY.md](../../docs/COMPATIBILITY.md) for the full workspace matrix.

## Features

- **Basic rendering**: Render Dear ImGui draw data using OpenGL
- **Texture support**: Handle font textures and user textures  
- **Multi-viewport support**: Support for multiple windows (feature-gated)
- **OpenGL compatibility**: Support for OpenGL 2.1+ and OpenGL ES 2.0+
- **Feature-gated optimizations**: Optional support for various OpenGL features

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
dear-imgui = "0.2"
dear-imgui-glow = "0.2"
```

### Basic Example

```rust
use dear_imgui::Context;
use dear_imgui_glow::GlowRenderer;
use glow::HasContext;

// Initialize your OpenGL context and Dear ImGui context
let gl = glow::Context::from_loader_function(|s| {
    // Your OpenGL loader function
    load_gl_function(s)
});
let mut imgui = Context::create();

// Create the renderer
let mut renderer = GlowRenderer::new(gl, &mut imgui)?;

// In your render loop:
imgui.new_frame();
// ... build your UI ...
let draw_data = imgui.render();
renderer.render(draw_data)?;
```

### Advanced Usage

For more control, you can use the `Renderer` directly:

```rust
use dear_imgui_glow::{Renderer, SimpleTextureMap};

let mut texture_map = SimpleTextureMap::new();
let mut renderer = Renderer::new(&gl, &mut imgui, &mut texture_map)?;

// In your render loop:
renderer.render(&gl, &texture_map, draw_data)?;
```

## Features

The following features are available:

- `docking`: Enable docking features (requires Dear ImGui docking branch)
- `multi-viewport`: Enable multi-viewport support
- `gl_extensions_support`: Enable checking for OpenGL extensions
- `debug_message_insert_support`: Support for `gl.debug_message_insert`
- `bind_vertex_array_support`: Support for `glBindVertexArray`
- `vertex_offset_support`: Support for `glDrawElementsBaseVertex`
- `clip_origin_support`: Support for `GL_CLIP_ORIGIN`
- `bind_sampler_support`: Support for `glBindSampler`
- `polygon_mode_support`: Support for `glPolygonMode`
- `primitive_restart_support`: Support for `GL_PRIMITIVE_RESTART`

All features except `docking` and `multi-viewport` are enabled by default.

## OpenGL Compatibility

This renderer supports:

- **Desktop OpenGL**: 2.1+ (with full feature support on 3.2+)
- **OpenGL ES**: 2.0+ (with full feature support on 3.0+)
- **WebGL**: 1.0 and 2.0

Feature support is automatically detected based on the OpenGL version and available extensions.

## Multi-Viewport Support

Multi-viewport support allows Dear ImGui to create additional windows outside the main application window. To enable this:

1. Enable the `multi-viewport` feature
2. Enable multi-viewport in Dear ImGui: `io.config_flags |= ConfigFlags::VIEWPORTS_ENABLE`
3. Handle platform events for additional windows

## Implementation Status

✅ **Completed Features:**
- **Core Rendering**: Complete OpenGL renderer with glow backend
- **Font Management**: Font atlas texture creation and management
- **Buffer Management**: Vertex and index buffer handling with dynamic sizing
- **State Management**: Complete OpenGL state backup and restoration
- **Shader System**: Dynamic shader compilation with version detection
- **Texture System**: Texture mapping and ImGui texture update support
- **Device Objects**: Proper creation/destruction of OpenGL resources
- **Frame Management**: NewFrame functionality for resource recreation
- **Multi-viewport**: Basic multi-viewport callback structure (feature-gated)
- **Version Support**: OpenGL 2.1+, ES 2.0+, ES 3.0+ compatibility
- **Feature Detection**: Runtime OpenGL capability detection
- **Projection Matrix**: Proper orthographic projection with clip origin support
- **GlowRenderer**: Simplified high-level renderer interface

🚧 **In Progress:**
- Complete multi-viewport implementation with context management
- Example applications and tutorials
- Performance optimizations

📋 **TODO:**
- Add comprehensive examples and documentation
- Implement texture atlas updates for dynamic fonts
- Add more OpenGL state optimizations
- Performance profiling and improvements
- Integration tests

## Architecture

The crate is structured as follows:

- `renderer.rs` - Main renderer implementation (`Renderer` and `GlowRenderer`)
- `shaders.rs` - Shader compilation and management
- `state.rs` - OpenGL state backup/restoration
- `texture.rs` - Texture management and mapping
- `versions.rs` - OpenGL version detection and feature support
- `error.rs` - Error types and handling

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](../../LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](../../LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
