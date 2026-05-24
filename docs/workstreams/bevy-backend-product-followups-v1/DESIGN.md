# Bevy Backend Product Follow-Ups Workstream

Status: Split
Last updated: 2026-05-23

## Why This Lane Exists

The Bevy backend has a closed native backend lane, a closed runtime productization lane, and a
closed first follow-up lane. The remaining work is now product-facing: examples need a stable
catalog, docking multi-viewport needs an explicit OS-window plan, the editor shell needs to grow
toward a real editor surface, and platform/CI coverage needs to be kept honest while Bevy remains
on its own Rust and release train.

## Relevant Authority

- ADRs:
  - `docs/adr/0001-bevy-native-imgui-backend.md`
- Related workstreams:
  - `docs/workstreams/bevy-native-backend/`
  - `docs/workstreams/bevy-runtime-productization/`
  - `docs/workstreams/bevy-backend-followups-v1/`
- Code and docs:
  - `backends/dear-imgui-bevy/Cargo.toml`
  - `backends/dear-imgui-bevy/README.md`
  - `backends/dear-imgui-bevy/examples/`
  - `backends/dear-imgui-bevy/src/`

## Problem

The backend is functional, but the next user-facing shape is still underdeveloped. Examples are
growing in one flat directory, docking multi-viewport is not the same thing as the current
multi-window render-target routing, the editor example is still a shell rather than a product
surface, and platform/CI expectations need a stable policy while Bevy's target matrix evolves.

## Target State

When this workstream closes:

- Bevy examples are organized by purpose, with README commands and maintenance guidance;
- docking multi-viewport OS windows are either implemented or split with a precise design boundary;
- the editor product layer has a concrete useful slice beyond the shell scaffold;
- platform and CI gates describe the supported Bevy backend matrix without hiding MSRV or target
  constraints;
- fresh evidence is recorded for every shipped slice.

## In Scope

- example directory organization and README index updates;
- Cargo example path maintenance while preserving example names;
- docking multi-viewport investigation, implementation, or narrower split;
- editor-facing product slices such as hierarchy, selection, inspector, viewport, or helper APIs;
- platform and CI policy for Bevy-specific gates;
- workstream evidence and handoff updates.

## Out Of Scope

- reopening the three closed Bevy lanes without a regression;
- replacing the Bevy-native renderer architecture with winit/wgpu wrappers;
- raising the whole workspace MSRV as an incidental side effect;
- shipping a separate editor application crate before the backend examples justify that boundary;
- promising mobile runtime support without a target-specific gate.

## Starting Assumptions

| Assumption | Confidence | Evidence | Consequence if wrong |
| --- | --- | --- | --- |
| The existing examples are few enough to reorganize without breaking public example names. | High | The crate currently declares five examples in `Cargo.toml`. | Keep the flat layout and add README grouping only. |
| Docking multi-viewport OS windows are larger than render-target fan-out and may need their own slice. | High | `ImguiBackendConfig::multi_viewport` remains disabled while render extraction already supports multiple camera targets. | Split a dedicated viewport-platform workstream before implementation. |
| A useful editor product slice can land inside the backend examples before a separate editor crate exists. | Medium | `editor_shell` already demonstrates dock layout and scene texture interop. | Split editor product work if it starts owning reusable app state or persistence. |
| Platform/CI stabilization can mostly document and gate the existing Bevy matrix. | Medium | The closed follow-up lane already proved wasm check gates and Bevy-specific package gates. | Add CI workflow changes or split if repository policy changes are required. |

## Architecture Direction

Keep `dear-imgui-bevy` as a backend crate with examples that prove integration patterns. Preserve
example command names even if the source files move into categorized directories. Prefer small,
vertical improvements that are easy to compile-check over broad product rewrites. Treat docking
multi-viewport as a platform contract, not as a cosmetic example option, because it implies Dear
ImGui platform-window callbacks and Bevy window lifecycle ownership.

## Closeout Condition

This lane can close when:

- the selected 1/2/3/5 follow-up targets are completed or intentionally split;
- `cargo +stable fmt --all --check` and the Bevy backend example/package gates are fresh;
- README and workstream docs reflect the shipped behavior;
- remaining work has an explicit follow-on path instead of staying implicit.

## Split Outcome

BBP-060 split real Dear ImGui docking multi-viewport OS-window implementation into
`docs/workstreams/bevy-docking-multi-viewport-v1/`. This lane remains the record for example
organization, the safe multi-viewport boundary slice, editor product surface, and platform/CI
stabilization. Continue implementation in the split lane.
