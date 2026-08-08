# Examples

Run every command below from the workspace root. Normal applications should start with
`dear-app`; the raw Winit, SDL3, WGPU, Glow, and Ash examples are integration references for
applications that already own their window or renderer stack.

Mobile host projects live in [`examples-ios`](../examples-ios/), Android host projects in
[`examples-android`](../examples-android/), and browser examples in
[`examples-wasm`](../examples-wasm/). Binaries under `examples/ci/` are private runtime probes and
are intentionally excluded from this learning path.

## Level 1: Build UI

These examples use the default `dear-app` runtime and keep infrastructure out of the way.

| Example | Category | Features | Command | Prerequisites | Next step |
| --- | --- | --- | --- | --- | --- |
| `hello_world` | First window | Default | `cargo run -p dear-imgui-examples --bin hello_world` | Desktop GPU | `fallible_frame` |
| `fallible_frame` | Fallible frame callback | Default | `cargo run -p dear-imgui-examples --bin fallible_frame` | Desktop GPU | `application_lifecycle` |
| `application_lifecycle` | Full application hooks | Default | `cargo run -p dear-imgui-examples --bin application_lifecycle` | Desktop GPU | `game_engine_showcase` |
| `input_text_minimal` | Text input | Default | `cargo run -p dear-imgui-examples --bin input_text_minimal` | Desktop GPU | `ime_debug` |
| `custom_font_minimal` | Embedded font | Default | `cargo run -p dear-imgui-examples --bin custom_font_minimal` | Bundled font asset | `style_and_fonts` |
| `managed_texture_minimal` | CPU-managed texture | Default | `cargo run -p dear-imgui-examples --bin managed_texture_minimal` | Desktop GPU | `dear_app_external_texture` |
| `tables_minimal` | Tables | Default | `cargo run -p dear-imgui-examples --bin tables_minimal` | Desktop GPU | `tables_property_grid` |
| `drawlist_minimal` | Custom drawing | Default | `cargo run -p dear-imgui-examples --bin drawlist_minimal` | Desktop GPU | `imguizmo_minimal` |
| `menus_and_popups` | Menus and modal state | Default | `cargo run -p dear-imgui-examples --bin menus_and_popups` | Desktop GPU | `task_organizer` |
| `tables_property_grid` | Property editor | Default | `cargo run -p dear-imgui-examples --bin tables_property_grid` | Desktop GPU | `task_organizer` |
| `list_clipper_log` | Large lists | Default | `cargo run -p dear-imgui-examples --bin list_clipper_log` | Desktop GPU | `console_log` |
| `task_organizer` | Combined workflow | Default | `cargo run -p dear-imgui-examples --bin task_organizer` | Desktop GPU | `game_engine_showcase` |

The three quickstarts share one runtime state machine. See
[`00-quickstart/README.md`](00-quickstart/README.md) for the capability added at each step.

## Level 2: Learn Specialized Features

Start with a `*_minimal` example. Use its matching `*_showcase` only after the basic Context and
token lifetime is clear.

| Example | Category | Features | Command | Prerequisites | Next step |
| --- | --- | --- | --- | --- | --- |
| `style_and_fonts` | Font atlas lifecycle | Default | `cargo run -p dear-imgui-examples --bin style_and_fonts` | Bundled font asset | `custom_font` in `dear-imgui-bevy` |
| `implot_minimal` | 2D plotting | `implot` | `cargo run -p dear-imgui-examples --bin implot_minimal --features implot` | Desktop GPU | `implot_showcase` |
| `implot_showcase` | 2D plotting catalog | `implot` | `cargo run -p dear-imgui-examples --bin implot_showcase --features implot` | Desktop GPU | ImPlot crate README |
| `implot3d_minimal` | 3D plotting | `implot3d` | `cargo run -p dear-imgui-examples --bin implot3d_minimal --features implot3d` | Desktop GPU | `implot3d_showcase` |
| `implot3d_showcase` | 3D plotting catalog | `implot3d` | `cargo run -p dear-imgui-examples --bin implot3d_showcase --features implot3d` | Desktop GPU | ImPlot3D crate README |
| `imguizmo_minimal` | Transform gizmo | `imguizmo` | `cargo run -p dear-imgui-examples --bin imguizmo_minimal --features imguizmo` | Desktop GPU | `imguizmo_showcase` |
| `imguizmo_showcase` | Transform gizmo catalog | `imguizmo` | `cargo run -p dear-imgui-examples --bin imguizmo_showcase --features imguizmo` | Desktop GPU | `game_engine_showcase --features imguizmo` |
| `imguizmo_quat_minimal` | Quaternion gizmo | `imguizmo-quat` | `cargo run -p dear-imgui-examples --bin imguizmo_quat_minimal --features imguizmo-quat` | Desktop GPU | `imguizmo_quat_showcase` |
| `imguizmo_quat_showcase` | Quaternion gizmo catalog | `imguizmo-quat` | `cargo run -p dear-imgui-examples --bin imguizmo_quat_showcase --features imguizmo-quat` | Desktop GPU | ImGuIZMO.quat crate README |
| `imnodes_minimal` | Node editor | `imnodes` | `cargo run -p dear-imgui-examples --bin imnodes_minimal --features imnodes` | Desktop GPU | `imnodes_showcase` |
| `imnodes_showcase` | Node editor catalog | `imnodes` | `cargo run -p dear-imgui-examples --bin imnodes_showcase --features imnodes` | Desktop GPU | ImNodes crate README |
| `node_editor_minimal` | Blueprint editor | `node-editor` | `cargo run -p dear-imgui-examples --bin node_editor_minimal --features node-editor` | Desktop GPU | `node_editor_showcase` |
| `node_editor_showcase` | Blueprint workflow | `node-editor-blueprints` | `cargo run -p dear-imgui-examples --bin node_editor_showcase --features node-editor-blueprints` | Desktop GPU | Node Editor crate README |
| `reflect_demo` | Reflected inspectors | `reflect` | `cargo run -p dear-imgui-examples --bin reflect_demo --features reflect` | Desktop GPU | Reflect crate README |

