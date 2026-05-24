# Bevy Backend Product Follow-Ups Workstream - TODO

Status: Split
Last updated: 2026-05-23

## M0 - Scope And Evidence Freeze

- [x] BBP-010 [owner=planner] [deps=none] [scope=docs/workstreams/bevy-backend-product-followups-v1]
  Goal: Freeze the four selected follow-up targets from the recovered session: example
  organization, docking multi-viewport OS windows, editor product layer, and platform/CI
  stabilization.
  Validation: `DESIGN.md`, `TODO.md`, `MILESTONES.md`, `EVIDENCE_AND_GATES.md`, `WORKSTREAM.json`,
  and `HANDOFF.md` exist and agree.
  Evidence: `docs/workstreams/bevy-backend-product-followups-v1/DESIGN.md`
  Handoff: DONE 2026-05-23. The lane tracks the user's selected items 1, 2, 3, and 5 as product
  follow-ups after the closed Bevy backend lanes.

## M1 - Example Catalog

- [x] BBP-020 [owner=codex] [deps=BBP-010] [scope=backends/dear-imgui-bevy/Cargo.toml,backends/dear-imgui-bevy/README.md,backends/dear-imgui-bevy/examples]
  Goal: Organize Bevy backend examples by purpose while preserving existing Cargo example names and
  adding a README index that explains when to run each one.
  Validation: `cargo +stable fmt --all --check` and `cargo +stable check -p dear-imgui-bevy --examples --features render`.
  Review: Review the Cargo example paths and README commands before accepting completion.
  Evidence: `backends/dear-imgui-bevy/examples/`, `backends/dear-imgui-bevy/README.md`, and this
  workstream's verification log.
  Handoff: DONE 2026-05-23. Examples are now grouped under `basic/`, `runtime/`, `ecosystem/`,
  and `editor/`; Cargo example names stayed stable, and the README has a purpose-based example
  index. The catalog is enough for future node-editor, runtime, and editor demos without returning
  to a flat directory.

## M2 - Docking Multi-Viewport OS Windows

- [x] BBP-030 [owner=codex] [deps=BBP-020] [scope=backends/dear-imgui-bevy/src,backends/dear-imgui-bevy/tests,backends/dear-imgui-bevy/examples]
  Goal: Define and land the smallest credible Dear ImGui docking multi-viewport OS-window slice, or
  split a narrower design lane if Bevy window ownership makes the implementation too large.
  Validation: Focused tests for viewport/window lifecycle plus `cargo +stable nextest run -p dear-imgui-bevy --features render`.
  Review: Requires review for Bevy window lifecycle ownership, platform callbacks, and regression
  risk against the existing multi-window render-target path.
  Evidence: `backends/dear-imgui-bevy/src/lib.rs`, `backends/dear-imgui-bevy/tests/plugin.rs`,
  `backends/dear-imgui-bevy/tests/lifecycle.rs`, and this workstream's verification log.
  Handoff: DONE_WITH_CONCERNS 2026-05-23. The first credible slice records multi-viewport requests
  in `ImguiBackendStatus` and proves the backend does not falsely advertise
  `ConfigFlags::VIEWPORTS_ENABLE` before Bevy OS-window platform callbacks are wired. The actual
  platform-window implementation remains a follow-up design/implementation split; existing
  multi-window camera routing is still not OS-level docking viewport support.

## M3 - Editor Product Layer

- [x] BBP-040 [owner=codex] [deps=BBP-020,BBP-030] [scope=backends/dear-imgui-bevy/examples/editor,backends/dear-imgui-bevy/src]
  Goal: Add one useful editor product slice beyond the shell scaffold, such as ECS hierarchy with
  selection, a component inspector, or a reusable viewport helper.
  Validation: `cargo +stable check -p dear-imgui-bevy --features render --example editor_shell`
  plus any focused helper tests added by the task.
  Review: Confirm the slice belongs in the backend example surface and does not prematurely become
  a separate editor crate.
  Evidence: `backends/dear-imgui-bevy/examples/editor/editor_shell.rs`,
  `docs/workstreams/bevy-backend-product-followups-v1/JOURNAL/2026-05-23-bbp-040.md`, and this
  workstream's verification log.
  Handoff: DONE 2026-05-23. `editor_shell` now seeds a real ECS scene hierarchy, supports entity
  selection, and lets the inspector edit `Name` and `Transform` data for the selected entity while
  keeping the slice inside the backend example surface.

## M4 - Platform And CI Stabilization

- [x] BBP-050 [owner=codex] [deps=BBP-020] [scope=backends/dear-imgui-bevy,docs,.github]
  Goal: Stabilize the Bevy backend platform/CI policy around Rust 1.95, Bevy `0.19.0-rc.2`,
  wasm gates, and native render gates.
  Validation: Bevy-specific check/nextest gates from `EVIDENCE_AND_GATES.md`; CI changes require
  the matching workflow validation or a documented dry-run limit.
  Review: Confirm this does not accidentally raise the whole workspace MSRV or make main-branch
  CI depend on unavailable platform runners.
  Evidence: `.github/workflows/ci.yml`, `backends/dear-imgui-bevy/README.md`,
  `docs/workstreams/bevy-backend-product-followups-v1/JOURNAL/2026-05-23-bbp-050.md`, and this
  workstream's verification log.
  Handoff: DONE 2026-05-23. The Bevy backend now has a dedicated CI gate lane for format, example,
  render test, and wasm checks, and the README makes it clear the backend gates are not covered by
  the root workspace workflow alone.

## M5 - Closeout

- [x] BBP-060 [owner=planner] [deps=BBP-020,BBP-030,BBP-040,BBP-050] [scope=docs/workstreams/bevy-backend-product-followups-v1]
  Goal: Close this lane or split any unfinished product slice with clear evidence.
  Validation: `verify-rust-workstream` records fresh final gate evidence.
  Review: `review-workstream` has no blocking findings.
  Evidence: `docs/workstreams/bevy-backend-product-followups-v1/EVIDENCE_AND_GATES.md`,
  `docs/workstreams/bevy-backend-product-followups-v1/WORKSTREAM.json`
  Handoff: DONE_WITH_SPLIT 2026-05-23. The product-facing slices are complete enough to stop using
  this lane for implementation, but real Dear ImGui docking multi-viewport OS-window support is now
  the next goal and has been split to `docs/workstreams/bevy-docking-multi-viewport-v1/`. That lane
  explicitly allows core `dear-imgui-rs` refactoring when it is required to make Bevy's
  engine-managed PlatformIO bridge safe and testable.
