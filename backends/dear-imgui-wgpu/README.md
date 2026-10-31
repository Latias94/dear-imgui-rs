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
// First update platform metrics/input, then open and build the UI.
let ui = imgui.frame();
ui.window("Hello").build(|| ui.text("Dear ImGui + WGPU"));
let frame = imgui.render(renderer.renderer_consumer()?);
let framebuffer_extent = FramebufferExtent::from_texture(&surface_texture.texture);
renderer.render(frame, &mut render_pass, framebuffer_extent)?;
```

Each `WgpuRenderer` is fully initialized and bound to the `Context` passed to `new`; there is no public empty or two-phase state. Create one renderer per context in multi-context applications. After `shutdown`, create a replacement renderer instead of reinitializing the old value. `render()` is the normal path: it consumes a `PendingFrame`, processes pointer-free managed texture requests, reconciles feedback, and only then reads draw commands. Integrations that must reconcile before acquiring the main surface can split that work into `reconcile_frame()` followed by `render_reconciled()`. Both render methods require the physical extent of the actual color attachment; when rendering to a surface, derive it from the acquired `SurfaceTexture` instead of assuming the configured size still matches.

## Render-thread snapshots

`WgpuSnapshotRenderer::new(init, &mut context)` returns two owners: keep the UI publication and
`DetachedRendererConsumer` in `WgpuSnapshotRenderer` on the Context thread, and move the separate
`WgpuSnapshotExecutor` to the render thread. The executor contains GPU resources and owned
identities, with no live Context binding, native pointer, or `unsafe Send/Sync` implementation.
Both renderer paths share command validation, clipping, texture lookup, buffer preparation,
sampler/reset handling, and draw execution.

Register each host render target with `executor.set_target(viewport_id, native_target)`. Send its
returned `WgpuSnapshotTarget` back to the UI thread and freeze it with the owned core snapshot:

```rust,no_run
# use dear_imgui_wgpu::{WgpuSnapshotSubmission, WgpuSnapshotTarget};
# fn publish(snapshot: dear_imgui_rs::render::FrameSnapshot, target: WgpuSnapshotTarget) {
let submission = WgpuSnapshotSubmission::new(snapshot, [target]);
// Send `submission` through an application-owned bounded channel to the render worker.
# let _ = submission;
# }
```

On the worker, call `executor.prepare(submission)`, `prepared.record(target, pass, extent)` for
each desired captured viewport, then `prepared.finish()`. Preparation applies managed requests but
does not submit commands. Record the engine's producers into the same encoder before UI, or pass
their command buffers before UI buffers in `Queue::submit`. The runnable
`threaded_snapshot_wgpu` example uses a channel with capacity one and explicit producer-before-UI
submission to an offscreen target.

Call `set_target` again whenever the host replaces an allocation or native registration. Earlier
queued capabilities fail before managed texture mutation. Context and consumer generations and
strictly increasing epochs are also validated. A failed request retains acknowledgements for
earlier successful uploads and requests retry for the failed and remaining requests; stale epochs
commit superseded feedback. `WgpuSnapshotError` retains the owned partial report and any completion
delivery failure. Dropping a prepared scope still commits its already-applied texture feedback.
The core rejects arbitrary raw callbacks during snapshot capture.

Targets and reports are metadata: they retain no Surface, native window, or resource lease. The
host owns acquisition and presentation, retains resources used by recorded or submitted commands,
and supplies the actual pass extent. A native association is the exact registration captured by
the integration, not a later lookup of the viewport ID. `recorded = true` proves only successful
command recording. Core snapshot completion proves CPU-side processing; neither proves queue
submission, presentation, GPU completion, compositor acceptance, or visible pixels. Poll
`context.poll_snapshot_completions()` on the UI thread, join the worker, then call
`owner.shutdown(&mut context, &mut executor)` before replacing a live renderer.

## External texture views

Register an application-owned texture view and retain the returned handle in application state.
The renderer clones the view handle. Keep the mapping registered until all UI frames referring to
it have been recorded, and do not explicitly destroy the underlying resource until both the
mapping is removed and GPU work using it has completed:

```rust,no_run
# use dear_imgui_wgpu::{ExternalTextureId, RendererResult, WgpuRenderer, wgpu};
# fn register_image(
#     renderer: &mut WgpuRenderer,
#     view: &wgpu::TextureView,
# ) -> RendererResult<ExternalTextureId> {
let texture = renderer.register_external_texture(view)?;
# Ok(texture)
# }
```

Use `ui.image(texture.texture_id(), size)` while building a later frame. Call
`update_external_texture(texture, replacement_view)` before recording a frame that should use the
replacement. Call `unregister_external_texture(texture)` only after recording the final frame
that uses the handle; unregistering immediately after `ui.image` leaves that frame without a
texture mapping.

Each registration starts at binding revision one. `renderer.external_texture_binding(texture)`
returns an owned `ExternalTextureBinding`; every successful `update_external_texture` advances
that revision, even when passed the same view. Exhaustion and invalid handles return typed errors
without changing the mapping or cached bind group. The revision identifies a view mapping, not
the engine's content frame. Keep engine resource IDs and content revisions alongside this value.

`render` and `render_reconciled` return `WgpuDrawRecording`. Its `dependencies.external_bindings`
contains each external mapping once, in first drawable-use order, after validation and framebuffer
clipping. Zero-count and fully clipped images are excluded. A non-standard raw callback sets
`contains_unknown_callback_dependencies`; the backend cannot infer resources used inside arbitrary
callbacks. Standard reset and sampler callbacks do not make the dependency list incomplete.

The recording contains the Context, renderer epoch and native UI frame count. Save it directly in
editor history; it retains no Context, native window or GPU lease. For an application-owned render
pass it proves recording only: the caller controls the attachment, `Queue::submit`, and presentation.

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

The Winit and SDL3 integrations use one shared owning renderer core with platform-specific route
types. Select exactly one platform adapter:

- `multi-viewport-winit`
- `multi-viewport-sdl3`

They are mutually exclusive and native-only. The selected feature enables
`dear-imgui-rs/multi-viewport`; do not enable both routes through `--all-features`.

Secondary windows need the `Instance` and `Adapter` that created the renderer's `Device`. Keep
them in `WgpuInitInfo`, enable the owning platform backend first, and then move the renderer into
the matching WGPU route before Dear ImGui creates a secondary platform window:

```rust,no_run
use dear_imgui_rs::Context;
use std::sync::Arc;
use dear_imgui_wgpu::{WgpuInitInfo, WgpuRenderer, WgpuViewportSurfaceConfig, wgpu};
use dear_imgui_wgpu::multi_viewport::WinitViewportRoute;
use dear_imgui_winit::{HiDpiMode, WinitPlatform};

