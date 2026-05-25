# Bevy Backend Prelaunch Refactor - Milestones

Status: Closed
Last updated: 2026-05-25

## M0 - Scope And Evidence Freeze

Status: Done

Exit criteria:

- Problem and target state are explicit.
- Breaking changes are explicitly allowed inside this lane.
- Related workstreams and issue themes are linked.
- First proof target is chosen.

Primary evidence:

- `docs/workstreams/bevy-backend-prelaunch-refactor-v1/DESIGN.md`
- `docs/workstreams/bevy-backend-prelaunch-refactor-v1/TODO.md`

## M1 - Overlay Target Contract And Camera Viewports

Status: Done

Exit criteria:

- Public API exposes an explicit way to select ImGui overlay cameras.
- The fallback target policy is documented and tested.
- Render pass viewport state follows Bevy camera viewport state.

Primary gates:

- focused render target tests
- `cargo +stable nextest run -p dear-imgui-bevy --features render`

## M2 - Color And Texture Interop Correctness

Status: Done

Exit criteria:

- Gamma selection considers Bevy compositing state.
- Registered Bevy image sampler behavior is either honored or explicitly represented by API.
- Tests cover the changed contract.

Primary gates:

- focused color and texture tests
- `cargo +stable nextest run -p dear-imgui-bevy --features render`

## M3 - Product Policy And CI Stability

Status: Done

Exit criteria:

- Input capture helpers exist and docs explain how to use them.
- Runtime support boundaries are documented.
- nextest discovery and test execution complete without manual intervention.

Primary gates:

- `cargo +stable nextest run -p dear-imgui-bevy --features render`
- `cargo +stable nextest run -p dear-imgui-bevy --features render,multi-viewport`
- README review

## M4 - Closeout

Status: Done

Exit criteria:

- Gate set is recorded.
- Remaining work is completed, deferred, or split into a follow-on.
- `WORKSTREAM.json` status is updated.
