# Changelog

All notable changes to this crate will be documented in this file.

## Unreleased

## 0.16.0-alpha.1

### Breaking

- `AshRenderer::cmd_draw` now consumes a Context-borrowed `RenderedFrame` and returns the highest pending `TextureRetirementBatch`; applications must prove GPU completion before acknowledging the batch and must call `AshRenderer::shutdown`. Shutdown prepares the Context texture-reset permit before releasing Vulkan texture resources, then commits it only after fence-safe destruction.
- Replace the Winit and SDL3 `enable` functions with owning `WinitViewportRuntime::attach` and `Sdl3ViewportRuntime::attach`. SDL3 attachment now also requires `&Sdl3PlatformBackend`, whose exclusive surface-provider lease prevents platform teardown and stale callback reuse while the Ash renderer is alive. Both attach functions remain unsafe because callers must prove the raw Vulkan device, queue, surface, and external host-synchronization lineage described by `VulkanViewportConfig`; the runtime consumes the renderer into stable internal storage, so callers no longer pin its address.
- Call the owning renderer runtime's `shutdown` before shutting down the platform runtime or dropping the Context, windows, validation surface, device, or instance. Context attachments preserve the same renderer-resources-before-platform-windows order during best-effort Context teardown.
- The owning runtime's `shutdown` rejects renderer callback ownership drift before mutating platform windows or runtime state.

### Changed

- Managed create/update feedback is request-bound, while destroy feedback is delayed until the renderer has actually released the Vulkan texture after fence- or device-idle-backed completion.
- Winit and SDL3 surface adapters now share one private Vulkan viewport runtime for callback ownership, surface and swapchain state, per-image synchronization, recovery, and ordered shutdown.
- Multi-viewport registration now claims only the five `Renderer_*` callback slots and fails instead of replacing foreign callbacks, foreign `RendererUserData`, an active renderer registration, or already-created secondary platform windows.
- `ViewportFlags::NO_RENDERER_CLEAR` now maps to Vulkan `DONT_CARE` rather than `LOAD`, matching Dear ImGui's discard semantics for a newly acquired swapchain image in both render-pass and dynamic-rendering modes.

### Fixed

- Winit and SDL3 multi-viewport renderer callbacks now verify `RendererUserData` ownership before reading or freeing per-viewport Vulkan data, ignoring foreign backend pointers instead of treating them as `dear-imgui-ash` state.
- Secondary swapchains now use per-image presentation semaphores, clamp surface extents, pause minimized viewports, and rebuild after out-of-date or suboptimal acquisition and presentation results.
- Secondary swapchains now also rebuild when framebuffer scale changes without a logical-size callback.
- Shutdown is a no-op for contexts owned by another renderer backend and fails transactionally if an active runtime no longer owns its complete callback table.
