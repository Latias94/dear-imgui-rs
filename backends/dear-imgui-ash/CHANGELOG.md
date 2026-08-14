# Changelog

All notable changes to this crate will be documented in this file.

## Unreleased

## 0.16.0 - 2026-08-14

### Breaking

- Replace `WinitViewportRuntime` and `Sdl3ViewportRuntime` with `WinitViewportRoute` and `Sdl3ViewportRoute`. Each route captures the exact platform generation at `attach` and exposes one preparation transaction: `prepare(FrameToken)` (plus Winit's `ActiveEventLoop`) reconciles textures, dispatches secondary viewports, and returns all renderer and platform callback faults together. The old `prepare_context`, `prepare_frame`, `poll_fault`, `attach_unchecked`, renderer inspection, and manual trace/bypass entries were removed.
- Ash multi-viewport routes no longer expose `TextureRetirementBatch`. `cmd_draw_main` and `AshPreparedViewportFrame::skip_main` produce a move-only `AshViewportFrameCompletion`; consume it with `wait_for_frame_completion` or unsafe `complete_frame_with_fences`. Dropping a prepared frame or completion defers retirement instead of freeing Vulkan resources early. The low-level batch API remains available on single-window `AshRenderer`.

## 0.16.0-alpha.1

### Breaking

- `AshRenderer::cmd_draw` now consumes a Context-borrowed `RenderedFrame` and returns the highest pending `TextureRetirementBatch`; applications must prove GPU completion before acknowledging the batch and must call `AshRenderer::shutdown`. Shutdown prepares the Context texture-reset permit before releasing Vulkan texture resources, then commits it only after fence-safe destruction.
- Ash renderer constructors and `cmd_draw` are now unsafe because raw Vulkan device lineage, queue/command-pool capability, render-target compatibility, command-buffer recording state, and future submission cannot be proven by Rust. Construct `AshRendererConfig` with `with_render_pass` or `with_dynamic_rendering`, optionally apply `with_options`, and pass the complete device-lineage contract to the allocator constructor instead of supplying a long positional argument list.
- `Options::max_textures` must be at least 8 and now counts sampled-image descriptor sets only. The renderer reserves its two standard sampler sets separately.
- `Options::texture_format` is removed. Managed font and Context textures are always uploaded as tightly packed `R8G8B8A8_UNORM`; arbitrary Vulkan formats never matched the RGBA upload contract. External sampled images remain application-owned and may use any view compatible with the fragment shader.
- Replace raw descriptor-set registration and `register_external_texture_with_sampler` with unsafe `register_external_texture(image_view, image_layout)`. Ash now owns only its sampled-image descriptor; select linear or nearest sampling with the standard Context draw callbacks instead of passing or mutating an application sampler. External update and unregister operations are unsafe because device idle cannot account for recorded command buffers that the application may submit later.
- Replace the Winit and SDL3 `enable` functions with owning `WinitViewportRuntime::attach(context, &platform, renderer, config)` and `Sdl3ViewportRuntime::attach(context, &platform, renderer, config)`. Both routes require their exact live platform owner; custom Winit-compatible platforms must use `unsafe attach_unchecked` and keep every `PlatformHandle`'s `winit::Window` alive through renderer shutdown. Both attach functions remain unsafe because callers must prove the raw Vulkan device, queue, surface, and external host-synchronization lineage described by `VulkanViewportConfig`; the runtime consumes the renderer into stable internal storage, so callers no longer pin its address.
- `VulkanViewportConfig` now requires `swapchain_image_usage`; use `vk::ImageUsageFlags::empty()` when secondary viewports need only the renderer's required color-attachment usage.
- Call the owning renderer runtime's `shutdown` before shutting down the platform runtime or dropping the Context, windows, validation surface, device, or instance. Context attachments preserve the same renderer-resources-before-platform-windows order during best-effort Context teardown.
- The owning runtime's `shutdown` rejects renderer callback ownership drift before mutating platform windows or runtime state.
- Multi-viewport render loops must call the owning runtime's `prepare_frame(&mut RenderedFrame)` before any secondary or main draw. This no-surface phase makes managed texture create/update/destroy requests visible regardless of platform-specific swapchain submission order.

### Changed

- The Vulkan pipeline now uses sampled-image set 0 plus renderer-owned linear/nearest sampler set 1. Standard sampler callbacks and raw callbacks execute in command order; raw callbacks can borrow scoped `AshRenderState`, and reset callbacks restore the renderer pipeline and linear sampler.
- Descriptor-pool accounting, shader artifacts, default texture upload, initialization rollback, and multi-viewport setup now use one transactional two-set resource model. Secondary surface capabilities are checked against all requested swapchain image usage before creation and rebuild.
- Managed create/update feedback is request-bound, while destroy feedback is delayed until the renderer has actually released the Vulkan texture after fence- or device-idle-backed completion.
- Winit and SDL3 examples submit secondary swapchains only after the no-surface texture preparation phase, so WSI-safe secondary-first ordering cannot expose stale managed textures.
- Ash examples now share one rollback-safe frame-sync implementation, use presentation semaphores indexed by acquired swapchain image, and recover every failed post-acquire/pre-submit step before any sync or layout state is reused. A failed recovery keeps the frame slot poisoned and blocks rebuild/acquire until device completion and frame-sync replacement succeed; device loss is terminal.
- Winit and SDL3 surface adapters now share one private Vulkan viewport runtime for callback ownership, surface and swapchain state, per-image synchronization, recovery, and ordered shutdown.
- Multi-viewport registration now claims only the five `Renderer_*` callback slots and fails instead of replacing foreign callbacks, foreign `RendererUserData`, an active renderer registration, or already-created secondary platform windows.
- `ViewportFlags::NO_RENDERER_CLEAR` now maps to Vulkan `DONT_CARE` rather than `LOAD`, matching Dear ImGui's discard semantics for a newly acquired swapchain image in both render-pass and dynamic-rendering modes.

### Fixed

- Callback-only frames now execute even without vertex data, callback errors restore the full-frame scissor, and invalid draw ranges fail before issuing geometry commands.
- Initialization failure now releases partially created sampler, descriptor, upload, and texture resources without waiting on work that was never submitted.
- Winit and SDL3 multi-viewport renderer callbacks now verify `RendererUserData` ownership before reading or freeing per-viewport Vulkan data, ignoring foreign backend pointers instead of treating them as `dear-imgui-ash` state.
- Secondary swapchains now use per-image presentation semaphores, clamp surface extents, pause minimized viewports, and rebuild after out-of-date or suboptimal acquisition and presentation results.
- Secondary swapchains now also rebuild when framebuffer scale changes without a logical-size callback.
- Shutdown is a no-op for contexts owned by another renderer backend and fails transactionally if an active runtime no longer owns its complete callback table.
- Shader generation embeds the exact GLSL source in checked-in SPIR-V, records source/artifact hashes, uses stable relative debug source names, and transactionally publishes a complete artifact set only after every compile succeeds. Replacement failures restore the previous set. The compiler-free check rejects stale binaries and checkout-path drift; authoritative CI also builds pinned glslang 15.1.0 and requires byte-identical regeneration.
