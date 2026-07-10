# dear-app

[![Crates.io](https://img.shields.io/crates/v/dear-app.svg)](https://crates.io/crates/dear-app)
[![Documentation](https://docs.rs/dear-app/badge.svg)](https://docs.rs/dear-app)

`dear-app` is the Winit + WGPU runtime for `dear-imgui-rs`. It keeps the main window, Dear ImGui
context, add-ons, and user `Application` alive while replacing only GPU-owned state after device
loss.

## Quick Start

```rust
use dear_app::{AppConfig, Application, FrameContext, RunError};
use dear_imgui_rs::{Condition, Ui};

struct Hello {
    frames: u64,
}

impl Application for Hello {
    fn frame(&mut self, context: &mut FrameContext<'_, '_>) -> Result<(), RunError> {
        self.frames += 1;
        let ui: &Ui = context.ui();
        ui.window("Hello")
            .size([360.0, 160.0], Condition::FirstUseEver)
            .build(|| ui.text(format!("Frame {}", self.frames)));
        Ok(())
    }
}

fn main() -> Result<(), RunError> {
    dear_app::run(AppConfig::default(), Hello { frames: 0 })
}
```

Configuration is a value, not a builder:

```rust
use dear_app::{AddOnsConfig, AppConfig, Theme, WgpuConfig, WgpuPreset};

let config = AppConfig {
    window_title: "Editor".to_owned(),
    theme: Some(Theme::Dark),
    addons: AddOnsConfig::auto(),
    wgpu: WgpuConfig::from_preset(WgpuPreset::HighPerformance),
    ..Default::default()
};
```

## Lifecycle

`Application` is the single owner of application state:

- `configure_imgui` runs once before renderer initialization.
- `initialized` runs once after the first GPU generation is ready.
- `event` receives events only for the live main window.
- `frame` is the only per-frame UI callback.
- `gpu_lost` runs before old GPU resources are invalidated.
- `gpu_recreated` runs after a replacement generation is committed.
- `shutdown` runs once before add-ons and the Dear ImGui context are destroyed.

Surface loss and resize do not recreate the application or Dear ImGui context. Only WGPU's
device-loss callback starts GPU recovery, and that callback communicates with the UI thread through
the Winit event loop.

## GPU Resources

External WGPU textures return an `ExternalTextureHandle` instead of a raw `TextureId`. Resolve the
handle through the current frame's `GpuApi` before submitting it to Dear ImGui. Resolution fails
after GPU recovery, so an old identifier cannot alias a resource from a newer generation.

Rebuild application-owned GPU resources from `gpu_recreated`. Managed Dear ImGui textures retain
their CPU data and are reset to `WantCreate` before the old renderer is torn down.

## Add-ons

The `implot`, `imnodes`, and `implot3d` features create extension contexts owned by the stable UI
state. Access them through `FrameContext::addons()`.
