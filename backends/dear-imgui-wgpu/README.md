# dear-imgui-wgpu

WGPU renderer for Dear ImGui.

## Quick Start

```rust
use dear_imgui_rs::Context;
use dear_imgui_wgpu::{WgpuRenderer, WgpuInitInfo, GammaMode};

// device, queue, surface_format prepared ahead
let mut imgui = Context::create();
let mut renderer = WgpuRenderer::new(WgpuInitInfo::new(device, queue, surface_format), &mut imgui)?;

// Optional: unify gamma policy across backends
renderer.set_gamma_mode(GammaMode::Auto); // Auto | Linear | Gamma22

// per-frame
let frame = imgui.render();
renderer.render(frame, &mut render_pass)?;
```

Each `WgpuRenderer` is bound to the `Context` passed to `new` or `init_with_context` and owns its sole renderer consumer generation. Create one renderer per context in multi-context applications. `render()` consumes the Context-borrowed frame, processes pointer-free managed texture requests, reconciles feedback, and only then reads draw commands. `render_context()` and `render_context_with_fb_size()` are convenience methods that finalize only the bound context and return `RendererError::ContextMismatch` for another context.

## Native multi-viewport

The Winit and SDL3 routes use one shared owning renderer core with platform-specific public
runtime types. Select exactly one platform adapter:

- `multi-viewport-winit`
- `multi-viewport-sdl3`

They are mutually exclusive and native-only. The selected feature enables
`dear-imgui-rs/multi-viewport`; do not enable both routes through `--all-features`.

Secondary windows need the `Instance` and `Adapter` that created the renderer's `Device`. Keep
them in `WgpuInitInfo`, attach the owning platform runtime first, and then move the renderer into
the matching WGPU runtime before Dear ImGui creates a secondary platform window:

```rust,no_run
use dear_imgui_rs::Context;
use std::sync::Arc;
use dear_imgui_wgpu::{WgpuInitInfo, WgpuRenderer, wgpu};
use dear_imgui_wgpu::multi_viewport::WinitViewportRuntime;
use dear_imgui_winit::multi_viewport::WinitPlatformRuntime;

# fn enable_viewports(
#     imgui: &mut Context,
#     main_window: Arc<winit::window::Window>,
#     instance: wgpu::Instance,
#     adapter: wgpu::Adapter,
#     device: wgpu::Device,
#     queue: wgpu::Queue,
#     format: wgpu::TextureFormat,
# ) -> Result<(WinitPlatformRuntime, WinitViewportRuntime), Box<dyn std::error::Error>> {
imgui.enable_multi_viewport();
let platform = WinitPlatformRuntime::new(imgui, main_window)?;

let init = WgpuInitInfo::new(device, queue, format)
    .with_instance(instance)
    .with_adapter(adapter);
let renderer = WgpuRenderer::new(init, imgui)?;
let renderer = WinitViewportRuntime::attach(imgui, renderer)?;
# Ok((platform, renderer))
# }
```

For SDL3, initialize `Sdl3PlatformBackend` first and then call
`dear_imgui_wgpu::multi_viewport_sdl3::Sdl3ViewportRuntime::attach`. Both typed constructors
consume `WgpuRenderer`; no caller-owned stable address or unsafe registration contract remains.

The renderer claims only the five `Renderer_*` slots in `ImGuiPlatformIO`. Registration fails
instead of replacing foreign renderer callbacks or `RendererUserData`, rejects secondary windows
that already exist, and requires an active Context `Platform` attachment. Attach is transactional:
the error returns the unchanged renderer through `WgpuViewportAttachError`. Moving the runtime does
not move callback-visible renderer storage. Callback replacement, panic, reentry, rendering, and
terminal surface failures are contained at the C ABI boundary and returned by `poll_fault` or the
next Rust runtime entry.

From a repository checkout, run the native Winit/WGPU Test Engine smoke to move a window into a real secondary OS viewport, render its surface, merge it back, and verify ordered teardown. `test-engine` is source-only, so this command intentionally builds Dear ImGui from source:

