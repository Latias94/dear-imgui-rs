# Changelog

All notable changes to `dear-imgui-sdl3` will be documented in this file.

The format follows Keep a Changelog and Semantic Versioning.

## [Unreleased]

## [0.21.0] - 2026-10-17

### Added

- Receive owned file/text drops with available coordinates and native target identity, including through callback event handoff. [PR #93](https://github.com/Latias94/dear-imgui-rs/pull/93)
- Configure secondary windows and query their state, supported policies, and lifecycle events. Native allocation identities remain distinct from reusable SDL window and ImGui viewport IDs. [PR #93](https://github.com/Latias94/dear-imgui-rs/pull/93)
- Keep native windows and their owners alive while renderer work still needs them. [PR #93](https://github.com/Latias94/dear-imgui-rs/pull/93)
- Configure the official OpenGL3 owner's clip origin with `Sdl3OpenGl3Backend::set_clip_origin`. The default is lower-left; applications using `GL_UPPER_LEFT` must select `Sdl3OpenGl3ClipOrigin::UpperLeft`, or `Auto` for driver detection. [PR #94](https://github.com/Latias94/dear-imgui-rs/pull/94)

### Fixed

- Stop advertising native viewport parenting on Cocoa, where the upstream SDL adapter leaves it disabled so detached windows can move between monitors. [PR #93](https://github.com/Latias94/dear-imgui-rs/pull/93)
- Preserve a valid framebuffer density when a platform scale callback fails and report the platform fault before reaching Dear ImGui's geometry assertion. [PR #94](https://github.com/Latias94/dear-imgui-rs/pull/94)
- Discover native SDL3 headers and runtime libraries with both the legacy Cargo build layout and Cargo 1.100's per-package layout. [PR #92](https://github.com/Latias94/dear-imgui-rs/pull/92)

See the [workspace release notes](../../CHANGELOG.md#0210---2026-10-17) for the shared Dear ImGui 1.93 and editor backend migrations. External SDL3 dependency versions are unchanged from 0.20.0.

## [0.20.0] - 2026-10-12

### Changed

- Upgrade to `sdl3` 0.20 and `sdl3-sys` 0.7. Applications passing SDL3 events or handles to this backend must upgrade their direct dependencies to the same versions. [PR #88](https://github.com/Latias94/dear-imgui-rs/pull/88)
- Forward the renamed `Event::GamepadAdded` and `Event::GamepadRemoved` events to the native platform backend. [PR #88](https://github.com/Latias94/dear-imgui-rs/pull/88)

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
