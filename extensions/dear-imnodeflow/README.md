# Dear ImNodeFlow - Rust Bindings

High-level Rust bindings for ImNodeFlow, the immediate mode node editor library built on top of Dear ImGui. This crate provides safe, idiomatic Rust bindings designed to work seamlessly with the `dear-imgui` ecosystem.

## Features

- **Safe, idiomatic Rust API** - Memory-safe wrappers around the C++ ImNodeFlow library
- **Full compatibility with dear-imgui** - Designed to work with `dear-imgui` rather than `imgui-rs`
- **Builder pattern support** - Fluent API for creating nodes and pins
- **Comprehensive styling** - Full access to ImNodeFlow's styling system
- **Node editor functionality** - Complete node editor with drag-and-drop, connections, and more

## Quick Start

Add this to your `Cargo.toml`:

```toml
[dependencies]
dear-imgui = "0.1"
dear-imnodeflow = "0.1"
```

Basic usage:

```rust
use dear_imgui::*;
use dear_imnodeflow::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize Dear ImGui context
    let mut ctx = Context::create_or_panic();
    
    // Create node editor
    let mut node_editor = NodeEditor::new("My Node Editor")?;
    node_editor.set_size(ImVec2 { x: 800.0, y: 600.0 });
    
    // Create a node
    let mut node = NodeBuilder::new()
        .title("My Node")
        .position(ImVec2 { x: 100.0, y: 100.0 })
        .style(NodeStyle::green())
        .build()?;
    
    node.set_handler(&node_editor);
    
    // In your main loop:
    let ui = ctx.frame();
    
    if let Some(window) = ui.window("Node Editor").begin() {
        node_editor.update(&ui);
        node.update();
        window.end();
    }
    
    Ok(())
}
```

## Architecture

This crate consists of two main components:

### dear-imnodeflow-sys

Low-level FFI bindings that provide direct access to the ImNodeFlow C++ library. This crate:

- Uses `bindgen` to generate Rust bindings from C++ headers
- Provides C wrapper functions for C++ template functions
- Links against the ImNodeFlow C++ library
- Re-exports necessary types from `dear-imgui-sys`

### dear-imnodeflow

High-level, safe Rust API that wraps the low-level bindings. This crate provides:

- Safe wrappers around raw pointers
- Builder patterns for easy object creation
- Idiomatic Rust error handling
- Memory management through RAII
- Integration with the `dear-imgui` ecosystem

## Core Concepts

### NodeEditor

The main node editor context that manages the infinite grid, nodes, and links:

```rust
let mut editor = NodeEditor::new("My Editor")?;
editor.set_size(ImVec2 { x: 800.0, y: 600.0 });

// In your update loop
editor.update(&ui);
```

### Nodes

Individual nodes in the editor that can contain custom content:

```rust
let mut node = NodeBuilder::new()
    .title("Processing Node")
    .position(ImVec2 { x: 200.0, y: 150.0 })
    .style(NodeStyle::cyan())
    .build()?;

node.set_handler(&editor);
```

### Pins and Links

Pins are connection points on nodes, and links connect pins together:

```rust
// Pins are typically created within custom node implementations
// Links are created by dragging between pins in the UI
```

### Styling

Comprehensive styling system for both nodes and pins:

```rust
// Predefined styles
let green_style = NodeStyle::green();
let cyan_pin_style = PinStyle::cyan();

// Custom styles
let custom_style = NodeStyleBuilder::new()
    .header_bg(colors::BLUE)
    .header_title_color(colors::WHITE)
    .radius(10.0)
    .build();
```

## Examples

### Simple Node Editor

See `examples/simple_node_editor.rs` for a complete example that demonstrates:

- Creating a node editor
- Adding multiple nodes with different styles
- Basic interaction and navigation
- Property inspection
- Menu integration

Run the example with:

```bash
cargo run --example simple_node_editor
```

### Custom Node Types

To create custom node types, you would typically extend the base functionality:

```rust
struct MathNode {
    node: Node,
    operation: String,
    value_a: f32,
    value_b: f32,
}

impl MathNode {
    fn new(operation: &str) -> Result<Self, NodeFlowError> {
        let mut node = NodeBuilder::new()
            .title(&format!("Math: {}", operation))
            .style(NodeStyle::blue())
            .build()?;
            
        Ok(Self {
            node,
            operation: operation.to_string(),
            value_a: 0.0,
            value_b: 0.0,
        })
    }
    
    fn update(&mut self, ui: &Ui) {
        self.node.update();
        
        // Custom node content would go here
        // This would typically involve creating pins and handling their values
    }
}
```

## Integration with Dear ImGui

This crate is designed to work seamlessly with the `dear-imgui` ecosystem:

- Uses the same `Context` and `Ui` types
- Compatible with dear-imgui's window management
- Shares the same underlying Dear ImGui context
- Follows similar patterns for lifetime management

## Building

### Prerequisites

- Rust 1.70 or later
- C++17 compatible compiler
- CMake (for building ImNodeFlow)
- Git (for submodules)

### Build Steps

1. Clone the repository with submodules:
   ```bash
   git clone --recursive https://github.com/your-repo/dear-imgui
   ```

2. Build the project:
   ```bash
   cargo build
   ```

3. Run tests:
   ```bash
   cargo test
   ```

4. Run examples:
   ```bash
   cargo run --example simple_node_editor
   ```

## Features

The crate supports the following cargo features:

- `default = ["docking"]` - Default features
- `docking` - Enable docking branch features (default)
- `multi-viewport` - Enable multi-viewport support (requires docking)
- `freetype` - Enable freetype font rasterizer
- `wasm` - Enable for WASM targets

## License

This project is licensed under the same terms as the Dear ImGui project. See the LICENSE file for details.

## Contributing

Contributions are welcome! Please see the contributing guidelines for more information.

## Acknowledgments

- [Dear ImGui](https://github.com/ocornut/imgui) - The immediate mode GUI library
- [ImNodeFlow](https://github.com/Fattorino/ImNodeFlow) - The node editor library
- [dear-imgui](https://github.com/your-repo/dear-imgui) - The Rust bindings for Dear ImGui

## Status

This is a work-in-progress implementation. The basic functionality is working, but some advanced features may not be fully implemented yet.

### Implemented Features

- ✅ Basic node editor creation and management
- ✅ Node creation, positioning, and styling
- ✅ Pin and link system (basic)
- ✅ Safe Rust wrappers
- ✅ Builder patterns
- ✅ Integration with dear-imgui

### TODO

- ⏳ Advanced pin types and data flow
- ⏳ Custom node content rendering
- ⏳ Serialization/deserialization
- ⏳ Advanced styling options
- ⏳ Performance optimizations
- ⏳ More comprehensive examples
- ⏳ Documentation improvements
