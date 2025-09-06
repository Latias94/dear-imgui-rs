# Dear ImGui Examples

This directory contains examples demonstrating how to use the dear-imgui Rust bindings with various backends.

## Hello World WGPU Example

The `hello_world_wgpu.rs` example demonstrates how to use dear-imgui with winit and wgpu for cross-platform GUI applications.

### Features Demonstrated

- **Window Management**: Creating and managing windows with winit
- **WGPU Rendering**: Using WGPU as the graphics backend for Dear ImGui
- **Basic Widgets**: Text, buttons, input fields, sliders, and checkboxes
- **Event Handling**: Processing keyboard and mouse events through winit
- **Frame Rate Display**: Showing frametime and FPS information

### Running the Example

To run the hello world WGPU example:

```bash
# From the repository root
cargo run -p dear-imgui-examples --bin hello_world_wgpu
```

Or from the examples directory:

```bash
# From the examples directory
cargo run --bin hello_world_wgpu
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
