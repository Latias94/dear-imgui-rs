# dear-imgui-bevy

Experimental Bevy-native backend for `dear-imgui-rs` on Bevy `0.19.0-rc.2`.

This crate targets Bevy `0.19.0-rc.2` first. See
`docs/workstreams/bevy-backend-product-followups-v1/` for the current integration notes and
follow-up scope. It is intentionally **not** a wrapper around
`dear-imgui-winit` plus `dear-imgui-wgpu`: Bevy owns winit, input events, WGPU resources, render
schedules, and camera targets.

## Gate policy

The root workspace currently declares Rust `1.92`, while Bevy `0.19.0-rc.2` declares Rust `1.95.0`.
For that reason this crate has `rust-version = "1.95.0"` and should be checked in a dedicated Bevy
lane until the whole repository MSRV is intentionally raised.

Recommended Bevy-backend gates:

```bash
cargo +stable check -p dear-imgui-bevy --no-default-features
cargo +stable check -p dear-imgui-bevy --features render
cargo +stable check -p dear-imgui-bevy --target wasm32-unknown-unknown --no-default-features
cargo +stable check -p dear-imgui-bevy --target wasm32-unknown-unknown --features render
cargo +stable nextest run -p dear-imgui-bevy
cargo +stable nextest run -p dear-imgui-bevy --features render
cargo +stable nextest run -p dear-imgui-bevy --features render,multi-viewport
cargo +stable check -p dear-imgui-bevy --features render,multi-viewport,ecosystem --examples
```

These gates are intended for a dedicated Bevy backend CI lane. The root workspace workflow does not
substitute for them, because this crate sits on a different Rust and Bevy release train than the
rest of the workspace.

For local release checks, prefer a clean or dedicated target directory for this lane:

```bash
export CARGO_TARGET_DIR=target/bevy-backend
```

This avoids stale artifacts from unrelated workspace builds making rustc or nextest discovery spend
minutes scanning a polluted `target/debug/deps` directory before any Bevy backend test starts.

The current backend shape is verified on `wasm32-unknown-unknown` for both the core and `render`
feature sets. Mobile-specific targets are not split out yet; if Bevy's mobile support matrix needs a
different gate, keep it as a separate follow-on instead of widening the current lane.

## Docking and multi-viewport status

Docking inside the primary Bevy window is supported by enabling Dear ImGui docking on the context;
the examples use `configure_example_context` for that setup.

Dear ImGui docking multi-viewport OS windows are enabled only when requested on native targets with
both the `render` and `multi-viewport` Cargo features and an installed Bevy `RenderApp` from the
render plugin stack. The backend installs a queued PlatformIO lifecycle bridge, maps
input/focus/cursor/IME messages for secondary viewport windows, feeds Bevy window
position/size/focus/DPI state back through Dear ImGui's PlatformIO query callbacks, and routes each
viewport's draw data to the matching Bevy `Window` render target.

This path is still experimental. A few z-order and dock-target edge cases remain around detached
windows, so treat multi-viewport as preview-grade rather than a fully polished window-manager
experience.

| Target / feature set | `multi_viewport_requested` | Lifecycle bridge | Input / platform feedback | Full `multi_viewport_supported` |
| --- | --- | --- | --- | --- |
| Native, no `multi-viewport` feature | Matches config | No | No | No |
| Native, `render,multi-viewport` features and Bevy `RenderApp` | Matches config | Yes, when requested | Yes, when requested | Yes, when requested |
| Native, `render,multi-viewport` features without Bevy `RenderApp` | Matches config | Yes, when requested | Yes, when requested | No |
| `wasm32-unknown-unknown` | Matches config | No | No | No |

Bevy `0.19.0-rc.2` does not expose a persistent minimized-window field on `Window`; the backend
maps `WindowOccluded` events into Dear ImGui minimized feedback for secondary viewport windows and
otherwise preserves the last observed feedback, falling back to `false` when no feedback exists.

This is separate from the existing multi-window camera/render-target routing, which can draw the
same ImGui overlay to multiple Bevy window targets but does not create Dear ImGui platform windows.

## Current scope

The crate currently installs the experimental plugin/resource surface from BEVY-050 plus the
primary-window input, lifecycle, render extraction, renderer, and texture interop slices from
BEVY-060 through BEVY-100:

