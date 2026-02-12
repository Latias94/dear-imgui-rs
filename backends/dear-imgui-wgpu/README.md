# dear-imgui-wgpu

WGPU renderer for Dear ImGui.

## Quick Start

```rust
use dear_imgui_rs::Context;
use dear_imgui_wgpu::{WgpuRenderer, WgpuInitInfo, GammaMode};

// device, queue, surface_format prepared ahead
let mut renderer = WgpuRenderer::new(WgpuInitInfo::new(device, queue, surface_format), &mut imgui)?;

// Optional: unify gamma policy across backends
renderer.set_gamma_mode(GammaMode::Auto); // Auto | Linear | Gamma22

// per-frame
renderer.render_draw_data(&imgui.render(), &mut render_pass)?;
```

## Selecting wgpu version

By default this crate uses `wgpu` v28.

If your ecosystem is pinned to `wgpu` v27 (e.g. a release train), select it explicitly:

```toml
[dependencies]
dear-imgui-wgpu = { version = "0.9", default-features = false, features = ["wgpu-27"] }
```

## What You Get

- ImGui v1.92 texture system integration (create/update/destroy)
- Multi-frame buffering and device-object management
- Format-aware or user-controlled gamma (see below)

## sRGB / Gamma

- Default `GammaMode::Auto`: picks `gamma=2.2` for sRGB targets and `1.0` for linear targets.
- You can force `Linear` (1.0) or `Gamma22` (2.2).
- Pair this with your swapchain format to avoid double correction.

## Compatibility

| Item            | Version |
|-----------------|---------|
| Crate           | 0.9.x   |
| dear-imgui-rs   | 0.9.x   |
| wgpu            | 28 (default), 27 (feature `wgpu-27`) |

See also: [docs/COMPATIBILITY.md](https://github.com/Latias94/dear-imgui-rs/blob/main/docs/COMPATIBILITY.md) for the full workspace matrix.

## Notes

- Targets native and Web (with `webgl`/`webgpu` features mapped to wgpu features).
- External dependency updates (wgpu) may require coordinated version bumps.

## Features

- Default: no extra features required for native builds
- WGPU version selection (mutually exclusive)
  - `wgpu-28` (default)
  - `wgpu-27`
- WASM targets
  - `webgl` / `webgpu` select the WASM route for the default `wgpu-28` build
  - With `wgpu-27`, use `webgl-wgpu27` / `webgpu-wgpu27` instead

Pick exactly one of `webgl` or `webgpu` for browser targets; for native leave both off.