# fn enable_viewports(
#     imgui: &mut Context,
#     main_window: Arc<winit::window::Window>,
#     instance: wgpu::Instance,
#     adapter: wgpu::Adapter,
#     device: wgpu::Device,
#     queue: wgpu::Queue,
#     format: wgpu::TextureFormat,
# ) -> Result<(WinitPlatform, WinitViewportRoute), Box<dyn std::error::Error>> {
imgui.enable_multi_viewport();
let mut platform = WinitPlatform::new(imgui)?;
platform.attach_window(Arc::clone(&main_window), HiDpiMode::Default, imgui)?;
platform.enable_viewports(imgui)?;

let viewport_surface = WgpuViewportSurfaceConfig {
    present_mode: wgpu::PresentMode::AutoNoVsync,
    ..Default::default()
};
let init = WgpuInitInfo::new(device, queue, format)
    .with_instance(instance)
    .with_adapter(adapter)
    .with_viewport_surface_config(viewport_surface);
let renderer = WgpuRenderer::new(init, imgui)?;
let route = WinitViewportRoute::attach(imgui, &platform, renderer)?;
# Ok((platform, route))
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
route owns matching per-window MSAA resolve and depth-stencil attachments, suspends acquisition
while a native framebuffer has a zero dimension, and rebuilds attachments from the platform
owner's current physical size after resize or DPI changes. Attachment fails transactionally when
the adapter cannot support the configured formats and sample count.

