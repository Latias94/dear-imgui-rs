# dear-app

[![Crates.io](https://img.shields.io/crates/v/dear-app.svg)](https://crates.io/crates/dear-app)
[![Documentation](https://docs.rs/dear-app/badge.svg)](https://docs.rs/dear-app)

`dear-app` is the Winit + WGPU runtime for `dear-imgui-rs`. It keeps the main window, Dear ImGui context, add-ons, and user application alive while replacing only GPU-owned state after device loss.

## Quick Start

The `0.22` release train requires Rust 1.95 or newer. Upgrade all workspace crates together and review the [native build policy migration](../docs/COMPATIBILITY.md#0220-native-build-policy). Create a binary crate and add the matching release:

```toml
[dependencies]
dear-app = "0.22"
```

Then use `dear_app::run_ui` for applications that only need persistent UI state:

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
                ui.same_line();
                ui.text(format!("Clicks: {clicks}"));
            });
    })
}
```

Run it with `cargo run`. `dear-app` re-exports the matching core crate as `dear_app::imgui`, so this starter cannot accidentally combine incompatible versions.

## Fallible Frame Closure

Move the same captured state to `run_frame` when a small application needs fallible work, explicit
exit, compiled add-ons, or the active GPU generation but does not need lifecycle hooks:

```rust
use std::io;

use dear_app::{AppConfig, RunError, run_frame};

fn main() -> Result<(), RunError> {
    let mut clicks = 0_u64;

    run_frame(AppConfig::default(), move |context| {
        let mut exit_requested = false;
        let mut failure_requested = false;
        let ui = context.ui();
        ui.window("Editor").build(|| {
            if ui.button("Increment") {
                clicks = clicks.saturating_add(1);
            }
            ui.text(format!("Clicks: {clicks}"));
            exit_requested = ui.button("Exit");
            failure_requested = ui.button("Return an error");
        });

        if exit_requested {
            context.request_exit();
        }
        if failure_requested {
            return Err(io::Error::other("the frame requested a user error"));
        }
        Ok(())
    })
}
```

The closure value remains alive across frames and GPU recovery. `FrameContext::addons()` exposes
the add-ons compiled and enabled by `AppConfig::addons`, while `FrameContext::gpu()` exposes the
current generation-aware WGPU API. `request_exit()` is normal control flow: the runtime performs
shutdown exactly once and returns `Ok(())`. A returned error wins even if the same callback also
requested exit; `RunError::Application` records `ApplicationStage::Frame` and retains the original
error through `std::error::Error::source`.

The complete runnable example is:

```text
cargo run -p dear-imgui-examples --bin fallible_frame
```

## Application Lifecycle

With `RedrawMode::Wait`, ordinary input redraws the live window. Background work and timers can
request frames through a thread-safe, coalescing handle obtained during `Application::initialized`:

```rust
use std::time::{Duration, Instant};
use dear_app::InitializedContext;

fn schedule_work(context: &InitializedContext<'_>) {
    let wake = context.wake_handle();
    wake.request_redraw_at(Instant::now() + Duration::from_millis(100)).unwrap();
    std::thread::spawn(move || {
        // Publish loaded assets to application-owned synchronized storage first.
        let _ = wake.request_redraw();
    });
}
```

Requests queue at most one wake event and retain the earliest deadline. Deadlines are one-shot;
schedule the next deadline after consuming the associated work. Handles survive GPU recreation
and return `AppWakeClosed` after runtime shutdown.

`Application::device_event` receives unmodified Winit raw input. Set `AppConfig::device_events` to
`DeviceEvents::WhenFocused` (the default), `Always`, or `Never`. Raw motion has no native window
target: use application focus and pointer ownership to choose a Scene/Game recipient, then request
a redraw through `DeviceEventContext::wake_handle()` when that input changes visible state.

Use `Application::record_render` for engine render-to-texture work. The hook runs after UI
construction on admitted frames and lends `RenderContext::encoder()`. Commands recorded there
precede Dear ImGui consumers on the renderer queue. The context also exposes the active device,
queue, GPU generation, admitted frame index, main framebuffer extent and surface format. The
runtime finishes the encoder and presents the surface; the application retains its scene targets.
`FrameContext::wake_handle()` can schedule an animation's next one-shot deadline from `run_frame`.

Move from `run_frame` to `Application` only when the program needs context initialization, raw
window events, GPU resources, device-loss recovery, or deterministic teardown. The frame body can
move unchanged into `Application::frame`:

```rust
use dear_app::{AppConfig, Application, FrameContext, RunError};

struct Editor {
    frames: u64,
}

impl Application for Editor {
    fn frame(&mut self, context: &mut FrameContext<'_>) -> Result<(), RunError> {
        self.frames += 1;
        let ui = context.ui();
        ui.window("Editor").build(|| {
            ui.text(format!("Frame {}", self.frames));
        });
        Ok(())
    }
}

