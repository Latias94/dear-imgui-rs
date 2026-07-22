# dear-imgui-bevy

Bevy-native backend for `dear-imgui-rs`.

This crate integrates Dear ImGui with Bevy's ECS, window/input messages, WGPU render world, camera
targets, and texture assets. It is intentionally not a wrapper around `dear-imgui-winit` plus
`dear-imgui-wgpu`: Bevy owns the app loop and render schedules, so this backend follows Bevy's
ownership model directly.

## Quick Facts

| Item | Status |
| --- | --- |
| Rust | `1.95.0` or newer |
| Bevy | `=0.19.0` |
| dear-imgui-rs | `0.16.0-alpha.1` |
| Primary-window overlay | Supported with the `render` feature |
| Docking in the primary window | Supported when the ImGui context enables docking |
| Native multi-viewport OS windows | Preview-grade with `render,multi-viewport` |
| WASM | Core and `render` feature sets compile for `wasm32-unknown-unknown`; browser runtime integration is still limited |

## Getting Started

Use the same Bevy version as the backend. Until `0.16.0-alpha.1` is published,
take both Dear ImGui packages from `main`:

```toml
[dependencies]
bevy = "=0.19.0"
dear-imgui-bevy = { git = "https://github.com/Latias94/dear-imgui-rs", branch = "main", features = ["render"] }
dear-imgui-rs = { git = "https://github.com/Latias94/dear-imgui-rs", branch = "main" }
```

After publication, use the exact prerelease requirements:

```toml
[dependencies]
bevy = "=0.19.0"
dear-imgui-bevy = { version = "=0.16.0-alpha.1", features = ["render"] }
dear-imgui-rs = "=0.16.0-alpha.1"
```

A minimal overlay app registers `ImguiPlugin`, marks the camera that should receive the ImGui
overlay, and draws UI from `ImguiPrimaryContextPass`:

```rust
use bevy::prelude::*;
use dear_imgui_bevy::{
    configure_example_context, ImguiContext, ImguiContexts, ImguiPlugin,
    ImguiPrimaryContextPass, render::ImguiOverlayCamera,
};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(ImguiPlugin::default())
        .add_systems(Startup, setup)
        .add_systems(ImguiPrimaryContextPass, tools_ui)
        .run();
}

fn setup(mut commands: Commands, mut imgui: NonSendMut<ImguiContext>) {
    commands.spawn((Camera2d, ImguiOverlayCamera));
    configure_example_context(&mut imgui, false);
}

fn tools_ui(mut contexts: ImguiContexts) {
    let Some(ui) = contexts.primary_ui_mut() else {
        return;
    };

    ui.window("Tools").build(|| {
        ui.text("Dear ImGui is drawing in Bevy.");
    });
}
```

Application UI systems should draw through the `Ui` returned by `ImguiContexts`. Do not call
`Context::frame()` or `Context::render()` yourself; the plugin owns the frame lifecycle.
`ImguiContext::context_mut()` is intentionally unavailable while `ImguiPrimaryContextPass` exposes
a live `Ui`, so safe systems cannot end or replace the frame behind another UI reference.

## Examples

Start with `simple`, then move to `app_integration` and `game_engine` as your integration needs
grow.

| Category | Example | Source | Run command | Purpose |
| --- | --- | --- | --- | --- |
| Basic | [`simple`][bevy-example-simple] | [`examples/basic/simple.rs`][bevy-example-simple] | `cargo run -p dear-imgui-bevy --features render --example simple` | Smallest visible Dear ImGui overlay in a normal Bevy app. |
| App | [`app_integration`][bevy-example-app-integration] | [`examples/app/app_integration.rs`][bevy-example-app-integration] | `cargo run -p dear-imgui-bevy --features render --example app_integration` | Plugin-style integration into an existing app/game loop with Bevy input policy. |
| Game engine | [`game_engine`][bevy-example-game-engine] | [`examples/game_engine/game_engine.rs`][bevy-example-game-engine] | `cargo run -p dear-imgui-bevy --features render --example game_engine`<br>`cargo run -p dear-imgui-bevy --features render,multi-viewport --example game_engine` | Docked editor surface with scene render-target texture interop, plus optional native multi-viewport. |
| Ecosystem | [`ecosystem`][bevy-example-ecosystem] | [`examples/ecosystem/ecosystem.rs`][bevy-example-ecosystem] | `cargo run -p dear-imgui-bevy --features render --example ecosystem` | Shared-frame ImPlot, ImNodes, and ImGuizmo integration. |
| Ecosystem | [`bevy_plot_controls`][bevy-example-plot-controls] | [`examples/ecosystem/bevy_plot_controls.rs`][bevy-example-plot-controls] | `cargo run -p dear-imgui-bevy --features render --example bevy_plot_controls` | Focused Bevy scene plus ImPlot controls demo. |

