# Bevy Runtime Productization Workstream — Handoff

Status: Active
Last updated: 2026-05-23

## Current State

The workstream is open. It follows the closed `docs/workstreams/bevy-native-backend/` lane, which
proved the Bevy-native backend architecture and one-frame examples.

## Active Task

- Task ID: BRP-030
- Owner: codex
- Files: `backends/dear-imgui-bevy/src/render.rs`, `backends/dear-imgui-bevy/tests`,
  `backends/dear-imgui-bevy/examples`
- Validation: targeted render harness gate plus `cargo +stable nextest run -p dear-imgui-bevy --features render`
- Status: TODO

## Decisions

- Keep the closed backend lane closed; this work lives in a new follow-on lane.
- Prefer a persistent Bevy windowed runner for BRP-020 instead of extending `ScheduleRunnerPlugin::run_once()` examples.
- Runtime renderer proof must touch real Bevy render resources or be clearly marked as an opt-in GPU smoke harness.

## Blockers / Constraints

- Bevy `0.19.0-rc.2` requires Rust `1.95.0`, so use `cargo +stable` gates for this backend.
- Adding Bevy's top-level `DefaultPlugins` may increase dev-dependency weight. Keep that dependency example-only/dev-only unless it becomes part of the backend API.

## Next Recommended Action

Implement BRP-030: add runtime renderer harness coverage for real `RenderDevice`,
`RenderAssets<GpuImage>`, and Bevy image texture bind-group preparation.
