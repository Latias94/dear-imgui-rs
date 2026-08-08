# Changelog

All notable changes to `dear-imgui-wgpu` will be documented in this file.

The format follows Keep a Changelog and Semantic Versioning.

## [Unreleased]

### Breaking

- Replace `WinitViewportRuntime` and `Sdl3ViewportRuntime` with `WinitViewportRoute` and `Sdl3ViewportRoute`. Each route captures the exact live platform generation at safe attachment and exposes one frame path: `prepare(FrameToken)` followed by `render_main(prepared, pass, FramebufferExtent)`.
- Remove `prepare_context`, `prepare_frame`, `poll_fault`, `with_renderer`, unchecked platform attachment/preparation, and the manual trace/reconcile/render bypasses. `prepare` returns a move-only `WgpuPreparedViewportFrame` plus same-scope secondary report, or a platform-specific aggregate that preserves every pending renderer and platform fault in per-source FIFO order.

## [0.16.0-alpha.1]

### Breaking

- Replace the unsafe Winit/SDL3 `enable` and free `shutdown_multi_viewport_support` functions with `WinitViewportRuntime::attach(context, &platform, renderer)` and `Sdl3ViewportRuntime::attach(context, &platform, renderer)`. Initialize and pass the exact live platform owner before renderer attachment; custom compatible platforms must use `unsafe attach_unchecked` and uphold the documented handle-lifetime contract. The typed owning runtime consumes `WgpuRenderer`, keeps callback-visible storage stable across moves, participates in ordered Context teardown, and exposes idempotent explicit shutdown.
- WGPU renderer shutdown no longer enters the platform-window phase. Shut down the renderer runtime before its Winit or SDL3 platform owner; Context-first teardown enforces the same renderer-resources-before-platform-windows order.
- Each `WgpuRenderer` now owns the renderer state of exactly one `Context` and stays on that context's UI thread. `WgpuRenderer::new` is the only public constructor; `empty`, `Default`, `init_with_context`, and `new_without_font_atlas` were removed. After `shutdown`, construct a replacement renderer rather than reinitializing the old value.
- External textures now register only an application-owned `TextureView` and return an opaque `ExternalTextureId`. Use `texture_id()` for ImGui drawing, `update_external_texture` to replace the view, and `unregister_external_texture` to release the registration. Stale and foreign handles return typed errors instead of aliasing another renderer's texture.
- Per-texture custom samplers were removed. Use `DrawListMut::set_sampler_linear` and `set_sampler_nearest` to enqueue Dear ImGui's standard renderer commands. The WGPU renderer owns the two compatible samplers and preserves raw-callback state until an explicit sampler or reset command.
- Renderer implementation types such as `WgpuTextureManager`, `WgpuTexture`, `RenderResources`, `FrameResources`, `ShaderManager`, and `Uniforms` are no longer exported.

### Changed

- Default `dear-imgui-wgpu` to `wgpu` 30, add the `wgpu-30` feature, and keep `wgpu-29`, `wgpu-28`, and `wgpu-27` as explicit compatibility features.
- Make renderer diagnostics opt-in through the `tracing` feature; the default dependency graph no longer includes `tracing`.
- Winit and SDL3 adapters now share one private multi-viewport runtime for callback ownership, viewport data, surface recovery, and shutdown ordering.
- Multi-viewport attach is transactional and returns the unchanged renderer on failure. Callback panic, reentry, ownership drift, render errors, and terminal surface failures are contained across FFI and reported at the next Rust runtime entry.
- Explicit runtime shutdown retains the renderer when detached texture epochs prevent reset so callers can complete the epochs and retry. Dropping either a viewport runtime or standalone `WgpuRenderer` now defers its GPU resources and renderer consumer to Context teardown instead of bypassing the reset contract; use explicit shutdown to release ownership before reusing a live Context. Foreign callback and backend-state replacements remain preserved.
- Multi-viewport registration now claims only the five `Renderer_*` callback slots and fails instead of replacing foreign callbacks, foreign `RendererUserData`, an active renderer registration, or already-created secondary platform windows.
- Secondary viewports now honor `ViewportFlags::NO_RENDERER_CLEAR` with a load operation instead of clearing the target.
- Secondary viewport surfaces now use `WgpuViewportSurfaceConfig` rather than forcing `Fifo`; copy a main `SurfaceConfiguration` with `WgpuViewportSurfaceConfig::from(&main_surface_config)` or select another presentation policy explicitly.
- Managed GPU resources are keyed by pointer-free snapshot texture identities. Create, update, and destroy results are reconciled through request-bound feedback before draw commands are read; application-owned views use renderer-issued `ExternalTextureId` handles.
- Device-object invalidation preserves external texture handles and views while rebuilding renderer-owned pipelines, buffers, samplers, and managed textures. After actual device loss, update each retained handle with its replacement view before drawing it again.
- WebGL and WebGPU features for WGPU 27 through 30 now enable the required `dear-imgui-rs/wasm` import route automatically.

### Fixed

- Winit and SDL3 multi-viewport renderer callbacks now verify `RendererUserData` ownership before reading or freeing per-viewport WGPU data, ignoring foreign backend pointers instead of treating them as `dear-imgui-wgpu` state.
- Surface acquisition now handles lost, outdated, suboptimal, timeout, occluded, validation, and out-of-memory outcomes without presenting an invalid frame.
- Secondary viewports now create MSAA resolve and depth-stencil attachments that match the renderer pipeline, use authoritative physical framebuffer sizes, suspend at zero size, and release the old surface bundle before loss recovery.
- Shutdown is a no-op for contexts owned by another renderer backend and fails transactionally if an active runtime no longer owns its complete callback table.
- Device-object invalidation now recreates render resources, shaders, the fallback texture, and the pipeline before the next render, then uploads managed textures again.
- Replayed create requests upload their current pixels instead of accepting stale GPU contents, and successful destroy reconciliation releases renderer tombstones instead of accumulating them for the lifetime of the device.