[bevy-example-simple]: https://github.com/Latias94/dear-imgui-rs/blob/main/backends/dear-imgui-bevy/examples/basic/simple.rs
[bevy-example-app-integration]: https://github.com/Latias94/dear-imgui-rs/blob/main/backends/dear-imgui-bevy/examples/app/app_integration.rs
[bevy-example-game-engine]: https://github.com/Latias94/dear-imgui-rs/blob/main/backends/dear-imgui-bevy/examples/game_engine/game_engine.rs
[bevy-example-ecosystem]: https://github.com/Latias94/dear-imgui-rs/blob/main/backends/dear-imgui-bevy/examples/ecosystem/ecosystem.rs
[bevy-example-plot-controls]: https://github.com/Latias94/dear-imgui-rs/blob/main/backends/dear-imgui-bevy/examples/ecosystem/bevy_plot_controls.rs

## Screenshots

<p>
  <img src="https://raw.githubusercontent.com/Latias94/dear-imgui-rs/main/screenshots/bevy-game-engine-multi-viewport.png" alt="dear-imgui-bevy game engine multi-viewport example" width="100%"/>
</p>
<p>
  <img src="https://raw.githubusercontent.com/Latias94/dear-imgui-rs/main/screenshots/bevy-app-integration.png" alt="dear-imgui-bevy app integration example" width="49%"/>
  <img src="https://raw.githubusercontent.com/Latias94/dear-imgui-rs/main/screenshots/bevy-ecosystem.png" alt="dear-imgui-bevy ecosystem example" width="49%"/>
</p>

## Cargo Features

| Feature | What it enables |
| --- | --- |
| `default` | Core plugin, context lifecycle, schedules, and input translation. No renderer is installed. |
| `wasm` | Selects the explicit `dear-imgui-rs` WASM provider contract. Required for every `wasm32-unknown-unknown` build. |
| `render` | Bevy `RenderApp` extraction, WGPU overlay renderer, `ImguiOverlayCamera`, `ImguiOverlayDisabled`, and `ImguiBevyTextures`. |
| `multi-viewport` | Native Dear ImGui PlatformIO window lifecycle bridge. Use with `render` for full routed rendering. |

ImPlot, ImNodes, ImGuizmo, and other `Ui`-based extension crates compose with this backend through
the same `ImguiPrimaryContextPass` frame. They are not `dear-imgui-bevy` feature flags; add the
extension crates you use to your application dependencies directly.

## Integration Guide

### Frame Lifecycle

`ImguiPlugin` installs three main-world schedules:

1. `ImguiBeginFrame` translates Bevy input and opens the Dear ImGui frame.
2. `ImguiPrimaryContextPass` runs your UI systems against the already-open frame.
3. `ImguiEndFrame` renders once and moves a one-shot `FrameSnapshot` through a bounded mailbox to
   the render world. `ImguiFrameOutput` exposes only its epoch and any capture error.

The main user-facing APIs are:

| Need | API |
| --- | --- |
| Add the backend | `ImguiPlugin` |
| Configure requested backend behavior | `ImguiBackendConfig` |
| Check runtime capability | `ImguiBackendStatus` |
| Draw UI inside Bevy schedules | `ImguiContexts` and `ImguiPrimaryContextPass` |
| Route gameplay/editor input | `input::ImguiInputCapture` |
| Select overlay render targets | `render::ImguiOverlayCamera` |
| Prevent offscreen scene targets from receiving the global overlay | `render::ImguiOverlayDisabled` |
| Show Bevy images in ImGui | `ImguiBevyTextures` |
| Observe queued viewport work | `ImguiViewportBridge::commands()` returns an owned `Vec` |

### Render Targets and Scene Views

With the `render` feature enabled, the backend moves ImGui frame snapshots into Bevy's render world,
commits request-bound texture feedback exactly once, and draws them through Bevy cameras. Add
`ImguiOverlayCamera` to the camera that should receive the overlay for a render target.

Editor-style scene views usually render Bevy content into an `Image`, register that image through
`ImguiBevyTextures`, and show it with `ui.image(...)`. Add `ImguiOverlayDisabled` to the offscreen
scene camera so the global ImGui overlay is not rendered back into the scene texture.

### Explicit Context Extraction

`ImguiContext::into_inner()` performs ordered, retryable backend shutdown. The error retains the
complete wrapper; inspect `error()` and recover it with `into_owner()`:

1. For `RenderWorldReleasePending`, insert the owner back into the main `World`, run the Bevy render
   schedule so it abandons the pending snapshot and releases managed bind groups, remove the owner,
   and retry `into_inner()`. Native texture bindings are not reset before that acknowledgement.
