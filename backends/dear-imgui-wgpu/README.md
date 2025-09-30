# dear-imgui-wgpu

WGPU renderer for Dear ImGui.

<p align="center">
  <img src="https://raw.githubusercontent.com/Latias94/dear-imgui/main/screenshots/game-engine-docking.png" alt="Docking (WGPU)" width="75%"/>
  <br/>
</p>

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
| Crate           | 0.3.x   |
| dear-imgui-rs   | 0.3.x   |
| wgpu            | 26      |

See also: [docs/COMPATIBILITY.md](https://github.com/Latias94/dear-imgui/blob/main/docs/COMPATIBILITY.md) for the full workspace matrix.

## Notes

- Targets native and Web (with `webgl`/`webgpu` features mapped to wgpu features).
- External dependency updates (wgpu) may require coordinated version bumps.

## Features

- Default: no extra features required for native builds
- `webgl`: forwards to `wgpu/webgl` (WASM + WebGL)
- `webgpu`: forwards to `wgpu/webgpu` (WASM + WebGPU)

Pick exactly one of `webgl` or `webgpu` for browser targets; for native leave both off.
