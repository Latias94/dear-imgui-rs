# Bevy Backend Follow-Ups Workstream

Status: Closed
Last updated: 2026-05-23

## Why This Lane Exists

The Bevy-native Dear ImGui backend is shipped and the runtime productization lane is closed, but a
few user-facing follow-ups still deserve a durable lane of their own. These are the remaining
polish and platform gaps that are worth tracking together instead of hiding them in ad hoc fixes.

## Relevant Authority

- ADRs:
  - `docs/adr/0001-bevy-native-imgui-backend.md`
- Related workstreams:
  - `docs/workstreams/bevy-native-backend/`
  - `docs/workstreams/bevy-runtime-productization/`
- Code:
  - `backends/dear-imgui-bevy/src/input.rs`
  - `backends/dear-imgui-bevy/src/render.rs`
  - `backends/dear-imgui-bevy/examples/windowed_overlay.rs`
  - `backends/dear-imgui-bevy/examples/editor_shell.rs`

## Problem

The backend core is stable, but five follow-up slices remain open: multi-window routing, cursor and
IME feedback polish, wasm/mobile support, runtime harness hardening, and a reusable editor/helper
surface. Each slice can be validated independently, but together they define the next useful Bevy
backend lane.

## Target State

When this workstream closes:

- multi-window input routing is explicit and tested;
- cursor and IME behavior matches platform expectations more closely;
- wasm/mobile support has a documented and checked path;
- runtime smoke or harness coverage keeps real Bevy render resources honest;
- editor/helper code is either extracted into a stable surface or explicitly split into a narrower
  follow-on.

## In Scope

- multi-window routing and overlay targeting;
- cursor, IME, and focus feedback polish;
- wasm/mobile compilation or documented opt-in gates;
- runtime harness or smoke coverage for Bevy render resources;
- reusable editor/helper code when the example surface becomes repetitive;
- docs and evidence updates for any shipped follow-up.

## Out Of Scope

- reworking the shipped core lifecycle or snapshot contracts;
- replacing the Bevy-native renderer architecture;
- raising the repository MSRV;
- a separate product editor crate unless helper code becomes clearly reusable;
- reopening the closed proof lanes without a regression.

## Starting Assumptions

| Assumption | Confidence | Evidence | Consequence if wrong |
| --- | --- | --- | --- |
| The shipped Bevy backend is stable enough to extend without reopening core lifecycle work. | High | The closed backend and runtime lanes already proved the main frame/render path. | Split a smaller regression lane instead of widening this one. |
| Multi-window and input polish can be validated with focused tests plus example runs. | Medium | The backend already has targeted input, render, and example gates. | Add a dedicated smoke harness if platform behavior proves too broad for unit tests. |
| wasm/mobile support will likely need opt-in gates and may require platform-specific pruning. | Medium | The backend currently targets native Bevy first. | Split a platform-specific follow-on if the support matrix diverges too far. |

## Architecture Direction

Keep `dear-imgui-bevy` as the owner of the backend behavior. Prefer small helper APIs, tests, and
example-level proof slices over a new abstraction layer. If any follow-up introduces a hard
contract boundary, record that in a new ADR or split it into a narrower workstream rather than
stretching this one past its useful shape.

## Closeout Condition

This lane can close when:

- all five follow-up targets are either completed or intentionally split out;
- fresh gates are recorded for the implemented slices;
- docs describe the shipped behavior clearly;
- and no hidden follow-up remains inside the lane.

Closed 2026-05-23 after all five follow-up targets were completed with fresh evidence and the
closeout gate was verified. Any future Bevy backend polish or product work should split into a new
follow-on workstream instead of reopening this lane.