```bash
DEAR_IMGUI_VIEWPORT_SMOKE=1 cargo run -p dear-imgui-examples --bin multi_viewport_wgpu --features "multi-viewport test-engine"
```

Shut down renderer ownership before the platform runtime:

```rust,no_run
# use dear_imgui_rs::Context;
# use dear_imgui_wgpu::multi_viewport::WinitViewportRuntime;
# use dear_imgui_winit::multi_viewport::WinitPlatformRuntime;
# fn shutdown(renderer: &mut WinitViewportRuntime, platform: &mut WinitPlatformRuntime, imgui: &mut Context) -> Result<(), Box<dyn std::error::Error>> {
renderer.shutdown(imgui)?;
platform.shutdown()?;
# Ok(())
# }
```

The renderer runtime releases `RendererUserData`, surfaces, callbacks, and renderer GPU resources;
it never enters the platform-window phase. The Winit or SDL3 platform owner remains solely
responsible for destroying native windows. Context-first teardown invokes the same shared state
machine in ordered renderer-resource and platform-window phases.

Explicit renderer shutdown is idempotent and retryable. In particular, an outstanding detached
snapshot leaves the runtime in `Detached` with its renderer retained; finish or abandon the epoch,
poll Context completions, and call `shutdown` again. Runtime `Drop` performs terminal best-effort
cleanup because it cannot report errors or safely wait for detached work. Use explicit shutdown
when managed texture reset or callback ownership errors matter. Foreign callback and backend-state
replacements are preserved rather than overwritten during either path.

## Selecting wgpu version

The current `main` branch, which is targeting the unpublished 0.16.0 release, defaults to WGPU 30.

If your ecosystem is pinned to `wgpu` v29, v28, or v27, select it explicitly:

```toml
[dependencies]
dear-imgui-wgpu = { git = "https://github.com/Latias94/dear-imgui-rs", branch = "main", default-features = false, features = ["wgpu-29"] }
```

```toml
[dependencies]
dear-imgui-wgpu = { git = "https://github.com/Latias94/dear-imgui-rs", branch = "main", default-features = false, features = ["wgpu-28"] }
```

```toml
[dependencies]
dear-imgui-wgpu = { git = "https://github.com/Latias94/dear-imgui-rs", branch = "main", default-features = false, features = ["wgpu-27"] }
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

| Track | wgpu support |
|-------|--------------|
| `main` (unpublished 0.16.0) | 30 (default), 29 (`wgpu-29`), 28 (`wgpu-28`), 27 (`wgpu-27`) |

See also: [docs/COMPATIBILITY.md](https://github.com/Latias94/dear-imgui-rs/blob/main/docs/COMPATIBILITY.md) for the full workspace matrix.

## Notes

- Targets native and Web (with `webgl`/`webgpu` features mapped to wgpu features).
- Native multi-viewport is not available on WebAssembly.
- External dependency updates (wgpu) may require coordinated version bumps.

## Features

- Default: no extra features required for native builds
- WGPU version selection (mutually exclusive)
  - `wgpu-30` (default)
  - `wgpu-29`
  - `wgpu-28`
  - `wgpu-27`
- Diagnostics
  - `tracing` enables renderer debug and warning events; it is off by default
- WASM targets
  - Every WebGL/WebGPU route automatically enables the matching `dear-imgui-rs/wasm` import path
  - `webgl` / `webgpu` select the WASM route for the default `wgpu-30` build
  - With `wgpu-29`, use `webgl-wgpu29` / `webgpu-wgpu29` instead
  - With `wgpu-28`, use `webgl-wgpu28` / `webgpu-wgpu28` instead
  - With `wgpu-27`, use `webgl-wgpu27` / `webgpu-wgpu27` instead
  - `wasm-font-atlas-experimental` also enables the required core WASM import provider

Select exactly one WGPU major. `webgl` and `webgpu` may be enabled individually or together for
the selected major; enabling both lets WGPU choose an available browser backend at runtime. Leave
both off for native builds.
