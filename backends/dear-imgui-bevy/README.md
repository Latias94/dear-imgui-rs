# dear-imgui-bevy

Experimental Bevy-native backend for `dear-imgui-rs`.

This crate targets Bevy `0.19.0-rc.2` first and follows the workstream in
`docs/workstreams/bevy-native-backend/`. It is intentionally **not** a wrapper around
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
cargo +stable nextest run -p dear-imgui-bevy
cargo +stable nextest run -p dear-imgui-bevy --features render
```

## Current scope

The crate currently installs the experimental plugin/resource surface from BEVY-050 plus the
primary-window input, lifecycle, and render extraction slices from BEVY-060/BEVY-070/BEVY-080:

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
- `render::ImguiCameraTarget`

Renderer pipelines, texture interop, and examples are later workstream tasks.

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

## Render extraction

With the `render` feature enabled, `ImguiPlugin` installs a Bevy `RenderApp` extraction system when
Bevy's render sub-app is available. The extraction system runs in `ExtractSchedule`, clones the
thread-safe `FrameSnapshot` from main-world `ImguiFrameOutput`, and stores it in render-world
`render::ImguiExtractedRenderFrame`.

The extracted frame also records active camera associations as `render::ImguiCameraTarget`, including
the main-world camera entity, camera order, and normalized render target. Raw Dear ImGui draw-data
pointers never cross the extract boundary; only the owned `FrameSnapshot` and its texture requests do.

BEVY-080 intentionally stops at this boundary. The Bevy-native WGPU pipeline, GPU texture feedback,
and Bevy `Handle<Image>` user-texture interop are follow-up renderer tasks.

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
`ImguiInputCapture` or `Context::io().want_capture_*()` as policy hints after the Dear ImGui frame
has had a chance to compute capture intent.