- `ImguiPlugin`
- `ImguiBackendConfig`
- `ImguiBackendStatus`
- non-send `ImguiContext`
- `input::ImguiInputState`
- `input::ImguiInputCapture`
- primary-window input message translation in `input::ImguiInputSystems`
- `ImguiBeginFrame`, `ImguiPrimaryContextPass`, and `ImguiEndFrame`
- `ImguiContexts`
- `ImguiFrameState`
- `ImguiFrameOutput`
- `render::ImguiExtractedRenderFrame`
- `render::ImguiOverlayCamera`
- `render::ImguiCameraTarget`
- `render::ImguiPreparedRenderFrame`
- `render::ImguiTextureBindGroups`
- `ImguiTextureFeedbackQueue`
- `ImguiBevyTextures` with the `render` feature

Examples live under `examples/` and are grouped by the integration question they answer.

## ECS frame lifecycle

`ImguiPlugin` installs three main-world schedules after Bevy `PreUpdate` input translation:

1. `ImguiBeginFrame` prepares IO from the primary window and opens one Dear ImGui frame.
2. `ImguiPrimaryContextPass` runs user UI systems against the already-open frame.
3. `ImguiEndFrame` renders the frame once and stores a thread-safe `FrameSnapshot` in
   `ImguiFrameOutput`.

User systems should be registered in `ImguiPrimaryContextPass` and access Dear ImGui through
`ImguiContexts`:

```rust
use bevy_app::App;
use dear_imgui_bevy::{ImguiContexts, ImguiPlugin, ImguiPrimaryContextPass};

fn tools_ui(mut contexts: ImguiContexts) {
    let Some(ui) = contexts.primary_ui_mut() else {
        return;
    };
    ui.window("Tools").build(|| {
        ui.text("Hello from Dear ImGui");
    });
}

let mut app = App::new();
app.add_plugins(ImguiPlugin::default());
app.add_systems(ImguiPrimaryContextPass, tools_ui);
```

The important invariant is that user systems draw into an already-open frame; they should not call
`Context::frame()` or `Context::render()` themselves. Extension crates can be used from the same
pass by taking the shared `&Ui` returned by `ImguiContexts`.

## Examples

Use the first three examples as the recommended progression. They are intentionally small enough to
copy into an application and then grow from there. The source links use absolute GitHub URLs so the
same README stays clickable from GitHub, docs.rs, and crates.io.

| Category | Example | Source | Run command | Purpose |
| --- | --- | --- | --- | --- |
| Basic | [`simple`][bevy-example-simple] | [`examples/basic/simple.rs`][bevy-example-simple] | `cargo run -p dear-imgui-bevy --features render --example simple` | Smallest visible Dear ImGui overlay in a normal Bevy app. |
| App | [`app_integration`][bevy-example-app-integration] | [`examples/app/app_integration.rs`][bevy-example-app-integration] | `cargo run -p dear-imgui-bevy --features render --example app_integration` | Plugin-style integration into an existing app/game loop with Bevy input policy. |
| Game engine | [`game_engine`][bevy-example-game-engine] | [`examples/game_engine/game_engine.rs`][bevy-example-game-engine] | `cargo run -p dear-imgui-bevy --features render --example game_engine`<br>`cargo run -p dear-imgui-bevy --features render,multi-viewport --example game_engine` | Docked editor surface with scene render-target texture interop, plus optional native multi-viewport. |
| Ecosystem | [`ecosystem`][bevy-example-ecosystem] | [`examples/ecosystem/ecosystem.rs`][bevy-example-ecosystem] | `cargo run -p dear-imgui-bevy --features ecosystem --example ecosystem` | Shared-frame ImPlot, ImNodes, and ImGuizmo integration. |
| Ecosystem | [`bevy_plot_controls`][bevy-example-plot-controls] | [`examples/ecosystem/bevy_plot_controls.rs`][bevy-example-plot-controls] | `cargo run -p dear-imgui-bevy --features render,implot --example bevy_plot_controls` | Focused Bevy scene plus ImPlot controls demo. |

[bevy-example-simple]: https://github.com/Latias94/dear-imgui-rs/blob/main/backends/dear-imgui-bevy/examples/basic/simple.rs
[bevy-example-app-integration]: https://github.com/Latias94/dear-imgui-rs/blob/main/backends/dear-imgui-bevy/examples/app/app_integration.rs
[bevy-example-game-engine]: https://github.com/Latias94/dear-imgui-rs/blob/main/backends/dear-imgui-bevy/examples/game_engine/game_engine.rs
[bevy-example-ecosystem]: https://github.com/Latias94/dear-imgui-rs/blob/main/backends/dear-imgui-bevy/examples/ecosystem/ecosystem.rs
[bevy-example-plot-controls]: https://github.com/Latias94/dear-imgui-rs/blob/main/backends/dear-imgui-bevy/examples/ecosystem/bevy_plot_controls.rs

