# Bevy Docking Multi-Viewport Workstream - Handoff

Status: Closed
Last updated: 2026-05-23

## Current State

The workstream is closed. Native Bevy `render,multi-viewport` builds now support requested Dear
ImGui docking multi-viewport OS windows through Bevy-owned secondary `Window` entities, queued
PlatformIO lifecycle callbacks, input/focus/DPI/cursor/IME feedback, and per-viewport render
routing. The product-facing `editor_shell` example and README document the native run command and
wasm boundary.

## Active Task

- None. DMV-080 is complete and the lane is closed.
- Closeout evidence: `docs/workstreams/bevy-docking-multi-viewport-v1/EVIDENCE_AND_GATES.md` and
  `docs/workstreams/bevy-docking-multi-viewport-v1/JOURNAL/2026-05-23-dmv-080.md`.

## Decisions Since Last Update

- Treat Bevy multi-window render-target routing as prior art, not as docking multi-viewport support.
- Use a queued bridge between Dear ImGui PlatformIO callbacks and Bevy ECS window lifecycle systems.
- Allow core `dear-imgui-rs` changes when they remove real multi-viewport integration friction.
- Do not set `ConfigFlags::VIEWPORTS_ENABLE` until lifecycle, status, input feedback, and render
  routing have enough evidence.
- DMV-020 found that existing core PlatformIO APIs are sufficient for lifecycle command capture:
  typed callbacks can read a backend-owned queue from `Io::BackendPlatformUserData` and copy
  viewport state without storing raw pointers.
- DMV-030 added `dear-imgui-bevy::viewport`: callback-captured commands, a viewport-to-window entity
  map, secondary `Window` spawning/updating/despawning, and cleanup of ImGui backend user-data on
  `ImguiContext` drop.
- DMV-040 made multi-viewport status explicit: requested, feature-enabled, native target,
  lifecycle bridge, input feedback, render routing, and full support are separate fields. The
  native lifecycle bridge only installs when `ImguiBackendConfig::multi_viewport` is requested.
  wasm normal `render` builds still compile, while wasm `render,multi-viewport` remains explicitly
  unsupported by the core compile-time gate. README now has a support matrix.
- DMV-050 generalized input and platform feedback across primary and secondary mapped Dear ImGui
  viewport windows. Secondary windows now feed Dear ImGui mouse viewport events, cursor/IME output
  is applied to the target viewport window, and Bevy window position/size/focus/DPI snapshots are
  exposed through PlatformIO getter callbacks. `viewport_input_feedback_enabled` is now true for
  native requested `multi-viewport` builds. Later prelaunch work maps Bevy `WindowOccluded` events
  into Dear ImGui minimized feedback for secondary viewport windows; Bevy still does not expose a
  persistent minimized-window field on `Window`.
- DMV-060 completed secondary viewport render routing. Core `FrameSnapshot` can now carry all
  platform viewport draw data, Bevy render extraction/preparation routes secondary viewport draws
  to the matching window target, and native `render,multi-viewport` requested configs now advertise
  `multi_viewport_supported = true`.
- Shutdown order is important: `ImguiContext` must call `destroy_platform_windows()` before
  clearing `BackendPlatformUserData` and PlatformIO handlers, otherwise Dear ImGui shutdown asserts
  because viewport platform markers remain set.
- The viewport lifecycle bridge stores backend state in a stable boxed allocation. Tests should not
  assert that `BackendPlatformUserData` equals the address of the Bevy non-send resource wrapper.
- Same-drain `Create` plus immediate `SetPos`/`SetSize`/`Show`/`SetTitle` commands are merged before
  Bevy `Commands` flushes, so initial secondary window state is preserved.
- DMV-070 made native multi-viewport visible in the product-facing editor shell. The example keeps
  its normal `render` compile gate and requests `multi_viewport = true` only when built with the
  `multi-viewport` feature. README now shows the native `render,multi-viewport` run command and
  directs wasm users to the plain `render` command.
- DMV-080 closed the lane after fresh final gates. A closeout regression in plain `render` builds
  was fixed by gating render extraction's viewport-target mapping on
  `ImguiBackendStatus::multi_viewport_supported`; this preserves ordinary multi-window overlay
  routing without misclassifying it as Dear ImGui platform viewport routing.

## Blockers / Constraints

- No blocking findings remain for this lane.
- Bevy `0.19.0-rc.2` does not expose persistent minimized-window state in `Window`; current backend
  code maps `WindowOccluded` events into PlatformIO minimized feedback and otherwise falls back to
  the last observed value or `false`.
- `wasm32-unknown-unknown` compiles for the normal core and `render` feature sets, but
  `render,multi-viewport` intentionally fails at the core compile-time unsupported-target gate.
- Mobile multi-window support is outside this lane and needs a target-specific Bevy gate before it
  can be claimed.

## Next Recommended Action

- Keep this lane closed unless a regression is found.
- Open a new follow-on only if Bevy exposes more precise persistent minimized-window state,
  wasm/mobile platform support becomes a target, or runtime screenshot/manual OS-window smoke
  automation is required.
