# dear-imgui-wgpu

WGPU renderer for Dear ImGui.

## Quick Start

```rust
use dear_imgui_rs::Context;
use dear_imgui_wgpu::{FramebufferExtent, GammaMode, WgpuInitInfo, WgpuRenderer};

// device, queue, surface_format prepared ahead
let mut imgui = Context::create();
let mut renderer = WgpuRenderer::new(WgpuInitInfo::new(device, queue, surface_format), &mut imgui)?;

// Optional: unify gamma policy across backends
renderer.set_gamma_mode(GammaMode::Auto); // Auto | Linear | Gamma22

// per-frame
let frame = imgui.render(renderer.renderer_consumer()?);
let framebuffer_extent = FramebufferExtent::from_texture(&surface_texture.texture);
renderer.render(frame, &mut render_pass, framebuffer_extent)?;
```

Each `WgpuRenderer` is fully initialized and bound to the `Context` passed to `new`; there is no public empty or two-phase state. Create one renderer per context in multi-context applications. After `shutdown`, create a replacement renderer instead of reinitializing the old value. `render()` is the normal path: it consumes a `PendingFrame`, processes pointer-free managed texture requests, reconciles feedback, and only then reads draw commands. Integrations that must reconcile before acquiring the main surface can split that work into `reconcile_frame()` followed by `render_reconciled()`. Both render methods require the physical extent of the actual color attachment; when rendering to a surface, derive it from the acquired `SurfaceTexture` instead of assuming the configured size still matches.

## External texture views

Register an application-owned texture view. The renderer clones the view handle, but the application must not explicitly destroy the underlying GPU resource until the handle is unregistered:

```rust,no_run
# use dear_imgui_rs::Ui;
# use dear_imgui_wgpu::{RendererResult, WgpuRenderer, wgpu};
# fn external_texture(
#     renderer: &mut WgpuRenderer,
#     ui: &Ui,
#     view: &wgpu::TextureView,
#     replacement_view: &wgpu::TextureView,
# ) -> RendererResult<()> {
let texture = renderer.register_external_texture(view)?;

ui.image(texture.texture_id(), [320.0, 180.0]);

renderer.update_external_texture(texture, replacement_view)?;
renderer.unregister_external_texture(texture)?;
# Ok(())
# }
```

Sampling is renderer state rather than texture ownership. The renderer owns the standard linear and nearest samplers; enqueue an explicit draw-list command around images that need nearest sampling:

```rust,no_run
# use dear_imgui_rs::Ui;
# use dear_imgui_wgpu::ExternalTextureId;
# fn sampling(ui: &Ui, texture: ExternalTextureId) {
let draw_list = ui.get_window_draw_list();
draw_list.set_sampler_nearest();
ui.image(texture.texture_id(), [320.0, 180.0]);
draw_list.set_sampler_linear();
# }
```

See `wgpu_rtt_gameview` for a runnable linear/nearest switching example.

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
use dear_imgui_wgpu::{WgpuInitInfo, WgpuRenderer, WgpuViewportSurfaceConfig, wgpu};
use dear_imgui_wgpu::multi_viewport::WinitViewportRuntime;
use dear_imgui_winit::{HiDpiMode, WinitPlatform, multi_viewport::WinitPlatformRuntime};

# fn enable_viewports(
#     imgui: &mut Context,
#     main_window: Arc<winit::window::Window>,
#     instance: wgpu::Instance,
#     adapter: wgpu::Adapter,
#     device: wgpu::Device,
#     queue: wgpu::Queue,
#     format: wgpu::TextureFormat,
# ) -> Result<(WinitPlatform, WinitPlatformRuntime, WinitViewportRuntime), Box<dyn std::error::Error>> {
imgui.enable_multi_viewport();
let mut platform = WinitPlatform::new(imgui)?;
platform.attach_window(Arc::clone(&main_window), HiDpiMode::Default, imgui)?;
let runtime = WinitPlatformRuntime::new(imgui, &platform)?;

let viewport_surface = WgpuViewportSurfaceConfig {
    present_mode: wgpu::PresentMode::AutoNoVsync,
    ..Default::default()
};
let init = WgpuInitInfo::new(device, queue, format)
    .with_instance(instance)
    .with_adapter(adapter)
    .with_viewport_surface_config(viewport_surface);
