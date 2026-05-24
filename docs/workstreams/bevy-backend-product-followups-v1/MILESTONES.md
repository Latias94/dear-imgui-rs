# Bevy Backend Product Follow-Ups Workstream - Milestones

Status: Split
Last updated: 2026-05-23

## M0 - Scope And Evidence Freeze

Exit criteria:

- the four selected follow-up targets are named;
- the closed Bevy lanes are linked as prior art;
- initial gates and evidence anchors are listed.

Primary evidence:

- `docs/workstreams/bevy-backend-product-followups-v1/DESIGN.md`
- `docs/workstreams/bevy-backend-product-followups-v1/TODO.md`

## M1 - Example Catalog

Exit criteria:

- examples are grouped by purpose under `examples/`;
- Cargo example names and run commands remain stable;
- README explains which example to run for each use case.

Primary gates:

- `cargo +stable fmt --all --check`
- `cargo +stable check -p dear-imgui-bevy --examples --features render`

## M2 - Docking Multi-Viewport OS Windows

Exit criteria:

- the difference between render-target multi-window routing and Dear ImGui OS-level viewports is
  made explicit;
- the smallest implementable slice lands, or a narrower workstream is opened with a concrete design
  boundary.

Primary gates:

- focused viewport/window lifecycle tests;
- `cargo +stable nextest run -p dear-imgui-bevy --features render`

## M3 - Editor Product Layer

Exit criteria:

- `editor_shell` gains a useful editor workflow slice beyond static panels;
- reusable helpers are extracted only when they reduce real example duplication;
- broader editor application work is split if needed.

Primary gates:

- `cargo +stable check -p dear-imgui-bevy --features render --example editor_shell`

## M4 - Platform And CI Stabilization

Exit criteria:

- Bevy-specific Rust, Bevy, native, and wasm gate policy is clear;
- any CI changes are scoped to the Bevy backend and do not silently raise workspace MSRV;
- mobile-specific uncertainty is recorded or split.

Primary gates:

- `cargo +stable check -p dear-imgui-bevy --target wasm32-unknown-unknown --no-default-features`
- `cargo +stable check -p dear-imgui-bevy --target wasm32-unknown-unknown --features render`
- `cargo +stable nextest run -p dear-imgui-bevy --features render`

## M5 - Closeout

Exit criteria:

- all shipped slices have fresh evidence;
- unfinished slices are split or explicitly deferred;
- `WORKSTREAM.json` and `HANDOFF.md` match the final state.

Outcome:

- BBP-060 split actual Dear ImGui docking multi-viewport OS-window support to
  `docs/workstreams/bevy-docking-multi-viewport-v1/`.
