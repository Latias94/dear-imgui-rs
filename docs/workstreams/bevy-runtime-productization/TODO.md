# Bevy Runtime Productization Workstream — TODO

Status: Active
Last updated: 2026-05-23

## M0 — Scope And Evidence Freeze

- [x] BRP-010 [owner=planner] [deps=none] [scope=docs/workstreams/bevy-runtime-productization]
  Goal: Freeze problem, target state, non-goals, and evidence anchors for Bevy runtime productization.
  Validation: `DESIGN.md`, `MILESTONES.md`, `EVIDENCE_AND_GATES.md`, `WORKSTREAM.json`, and `HANDOFF.md` exist and agree.
  Evidence: `docs/workstreams/bevy-runtime-productization/DESIGN.md`
  Handoff: DONE 2026-05-23. Continue with BRP-020.

## M1 — Persistent Windowed Runtime Proof

- [x] BRP-020 [owner=codex] [deps=BRP-010] [scope=backends/dear-imgui-bevy/Cargo.toml,backends/dear-imgui-bevy/examples,backends/dear-imgui-bevy/README.md]
  Goal: Add a persistent windowed Bevy example or smoke app that uses Bevy's normal windowed runner and demonstrates ImGui overlay interaction in a real window.
  Validation: `cargo +stable check -p dear-imgui-bevy --features render --example windowed_overlay` plus the documented manual run command.
  Review: `review-workstream` before accepting completion.
  Evidence: `backends/dear-imgui-bevy/examples/windowed_overlay.rs`, README command, fresh gate output.
  Handoff: DONE 2026-05-23. Added `windowed_overlay.rs` using Bevy `DefaultPlugins` and a dev-only top-level `bevy` dependency. The example opens a persistent runtime window, draws ImGui overlay state across frames, and exits on Escape.

## M2 — Runtime Renderer Harness

- [ ] BRP-030 [owner=codex] [deps=BRP-020] [scope=backends/dear-imgui-bevy/src/render.rs,backends/dear-imgui-bevy/tests,backends/dear-imgui-bevy/examples]
  Goal: Add a runtime renderer harness or smoke test that covers real `RenderDevice`, `RenderAssets<GpuImage>`, and texture bind-group preparation for Bevy `Handle<Image>` user textures.
  Validation: targeted render harness gate plus `cargo +stable nextest run -p dear-imgui-bevy --features render`.
  Review: `review-workstream` before accepting completion.
  Evidence: harness path and `EVIDENCE_AND_GATES.md`.
  Handoff: If GPU availability requires opt-in execution, document the skipped default gate and provide a reliable manual command.

## M3 — Editor Shell Productization

- [ ] BRP-040 [owner=codex] [deps=BRP-020,BRP-030] [scope=backends/dear-imgui-bevy/examples,backends/dear-imgui-bevy/src,backends/dear-imgui-bevy/README.md]
  Goal: Productize the editor shell into a richer example and/or helper layer with scene viewport, panels, input policy, and extension-friendly composition.
  Validation: `cargo +stable check -p dear-imgui-bevy --features render --example editor_shell` and any helper tests added by the task.
  Review: `review-workstream` before accepting completion.
  Evidence: updated editor shell/helper paths and fresh gates.
  Handoff: Split a separate editor crate if helper scope grows beyond backend-local policy.

## M4 — Closeout

- [ ] BRP-050 [owner=planner] [deps=BRP-020,BRP-030,BRP-040] [scope=docs/workstreams/bevy-runtime-productization,CHANGELOG.md]
  Goal: Finalize docs, record fresh gates, split remaining risks, and close or continue this runtime productization lane.
  Validation: `verify-rust-workstream` records final fresh evidence.
  Review: `review-workstream` has no blocking findings.
  Evidence: `EVIDENCE_AND_GATES.md`, `WORKSTREAM.json`, closeout journal.
  Handoff: Summarize remaining risks in `HANDOFF.md`.

## Parallelization Notes

BRP-020 and BRP-030 should not run in parallel until the runtime dependency shape is settled. BRP-040
can begin after BRP-020 proves the example dependency set.
