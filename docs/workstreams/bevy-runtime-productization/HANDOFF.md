# Bevy Runtime Productization Workstream — Handoff

Status: Closed
Last updated: 2026-05-23

## Current State

The workstream is closed. It follows the closed `docs/workstreams/bevy-native-backend/` lane, which
proved the Bevy-native backend architecture and one-frame examples.

## Active Task

- None. BRP-050 is complete and the lane is closed.

## Decisions

- Keep the closed backend lane closed; this work lives in a new follow-on lane.
- Prefer a persistent Bevy windowed runner for BRP-020 instead of extending `ScheduleRunnerPlugin::run_once()` examples.
- Runtime renderer proof must touch real Bevy render resources or be clearly marked as an opt-in GPU smoke harness.
- BRP-030 uses ignored opt-in GPU harness tests because the checked path initializes a real native
  Bevy/wgpu adapter. The default package gate must show those tests skipped; use
  `DEAR_IMGUI_BEVY_GPU_HARNESS=1 cargo +stable test -p dear-imgui-bevy --features render --lib bevy_image_texture_bind_groups -- --ignored --nocapture`
  to run the GPU path.
- BRP-040 keeps editor productization inside the backend example plus one small backend helper
  marker. `render::ImguiOverlayDisabled` marks offscreen scene cameras that should not receive the
  global ImGui overlay pass.
- BRP-050 closes the lane after fresh gate evidence, status updates, and changelog coverage are
  recorded.

## Blockers / Constraints

- Bevy `0.19.0-rc.2` requires Rust `1.95.0`, so use `cargo +stable` gates for this backend.
- Adding Bevy's top-level `DefaultPlugins` may increase dev-dependency weight. Keep that dependency example-only/dev-only unless it becomes part of the backend API.

## Next Recommended Action

Start a new follow-on workstream if future runtime/editor product work is needed; otherwise keep
this lane closed.
