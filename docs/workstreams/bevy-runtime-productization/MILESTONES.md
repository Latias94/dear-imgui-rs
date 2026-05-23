# Bevy Runtime Productization Workstream — Milestones

Status: Active
Last updated: 2026-05-23

## M0 — Scope And Evidence Freeze

Exit criteria:

- Follow-on scope is separated from the closed Bevy-native backend lane.
- Target state, non-goals, task ledger, and gate plan exist.

Primary evidence:

- `docs/workstreams/bevy-runtime-productization/DESIGN.md`
- `docs/workstreams/bevy-runtime-productization/TODO.md`
- `docs/workstreams/bevy-runtime-productization/EVIDENCE_AND_GATES.md`

## M1 — Persistent Windowed Runtime Proof

Exit criteria:

- A windowed example uses Bevy's normal event loop/runner.
- The example demonstrates ImGui overlay interaction and frame state over multiple frames.
- README distinguishes this runtime app from one-frame compile proofs.

Primary gates:

- `cargo +stable check -p dear-imgui-bevy --features render --example windowed_overlay`
- Manual run command recorded in README.

## M2 — Runtime Renderer Harness

Exit criteria:

- The `RenderDevice` / `RenderAssets<GpuImage>` / texture bind-group path is exercised by a runtime
  harness or explicitly documented opt-in smoke test.
- Deterministic package gates remain green.

Primary gates:

- targeted runtime harness command;
- `cargo +stable nextest run -p dear-imgui-bevy --features render`.

## M3 — Editor Shell Productization

Exit criteria:

- The editor shell is materially richer than the proof example.
- Reusable helper APIs are added only when they remove real repeated policy or state code.
- Scene viewport, panels, and input routing policy are documented.

Primary gates:

- `cargo +stable check -p dear-imgui-bevy --features render --example editor_shell`
- helper tests if helper APIs are added.

## M4 — Closeout

Exit criteria:

- Fresh gates are recorded.
- Workstream docs match shipped behavior.
- Remaining work is completed, explicitly deferred, or split into follow-ons.
- `WORKSTREAM.json` status is updated.
