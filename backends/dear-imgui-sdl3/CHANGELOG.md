# Changelog

All notable changes to `dear-imgui-sdl3` will be documented in this file.

The format follows Keep a Changelog and Semantic Versioning.

## [Unreleased]

## [0.16.0] - 2026-08-14

### Changed

- SDL callback applications now call `Sdl3CallbackEventHandoff::drain` and inspect the returned atomic `Sdl3CallbackEventBatch` before replaying retained events with the owning backend's `process_callback_event`; the former `try_drain` and `Sdl3CallbackEventQueue` surface was removed. Every distinct queue fault is retained in observation order, and bounded O(1) state-event coalescing prevents sustained callback traffic from starving ordered input.
- The official OpenGL3 and SDL_GPU renderer owners now consume `FrameToken` through backend-owned
  viewport transactions. Managed-texture reconciliation, capability-aware secondary-window work,
  and ordered fault collection are no longer assembled from public `consumer`, `reconcile_frame`,
  `render_reconciled`, or `prepare_render_reconciled` steps.
- OpenGL3 preparation always attempts the application-provided main-context restoration operation
  after its route attempt, drains deferred faults after restoration, and resumes a caught route
  panic only after both cleanup steps run. Main drawing consumes a move-only prepared capability.
- SDL_GPU preparation now stays independent of the main swapchain. Applications either transfer
  its move-only capability through `prepare_render_main` and `render_main`, or call `skip_main`
  when the main surface is unavailable while secondary viewports remain live.

### Fixed

- SDL_GPU reports command-buffer cancellation failure separately when swapchain acquisition also
  fails, while preserving acquisition failure as the first fault and skipping later GPU work.
- First-party SDL3 renderer routes now return every callback fault in FIFO order and refuse to run
  a native viewport pump when an older fault is pending.
- Wayland and other drivers without the complete native viewport capability now skip only the
  secondary OS-window pump while retaining normal main-viewport rendering.
