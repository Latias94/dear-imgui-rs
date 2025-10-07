# dear-app

[![Crates.io](https://img.shields.io/crates/v/dear-app.svg)](https://crates.io/crates/dear-app)
[![Documentation](https://docs.rs/dear-app/badge.svg)](https://docs.rs/dear-app)

Convenient Dear ImGui application runner for `dear-imgui-rs`, bundling Winit + WGPU setup into a tiny API. It hides boilerplate, exposes ergonomic callbacks, and can initialize popular add-ons (ImPlot, ImNodes, ImPlot3D) behind feature flags.

## Features

- Winit + WGPU app bootstrap with sensible defaults
- Per-frame UI closure (`run_simple`) and a configurable builder (`AppBuilder`)
- Optional add-ons via features: `implot`, `imnodes`, `implot3d`
- Docking helpers, theme presets, INI path selection
- Lifecycle callbacks: setup/style/fonts/post-init/event/exit

## Quick Start

```toml
[dependencies]
dear-app = "0.4"

# Optional add-ons (enable any subset)
dear-app = { version = "0.4", features = ["implot", "imnodes", "implot3d"] }
```

Minimal usage:

```rust
use dear_app::run_simple;
use dear_imgui_rs::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    run_simple(|ui| {
        ui.window("Hello")
            .size([360.0, 160.0], Condition::FirstUseEver)
            .build(|| ui.text("Hello from dear-app!"));
    })?;
    Ok(())
}
```

Builder with add-ons and docking/theme presets:

```rust
use dear_app::{AppBuilder, AddOnsConfig, RunnerConfig, Theme};
use dear_imgui_rs as imgui;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = RunnerConfig { theme: Some(Theme::Dark), ..Default::default() };
    let addons = AddOnsConfig::auto(); // enable compiled add-ons

    AppBuilder::new()
        .with_config(cfg)
        .with_addons(addons)
        .on_frame(|ui: &imgui::Ui, addons| {
            ui.window("App").build(|| {
                ui.text("Docking and WGPU are ready!");

                #[cfg(feature = "implot")]
                if let Some(pc) = addons.implot { let plot = ui.implot(pc); let _ = plot; }

                #[cfg(feature = "imnodes")]
                if let Some(nc) = addons.imnodes { let _ = nc; /* ui.imnodes(nc) ... */ }

                #[cfg(feature = "implot3d")]
                if let Some(pc3) = addons.implot3d { let _ = pc3; }
            });
        })
        .run()?;

    Ok(())
}
```

## Notes

- Backends: Uses `dear-imgui-winit` and `dear-imgui-wgpu` internally.
- Fonts/FreeType: Configure in the `on_fonts` callback; FreeType can be enabled via `dear-imgui-rs/freetype`.
