# dear-app

[![Crates.io](https://img.shields.io/crates/v/dear-app.svg)](https://crates.io/crates/dear-app)
[![Documentation](https://docs.rs/dear-app/badge.svg)](https://docs.rs/dear-app)

`dear-app` is the Winit + WGPU runtime for `dear-imgui-rs`. It keeps the main window, Dear ImGui context, add-ons, and user application alive while replacing only GPU-owned state after device loss.

## Quick Start

Create a binary crate and add the unreleased `0.16.0-alpha.2` candidate from `main`:

```toml
[dependencies]
dear-app = { git = "https://github.com/Latias94/dear-imgui-rs", branch = "main", package = "dear-app" }
```

After publication, replace that dependency with `dear-app = "=0.16.0-alpha.2"`.

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

## Application Lifecycle

Implement `Application` when the program needs context initialization, raw window events, GPU resources, device-loss recovery, or deterministic teardown:

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
- `event` receives events only for the live main window.
- `prepare_frame` mutates Context-owned resources before the next frame opens.
- `frame` is the only per-frame UI callback.
- `gpu_lost` runs before old GPU resources are invalidated.
- `gpu_recreated` runs after a replacement generation is committed.
- `shutdown` runs once before add-ons and the Dear ImGui context are destroyed.

The feature-free lifecycle example implements every hook and displays real hook activity, persistent state, and the active GPU generation:

```text
cargo run -p dear-imgui-examples --bin application_lifecycle
```

Surface loss and resize do not recreate the application or Dear ImGui context. `dear-app` acquires and, when necessary, recovers the main surface before calling any per-frame application hook. Timeout and occlusion skip the frame without advancing application or test state. Only WGPU device loss starts GPU recovery, and that callback communicates with the UI thread through the Winit event loop.

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

`DockingConfig` configures docking in the main window; it does not enable Dear ImGui platform multi-viewport. `dear-app` rejects `ConfigFlags::VIEWPORTS_ENABLE` in 0.16 because its single-window recovery model does not own secondary platform windows. Use the Winit or SDL3 owning runtime examples when an application needs native secondary windows.

## GPU Resources

External WGPU textures return an `ExternalTextureHandle` instead of a raw `TextureId`. Resolve the handle through the current frame's `GpuApi` before submitting it to Dear ImGui. Resolution fails after GPU recovery, so an old identifier cannot alias a resource from a newer generation.

Rebuild application-owned GPU resources from `gpu_recreated`. Managed Dear ImGui textures retain their CPU data and are reset to `WantCreate` before the old renderer is torn down.

## Add-ons

The `implot`, `imnodes`, and `implot3d` features create extension contexts owned by the stable UI state. Access them through `FrameContext::addons()`.
