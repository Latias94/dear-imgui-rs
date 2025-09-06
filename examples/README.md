# Dear ImGui Examples

This directory contains examples demonstrating how to use the Dear ImGui Rust bindings with various backends.

## Examples Overview

### Basic Integration
- **`hello_world.rs`** - Clean, minimal example showing Dear ImGui integration with WGPU and Winit
- **`hello_world_wgpu.rs`** - Extended example with comprehensive feature testing

### Feature Demonstrations
- **`draw_demo.rs`** - Showcase of drawing capabilities including:
  - Basic shapes (lines, rectangles, circles)
  - Bezier curves and polylines
  - Path drawing functions
  - Interactive drawing controls

- **`widgets_demo.rs`** - Comprehensive widget showcase including:
  - Input widgets (text, sliders, checkboxes)
  - Display widgets (buttons, radio buttons, combos)
  - Color editors and style customization
  - System information display

### Advanced Features
- **`drag_drop_demo.rs`** - Drag and drop functionality demonstration

## Running Examples

To run any example, use:

```bash
cargo run --bin <example_name>
```

For example:
```bash
cargo run --bin hello_world
cargo run --bin draw_demo
cargo run --bin widgets_demo
```

Or from the repository root:
```bash
cargo run -p dear-imgui-examples --bin hello_world
```

### Controls

- **ESC**: Exit the application
- **Click me! button**: Increments a counter
- **Text input**: Edit text in the input field
- **Slider**: Adjust a floating-point value
- **Show Demo Window checkbox**: Toggle the demo window (not yet implemented)

### Code Structure

The example follows a typical Dear ImGui application structure:

1. **Initialization**: Set up winit event loop, WGPU device, and Dear ImGui context
2. **Platform Integration**: Configure the winit platform backend for input handling
3. **Renderer Setup**: Initialize the WGPU renderer for Dear ImGui
4. **Main Loop**: Handle events, update UI, and render frames
5. **Cleanup**: Automatic cleanup when the application exits

### Current Limitations

This example demonstrates the basic structure, but some features are not yet fully implemented in the dear-imgui library:

- Font configuration and custom fonts
- Demo window display
- Mouse cursor handling
- Color editing widgets
- Advanced input/output features

These limitations are due to the early stage of the dear-imgui library development. The example serves as a foundation that will be enhanced as more features are implemented.

### Dependencies

The example uses the following key dependencies:

- `dear-imgui`: The main Dear ImGui Rust bindings
- `dear-imgui-winit`: Winit platform backend
- `dear-imgui-wgpu`: WGPU rendering backend
- `winit`: Cross-platform windowing
- `wgpu`: Cross-platform graphics API
- `pollster`: Async runtime utilities
- `env_logger`: Logging support

### Architecture

The example demonstrates a clean separation of concerns:

- **App**: Main application state and event handling
- **AppWindow**: Window and graphics context management
- **ImguiState**: Dear ImGui context and rendering state

This architecture makes it easy to extend the example with additional features and integrate it into larger applications.

## Adding More Examples

To add new examples:

1. Create a new `.rs` file in the `examples/` directory
2. Add a `[[bin]]` entry in `examples/Cargo.toml`
3. Follow the same basic structure as the hello world example
4. Document the example in this README

## Building and Development

The examples are built as part of the workspace. To build all examples:

```bash
cargo build -p dear-imgui-examples
```

To check for compilation errors:

```bash
cargo check -p dear-imgui-examples
```

## Platform Support

The examples are designed to work on all platforms supported by winit and wgpu:

- **Windows**: Full support
- **macOS**: Full support  
- **Linux**: Full support
- **Web (WASM)**: Potential support (not yet tested)

## Contributing

When contributing new examples:

1. Follow the existing code style and structure
2. Include comprehensive comments explaining key concepts
3. Update this README with information about your example
4. Test on multiple platforms if possible
5. Keep examples focused on demonstrating specific features

## License

These examples are provided under the same license as the dear-imgui project (MIT OR Apache-2.0).
