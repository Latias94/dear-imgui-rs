# Bevy Backend Prelaunch Refactor - TODO

Status: Closed
Last updated: 2026-05-25

## M0 - Scope And Evidence Freeze

- [x] BPR-010 [owner=planner] [deps=none] [scope=docs/workstreams/bevy-backend-prelaunch-refactor-v1]
  Goal: Open the prelaunch refactor lane and make breaking Bevy backend and core crate changes
  explicitly allowed when they improve the release contract.
  Validation: `DESIGN.md`, `TODO.md`, `MILESTONES.md`, `EVIDENCE_AND_GATES.md`,
  `WORKSTREAM.json`, and `HANDOFF.md` exist and agree.
  Evidence: `docs/workstreams/bevy-backend-prelaunch-refactor-v1/DESIGN.md`
  Handoff: DONE 2026-05-25. Implementation starts with render target ownership because it affects
  public API, extraction, viewport rendering, tests, and examples.

## M1 - Overlay Target Contract And Camera Viewports

- [x] BPR-020 [owner=codex] [deps=BPR-010] [scope=backends/dear-imgui-bevy/src/render.rs,backends/dear-imgui-bevy/tests,examples]
  Goal: Replace the implicit highest-order camera contract with an explicit overlay camera API and
  honor Bevy `Camera.viewport` during ImGui overlay rendering.
  Validation: focused render target tests plus `cargo +stable nextest run -p dear-imgui-bevy --features render`.
  Review: Confirm existing simple examples still work, explicit markers are documented, and
  split-screen/camera viewport behavior cannot draw outside the intended render region.
  Evidence: render tests and README render extraction section.
  Handoff: DONE 2026-05-25. Added `ImguiOverlayCamera`, preserved highest-order fallback,
  extracted camera viewports, applied render pass viewports, clipped scissors to camera viewports,
  and updated examples/docs.

## M2 - Color And Compositing Correctness

- [x] BPR-030 [owner=codex] [deps=BPR-020] [scope=backends/dear-imgui-bevy/src/render.rs,tests]
  Goal: Make ImGui gamma/color correction account for Bevy target format and compositing space.
  Validation: focused unit tests around target format/compositing combinations plus the render
  feature nextest gate.
  Review: Confirm the implementation follows Bevy 0.19 render docs and avoids special-casing only
  the default window path.
  Evidence: color/compositing tests.
  Handoff: DONE 2026-05-25. `ImguiUniforms::gamma_for_target` accounts for sRGB formats and
  `CompositingSpace::Srgb`; render pass reads extracted camera compositing metadata.

## M3 - Bevy Image Sampler Contract

- [x] BPR-040 [owner=codex] [deps=BPR-020] [scope=backends/dear-imgui-bevy/src/render.rs,src/texture.rs,tests,README]
  Goal: Either honor Bevy image sampler semantics for registered textures or replace the public
  contract with an explicit ImGui sampler policy that is backed by tests and docs.
  Validation: focused texture registration/bind group preparation tests plus render feature nextest
  gate.
  Review: Confirm the final API does not silently ignore user-visible Bevy asset settings.
  Evidence: texture interop tests and README texture interop section.
  Handoff: DONE 2026-05-25. Texture bind groups now include samplers; registered Bevy images use
  `GpuImage::sampler`, while managed ImGui textures keep linear/nearest sampler selection.

## M4 - Input Capture Helpers And Support Matrix

- [x] BPR-050 [owner=codex] [deps=BPR-010] [scope=backends/dear-imgui-bevy/src/input.rs,README,examples]
  Goal: Add ergonomic capture predicates/run conditions and document runtime limitations for
  picking, game input, clipboard, IME, wasm, gamepad, and file drop.
  Validation: focused input capture tests plus no-default/render check gates.
  Review: Confirm helpers are policy hints and the backend still does not consume Bevy messages.
  Evidence: input tests and README policy section.
  Handoff: DONE 2026-05-25. Added capture predicate methods and `imgui_wants_*` run conditions;
  README documents policy-hint behavior and runtime support boundaries.

## M5 - Test Discovery And CI Stability

- [x] BPR-060 [owner=codex] [deps=BPR-020,BPR-030,BPR-040,BPR-050] [scope=backends/dear-imgui-bevy/tests,Cargo.toml,CI docs]
  Goal: Reproduce and fix the `cargo nextest` test discovery hang or isolate the exact blocking
  test binary with a documented follow-up.
  Validation: `cargo +stable nextest run -p dear-imgui-bevy --features render` and
  `cargo +stable nextest run -p dear-imgui-bevy --features render,multi-viewport` complete.
  Review: Confirm the gates are viable for CI without manual timeouts.
  Evidence: command output recorded in `EVIDENCE_AND_GATES.md`.
  Handoff: DONE_WITH_CONTEXT 2026-05-25. The hang reproduced with the polluted shared root
  `target/debug/deps`; fresh gates complete using `CARGO_TARGET_DIR=target/bevy-backend-prelaunch`.
  README documents the dedicated target-dir requirement for local release checks.

## M6 - Closeout

- [x] BPR-070 [owner=planner] [deps=BPR-060] [scope=docs/workstreams/bevy-backend-prelaunch-refactor-v1]
  Goal: Review, verify, and close the lane or split remaining platform-specific follow-ons.
  Validation: `verify-rust-workstream` records fresh final gate evidence.
  Review: `review-workstream` has no blocking findings.
  Evidence: `EVIDENCE_AND_GATES.md`, `WORKSTREAM.json`, and `HANDOFF.md`.
  Handoff: DONE 2026-05-25. Verification evidence, review status, shipped API changes, and
  residual risks are recorded in closeout docs.
