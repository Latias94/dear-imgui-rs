# dear-imgui-bevy

A Bevy plugin for integrating Dear ImGui using the `dear-imgui` and `dear-imgui-wgpu` crates.

## Features

- ✅ Full Dear ImGui integration with Bevy
- ✅ WGPU-based rendering backend
- ✅ Input handling (keyboard, mouse)
- ✅ Font customization and DPI scaling
- ✅ Multiple render targets (2D and 3D)
- 🚧 Bevy texture integration (partial)
- 🚧 Docking support (when enabled)

## Quick Start

Add this to your `Cargo.toml`:

```toml
[dependencies]
dear-imgui-bevy = { path = "path/to/dear-imgui-bevy" }
# Or when published:
# dear-imgui-bevy = "0.1"

# For latest Bevy with wgpu 26 support
bevy = { git = "https://github.com/bevyengine/bevy", default-features = false, features = [
    "bevy_core_pipeline", 
    "bevy_render", 
    "bevy_window",
    "bevy_winit"
] }
```

## Basic Usage

```rust
use bevy::prelude::*;
use dear_imgui_bevy::prelude::*;

#[derive(Resource)]
struct UiState {
    demo_window_open: bool,
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(ImguiPlugin::default())
        .insert_resource(UiState { demo_window_open: true })
        .add_systems(Startup, setup)
        .add_systems(Update, ui_system)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera3d::default());
}

fn ui_system(mut context: NonSendMut<ImguiContext>, mut state: ResMut<UiState>) {
    let ui = context.ui();
    
    if state.demo_window_open {
        ui.show_demo_window(&mut state.demo_window_open);
    }
    
    ui.window("Hello World")
        .build(|| {
            ui.text("Hello from dear-imgui-bevy!");
        });
}
```

## Plugin Configuration

```rust
use dear_imgui_bevy::ImguiPlugin;

App::new()
    .add_plugins(ImguiPlugin {
        ini_filename: Some("my_app.ini".into()),
        font_size: 16.0,
        font_oversample_h: 2,
        font_oversample_v: 2,
        apply_display_scale_to_font_size: true,
        apply_display_scale_to_font_oversample: true,
    })
    // ... rest of your app
```

## Texture Integration

```rust
// Register a Bevy texture with ImGui
fn register_texture_system(
    mut context: NonSendMut<ImguiContext>,
    asset_server: Res<AssetServer>,
) {
    let texture_handle: Handle<Image> = asset_server.load("my_texture.png");
    
    // Convert to strong handle first
    if let Handle::Strong(strong_handle) = texture_handle {
        let texture_id = context.register_bevy_texture(Handle::Strong(strong_handle));
        // Use texture_id in ImGui draw calls
    }
}
```

## Examples

Run the examples to see the plugin in action:

```bash
# Minimal example
cargo run --example minimal

# Hello world with 3D scene
cargo run --example hello_world
```

## Architecture

This plugin integrates several components:

- **ImguiContext**: Thread-safe resource managing the Dear ImGui context
- **ImguiPlugin**: Bevy plugin that sets up the integration
- **Render Systems**: Handle frame extraction and rendering
- **Input Systems**: Process keyboard and mouse input for ImGui

## Limitations

- Texture integration is currently limited (work in progress)
- Some advanced ImGui features may not be fully supported yet
- Requires Bevy main branch for wgpu 26 compatibility

## Contributing

This plugin is part of the `dear-imgui` project. Contributions are welcome!

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](../../LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](../../LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