## Level 3: Build Applications

These examples combine UI with application-owned state, GPU resources, threads, files, or testing.

| Example | Category | Features | Command | Prerequisites | Next step |
| --- | --- | --- | --- | --- | --- |
| `dear_app_docking` | Runtime-owned dockspace | Default | `cargo run -p dear-imgui-examples --bin dear_app_docking` | Desktop GPU | `dockspace_minimal` |
| `dockspace_minimal` | Application dockspace | Default | `cargo run -p dear-imgui-examples --bin dockspace_minimal` | Desktop GPU | `game_engine_showcase` |
| `game_engine_showcase` | Engine-style application | Default; optional `imguizmo` | `cargo run -p dear-imgui-examples --bin game_engine_showcase` | Desktop GPU | Re-run with `--features imguizmo` |
| `dear_app_external_texture` | Application-owned WGPU texture | Default | `cargo run -p dear-imgui-examples --bin dear_app_external_texture` | Desktop GPU | `wgpu_rtt_gameview` |
| `wgpu_rtt_gameview` | Render to texture | Default | `cargo run -p dear-imgui-examples --bin wgpu_rtt_gameview` | Desktop GPU | `game_engine_showcase` |
| `console_log` | Filterable console | Default | `cargo run -p dear-imgui-examples --bin console_log` | Desktop GPU | `game_engine_showcase` |
| `asset_browser_grid` | Asset grid | Default | `cargo run -p dear-imgui-examples --bin asset_browser_grid` | Bundled image assets | `game_engine_showcase` |
| `file_dialog_native` | Native file dialog | `file-browser` | `cargo run -p dear-imgui-examples --bin file_dialog_native --features file-browser` | Desktop file system | `file_browser_imgui` |
| `file_browser_imgui` | In-UI file browser | `file-browser` | `cargo run -p dear-imgui-examples --bin file_browser_imgui --features file-browser` | Desktop file system | File Browser crate README |
| `ime_debug` | IME diagnostics | Default | `cargo run -p dear-imgui-examples --bin ime_debug` | OS input method | Winit backend README |
| `threaded_snapshot_minimal` | Render-thread handoff | Default | `cargo run -p dear-imgui-examples --bin threaded_snapshot_minimal` | Desktop GPU | Custom renderer contract |
| `multi_context_switch` | Multiple Contexts | Default | `cargo run -p dear-imgui-examples --bin multi_context_switch` | Desktop GPU | Context activation docs |
| `test_engine_integration` | Automated UI tests | `test-engine` | `cargo run -p dear-imgui-examples --bin test_engine_integration --features test-engine` | Native source build; not WASM | Test Engine crate README |

`dear-app` supports docking in its main window but does not own native platform multi-viewport.
Choose a Level 4 owning runtime when Dear ImGui must create secondary OS windows.

## Level 4: Integrate Backends

These references expose surface acquisition, renderer reconciliation, presentation, native context
requirements, and deterministic teardown. They are deliberately more verbose than application
examples.