A lost secondary surface is rebuilt from that viewport's still-live platform window. Successful
surface recreation leaves the renderer route and every other viewport attached. If recreation
fails, the backend requests closure of only the affected viewport and reports the creation error.
WGPU exposes Device loss separately through an application-owned, single-slot callback; if that
callback fires, recreate the Device, Queue, renderer, and all GPU resources before reattaching the
viewport route.

### Native discard teardown validation

The ignored native pending-frame tests require an explicit `DEAR_IMGUI_TEST_WGPU_BACKEND=vulkan`
or `dx12` selection. On Windows 11 with WGPU 30.0.1, an NVIDIA RTX 4090 and driver 617.14, creating
an Instance with `Backends::PRIMARY` and discarding a submitted surface frame intermittently
terminated the process with exit code 2173 during GPU teardown. The pure WGPU diagnostic reproduced
the failure without a Dear ImGui Context, platform adapter, or backend window lease. Explicit
Vulkan-only and DX12-only instances passed the separate present, discard, Drop, and pure WGPU cases.
The precise driver or WGPU cause remains unresolved; successful retries do not erase this failure.

Run native cases in separate processes with a selected backend. The pure diagnostic also accepts
`DEAR_IMGUI_TEST_WGPU_BACKEND=primary` to retain the multi-backend reproducer:

```powershell
$env:DEAR_IMGUI_TEST_WGPU_BACKEND = 'vulkan' # or 'dx12' on Windows
cargo test -p dear-imgui-wgpu --features multi-viewport-winit native_pending_discard_survives_context_retirement -- --ignored --nocapture

$env:DEAR_IMGUI_TEST_WGPU_BACKEND = 'primary' # known failing diagnostic path on the system above
cargo test -p dear-imgui-wgpu --features multi-viewport-winit native_pure_wgpu_submit_discard_teardown_without_imgui_context -- --ignored --nocapture
```

Production code keeps the application's Instance/backend policy. It does not force a backend,
present discarded frames implicitly, or insert a CPU wait into the pending-frame release path.

The renderer opens a fresh upload-resource arena for each `PendingFrame` epoch. Every viewport
draw then uses a separate pass slot with its own vertex, index, uniform, and sampler bindings, so
command buffers cannot observe data uploaded for another viewport or a later epoch. The renderer
does not recycle upload buffers across epochs because submission of the application-owned encoder
is not observable.

Finish UI construction with a `FrameToken`, then call `route.prepare(event_loop, frame)` on Winit
or `route.prepare(frame)` on SDL3. Preparation reconciles managed textures and updates native
windows. It does not acquire surface frames, record commands, submit commands, or present.

For an engine render graph, continue explicitly:

```rust,no_run
# use dear_imgui_wgpu::{multi_viewport::{WinitViewportRoute, WgpuPreparedViewportFrame}, wgpu};
# fn submit_secondaries<'frame>(route: &WinitViewportRoute, prepared: WgpuPreparedViewportFrame<'frame>, producer_commands: Vec<wgpu::CommandBuffer>) -> Result<(), Box<dyn std::error::Error>> {
let recorded = route.record(prepared)?;
let (main, secondary_work) = recorded.split();
// SceneColor producers execute before every secondary UI consumer in one Queue::submit.
let pending = secondary_work.submit(producer_commands)?;
// Keep pending until presentation is desired. Its acquired frame, Surface, exact native window,
// and owner chain remain alive even if platform registrations retire in the meantime.
let secondary_report = pending.present()?;
// The host independently acquires its main surface and calls route.record_main(main, pass, extent).
// If acquisition is unavailable, dropping main does not undo the completed secondary work.
drop(main);
# let _ = secondary_report;
# Ok(())
# }
```

`route.record` returns `WgpuRecordedViewportFrame`. Its move-only work owns GPU command buffers,
acquired frames and native target leases. `submit` rejects retired targets before producer
iteration and submits producer commands before UI commands on the renderer's queue. Successful
submission transfers acquired frames and leases into `WgpuPendingPresentations`. `present`
requests presentation exactly once; `discard` and Drop discard without requesting presentation.
Both recorded and pending work release an acquired frame before its retained Surface and native
window/owner-chain lease. Submission does not end this lifetime and does not wait for GPU completion.

