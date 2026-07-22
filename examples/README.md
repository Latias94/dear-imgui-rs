# Examples

The workspace examples are organized by what a user is trying to learn. Normal applications start with `dear-app`; renderer examples intentionally show the lower-level Winit, SDL3, WGPU, Glow, or Ash integration code.

Mobile host projects live in `examples-ios/` and `examples-android/`. Browser examples live in `examples-wasm/`.

## Start Here

Run the smallest stateful application from the workspace:

```text
cargo run -p dear-imgui-examples --bin hello_world
```

The same minimal entry point is also a directly runnable `dear-app` package example:

```text
cargo run -p dear-app --example hello
```

Continue with the complete, feature-free `Application` lifecycle when the program needs initialization, events, Context-owned resources, GPU recovery, or deterministic teardown:

```text
cargo run -p dear-imgui-examples --bin application_lifecycle
```

For a new project, add the unreleased `0.16.0-alpha.1` candidate from `main`:

```toml
[dependencies]
dear-app = { git = "https://github.com/Latias94/dear-imgui-rs", branch = "main", package = "dear-app" }
```

After publication, replace that dependency with `dear-app = "=0.16.0-alpha.1"`.

```rust
use dear_app::{AppConfig, RunError, imgui::Condition, run_ui};

fn main() -> Result<(), RunError> {
    let mut clicks = 0;
    run_ui(AppConfig::default(), move |ui| {
        ui.window("Hello")
            .size([360.0, 160.0], Condition::FirstUseEver)
            .build(|| {
                if ui.button("Click me") {
                    clicks += 1;
                }
                ui.text(format!("Clicks: {clicks}"));
            });
    })
}
```

Use `dear_app::Application` when the application needs initialization, events, GPU resources, device-loss recovery, or teardown hooks. The [`dear-app` README](../dear-app/README.md) shows both paths.

## Learning Paths

| Directory | Purpose | Expected abstraction level |
| --- | --- | --- |
| `00-quickstart` | First working application and the full lifecycle step-up | `dear_app::run_ui` and `dear_app::Application` |
| `03-features` | Core widgets, fonts, and optional extensions | UI code first; advanced examples may use `Application` or a raw backend |
| `01-renderers` | Implementing or embedding a renderer/platform stack | Explicit window, surface, renderer, and GPU lifecycle |
| `02-docking` | Dock layouts and native multi-viewport | From one dockspace to full backend lifecycle references |
| `04-integration` | Textures, background work, multiple Contexts, Test Engine, and real application patterns | End-to-end integration |

## Common UI Features

These examples use the same high-level runtime as `hello_world`, so their source focuses on the named UI feature:

```text
cargo run -p dear-imgui-examples --bin input_text_minimal
cargo run -p dear-imgui-examples --bin custom_font_minimal
cargo run -p dear-imgui-examples --bin managed_texture_minimal
cargo run -p dear-imgui-examples --bin task_organizer
cargo run -p dear-imgui-examples --bin tables_minimal
cargo run -p dear-imgui-examples --bin tables_property_grid
cargo run -p dear-imgui-examples --bin drawlist_minimal
cargo run -p dear-imgui-examples --bin menus_and_popups
cargo run -p dear-imgui-examples --bin list_clipper_log
```

`custom_font_minimal` embeds a trusted TTF at compile time and demonstrates font registration plus a scoped `push_font`. `managed_texture_minimal` covers renderer-agnostic registration, update, removal, and recreation using CPU-generated pixels. `task_organizer` combines stable-ID multi-select, typed drag and drop, and routed shortcuts in one command-driven workflow.

`style_and_fonts` is the advanced font-lifecycle reference. It includes a bundled Roboto font, runtime atlas updates, baked glyph queries, CJK/Emoji fallback discovery, and managed custom rectangles:

```text
cargo run -p dear-imgui-examples --bin style_and_fonts
cargo run -p dear-imgui-examples --bin style_and_fonts --features freetype
```

## Optional Extensions

Extension binaries require their matching feature:

