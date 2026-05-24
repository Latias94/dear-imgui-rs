# Bevy Runtime Productization Workstream

Status: Closed
Last updated: 2026-05-23

## Why This Lane Exists

The first Bevy backend lane proved the core architecture, but most examples still run as one-frame
compile proofs. The next step is to prove the backend in real Bevy runtime conditions: a persistent
window, real render assets, GPU texture bind-group preparation, and an editor shell that feels like
a reusable product surface instead of a minimal proof.

## Relevant Authority

- ADRs:
  - `docs/adr/0001-bevy-native-imgui-backend.md`
- Related workstreams:
  - `docs/workstreams/bevy-native-backend/`
- Code:
  - `backends/dear-imgui-bevy/`
  - `backends/dear-imgui-bevy/examples/basic/simple.rs`
  - `backends/dear-imgui-bevy/examples/editor/editor_shell.rs`
  - `backends/dear-imgui-bevy/examples/ecosystem/ecosystem.rs`

## Problem

The backend currently has strong unit/integration proof for ECS lifecycle, render extraction, and
texture interop, but the public examples do not yet prove normal windowed runtime behavior. The
texture path is also still weakly covered at the real `RenderDevice` / `RenderAssets<GpuImage>`
boundary, and the editor shell remains a proof composition rather than a reusable editor-oriented
surface.

## Target State

When this workstream closes:

- `dear-imgui-bevy` has a persistent windowed example or smoke app using Bevy's normal windowed
  runner.
- There is a runtime renderer harness or smoke test that exercises real `RenderDevice`,
  `RenderAssets<GpuImage>`, and ImGui texture bind-group preparation.
- The editor shell is productized into a richer example and/or helper layer with scene viewport,
  panels, input policy, and extension-friendly composition.
- README and evidence docs explain which examples are compile proofs, runtime smoke apps, and editor
  productization examples.
- Fresh Bevy `cargo +stable` gates prove the shipped behavior.

## In Scope

- Add or update examples under `backends/dear-imgui-bevy/examples`.
- Add dev-only dependencies needed for real Bevy runtime examples.
- Add test/harness code for the render-device and user-image bind-group path.
- Add small reusable editor helper APIs only if they reduce real example complexity.
- Update README, workstream evidence, and changelog when user-facing examples change.

## Out Of Scope

- Multi-window routing beyond what is necessary to keep the new examples honest.
- Docking multi-viewport OS windows.
- WASM/mobile support.
- Raising the whole repository MSRV.
- A separate production editor crate unless the helper surface becomes too large for this backend.

## Starting Assumptions

| Assumption | Confidence | Evidence | Consequence if wrong |
| --- | --- | --- | --- |
| A real windowed example should use Bevy's normal runner, not `ScheduleRunnerPlugin::run_once()`. | High | The closed backend lane explicitly called the current examples one-frame proofs. | If Bevy's exact-pinned split dependencies make this too heavy, add a narrow dev-only plugin set and document it. |
| The GPU texture path can be proven with a runtime smoke harness without requiring screenshot-perfect rendering. | Medium | The renderer already has bind-group preparation logic and Bevy exposes `RenderDevice` / `RenderAssets<GpuImage>`. | If headless GPU setup is unstable on CI, keep the harness opt-in and retain compile/tests for deterministic gates. |
| The editor shell should become richer before extracting a new crate. | High | The current helper needs are still example-local: panels, scene viewport state, input policy, and extension contexts. | If helper code grows into reusable product API, split an explicit `dear-imgui-bevy-editor` follow-on. |

## Architecture Direction

Keep `dear-imgui-bevy` as the backend crate and use examples/harnesses to prove runtime behavior.
Only add public helper APIs when they directly simplify multiple examples or represent stable backend
policy. The runtime example should exercise normal Bevy lifecycle ownership: Bevy owns the event loop,
window, render app, WGPU device, camera targets, and image preparation; `ImguiPlugin` owns the ImGui
frame lifecycle and overlay rendering.

The runtime renderer harness should prefer real Bevy resources over hand-built mocks. A deterministic
unit test can still cover CPU-side preparation, but it must not be the only evidence for the
`RenderDevice` / `RenderAssets<GpuImage>` bind-group path.

## Closeout Condition

This lane can close when:

- the persistent windowed example compiles and has a documented manual run command;
- the runtime renderer harness compiles and runs in its intended environment, or an explicit
  documented opt-in gate exists when GPU availability prevents default CI execution;
- the editor shell is materially richer or backed by a reusable helper surface;
- fresh gates are recorded in `EVIDENCE_AND_GATES.md`;
- follow-ons are explicit rather than hidden in handoff notes.

## Result

- Closed 2026-05-23.
- The lane ships the persistent windowed example, runtime renderer harness coverage, and richer
  editor shell surface described above.
- Any future runtime/editor expansion should open a new follow-on workstream.