fn main() -> Result<(), RunError> {
    dear_app::run(AppConfig::default(), Editor { frames: 0 })
}
```

`Application` is the single owner of application state:

- `configure_imgui` runs once before renderer initialization.
- `initialized` runs once after the first GPU generation is ready.
- `event` receives events for every registered native window. `EventContext::outcome()` identifies
  the exact native instance, actual forwarding, capture intent, and platform action. `window()` is
  the event's native window; `main_window()` is the stable host window. Closing a secondary does
  not exit the application.
- `prepare_frame` mutates Context-owned resources before the next frame opens.
- `frame` is the only per-frame UI callback.
- `gpu_lost` runs before old GPU resources are invalidated.
- `gpu_recreated` runs after a replacement generation is committed.
- `shutdown` runs once before add-ons and the Dear ImGui context are destroyed.

`InitContext` exposes the complete Dear ImGui `Context` only from `configure_imgui`, before platform
and renderer attachment, for extension APIs that require it. `initialized` receives
`InitializedContext`; runtime event, pre-frame, and shutdown hooks likewise expose only narrow IO,
style, font-atlas, and managed-texture capabilities. The runtime compares the Context identity,
native frame sequence, and lifecycle state at every callback boundary, so opening and closing a
frame inside a callback is still reported as `RunError::ImGuiFrameOwnership`.

Every failing hook is wrapped with its typed `ApplicationStage`, while the hook's original
`RunError` remains in the source chain. The first actual failure remains primary. Shutdown still
runs exactly once, and a shutdown or backend-release failure is returned only when no earlier
runtime or event-loop failure exists.

The feature-free lifecycle example implements every hook and displays real hook activity, persistent state, and the active GPU generation:

```text
cargo run -p dear-imgui-examples --bin application_lifecycle
```

Surface loss and resize do not recreate the application or Dear ImGui context. In the default single-window mode, `dear-app` acquires and, when necessary, recovers the main surface before calling any per-frame application hook. Timeout and occlusion skip that frame without advancing application or test state. Native multi-viewport mode completes secondary work first, as described below. Only WGPU device loss starts GPU recovery, and that callback communicates with the UI thread through the Winit event loop.

## Test Engine

Enable the optional `test-engine` feature when the application owns an interactive Dear ImGui Test Engine. The matching crate is available as `dear_app::test_engine`; return its `TestEngine` from `Application::test_engine`. The runtime then owns the complete `render -> pre-swap -> present -> post-swap` transaction for every admitted frame. Applications must not call presentation hooks themselves.

Standalone test programs that own their whole frame loop should use `TestRunner::run_graphical` instead. Headless runs use the explicitly virtual presentation path and do not claim graphical swap coverage.

## Configuration

Configuration is a value, not a builder. Docking is opt-in:

```rust
use dear_app::{AddOnsConfig, AppConfig, DockingConfig, Theme, WgpuConfig, WgpuPreset};

let config = AppConfig {
    window_title: "Editor".to_owned(),
    theme: Some(Theme::Dark),
    docking: DockingConfig::full_viewport(),
    addons: AddOnsConfig::auto(),
    wgpu: WgpuConfig::from_preset(WgpuPreset::HighPerformance),
    ..Default::default()
};
```

`DockingConfig` enables docking independently of native secondary windows. To enable native
multi-viewport, compile the `multi-viewport` Cargo feature and set `AppConfig::multi_viewport = true`.
The default remains a single native window. Setting `VIEWPORTS_ENABLE` manually without this opt-in
is rejected; application callbacks cannot change the runtime's configured viewport mode.

```text
cargo run -p dear-app --example multi_viewport --features multi-viewport
```

The runtime composes `WinitPlatform` and `WinitViewportRoute`, using their single native registry
and secondary-surface owner. `Application::record_render` commands execute before every UI consumer,
including undocked windows. Secondary work completes before main-surface acquisition, so timeout,
occlusion, or main-surface recovery does not suppress those windows' updates. In multi-viewport mode,
the UI and recording hooks therefore run even when main presentation is skipped.

`Application::viewport_frame` receives `ViewportFrameContext` with secondary reports and an optional
main observation. Clone observations into editor history to preserve native instance IDs, external
texture revisions, and observed recording/submission/presentation stages. The hook still receives
earlier secondary evidence if main acquisition or recording fails. These stages do not prove GPU
completion or visible pixels, and history retains no native windows or GPU objects.

GPU recovery preserves the Context, main native registration, docking/application state, and add-ons.
It releases the old renderer and every configured Surface before destroying that generation's
Device. The replacement generation creates a fresh main Surface for the same main native window;
it never reconfigures an old generation's Surface with a new Device. It retires secondary windows
and rebuilds the platform runtime before attaching the new GPU route. Undocked UI creates replacement native windows
with new `NativeWindowInstanceId` values. Logical window names and mutable viewport routing IDs do
not stand in for those allocation identities. Rebuild engine GPU resources in `gpu_recreated`, just
as in single-window mode.

GPU callbacks publish generation-specific loss/fault signals before waking the event loop. A
redraw can therefore recover confirmed device loss even when its user event has not been dispatched.
Surface rejection alone does not imply device loss; independent validation faults remain errors.

## GPU Resources

External WGPU textures return an `ExternalTextureHandle` instead of a raw `TextureId`. Resolve the handle through the current frame's `GpuApi` before submitting it to Dear ImGui. Resolution fails after GPU recovery, so an old identifier cannot alias a resource from a newer generation.

Rebuild application-owned GPU resources from `gpu_recreated`. Managed Dear ImGui textures retain their CPU data and are reset to `WantCreate` before the old renderer is torn down.

## Add-ons

The `implot`, `imnodes`, and `implot3d` features create extension contexts owned by the stable UI
state. Access them through `FrameContext::addons()` from `run_frame` or `Application::frame`.
