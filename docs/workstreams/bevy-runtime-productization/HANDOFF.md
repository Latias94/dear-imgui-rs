# Bevy Runtime Productization Workstream — Handoff

Status: Active
Last updated: 2026-05-23

## Current State

The workstream is open. It follows the closed `docs/workstreams/bevy-native-backend/` lane, which
proved the Bevy-native backend architecture and one-frame examples.

## Active Task

- Task ID: BRP-040
- Owner: codex
- Files: `backends/dear-imgui-bevy/examples`, `backends/dear-imgui-bevy/src`,
  `backends/dear-imgui-bevy/README.md`
- Validation: `cargo +stable check -p dear-imgui-bevy --features render --example editor_shell`
- Status: TODO

## Decisions

- Keep the closed backend lane closed; this work lives in a new follow-on lane.
- Prefer a persistent Bevy windowed runner for BRP-020 instead of extending `ScheduleRunnerPlugin::run_once()` examples.
- Runtime renderer proof must touch real Bevy render resources or be clearly marked as an opt-in GPU smoke harness.
- BRP-030 uses ignored opt-in GPU harness tests because the checked path initializes a real native
  Bevy/wgpu adapter. The default package gate must show those tests skipped; use
  `DEAR_IMGUI_BEVY_GPU_HARNESS=1 cargo +stable test -p dear-imgui-bevy --features render --lib bevy_image_texture_bind_groups -- --ignored --nocapture`
  to run the GPU path.

## Blockers / Constraints

- Bevy `0.19.0-rc.2` requires Rust `1.95.0`, so use `cargo +stable` gates for this backend.
- Adding Bevy's top-level `DefaultPlugins` may increase dev-dependency weight. Keep that dependency example-only/dev-only unless it becomes part of the backend API.

## Next Recommended Action

Implement BRP-040: productize the editor shell into a richer example and/or helper layer with scene
viewport, panels, input policy, and extension-friendly composition.