Native owner changes are rejected only while work leases pin the affected allocations. Idle
baseline Surfaces follow renderer-before-platform teardown and do not permanently freeze native
owner policy. Surface configuration is deferred while an older recorded or submitted frame still
retains the acquisition. The next recording captures current geometry and the current owner chain.

For a conventional application loop, `route.complete_secondary(prepared, producer_commands)` runs
the same record/submit/present pipeline and returns `WgpuPreparedMainFrame`. Pass an empty iterator
when producers were already submitted. After acquiring the main surface, call
`route.record_main(main, &mut render_pass, FramebufferExtent::from_texture(&surface_frame.texture))`.
Main submission and presentation remain host-owned. There is one transaction model; the convenience
driver does not bypass lease or observation checks.

### Inspect native target observations

Prepared, recorded, pending, and error values expose owned reports. Entries freeze
`NativeWindowTarget`, `WgpuFrameScope`, external binding revisions, the highest completed API stage,
and an independent skip/failure/discard outcome. Use `target.instance_id` for the exact native
registration across docking changes. The stored `target.viewport_id` is the routing ID at observation
time; resolving an old ID against a newer registry can identify a different window.

```rust,no_run
# use dear_imgui_wgpu::multi_viewport::WgpuViewportFrameReport;
# fn inspect(report: &WgpuViewportFrameReport) {
for observation in report.observations() {
    for binding in &observation.dependencies.external_bindings {
        println!("native {:?}, UI {:?}, texture {:?}, revision {}, stage {:?}",
            observation.target.instance_id, observation.scope.epoch,
            binding.texture, binding.revision, observation.stage);
    }
}
# }
```

Stages are `Unrecorded`, `Recorded`, `Submitted`, and `PresentationRequested`. The last stage means
the backend invoked WGPU's presentation API. It does not prove GPU completion, compositor
acceptance, scanout, or visible pixels. `submission_index()` supports application-owned completion
tracking. A submitted clear-only pass can have empty dependencies and a `NoDrawCommands` outcome.
Zero-size, missing draw data, timeout, outdated/lost recovery, occlusion, retired targets, and a
surface still leased by earlier work have distinct skip reasons.

Preparation errors and `WgpuViewportWorkError` expose `report()` and `into_parts()` so late failures
retain earlier observations. Recording failures retain successfully prepared dependencies,
including unknown callback resources, without claiming successful recording. Dropping a failed
transaction discards earlier unsubmitted commands. `WgpuPreparedMainFrame::secondary_report()` is a
frozen snapshot from split/completion; an explicit split returns later submission/presentation
observations through its separate work value. No global last-report state is updated implicitly.

For SDL3, initialize `Sdl3PlatformBackend` first and then call
`dear_imgui_wgpu::multi_viewport_sdl3::Sdl3ViewportRoute::attach(imgui, &platform, renderer)`.
Both route constructors require the matching live platform owner, capture its exact Context
generation, and reject Context mismatches, shutdown owners, and callback ownership drift before
interpreting any native handle. They consume `WgpuRenderer`; no caller-owned stable address is
required and there is no unchecked custom-platform attachment path.

The renderer claims only the five `Renderer_*` slots in `ImGuiPlatformIO`. Registration fails
instead of replacing foreign renderer callbacks or `RendererUserData`, rejects secondary windows
that already exist, and requires an active Context `Platform` attachment. Attach is transactional:
the error returns the unchanged renderer through `WgpuViewportAttachError`. Moving the route does
not move callback-visible renderer storage. Callback replacement, panic, reentry, rendering, and
unrecoverable surface validation failures are contained at the C ABI boundary. `prepare` returns
all pending renderer and platform faults without dropping either source's FIFO ordering. A
terminal fault revokes renderer viewport capability and stops create/resize/render/present work.
Its `Renderer_DestroyWindow` callback remains
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

