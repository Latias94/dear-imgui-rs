# Bevy Backend Product Follow-Ups Workstream - Handoff

Status: Split
Last updated: 2026-05-23

## Current State

The previous Bevy backend workstreams are closed. This lane tracks the next selected product-facing
follow-ups: example organization, docking multi-viewport OS windows, editor product layer, and
platform/CI stabilization.

## Active Task

- None in this lane. BBP-060 split real docking multi-viewport implementation to
  `docs/workstreams/bevy-docking-multi-viewport-v1/`.

## Decisions Since Last Update

- Keep the closed Bevy lanes closed.
- Preserve public Cargo example names even when moving source files into categorized directories.
- Treat docking multi-viewport OS windows as a separate platform-window problem, not as already
  completed by multi-window render-target routing.
- Keep editor product work in examples until reusable ownership boundaries become clear.
- BBP-020 grouped examples into `basic/`, `runtime/`, `ecosystem/`, and `editor/` while preserving
  Cargo example names.
- BBP-030 completed the first safe multi-viewport boundary slice: requests are visible in
  `ImguiBackendStatus`, support is explicitly false, and the backend does not set
  `ConfigFlags::VIEWPORTS_ENABLE` before Bevy OS-window PlatformIO callbacks exist.
- BBP-040 completed the editor product slice in `editor_shell`: the scene now seeds a real ECS
  hierarchy, the hierarchy panel selects actual entities, and the inspector edits selected
  `Name`/`Transform` data with focused example tests covering selection buffer behavior.
- BBP-050 completed the platform and CI stabilization slice: the Bevy backend now has a dedicated
  CI job in `.github/workflows/ci.yml` that runs format, example, render test, and wasm gates, and
  the backend README now says those gates belong to a dedicated Bevy lane rather than the root
  workspace workflow.
- BBP-060 split the actual Dear ImGui docking multi-viewport OS-window implementation into
  `docs/workstreams/bevy-docking-multi-viewport-v1/`. The new lane treats core `dear-imgui-rs`
  refactoring as acceptable when it is required by the Bevy multi-viewport contract.

## Blockers

- This lane has no blocker. Full Dear ImGui docking multi-viewport OS windows remain work in the
  split lane, starting with PlatformIO command capture and Bevy window lifecycle mapping.

## Next Recommended Action

- Continue in `docs/workstreams/bevy-docking-multi-viewport-v1/`, starting with DMV-020.