The shared example setup lives in `configure_example_context`. It disables input trickling, can
toggle docking, builds the default font atlas, and disables `.ini` persistence so the examples do
not repeat the same initialization boilerplate. `ImguiBevyTextures` and
`render::ImguiOverlayDisabled` remain the reusable editor-facing backend helpers for texture
binding and offscreen scene cameras.

## Render extraction

With the `render` feature enabled, `ImguiPlugin` installs a Bevy `RenderApp` extraction system when
Bevy's render sub-app is available. The extraction system runs in `ExtractSchedule`, clones the
thread-safe `FrameSnapshot` from main-world `ImguiFrameOutput`, and stores it in render-world
`render::ImguiExtractedRenderFrame`.

The extracted frame also records active camera associations as `render::ImguiCameraTarget`,
including the main-world camera entity, camera order, normalized render target, and optional Bevy
camera viewport. Raw Dear ImGui draw-data pointers never cross the extract boundary; only the owned
`FrameSnapshot` and its texture requests do. Add `render::ImguiOverlayCamera` to the camera that
should receive the overlay for a render target. If no active camera on a target has that marker, the
backend falls back to the highest-order non-disabled camera so small applications keep working.
When the selected camera has `Camera.viewport`, the renderer applies the same physical viewport and
clips ImGui scissors to that region, so the overlay cannot draw outside the camera's Bevy viewport.

The renderer consumes only the owned snapshot and prepared render data. It does not borrow raw
Dear ImGui draw pointers across the Bevy main/render-world boundary and does not wrap
`dear-imgui-wgpu`.

## Texture interop

With the `render` feature enabled, ImGui-managed texture requests are handled in Bevy render-world
code and renderer feedback is queued through `ImguiTextureFeedbackQueue`. The queue is applied on
the main world before the next `ImguiBeginFrame`, which updates ImGui texture status/TexID from the
UI thread.

Bevy user images are registered through `ImguiBevyTextures`:

```rust
use bevy_ecs::system::ResMut;
use bevy_image::Image;
use bevy_asset::Handle;
use dear_imgui_bevy::ImguiBevyTextures;
use dear_imgui_rs::TextureId;

fn register_image(mut textures: ResMut<ImguiBevyTextures>, image: Handle<Image>) -> TextureId {
    textures.register(&image)
}
```

The returned `TextureId` can be passed to `ui.image(texture_id, size)`. Render-world code extracts
the registry and resolves the underlying `Handle<Image>` through Bevy `RenderAssets<GpuImage>` when
the GPU image is available. Registered Bevy images use the prepared `GpuImage` texture view and
sampler, so Bevy image sampler settings are preserved. Missing images keep using the renderer
fallback bind group until the asset is prepared by Bevy.

Editor-style render targets that are shown inside ImGui viewports can add
`render::ImguiOverlayDisabled` to their Bevy camera to prevent the global ImGui overlay pass from
drawing back into the offscreen scene image.

## Primary-window input policy

BEVY-060 maps one Bevy `PrimaryWindow` to the single Dear ImGui context owned by `ImguiContext`.
The input system reads Bevy messages and queues Dear ImGui IO events:

- window size and DPI: `Window` / resize / scale-factor messages update `io.DisplaySize` and
  `io.DisplayFramebufferScale` using Bevy logical coordinates plus the window scale factor;
- mouse: cursor position, leave, buttons, and wheel messages map to Dear ImGui mouse position,
  buttons, source, and wheel deltas;
- keyboard: Bevy physical `KeyCode` values map to Dear ImGui `Key` values, and key text is queued
  as input characters;
- touch: the first active touch is translated to a touchscreen mouse source and left-button press;
- focus: focus-loss messages release tracked keys/buttons to avoid stuck Dear ImGui input state;
- IME: committed text is queued as Dear ImGui input characters; preedit text is not injected.

The backend does **not** consume or delete Bevy messages. Bevy gameplay/editor systems should use
`ImguiInputCapture` or the `imgui_wants_*` run-condition helpers as policy hints after the Dear
ImGui frame has had a chance to compute capture intent.

Current input/runtime boundaries:

- pointer and keyboard capture are policy hints only;
- clipboard is application-provided: the plugin preserves an existing
  `Context::set_clipboard_backend` installation but does not install a native or browser clipboard
  backend itself;
- accessibility is not integrated: the backend does not generate Bevy `AccessibilityNode` or
  AccessKit data for Dear ImGui widgets;
- file drop, gamepad navigation, and Bevy picking integration are not part of this backend yet;
- wasm builds compile, but browser runtime IME and clipboard behavior still depends on the target
  host.