Winit + WGPU has a route-level native smoke (`A`). The bounded SDL3/WGPU example also establishes
native target/binding attribution, merge/close retirement, and ordered shutdown on Windows (`A`);
surface-retry injection and other desktop targets remain unverified for that route. Consult the
[`renderer route evidence matrix`](../../docs/renderer-route-evidence.md) for the exact claim
boundary and follow-up gaps.

Shut down renderer ownership before disabling platform viewport ownership:

```rust,no_run
# use dear_imgui_rs::Context;
# use dear_imgui_wgpu::multi_viewport::WinitViewportRoute;
# use dear_imgui_winit::WinitPlatform;
# fn shutdown(route: &mut WinitViewportRoute, platform: &mut WinitPlatform, imgui: &mut Context) -> Result<(), Box<dyn std::error::Error>> {
route.shutdown(imgui)?;
platform.disable_viewports(imgui)?;
# Ok(())
# }
```

The renderer route releases `RendererUserData`, surfaces, callbacks, and renderer GPU resources;
it never enters the platform-window phase. The Winit or SDL3 platform owner remains solely
responsible for destroying native windows. Context-first teardown invokes the same shared state
machine in ordered renderer-resource and platform-window phases.

Managed texture shutdown follows the same ownership rule. The route first obtains
`Context::prepare_renderer_texture_reset(&consumer)` while its complete GPU texture map is still
intact. A live renderer epoch rejects that preparation without changing either side. After
preparation succeeds, it destroys the WGPU map and commits the permit, which
infallibly clears native bindings before releasing the consumer. This causes live textures to be
requested again after a device rebuild without acknowledging a destroy that never happened.

Explicit renderer shutdown is idempotent and retryable. A rejected reset leaves the route
attached with its renderer retained so the caller can settle the epoch and call `shutdown` again.
Route `Drop` cannot prepare the required Context-owned renderer reset, so it defers its
attachment unchanged to Context teardown: it does not destroy WGPU resources, clear callbacks, or
alter native renderer publication while Context is alive. Dropping the wrapper therefore does not
make the Context available for a replacement route; use explicit shutdown when the application
needs to release renderer ownership before Context teardown. Foreign callback and backend-state
replacements are preserved rather than overwritten.

## Selecting wgpu version

The `0.22` release train defaults to WGPU 30 and requires Rust 1.95 or newer. Upgrade all workspace crates together and review the [0.22 migration notes](../../docs/COMPATIBILITY.md#022-release-train):

```toml
[dependencies]
dear-imgui-wgpu = "0.22"
```

If your ecosystem is pinned to `wgpu` v29, v28, or v27, select it explicitly:

```toml
[dependencies]
dear-imgui-wgpu = { version = "0.22", default-features = false, features = ["wgpu-29"] }
```

```toml
[dependencies]
dear-imgui-wgpu = { version = "0.22", default-features = false, features = ["wgpu-28"] }
```

```toml
[dependencies]
dear-imgui-wgpu = { version = "0.22", default-features = false, features = ["wgpu-27"] }
```

## What You Get

- ImGui v1.92 texture system integration (create/update/destroy)
- Upload buffers isolated by render epoch and viewport pass, with device-object invalidation
- Format-aware or user-controlled gamma (see below)

## sRGB / Gamma

- Default `GammaMode::Auto`: picks `gamma=2.2` for sRGB targets and `1.0` for linear targets.
- You can force `Linear` (1.0) or `Gamma22` (2.2).
- Pair this with your swapchain format to avoid double correction.

## Compatibility

| Track | wgpu support |
|-------|--------------|
| `0.22.0` | 30 (default), 29 (`wgpu-29`), 28 (`wgpu-28`), 27 (`wgpu-27`) |
| `0.21.0` | 30 (default), 29 (`wgpu-29`), 28 (`wgpu-28`), 27 (`wgpu-27`) |
| `0.20.0` | 30 (default), 29 (`wgpu-29`), 28 (`wgpu-28`), 27 (`wgpu-27`) |
| `0.19.0` | 30 (default), 29 (`wgpu-29`), 28 (`wgpu-28`), 27 (`wgpu-27`) |

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
