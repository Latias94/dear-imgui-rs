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
renderer.render_draw_data(imgui.render(), &mut render_pass)?;
```

Each `WgpuRenderer` is bound to the `Context` passed to `new` or `init_with_context`. Create one renderer per context in multi-context applications. `render_context()` and `render_context_with_fb_size()` finalize only that bound context and return `RendererError::ContextMismatch` for another context. The draw-data entry points likewise use the bound context's `PlatformIO` and require that context to be current.

## Native multi-viewport

The Winit and SDL3 routes use one shared renderer runtime. Select exactly one platform adapter:

- `multi-viewport-winit`
- `multi-viewport-sdl3`

They are mutually exclusive and native-only. The selected feature enables
`dear-imgui-rs/multi-viewport`; do not enable both routes through `--all-features`.

Secondary windows need the `Instance` and `Adapter` that created the renderer's `Device`. Keep
them in `WgpuInitInfo`, place the renderer at a stable address, initialize the platform callbacks,
and then install renderer callbacks before Dear ImGui creates any secondary platform window:

```rust,no_run
use dear_imgui_rs::Context;
use dear_imgui_wgpu::{WgpuInitInfo, WgpuRenderer, multi_viewport as wgpu_mvp, wgpu};
use dear_imgui_winit::multi_viewport as winit_mvp;

# fn enable_viewports(
#     imgui: &mut Context,
#     main_window: &winit::window::Window,
#     instance: wgpu::Instance,
#     adapter: wgpu::Adapter,
#     device: wgpu::Device,
#     queue: wgpu::Queue,
#     format: wgpu::TextureFormat,
# ) -> Result<Box<WgpuRenderer>, Box<dyn std::error::Error>> {
imgui.enable_multi_viewport();
winit_mvp::init_multi_viewport_support(imgui, main_window);

let init = WgpuInitInfo::new(device, queue, format)
    .with_instance(instance)
    .with_adapter(adapter);
let mut renderer = Box::new(WgpuRenderer::new(init, imgui)?);

// SAFETY: Box keeps the renderer address stable. The context, renderer, Winit
// windows, and GPU objects remain alive and single-threaded until shutdown.
unsafe { wgpu_mvp::enable(renderer.as_mut(), imgui)? };
# Ok(renderer)
# }
```

For SDL3, use `dear_imgui_wgpu::multi_viewport_sdl3::enable` after the SDL3 platform backend has
installed its viewport handlers. The same safety and ordering contract applies.

The renderer claims only the five `Renderer_*` slots in `ImGuiPlatformIO`. Registration fails
instead of replacing foreign renderer callbacks or `RendererUserData`, rejects secondary windows
that already exist, and prevents one renderer from backing multiple ImGui contexts. While
registered, do not move, reinitialize, shut down, concurrently access, or drop the renderer, and
do not replace viewport `RendererUserData`.

From a repository checkout, run the native Winit/WGPU Test Engine smoke to move a window into a real secondary OS viewport, render its surface, merge it back, and verify ordered teardown. `test-engine` is source-only, so this command intentionally builds Dear ImGui from source:

```bash
DEAR_IMGUI_VIEWPORT_SMOKE=1 cargo run -p dear-imgui-examples --bin multi_viewport_wgpu --features "multi-viewport test-engine"
```

Shut down in ownership order:

```rust,no_run
# use dear_imgui_rs::Context;
# use dear_imgui_wgpu::multi_viewport as wgpu_mvp;
# use dear_imgui_winit::multi_viewport as winit_mvp;
# use dear_imgui_wgpu::WgpuRenderer;
# fn shutdown(renderer: &mut WgpuRenderer, imgui: &mut Context) -> Result<(), Box<dyn std::error::Error>> {
wgpu_mvp::shutdown_multi_viewport_support(imgui)?;
renderer.shutdown(imgui)?;
winit_mvp::shutdown_multi_viewport_support(imgui);
# Ok(())
# }
```

The renderer helper destroys secondary windows before releasing renderer resources and clears only the callback slots it still owns. Explicit shutdown is required before the renderer, context, platform backend, windows, instance, adapter, device, or queue is dropped. `WgpuRenderer::shutdown` requires the matching context so GPU resources and managed texture IDs are invalidated together; shutdown and renderer reinitialization return an error while the viewport runtime is active. Shut down before dropping the bound context. The renderer retains a context liveness token and remains on the context's UI thread.

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
  - `webgl` / `webgpu` select the WASM route for the default `wgpu-30` build
  - With `wgpu-29`, use `webgl-wgpu29` / `webgpu-wgpu29` instead
  - With `wgpu-28`, use `webgl-wgpu28` / `webgpu-wgpu28` instead
  - With `wgpu-27`, use `webgl-wgpu27` / `webgpu-wgpu27` instead

Select exactly one WGPU major. `webgl` and `webgpu` may be enabled individually or together for
the selected major; enabling both lets WGPU choose an available browser backend at runtime. Leave
both off for native builds.