2. For `ViewportWorldReleasePending`, insert the owner back into the main `World`, run one
   `app.update()` so the queued secondary-window/camera despawns pass `ApplyDeferred` and their
   absence is acknowledged, remove the owner, and retry `into_inner()`.
3. For `RendererOwnership`, a foreign integration replaced only part of the Bevy renderer
   contract. The failed preflight leaves the native frame, render-world release state, callbacks,
   flags, and texture bindings unchanged. Restore the reported field through the integration that
   replaced it, or complete the external renderer takeover, then retry. A complete takeover is
   detected automatically and its foreign raw fields are preserved. Bevy-managed texture bindings
   are reset after the render world releases its GPU map, so the new renderer receives fresh create
   requests instead of inheriting stale Bevy texture IDs.

Once renderer release is requested or acknowledged, extra updates remain frozen and cannot reopen
a Dear ImGui frame while either retry path is pending. Backend userdata, exact name allocations,
capability flags, callback tables, monitor storage, and renderer fields are ownership contracts. A
replacement is reported as a sticky fail-closed error, the affected capability is revoked, and the
foreign value is preserved rather than silently reclaimed. Ordinary resource drop requests release
and revokes Bevy `Ui` access, but deterministic applications should use `into_inner()`: core Context
teardown does not continue into native destruction after an attachment release phase fails.

### Docking and Multi-Viewport

Docking inside the primary Bevy window works like normal Dear ImGui docking: enable docking on the
context, create a dockspace, then dock or drag windows inside it.

Native multi-viewport OS windows are experimental. They require a native target, `render`, and
`multi-viewport`. The backend creates Bevy windows for Dear ImGui platform viewports, maps
input/focus/cursor/IME feedback, and routes each viewport's draw data to the matching Bevy
`Window` render target. Detached-window z-order and dock-target edge cases are still preview-grade.

`ImguiViewportBridge::commands()` copies an owned observer snapshot, so retaining it cannot block a
native callback from enqueueing later Create or Destroy work. ECS integration consumes work through
`drain_commands()`. If callback reentry contends with the queue, or any callback/name/flag/monitor
contract drifts, `callback_error()` and `drain_commands()` report the precise sticky Rust-side
error; the callback never unwinds through C and the partial queue is not consumed.

Configure secondary-window presentation through `ImguiBackendConfig::viewport_window`. The
`ImguiViewportWindowConfig` value carries present mode, compositor alpha mode, maximum frame
latency, theme, and transparency as one policy so recreation cannot silently fall back to Bevy's
defaults. `ImguiViewportWindowConfig::from_window(&primary_window)` copies the primary window's
complete presentation policy.

Transparent secondary windows require an explicit alpha-preserving compositor mode:
`CompositeAlphaMode::PreMultiplied` or `CompositeAlphaMode::PostMultiplied`. `Auto`, `Opaque`, and
`Inherit` are rejected by `ImguiViewportWindowConfig::validate()` and `try_from_window()` because
they cannot guarantee that the compositor preserves the viewport alpha channel.

Each live secondary window owns a routed overlay camera. `ViewportFlags::NO_RENDERER_CLEAR`
configures that camera to preserve the existing target contents, and the bridge recreates the
camera and repairs its mapping if another system despawns it.

| Target / feature set | Lifecycle bridge | Input / feedback | Full rendering |
| --- | --- | --- | --- |
| Native without `multi-viewport` | No | No | No |
| Native with `render,multi-viewport` and Bevy `RenderApp` | Yes | Yes | Yes |
| Native with `multi-viewport` but no `RenderApp` | Yes | Yes | No |
| `wasm32-unknown-unknown` | No | No | No |

### Input Policy

The backend forwards Bevy input messages into Dear ImGui IO and does not consume or delete Bevy
messages. Gameplay and editor systems should use `ImguiInputCapture` as a policy hint after ImGui
has computed capture intent.

Current boundaries:

- pointer and keyboard capture are policy hints only;
- clipboard remains application-provided;
- accessibility nodes are not generated for Dear ImGui widgets;
- file drop, gamepad navigation, and Bevy picking integration are not part of this backend yet;
- wasm builds compile, but browser runtime IME and clipboard behavior depend on the host.

## Development Checks

Recommended checks for this crate:

```bash
cargo check -p dear-imgui-bevy --no-default-features
cargo check -p dear-imgui-bevy --features render
cargo check -p dear-imgui-bevy --target wasm32-unknown-unknown --no-default-features --features wasm
cargo check -p dear-imgui-bevy --target wasm32-unknown-unknown --no-default-features --features render,wasm
cargo nextest run -p dear-imgui-bevy
cargo nextest run -p dear-imgui-bevy --features render
cargo nextest run -p dear-imgui-bevy --features render,multi-viewport
cargo check -p dear-imgui-bevy --features render,multi-viewport --examples
```

Use `cargo run` commands from the examples table for manual smoke tests.