```text
cargo run -p dear-imgui-examples --bin implot_basic --features implot
cargo run -p dear-imgui-examples --bin implot3d_basic --features implot3d
cargo run -p dear-imgui-examples --bin imnodes_basic --features imnodes
cargo run -p dear-imgui-examples --bin imguizmo_basic --features imguizmo
cargo run -p dear-imgui-examples --bin imguizmo_quat_basic --features imguizmo-quat
cargo run -p dear-imgui-examples --bin node_editor_basic --features node-editor
cargo run -p dear-imgui-examples --bin node_editor_showcase --features node-editor-blueprints
cargo run -p dear-imgui-examples --bin reflect_demo --features reflect
```

## Renderer Integration

Use these only when integrating Dear ImGui into an existing engine or writing a backend. They deliberately expose infrastructure that `dear-app` owns for normal applications:

- `wgpu_basic`, `wgpu_textures`, and `dear_app_wgpu_textures`
- `glow_basic`, `glow_textures`, and `glow_external_context_regression`
- `ash_basic` and `ash_textures`
- `sdl3_wgpu` and `sdl3_sdlrenderer` with their required SDL3 features

The v0.16 multi-viewport adapters own their renderer and callback storage. Call the renderer runtime's shutdown first, then the platform runtime's shutdown, before dropping the Context, windows, and GPU objects. No caller-address pinning or boxed renderer workaround is required.

## Docking and Multi-viewport

`dear-app` applications choose ownership explicitly: `DockingConfig::full_viewport()` lets the runtime draw the host, while `DockingConfig::application_managed()` only enables docking for an application-owned layout.

`dear-app` does not support Dear ImGui platform multi-viewport in 0.16. Docking remains available in its main window; use the Winit or SDL3 owning-runtime examples below for native secondary windows.

```text
cargo run -p dear-imgui-examples --bin dear_app_docking
cargo run -p dear-imgui-examples --bin dockspace_minimal
cargo run -p dear-imgui-examples --bin game_engine_docking --features multi-viewport
cargo run -p dear-imgui-examples --bin multi_viewport_wgpu --features multi-viewport
cargo run -p dear-imgui-examples --bin multi_viewport_ash --features multi-viewport
cargo run -p dear-imgui-examples --bin sdl3_wgpu_multi_viewport --features sdl3-wgpu-multi-viewport
cargo run -p dear-imgui-examples --bin sdl3_ash_multi_viewport --features sdl3-ash-multi-viewport
cargo run -p dear-imgui-examples --bin sdl3_sdlgpu_multi_view --features sdl3-gpu-multi-viewport
```

Secondary windows are rendered by the owning platform/renderer runtimes; only the main window should drive the application's primary surface render loop.

## Integration Examples

- `wgpu_rtt_gameview`: render-to-texture game view.
- `console_log`: filterable console with history.
- `asset_browser_grid`: thumbnail grid and filtering.
- `file_dialog_native` and `file_browser_imgui`: native and pure-ImGui file workflows (`file-browser` feature); the native dialog wakes a waiting event loop when its worker finishes.
- `threaded_snapshot_minimal`: move-only frame snapshot handoff to a renderer thread.
- `multi_context_switch`: explicit activation and suspension of multiple Contexts.
- `imgui_test_engine_basic`: interactive or bounded Test Engine runner (`test-engine` feature).

Run the Test Engine smoke route with:

```text
cargo run -p dear-imgui-examples --bin imgui_test_engine_basic --features test-engine -- --exit-when-done --group tests
```

## Persistence and Assets

`AppConfig::default()` does not enable docking and does not select an INI file. Set `ini_filename` explicitly when an application should persist window or dock layout state. Declarative dock layouts should normally use `DockLayoutApply::IfMissing`; reserve `Replace` for an explicit reset command.

The minimal custom-font example embeds its vendored TTF at compile time. File-backed texture and advanced font examples resolve assets from `examples/assets` or the workspace manifest path rather than the process working directory. This keeps IDE and terminal launches consistent.

## Contributing

- Put normal UI examples on `dear-app` and keep their source focused on the demonstrated feature.
- Put raw backend lifecycle code in `01-renderers` or `02-docking`.
- Keep unsafe FFI or native-handle lineage behind the narrowest documented integration boundary.
- Add a runnable command here for every feature-gated example.