let renderer = WgpuRenderer::new(init, imgui)?;
let renderer = WinitViewportRuntime::attach(imgui, &runtime, renderer)?;
# Ok((platform, runtime, renderer))
# }
```

`WgpuViewportSurfaceConfig` defaults to `Fifo`, opaque composition, and a maximum frame latency of
`2` for compatibility with WGPU's normal surface defaults. Use
`WgpuViewportSurfaceConfig::from(&main_surface_config)` when secondary windows should inherit the
main surface's scheduling and compositor policy, or set `present_mode` to `AutoNoVsync` to prefer
`Immediate` and then `Mailbox` while retaining WGPU's portable `Fifo` fallback.

The renderer currently produces sRGB UI output and does not perform HDR transfer-function or
wide-gamut conversion. WGPU 30 secondary surfaces therefore request `SurfaceColorSpace::Srgb`
explicitly and reject a render-target format that the surface cannot present in sRGB. Keep the
main surface in the same sRGB contract; HDR or wide-gamut output needs an application-owned color
conversion pass rather than a different secondary-surface setting.

Secondary viewports inherit the renderer pipeline's multisample and depth-stencil contract. The
runtime owns matching per-window MSAA resolve and depth-stencil attachments, suspends acquisition
while a native framebuffer has a zero dimension, and rebuilds attachments from the platform
owner's current physical size after resize or DPI changes. Attachment fails transactionally when
the adapter cannot support the configured formats and sample count.

A lost secondary surface is rebuilt from that viewport's still-live platform window. Successful
surface recreation leaves the renderer runtime and every other viewport attached. If recreation
fails, the backend requests closure of only the affected viewport and reports the creation error.
WGPU exposes Device loss separately through an application-owned, single-slot callback; if that
callback fires, recreate the Device, Queue, renderer, and all GPU resources before reattaching the
viewport runtime.

The renderer opens a fresh upload-resource arena for each `PendingFrame` epoch. Every viewport
draw then uses a separate pass slot with its own vertex, index, uniform, and sampler bindings, so
command buffers cannot observe data uploaded for another viewport or a later epoch. The renderer
does not recycle upload buffers across epochs because submission of the application-owned encoder
is not observable. Before invoking default multi-viewport callbacks, call
`runtime.reconcile_context(&mut imgui)` and retain the returned `ReconciledFrame`; this both
prepares the exact frame epoch and applies managed-texture feedback before exposing draw data or
platform-window rendering. Engine-style frame runners that already own a `FrameToken` can instead
call `runtime.reconcile_frame(frame)` without reborrowing the Context. A callback reached without
that preparation fails with
`RendererError::FrameNotPrepared` instead of reusing the preceding frame's resources.

For SDL3, initialize `Sdl3PlatformBackend` first and then call
`dear_imgui_wgpu::multi_viewport_sdl3::Sdl3ViewportRuntime::attach(imgui, &platform, renderer)`.
The safe constructors require the matching live platform owner and reject Context mismatches,
shutdown owners, and callback ownership drift before interpreting any native handle. Custom
platforms can use the explicitly unsafe `attach_unchecked` escape hatch only after proving the
Winit or SDL3 `PlatformHandle` contract. Both typed constructors consume `WgpuRenderer`; no
caller-owned stable address is required.

The renderer claims only the five `Renderer_*` slots in `ImGuiPlatformIO`. Registration fails
instead of replacing foreign renderer callbacks or `RendererUserData`, rejects secondary windows
that already exist, and requires an active Context `Platform` attachment. Attach is transactional:
the error returns the unchanged renderer through `WgpuViewportAttachError`. Moving the runtime does
not move callback-visible renderer storage. Callback replacement, panic, reentry, rendering, and
unrecoverable surface validation failures are contained at the C ABI boundary and returned by
`poll_fault` or the next Rust runtime entry. A terminal fault revokes renderer viewport capability
and stops create/resize/render/present work. Its `Renderer_DestroyWindow` callback remains
available only for cleanup: a Context- and viewport-identity sidecar releases the owned WGPU
surface even when foreign code cleared or replaced `RendererUserData`, before the platform backend
destroys the native window.

From a repository checkout, run the same native Winit/WGPU Test Engine contract used by the
release gate. It moves a window into a real secondary OS viewport, renders its GPU surface, merges
it back, and verifies ordered teardown. `test-engine` is source-only, so this command intentionally
builds Dear ImGui from source:

```bash
python3 tools/ci/run_contract.py multi-viewport-smoke
```

Linux CI supplies Xvfb and Mesa/Lavapipe. Missing display or software-GPU infrastructure is an
infrastructure failure, not a skipped success.

Shut down renderer ownership before the platform runtime:

```rust,no_run
# use dear_imgui_rs::Context;
# use dear_imgui_wgpu::multi_viewport::WinitViewportRuntime;
# use dear_imgui_winit::multi_viewport::WinitPlatformRuntime;
# fn shutdown(renderer: &mut WinitViewportRuntime, platform: &mut WinitPlatformRuntime, imgui: &mut Context) -> Result<(), Box<dyn std::error::Error>> {
renderer.shutdown(imgui)?;
platform.shutdown(imgui)?;
# Ok(())
# }
```

The renderer runtime releases `RendererUserData`, surfaces, callbacks, and renderer GPU resources;
it never enters the platform-window phase. The Winit or SDL3 platform owner remains solely
responsible for destroying native windows. Context-first teardown invokes the same shared state
machine in ordered renderer-resource and platform-window phases.

Managed texture shutdown follows the same ownership rule. The runtime first obtains
`Context::prepare_renderer_texture_reset(&consumer)` while its complete GPU texture map is still
intact. A live renderer epoch rejects that preparation without changing either side. After
preparation succeeds, it destroys the WGPU map and commits the permit, which
infallibly clears native bindings before releasing the consumer. This causes live textures to be
requested again after a device rebuild without acknowledging a destroy that never happened.

Explicit renderer shutdown is idempotent and retryable. A rejected reset leaves the runtime
attached with its renderer retained so the caller can settle the epoch and call `shutdown` again.
Runtime `Drop` cannot prepare the required Context-owned renderer reset, so it defers its
attachment unchanged to Context teardown: it does not destroy WGPU resources, clear callbacks, or
alter native renderer publication while Context is alive. Dropping the wrapper therefore does not
make the Context available for a replacement runtime; use explicit shutdown when the application
needs to release renderer ownership before Context teardown. Foreign callback and backend-state
replacements are preserved rather than overwritten.

## Selecting wgpu version

The `0.16.0-alpha.2` candidate defaults to WGPU 30. Until it is published, test
the candidate from `main`:

```toml
[dependencies]
dear-imgui-wgpu = { git = "https://github.com/Latias94/dear-imgui-rs", branch = "main" }
```

After publication, use the exact prerelease requirement for the compatibility
routes below.

If your ecosystem is pinned to `wgpu` v29, v28, or v27, select it explicitly:

```toml
[dependencies]
dear-imgui-wgpu = { version = "=0.16.0-alpha.2", default-features = false, features = ["wgpu-29"] }
```

```toml
[dependencies]
dear-imgui-wgpu = { version = "=0.16.0-alpha.2", default-features = false, features = ["wgpu-28"] }
```

```toml
[dependencies]
dear-imgui-wgpu = { version = "=0.16.0-alpha.2", default-features = false, features = ["wgpu-27"] }
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
| `main` (unpublished 0.16.0-alpha.2) | 30 (default), 29 (`wgpu-29`), 28 (`wgpu-28`), 27 (`wgpu-27`) |

See also: [docs/COMPATIBILITY.md](https://github.com/Latias94/dear-imgui-rs/blob/main/docs/COMPATIBILITY.md) for the full workspace matrix.

## Notes

- Targets native and Web (with `webgl`/`webgpu` features mapped to wgpu features).
- Native multi-viewport is not available on WebAssembly.
- External dependency updates (wgpu) may require coordinated version bumps.

## Features

- Default: `wgpu-30`; no extra feature is required for a native WGPU 30 build
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
