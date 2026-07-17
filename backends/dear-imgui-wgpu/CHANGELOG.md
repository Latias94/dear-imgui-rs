# Changelog

All notable changes to `dear-imgui-wgpu` will be documented in this file.

The format follows Keep a Changelog and Semantic Versioning.

## [Unreleased]

### Breaking

- Multi-viewport `enable` entry points are now `unsafe`: keep the renderer at a stable address, serialize renderer access on the enabling thread, and keep all platform and GPU objects alive until shutdown completes.
- Applications must call the renderer adapter's `shutdown_multi_viewport_support` before shutting down the platform backend or dropping the renderer, context, windows, instance, adapter, device, or queue.
- `shutdown_multi_viewport_support` now returns `Result` and rejects renderer callback ownership drift before mutating platform windows or runtime state.
- Each `WgpuRenderer` now owns the renderer state of exactly one `Context` and stays on that context's UI thread. Context-taking render and lifecycle methods reject another context before mutation; contextless draw-data rendering uses the bound context's `PlatformIO`. Create one renderer per context, call `shutdown` with its matching context before dropping either value, and reinitialize through `init_with_context`. The contextless `init` and manual `configure_imgui_context`/`prepare_font_atlas` entry points were removed.

### Changed

- Default `dear-imgui-wgpu` to `wgpu` 30, add the `wgpu-30` feature, and keep `wgpu-29`, `wgpu-28`, and `wgpu-27` as explicit compatibility features.
- Make renderer diagnostics opt-in through the `tracing` feature; the default dependency graph no longer includes `tracing`.
- Winit and SDL3 adapters now share one private multi-viewport runtime for callback ownership, viewport data, surface recovery, and shutdown ordering.
- Multi-viewport registration now claims only the five `Renderer_*` callback slots and fails instead of replacing foreign callbacks, foreign `RendererUserData`, an active renderer registration, or already-created secondary platform windows.
- Secondary viewports now honor `ViewportFlags::NO_RENDERER_CLEAR` with a load operation instead of clearing the target.

### Fixed

- Winit and SDL3 multi-viewport renderer callbacks now verify `RendererUserData` ownership before reading or freeing per-viewport WGPU data, ignoring foreign backend pointers instead of treating them as `dear-imgui-wgpu` state.
- Surface acquisition now handles lost, outdated, suboptimal, timeout, occluded, validation, and out-of-memory outcomes without presenting an invalid frame.
- Shutdown is a no-op for contexts owned by another renderer backend and fails transactionally if an active runtime no longer owns its complete callback table.
- Device-object invalidation now recreates render resources, shaders, the fallback texture, and the pipeline before the next render, then uploads managed textures again.
