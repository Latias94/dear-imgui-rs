# Changelog

All notable changes to this crate will be documented in this file.

## Unreleased

### Breaking

- Winit and SDL3 multi-viewport `enable` entry points are now `unsafe`: keep the renderer at a stable address, serialize renderer access on the enabling thread, and preserve the declared Vulkan device lineage until shutdown completes.
- Applications must call the renderer adapter's `shutdown_multi_viewport_support` before shutting down the platform backend or dropping the renderer, context, windows, validation surface, device, or instance.
- `shutdown_multi_viewport_support` rejects renderer callback ownership drift before mutating platform windows or runtime state.

### Changed

- Winit and SDL3 surface adapters now share one private Vulkan viewport runtime for callback ownership, surface and swapchain state, per-image synchronization, recovery, and ordered shutdown.
- Multi-viewport registration now claims only the five `Renderer_*` callback slots and fails instead of replacing foreign callbacks, foreign `RendererUserData`, an active renderer registration, or already-created secondary platform windows.
- `ViewportFlags::NO_RENDERER_CLEAR` now maps to Vulkan `DONT_CARE` rather than `LOAD`, matching Dear ImGui's discard semantics for a newly acquired swapchain image in both render-pass and dynamic-rendering modes.

### Fixed

- Winit and SDL3 multi-viewport renderer callbacks now verify `RendererUserData` ownership before reading or freeing per-viewport Vulkan data, ignoring foreign backend pointers instead of treating them as `dear-imgui-ash` state.
- Secondary swapchains now use per-image presentation semaphores, clamp surface extents, pause minimized viewports, and rebuild after out-of-date or suboptimal acquisition and presentation results.
- Shutdown is a no-op for contexts owned by another renderer backend and fails transactionally if an active runtime no longer owns its complete callback table.
