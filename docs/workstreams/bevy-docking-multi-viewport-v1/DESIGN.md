# Bevy Docking Multi-Viewport Workstream

Status: Closed
Last updated: 2026-05-23

## Why This Lane Exists

The Bevy backend product follow-up lane proved that `ImguiBackendConfig::multi_viewport` is only a
recorded request today. The next target is real Dear ImGui docking multi-viewport support: detached
Dear ImGui windows should become Bevy-owned OS windows, receive input and focus feedback, and render
through Bevy's render graph instead of being confused with the existing multi-window camera target
routing.

The user explicitly accepts fearless `dear-imgui-rs` refactoring when the core API is the wrong
shape for this engine integration. This lane therefore treats core crate changes as in scope, but
only when they are proven by the Bevy multi-viewport slice rather than speculative cleanup.

## Relevant Authority

- ADRs:
  - `docs/adr/0001-bevy-native-imgui-backend.md`
- Related workstreams:
  - `docs/workstreams/bevy-native-backend/`
  - `docs/workstreams/bevy-runtime-productization/`
  - `docs/workstreams/bevy-backend-followups-v1/`
  - `docs/workstreams/bevy-backend-product-followups-v1/`
- Core `dear-imgui-rs` surfaces:
  - `dear-imgui/src/platform_io.rs`
  - `dear-imgui/src/platform_io/`
  - `dear-imgui/src/context/platform.rs`
  - `dear-imgui/src/context/frame.rs`
  - `dear-imgui/src/render/snapshot.rs`
- Bevy backend surfaces:
  - `backends/dear-imgui-bevy/src/lib.rs`
  - `backends/dear-imgui-bevy/src/context.rs`
  - `backends/dear-imgui-bevy/src/input.rs`
  - `backends/dear-imgui-bevy/src/render.rs`
  - `backends/dear-imgui-bevy/tests/`

## Problem

Dear ImGui's multi-viewport model expects `PlatformIO` callbacks that can create, destroy, move,
resize, focus, query, and render platform windows. Bevy owns OS windows through ECS entities and the
runner, and the current backend only maps the primary Bevy window into one Dear ImGui context.

Calling into Bevy world directly from C ABI `PlatformIO` callbacks would be the wrong ownership
model. The backend needs an explicit bridge: callbacks must capture viewport intent, Bevy systems
must apply that intent to ECS windows, and rendering/input must be routed back to the matching Dear
ImGui viewport.

The existing `dear-imgui-rs` platform APIs may also be too callback-centric for Bevy. If so, the
core crate should grow engine-friendly primitives such as viewport lifecycle command capture,
per-viewport draw snapshots, or safer callback state wiring.

## Target State

When this workstream closes:

- `ImguiBackendConfig::multi_viewport = true` enables real OS-level Dear ImGui platform windows on
  supported native Bevy targets;
- unsupported targets fail closed with visible status instead of silently pretending support;
- Dear ImGui `PlatformIO` callbacks never mutate Bevy world directly and never unwind across FFI;
- secondary Dear ImGui viewports are mapped to Bevy `Window` entities with deterministic cleanup;
- input, focus, DPI, cursor, IME, title, position, and size feedback are synchronized for platform
  windows, not only the primary window;
- render extraction can route Dear ImGui draw data for secondary viewports to the correct Bevy
  window targets;
- README, examples, and gates demonstrate the shipped support and document any remaining platform
  limits.

## In Scope

- refactoring `dear-imgui-rs` platform/window APIs when needed for engine-managed backends;
- adding core tests for `PlatformIO` callback storage, command capture, and viewport snapshots;
- adding a Bevy-side viewport command queue and viewport-to-window entity map;
- creating, updating, and destroying non-primary Bevy `Window` entities for Dear ImGui viewports;
- generalizing input/focus/IME/cursor mapping beyond `PrimaryWindow`;
- extending render extraction/preparation so secondary viewport draw data targets Bevy windows;
- adding a Bevy example or editor option that exercises detached docked windows;
- keeping wasm and unsupported targets explicit.

## Out Of Scope

