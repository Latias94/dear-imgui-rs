# Bevy Docking Multi-Viewport Workstream - Milestones

Status: Closed
Last updated: 2026-05-23

## M0 - Scope And Evidence Freeze

Exit criteria:

- real OS-level docking multi-viewport support is split from the product follow-up boundary slice;
- core `dear-imgui-rs` refactoring is explicitly allowed when Bevy evidence requires it;
- first implementation slice is narrow enough to validate without a full windowing demo.

Primary evidence:

- `docs/workstreams/bevy-docking-multi-viewport-v1/DESIGN.md`
- `docs/workstreams/bevy-docking-multi-viewport-v1/TODO.md`

## M1 - Core Platform Contract

Exit criteria:

- C ABI PlatformIO callbacks have a safe engine-facing command capture story;
- any new core APIs are documented as engine abstractions, not Bevy-only hooks;
- core tests prove callback storage, lifetime, and panic boundaries.

Primary gates:

- `cargo +stable test -p dear-imgui-rs --features multi-viewport platform_io`

## M2 - Bevy Window Lifecycle Bridge

Exit criteria:

- Bevy systems can turn captured viewport commands into secondary `Window` entity lifecycle changes;
- viewport IDs map deterministically to window entities;
- destroy and stale-window cleanup behavior is covered by focused tests.

Primary gates:

- focused Bevy viewport lifecycle tests;
- `cargo +stable nextest run -p dear-imgui-bevy --features render`

## M3 - PlatformIO Enablement And Status

Exit criteria:

- the backend registers the minimum PlatformIO callback set for native Bevy targets;
- `ImguiBackendStatus::multi_viewport_supported` becomes true only when the contract is actually
  wired;
- wasm and other unsupported targets remain explicit.

Primary gates:

- `cargo +stable nextest run -p dear-imgui-bevy multi_viewport --features render`
- `cargo +stable check -p dear-imgui-bevy --target wasm32-unknown-unknown --features render`

## M4 - Multi-Window Input And Platform Feedback

Exit criteria:

- primary and secondary windows feed Dear ImGui input/focus events without regressing existing
  primary-window behavior;
- cursor, IME, DPI, position, size, focus, and title feedback are synchronized where Bevy exposes
  the required primitives;
- any missing hovered-viewport limitation is recorded with a concrete follow-on.

Primary gates:

- focused input/window tests;
- `cargo +stable nextest run -p dear-imgui-bevy --features render`

## M5 - Secondary Viewport Rendering

Exit criteria:

- draw data for each platform viewport is captured with the correct display position and size;
- render extraction routes secondary viewport draw commands to the matching Bevy window target;
- texture feedback and primary overlay rendering still work.

Primary gates:

- focused render preparation tests;
- `cargo +stable check -p dear-imgui-bevy --examples --features render`

## M6 - Example, Docs, And Closeout

Exit criteria:

- a native Bevy example demonstrates detached Dear ImGui windows;
- README documents commands, target support, and known limitations;
- final verification and review evidence are recorded;
- remaining platform quirks are split or explicitly deferred.

Status: Complete. DMV-070 made the native multi-viewport path visible in `editor_shell` and README.
DMV-080 recorded final gates and closed the lane with known limitations limited to platform-specific
follow-ons such as minimized-window feedback and wasm/mobile multi-window support.
