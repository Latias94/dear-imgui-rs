# Bevy Backend Follow-Ups Workstream - TODO

Status: Closed
Last updated: 2026-05-23

## M0 - Scope And Evidence Freeze

- [x] BBF-010 [owner=planner] [deps=none] [scope=docs/workstreams/bevy-backend-followups-v1]
  Goal: Freeze the follow-up problem statement, the five target slices, and the initial evidence
  anchors.
  Validation: `DESIGN.md`, `MILESTONES.md`, `EVIDENCE_AND_GATES.md`, and `WORKSTREAM.json` exist and
  agree.
  Evidence: `docs/workstreams/bevy-backend-followups-v1/DESIGN.md`
  Handoff: DONE 2026-05-23. The follow-up lane records the five target slices, initial gates, and
  authoritative evidence anchors.

## M1 - Multi-Window And Input Polish

- [x] BBF-020 [owner=codex] [deps=BBF-010] [scope=backends/dear-imgui-bevy/src/input.rs,backends/dear-imgui-bevy/src/render.rs,backends/dear-imgui-bevy/tests]
  Goal: Make multi-window routing and overlay targeting explicit, with tests for primary and
  non-primary windows.
  Validation: `cargo +stable nextest run -p dear-imgui-bevy input` plus any focused multi-window
  filter added by the task.
  Review: `review-workstream` completed 2026-05-23 with no blocking findings.
  Evidence: `backends/dear-imgui-bevy/tests/render_extract.rs`, `backends/dear-imgui-bevy/src/render.rs`, fresh `cargo +stable nextest run -p dear-imgui-bevy input`, fresh `cargo +stable nextest run -p dear-imgui-bevy --features render render_extract`, and fresh `cargo +stable nextest run -p dear-imgui-bevy --features render`.
  Handoff: DONE 2026-05-23. Kept the primary-window input contract explicit, and added render extraction coverage for both primary and non-primary window camera targets. Overlay routing now proves the same extracted frame is fanned out to primary and secondary window cameras without assuming the primary window is the only render target.

- [x] BBF-030 [owner=codex] [deps=BBF-010] [scope=backends/dear-imgui-bevy/src/context.rs,backends/dear-imgui-bevy/src/input.rs,backends/dear-imgui-bevy/tests]
  Goal: Tighten cursor, IME, and focus feedback so platform behavior is less approximate.
  Validation: `cargo +stable nextest run -p dear-imgui-bevy input` plus any cursor/IME-specific tests
  added by the task.
  Review: `review-workstream` completed 2026-05-23 with no blocking findings.
  Evidence: `backends/dear-imgui-bevy/src/context.rs`, `backends/dear-imgui-bevy/src/input.rs`, `backends/dear-imgui-bevy/tests/input.rs`, fresh `cargo +stable nextest run -p dear-imgui-bevy input`, fresh `cargo +stable nextest run -p dear-imgui-bevy --features render`, and fresh example checks for `windowed_overlay` and `editor_shell`.
  Handoff: DONE 2026-05-23. Kept the current primary-window input policy, and synchronized the
  completed Dear ImGui frame's cursor and IME platform feedback into the primary Bevy window.
  Covered OS cursor icon selection, software/no-cursor hiding, IME enablement, and IME position.

## M2 - Platform And Runtime Proof

- [x] BBF-040 [owner=codex] [deps=BBF-010] [scope=backends/dear-imgui-bevy,examples-wasm]
  Goal: Establish a wasm/mobile compile path or a clearly documented opt-in gate for the current
  backend shape.
  Validation: `cargo +stable check -p dear-imgui-bevy --target wasm32-unknown-unknown --no-default-features`
  or the chosen equivalent documented by the task.
  Review: `review-workstream` before accepting completion.
  Evidence: `backends/dear-imgui-bevy/src/lib.rs`, `backends/dear-imgui-bevy/README.md`, wasm32-unknown-unknown compile gates, and `docs/workstreams/bevy-backend-followups-v1/JOURNAL/2026-05-23-bbf-040.md`.
  Handoff: DONE 2026-05-23. Verified wasm32-unknown-unknown compilation for both the core and
  `render` feature sets, and documented the current mobile split as a follow-on if Bevy's support
  matrix needs a distinct gate.

- [x] BBF-050 [owner=codex] [deps=BBF-010] [scope=backends/dear-imgui-bevy/src/render.rs,backends/dear-imgui-bevy/tests,backends/dear-imgui-bevy/examples]
  Goal: Harden runtime smoke coverage so real Bevy render resources keep the renderer honest.
  Validation: `cargo +stable nextest run -p dear-imgui-bevy --features render` plus the opt-in GPU or
  runtime smoke command chosen by the task.
  Review: `review-workstream` before accepting completion.
  Evidence: `backends/dear-imgui-bevy/src/render.rs`, `backends/dear-imgui-bevy/tests/texture.rs`,
  fresh `cargo +stable nextest run -p dear-imgui-bevy --features render`, fresh
  `DEAR_IMGUI_BEVY_GPU_HARNESS=1 cargo +stable test -p dear-imgui-bevy --features render --lib bevy_image_texture_bind_groups -- --ignored --nocapture`,
  and `docs/workstreams/bevy-backend-followups-v1/JOURNAL/2026-05-23-bbf-050.md`.
  Handoff: DONE 2026-05-23. The runtime path now has a real Bevy render-resource smoke harness
  without turning the lane into a screenshot-only project.

## M3 - Editor Helper Surface

- [x] BBF-060 [owner=codex] [deps=BBF-010] [scope=backends/dear-imgui-bevy/src,backends/dear-imgui-bevy/examples,backends/dear-imgui-bevy/README.md]
  Goal: Extract or formalize a reusable editor/helper surface if the example code starts to repeat.
  Validation: `cargo +stable check -p dear-imgui-bevy --features render --example editor_shell`
  and any new helper tests added by the task.
  Review: `review-workstream` before accepting completion.
  Evidence: `backends/dear-imgui-bevy/src/helpers.rs`, `backends/dear-imgui-bevy/tests/helpers.rs`,
  `backends/dear-imgui-bevy/examples/simple.rs`,
  `backends/dear-imgui-bevy/examples/windowed_overlay.rs`,
  `backends/dear-imgui-bevy/examples/editor_shell.rs`,
  `backends/dear-imgui-bevy/examples/ecosystem.rs`,
  `backends/dear-imgui-bevy/README.md`, fresh
  `cargo +stable nextest run -p dear-imgui-bevy configure_example_context`, fresh
  `cargo +stable check -p dear-imgui-bevy --examples --features render`, and fresh
  `cargo +stable nextest run -p dear-imgui-bevy --features render`.
  Handoff: DONE 2026-05-23. The reusable editor/helper surface is a small public context helper
  rather than a new crate.

## M4 - Closeout

- [x] BBF-070 [owner=planner] [deps=BBF-020,BBF-030,BBF-040,BBF-050,BBF-060] [scope=docs/workstreams/bevy-backend-followups-v1]
  Goal: Close the lane or split any wider product slice.
  Validation: `verify-rust-workstream` records fresh final evidence.
  Review: `review-workstream` completed 2026-05-23 with no blocking findings.
  Evidence: `docs/workstreams/bevy-backend-followups-v1/EVIDENCE_AND_GATES.md`, `docs/workstreams/bevy-backend-followups-v1/WORKSTREAM.json`
  Handoff: DONE 2026-05-23. The five follow-up targets are complete, the final closeout evidence is recorded, and any future Bevy backend polish should open a new follow-on workstream instead of reusing this lane.
