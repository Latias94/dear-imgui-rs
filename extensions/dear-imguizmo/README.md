# Dear ImGuizmo

High-level Rust bindings for [ImGuizmo](https://github.com/CedricGuillemet/ImGuizmo), a 3D gizmo library for Dear ImGui.

ImGuizmo provides interactive 3D manipulation widgets (gizmos) for translation, rotation, and scaling operations in 3D space.

## Features

- **Translation gizmos**: Move objects in 3D space along individual axes or planes
- **Rotation gizmos**: Rotate objects around axes or screen space
- **Scale gizmos**: Scale objects uniformly or per-axis
- **View manipulation**: Interactive camera controls with view cube
- **Grid rendering**: Draw reference grids in 3D space
- **Matrix utilities**: Decompose and recompose transformation matrices
- **Customizable styling**: Configure colors, sizes, and appearance
- **Safe Rust API**: Memory-safe wrappers with RAII lifetime management
- **Builder patterns**: Fluent, type-safe configuration APIs

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
dear-imgui = { path = "path/to/dear-imgui" }
dear-imguizmo = { path = "path/to/extensions/dear-imguizmo" }
```

Basic usage:

```rust
use dear_imgui::*;
use dear_imguizmo::*;

fn main() {
    let mut imgui_ctx = Context::create_or_panic();
    let gizmo_ctx = GuizmoContext::create(&imgui_ctx);

    // In your render loop
    let ui = imgui_ctx.frame();
    let gizmo_ui = gizmo_ctx.get_ui(&ui);

    // Set up the viewport
    gizmo_ui.set_rect(0.0, 0.0, 800.0, 600.0);

    // Your transformation matrix
    let mut matrix = [1.0, 0.0, 0.0, 0.0,
                      0.0, 1.0, 0.0, 0.0,
                      0.0, 0.0, 1.0, 0.0,
                      0.0, 0.0, 0.0, 1.0];

    // Camera matrices
    let view = get_view_matrix();
    let projection = get_projection_matrix();

    // Manipulate the object
    if let Some(result) = gizmo_ui.manipulate(&view, &projection)
        .operation(Operation::TRANSLATE)
        .mode(Mode::WORLD)
        .matrix(&mut matrix)
        .build() {
        
        if result.used {
            println!("Object was manipulated!");
        }
    }
}
```

## Operations

ImGuizmo supports various manipulation operations:

```rust
// Individual axis operations
Operation::TRANSLATE_X | Operation::TRANSLATE_Y | Operation::TRANSLATE_Z
Operation::ROTATE_X | Operation::ROTATE_Y | Operation::ROTATE_Z
Operation::SCALE_X | Operation::SCALE_Y | Operation::SCALE_Z

// Combined operations
Operation::TRANSLATE  // All translation axes
Operation::ROTATE     // All rotation axes + screen rotation
Operation::SCALE      // All scale axes
Operation::UNIVERSAL  // All operations combined
```

## Manipulation Modes

- `Mode::Local`: Manipulate in object's local coordinate space
- `Mode::World`: Manipulate in world coordinate space

## Advanced Features

### Snapping

```rust
let snap_values = [1.0, 1.0, 1.0]; // Snap to 1-unit grid
gizmo_ui.manipulate(&view, &projection)
    .operation(Operation::TRANSLATE)
    .matrix(&mut matrix)
    .snap(&snap_values)
    .build();
```

### View Manipulation

```rust
let mut view_matrix = get_view_matrix();
gizmo_ui.view_manipulate(&mut view_matrix)
    .position(10.0, 10.0)
    .size(128.0, 128.0)
    .length(8.0)
    .build();
```

### Custom Styling

```rust
let style = StyleBuilder::new()
    .translation_line_thickness(4.0)
    .color(ColorType::DirectionX, [1.0, 0.0, 0.0, 1.0])
    .color(ColorType::DirectionY, [0.0, 1.0, 0.0, 1.0])
    .color(ColorType::DirectionZ, [0.0, 0.0, 1.0, 1.0])
    .build();

gizmo_ui.set_style(&style);
```

### Matrix Utilities

```rust
// Decompose matrix into components
let (translation, rotation, scale) = gizmo_ui.decompose_matrix(&matrix)?;

// Modify components
let mut new_translation = translation;
new_translation[0] += 1.0; // Move 1 unit along X

// Recompose matrix
let new_matrix = gizmo_ui.recompose_matrix(&new_translation, &rotation, &scale);
```

### ID Management

```rust
// Automatic ID management with RAII
{
    let _id_guard = IdGuard::new(&gizmo_ui, "object1");
    // Gizmo operations for object1
} // ID automatically popped

// Manual ID management
gizmo_ui.push_id("object2");
// Gizmo operations for object2
gizmo_ui.pop_id();
```

## Examples

Run the basic usage example:

```bash
cargo run --example basic_usage
```

## Architecture

This crate follows the same architecture pattern as other Dear ImGui extensions:

- `dear-imguizmo-sys`: Low-level FFI bindings to ImGuizmo C++
- `dear-imguizmo`: High-level safe Rust API (this crate)

## License

This project is licensed under the same terms as Dear ImGui.

ImGuizmo is licensed under the MIT License. See the [ImGuizmo repository](https://github.com/CedricGuillemet/ImGuizmo) for details.

## Contributing

Contributions are welcome! Please feel free to submit issues and pull requests.