| Example | Category | Features | Command | Prerequisites | Next step |
| --- | --- | --- | --- | --- | --- |
| `winit_wgpu` | Winit + WGPU lifecycle | Default | `cargo run -p dear-imgui-examples --bin winit_wgpu` | WGPU-compatible driver | `multi_viewport_wgpu` |
| `winit_glow` | Winit + Glow lifecycle | Default | `cargo run -p dear-imgui-examples --bin winit_glow` | OpenGL driver | `glow_renderer_texture` |
| `winit_ash` | Winit + Ash lifecycle | Default | `cargo run -p dear-imgui-examples --bin winit_ash` | Vulkan loader and driver | `multi_viewport_ash` |
| `glow_renderer_texture` | Renderer-owned GL texture | Default | `cargo run -p dear-imgui-examples --bin glow_renderer_texture` | OpenGL driver | Glow backend README |
| `sdl3_wgpu` | SDL3 + WGPU lifecycle | `sdl3-platform` | `cargo run -p dear-imgui-examples --bin sdl3_wgpu --features sdl3-platform` | CMake/compiler or app-provided SDL3 | `sdl3_wgpu_multi_viewport` |
| `sdl3_sdlrenderer3` | Official SDLRenderer3 | `sdl3-sdlrenderer3` | `cargo run -p dear-imgui-examples --bin sdl3_sdlrenderer3 --features sdl3-sdlrenderer3` | CMake/compiler or app-provided SDL3 | SDL3 backend README |
| `multi_viewport_wgpu` | Winit + WGPU multi-viewport | `multi-viewport` | `cargo run -p dear-imgui-examples --bin multi_viewport_wgpu --features multi-viewport` | Windows/macOS or X11; WGPU driver | WGPU backend README |
| `multi_viewport_ash` | Winit + Ash multi-viewport | `ash-winit-multi-viewport` | `cargo run -p dear-imgui-examples --bin multi_viewport_ash --features ash-winit-multi-viewport` | Windows/macOS or X11; Vulkan | Ash backend README |
| `sdl3_opengl_multi_viewport` | Official SDL3 + OpenGL3 multi-viewport | `multi-viewport,sdl3-opengl3` | `cargo run -p dear-imgui-examples --bin sdl3_opengl_multi_viewport --features multi-viewport,sdl3-opengl3` | Native SDL3 and OpenGL | `sdl3_glow_multi_viewport` |
| `sdl3_glow_multi_viewport` | SDL3 + Glow multi-viewport | `sdl3-glow-multi-viewport` | `cargo run -p dear-imgui-examples --bin sdl3_glow_multi_viewport --features sdl3-glow-multi-viewport` | Native SDL3 and OpenGL | Glow backend README |
| `sdl3_wgpu_multi_viewport` | SDL3 + WGPU multi-viewport | `sdl3-wgpu-multi-viewport` | `cargo run -p dear-imgui-examples --bin sdl3_wgpu_multi_viewport --features sdl3-wgpu-multi-viewport` | Native SDL3 and WGPU driver | WGPU backend README |
| `sdl3_ash_multi_viewport` | SDL3 + Ash multi-viewport | `sdl3-ash-multi-viewport` | `cargo run -p dear-imgui-examples --bin sdl3_ash_multi_viewport --features sdl3-ash-multi-viewport` | Native SDL3 and Vulkan | Ash backend README |
| `sdl3_sdlgpu_multi_viewport` | Official SDL3 + SDLGPU3 multi-viewport | `sdl3-gpu-multi-viewport` | `cargo run -p dear-imgui-examples --bin sdl3_sdlgpu_multi_viewport --features sdl3-gpu-multi-viewport` | Native SDL3 with SDLGPU support | SDL3 backend README |

Winit native multi-viewport is disabled on Wayland because Winit does not expose programmatic
top-level window positioning there. Use X11 (`WINIT_UNIX_BACKEND=x11`) or an SDL3 route when native
secondary windows are required.

Before implementing a custom renderer, run the headless contract example. It covers synchronous
texture feedback, draw traversal, and reset without choosing a GPU API:

```text
cargo run -j 1 -p dear-imgui-rs --example custom_renderer_headless
```

## Persistence and Assets

`AppConfig::default()` does not enable docking or choose an INI file. Set `ini_filename` when an
application should persist window or dock layout state. Declarative dock layouts should normally
use `DockLayoutApply::IfMissing`; reserve `Replace` for an explicit reset command.

Examples resolve bundled assets from the workspace manifest path rather than the process working
directory, so IDE and terminal launches behave the same.

## Contributing

- Put focused UI examples on `dear-app`.
- Put raw backend lifecycle code in `01-renderers` or `02-docking`.
- Keep CI arguments, evidence JSON, and renderer forcing under `examples/ci`.
- Keep unsafe FFI or native-handle lineage behind the narrowest documented integration boundary.
- Add every public binary to exactly one level above with its feature-complete command.
