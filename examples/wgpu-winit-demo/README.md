# Dear ImGui WGPU + Winit Demo

A comprehensive demonstration of Dear ImGui running with WGPU backend and Winit windowing, supporting both native and WebAssembly targets.

## Features

- **Dear ImGui 1.92+**: Latest Dear ImGui with full API support
- **WGPU Backend**: Modern graphics API with WebGPU/WebGL support
- **Winit Integration**: Cross-platform windowing and event handling
- **WebAssembly Support**: Runs in web browsers with full functionality
- **Cross-Platform**: Works on Windows, macOS, Linux, and Web

## Architecture

This demo showcases the integration of three key components:

1. **Dear ImGui**: Immediate mode GUI library
2. **WGPU**: Modern graphics API abstraction
3. **Winit**: Cross-platform windowing library

The architecture follows the pattern established by learn-wgpu-zh tutorials, providing a solid foundation for building cross-platform applications.

## Building and Running

### Prerequisites

- Rust 1.70+ with `wasm32-unknown-unknown` target
- For WASM: `wasm-bindgen-cli` and optionally `wasm-opt` (from binaryen)

```bash
# Install WASM target
rustup target add wasm32-unknown-unknown

# Install wasm-bindgen-cli
cargo install wasm-bindgen-cli

# Optional: Install binaryen for WASM optimization
# On macOS: brew install binaryen
# On Ubuntu: apt install binaryen
# On Windows: Download from https://github.com/WebAssembly/binaryen/releases
```

### Native Build

```bash
# Run directly
cargo run

# Or build and run
cargo build --release
./target/release/wgpu-winit-demo
```

### WASM Build

```bash
# Build for WASM
./build-wasm.sh

# Serve the demo
cd dist
./serve.sh

# Open http://localhost:8000 in your browser
```

## Project Structure

```
wgpu-winit-demo/
├── src/
│   └── main.rs              # Main application code
├── Cargo.toml               # Project configuration
├── index.html               # HTML template for WASM
├── build-wasm.sh           # WASM build script
├── README.md               # This file
└── dist/                   # Generated WASM files (after build)
    ├── wgpu-winit-demo.js
    ├── wgpu-winit-demo_bg.wasm
    ├── index.html
    └── serve.sh
```

## Key Components

### Application Structure

The demo follows a clean architecture pattern:

- **ImGuiWgpuApp**: Main application state and rendering logic
- **ImGuiWgpuAppHandler**: Winit event handling and application lifecycle
- **WGPU Integration**: Surface management and rendering pipeline
- **Dear ImGui Integration**: UI rendering and event processing

### WASM Considerations

The demo handles WASM-specific requirements:

- **Async Initialization**: WGPU device creation is async in WASM
- **Canvas Integration**: Automatic canvas creation and DOM insertion
- **Event Handling**: Proper event forwarding between browser and application
- **Resource Management**: Efficient memory usage for web deployment

## Browser Compatibility

### WebGPU Support (Recommended)
- Chrome 113+
- Edge 113+
- Firefox 110+ (with `dom.webgpu.enabled` flag)

### WebGL Fallback
- All modern browsers with WebGL 2.0 support
- Automatically used when WebGPU is not available

## Performance Notes

- **WASM File Size**: ~1.2MB (optimized with wasm-opt)
- **Memory Usage**: Efficient memory management for web deployment
- **Rendering**: 60 FPS target with adaptive frame timing
- **Startup Time**: Fast initialization with progressive loading

## Troubleshooting

### Common Issues

1. **WASM Build Fails**
   - Ensure `wasm32-unknown-unknown` target is installed
   - Check that `wasm-bindgen-cli` version matches `wasm-bindgen` dependency

2. **Browser Compatibility**
   - Enable WebGPU in browser flags if needed
   - Check browser console for detailed error messages

3. **Performance Issues**
   - Use optimized WASM build (`wasm-opt`)
   - Check browser developer tools for performance bottlenecks

### Debug Mode

For development, you can build in debug mode:

```bash
# Debug build (larger but with debug symbols)
cargo build --target wasm32-unknown-unknown --features wasm
wasm-bindgen --target web --out-dir dist --out-name wgpu-winit-demo \
    ../../target/wasm32-unknown-unknown/debug/wgpu-winit-demo.wasm
```

## Integration with Your Project

This demo serves as a template for integrating Dear ImGui with WGPU and Winit in your own projects. Key integration points:

1. **Cargo.toml**: Dependencies and feature configuration
2. **WGPU Setup**: Device creation and surface configuration
3. **Dear ImGui Integration**: Context creation and renderer setup
4. **Event Handling**: Winit event processing and Dear ImGui integration
5. **WASM Support**: Conditional compilation and web-specific code

## License

This demo is part of the dear-imgui project and follows the same license terms.