- replacing the Bevy-native backend with `dear-imgui-winit` plus `dear-imgui-wgpu`;
- implementing every platform quirk in the first slice, such as perfect passthrough hover on every
  window manager;
- mobile multi-window support without a target-specific Bevy gate;
- making the entire workspace adopt the Bevy crate's Rust target as an incidental side effect;
- extracting a separate editor application crate just to prove platform windows.

## Starting Assumptions

| Assumption | Confidence | Evidence | Consequence if wrong |
| --- | --- | --- | --- |
| Bevy can own multiple native OS windows through extra `Window` entities. | High | Bevy `0.19.0-rc.2` exposes `Window`, `WindowCreated`, `WindowClosed`, and `RenderTarget::Window(WindowRef::Entity(_))`. | The lane becomes blocked on a Bevy runner limitation and must document an unsupported target. |
| Dear ImGui's Rust layer already exposes most raw `PlatformIO` callbacks behind `multi-viewport`. | High | `dear-imgui/src/platform_io/` has typed platform and renderer callback setters plus viewport accessors. | First task must add missing core callback APIs before Bevy code starts. |
| The current Bevy backend render snapshot is primary-viewport oriented. | Medium | `context.rs` snapshots `Context::render()` once and `render.rs` associates that snapshot with camera targets. | Add a core or backend snapshot abstraction for per-viewport draw data before enabling support. |
| Direct Bevy world mutation from C ABI callbacks is unacceptable. | High | Bevy window lifecycle is ECS-owned; `PlatformIO` callbacks can run inside Dear ImGui update/render calls. | Use a queue or command sink even if it requires core API changes. |
| Some core refactor is likely cheaper than backend-only contortions. | Medium | Current core API is callback setter oriented, not engine command oriented. | Keep core changes narrow and require Bevy evidence before accepting them. |

## Architecture Direction

Use a queued bridge. Dear ImGui `PlatformIO` callbacks should be small C ABI trampolines that record
viewport commands into backend-owned state associated with the active ImGui context. Bevy systems
then drain those commands during the normal schedule and mutate ECS world state by spawning,
updating, or despawning `Window` entities.

Use Dear ImGui viewport IDs as the stable external key and Bevy `Entity` as the internal window key.
The bridge owns the map between them and records whether each viewport has a created Bevy window,
pending creation, pending destruction, current logical position/size, scale factor, focus state,
and render target.

Enable `ConfigFlags::VIEWPORTS_ENABLE` and viewport backend flags only after the platform callback
set, lifecycle map, and render route can satisfy the minimum Dear ImGui contract. Until then,
`multi_viewport_requested` may be true but `multi_viewport_supported` must remain false.

Refactor `dear-imgui-rs` core when it makes the engine contract simpler and safer. Acceptable core
changes include a typed viewport command abstraction, context-owned callback state helpers,
per-viewport draw snapshot support, and lifecycle methods that make `update_platform_windows()` and
platform rendering usable without leaking raw pointers into engine code.

## Closeout Condition

This lane can close when:

- a native Bevy example can detach Dear ImGui windows into OS windows with `multi_viewport = true`;
- tests prove unsupported targets and incomplete feature sets fail closed;
- callback-to-ECS bridging, window lifecycle, input feedback, and render routing have focused tests;
- `cargo +stable fmt --all --check`, core multi-viewport tests, Bevy package tests, example checks,
  and wasm unsupported/compile gates are recorded;
- docs clearly distinguish shipped OS-level multi-viewport support from render-target routing.

## Closeout Summary

Closed on 2026-05-23. Native Bevy `render,multi-viewport` builds now support requested Dear ImGui
docking multi-viewport OS windows through queued PlatformIO lifecycle callbacks, Bevy-owned
secondary `Window` entities, all-window input/platform feedback, and per-viewport render routing.
Unsupported target and feature combinations fail closed through `ImguiBackendStatus` and the
existing wasm compile-time `multi-viewport` gate. The product-facing `editor_shell` example and
README document both the normal `render` path and native `render,multi-viewport` path.
