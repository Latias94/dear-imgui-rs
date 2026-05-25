# Bevy Docking Multi-Viewport Workstream - TODO

Status: Closed
Last updated: 2026-05-23

## M0 - Scope And Evidence Freeze

- [x] DMV-010 [owner=planner] [deps=none] [scope=docs/workstreams/bevy-docking-multi-viewport-v1]
  Goal: Open the dedicated implementation lane for real Dear ImGui docking multi-viewport OS
  windows and make core `dear-imgui-rs` refactoring explicitly allowed when it is proven by Bevy
  evidence.
  Validation: `DESIGN.md`, `TODO.md`, `MILESTONES.md`, `EVIDENCE_AND_GATES.md`, `WORKSTREAM.json`,
  and `HANDOFF.md` exist and agree.
  Evidence: `docs/workstreams/bevy-docking-multi-viewport-v1/DESIGN.md`
  Handoff: DONE 2026-05-23. The product follow-up lane remains the history of the boundary slice;
  this lane owns the actual implementation.

## M1 - Core Platform Contract

- [x] DMV-020 [owner=codex] [deps=DMV-010] [scope=dear-imgui/src/platform_io,dear-imgui/src/context,dear-imgui/src/render]
  Goal: Refactor or extend `dear-imgui-rs` so an engine backend can safely capture PlatformIO
  viewport lifecycle intent without mutating engine state inside C ABI callbacks.
  Validation: `cargo +stable test -p dear-imgui-rs --features multi-viewport platform_io` plus any
  focused context/render tests added by the task.
  Review: Confirm the API is engine-oriented and not Bevy-specific, callbacks cannot unwind across
  FFI, and raw viewport pointers do not escape their valid lifetime.
  Evidence: core tests and updated platform/context API docs.
  Handoff: DONE 2026-05-23. No new core command ABI was needed: Bevy can use the existing typed
  PlatformIO callback setters plus `Io::BackendPlatformUserData` and copied `Viewport` snapshots.
  Added core evidence that callbacks can queue engine-owned viewport intent without storing raw
  viewport pointers. Also serialized context-owning core tests so the documented gate is stable
  without `--test-threads=1`.

## M2 - Bevy Window Lifecycle Bridge

- [x] DMV-030 [owner=codex] [deps=DMV-020] [scope=backends/dear-imgui-bevy/src,backends/dear-imgui-bevy/tests]
  Goal: Add a Bevy-side viewport command queue and viewport-to-window entity map that can spawn,
  update, and despawn secondary `Window` entities from captured Dear ImGui platform commands.
  Validation: `cargo +stable nextest run -p dear-imgui-bevy viewport --features render` or the
  narrowest available lifecycle test filter added by this task.
  Review: Confirm callbacks only enqueue intent, Bevy systems own ECS mutation, and cleanup is
  deterministic when viewports or windows disappear.
  Evidence: `backends/dear-imgui-bevy/src/` lifecycle bridge code and focused tests.
  Handoff: DONE 2026-05-23. Added `viewport` bridge module, `multi-viewport` feature forwarding,
  `BackendPlatformUserData` attachment, PlatformIO lifecycle callbacks that only enqueue commands,
  and an `ImguiEndFrame` ECS system that spawns/updates/despawns secondary `Window` entities.
  `ConfigFlags::VIEWPORTS_ENABLE` remains disabled and `multi_viewport_supported` remains false
  until status, input feedback, and secondary render routing are proven.

## M3 - PlatformIO Enablement And Status

- [x] DMV-040 [owner=codex] [deps=DMV-030] [scope=backends/dear-imgui-bevy/src,backends/dear-imgui-bevy/tests,backends/dear-imgui-bevy/Cargo.toml]
  Goal: Wire the required Dear ImGui PlatformIO handlers into the Bevy backend and make
  `multi_viewport_supported` true only for supported native configurations.
  Validation: `cargo +stable nextest run -p dear-imgui-bevy multi_viewport --features render` and
  `cargo +stable check -p dear-imgui-bevy --target wasm32-unknown-unknown --features render`.
  Review: Confirm unsupported targets fail closed, feature gating is explicit, and backend flags are
  not advertised before their contract is met.
  Evidence: status tests, feature gates, and README support matrix updates.
  Handoff: DONE 2026-05-23. Added explicit `ImguiBackendStatus` fields for requested,
  feature-enabled, native-target, lifecycle-bridge, input-feedback, render-routing, and full-support
  state. The native `multi-viewport` feature now installs the lifecycle bridge only when
  `ImguiBackendConfig::multi_viewport` is requested; default configs leave
  `BackendPlatformUserData` unset. wasm remains fail-closed: normal `render` builds compile, while
  `render,multi-viewport` hits the existing core compile-time unsupported-target error.
  `ConfigFlags::VIEWPORTS_ENABLE` and `multi_viewport_supported` remain false until DMV-050 and
  DMV-060 prove input feedback and secondary render routing.

## M4 - Multi-Window Input And Platform Feedback

- [x] DMV-050 [owner=codex] [deps=DMV-040] [scope=backends/dear-imgui-bevy/src/input.rs,backends/dear-imgui-bevy/src/context.rs,backends/dear-imgui-bevy/src/viewport.rs,backends/dear-imgui-bevy/tests]
  Goal: Generalize input, focus, cursor, DPI, IME, position, and size synchronization from
  `PrimaryWindow` to all mapped Dear ImGui platform windows.
  Validation: focused Bevy input/window tests plus `cargo +stable nextest run -p dear-imgui-bevy --features render`.
  Review: Confirm primary-window behavior does not regress and secondary windows feed
  `add_mouse_viewport_event` or an explicitly documented fallback.
  Evidence: input lifecycle tests and backend docs.
  Handoff: DONE 2026-05-23. Secondary viewport windows now feed mouse, wheel, keyboard, touch, IME,
  focus, cursor, and hovered-viewport events into Dear ImGui IO. Platform feedback now applies
  cursor/IME output to the target viewport window and caches Bevy window position, size, focus, and
  DPI state behind PlatformIO getter callbacks. `viewport_input_feedback_enabled` is true for
  native requested `multi-viewport` builds, while `ConfigFlags::VIEWPORTS_ENABLE` and
  `multi_viewport_supported` remain false until DMV-060 proves secondary render routing. Bevy
  currently exposes no current minimized-window state, so PlatformIO minimized feedback returns
  false and is documented as a limitation.

## M5 - Secondary Viewport Rendering

- [x] DMV-060 [owner=codex] [deps=DMV-040,DMV-050] [scope=dear-imgui/src/render,backends/dear-imgui-bevy/src/render.rs,backends/dear-imgui-bevy/tests]
  Goal: Capture and render Dear ImGui draw data for secondary platform viewports to their matching
  Bevy window render targets.
  Validation: `cargo +stable nextest run -p dear-imgui-bevy render --features render` plus
  `cargo +stable check -p dear-imgui-bevy --examples --features render`.
  Review: Confirm render extraction preserves viewport/display coordinates, texture feedback still
  works, and secondary windows do not duplicate the primary overlay.
  Evidence: render preparation tests and an example compile gate.
  Handoff: DONE 2026-05-23. `dear-imgui-rs` now snapshots all platform viewport draw data via
  `FrameSnapshot::from_platform_io` and `FrameSnapshot::viewport_draw`, and the Bevy backend routes
  secondary viewport draw data to matching window render targets without duplicating the primary
  overlay. The shutdown path now calls `DestroyPlatformWindows()` before clearing backend user-data
  and PlatformIO handlers, and the viewport bridge keeps a stable boxed backend pointer. Validated
  with `cargo +stable test -p dear-imgui-rs --features multi-viewport platform_io_snapshot_captures_draw_data_per_viewport`,
  `cargo +stable nextest run -p dear-imgui-bevy render --features render,multi-viewport`,
  `cargo +stable nextest run -p dear-imgui-bevy viewport_platform --features render,multi-viewport`,
  `cargo +stable nextest run -p dear-imgui-bevy --features render,multi-viewport`, and
  `cargo +stable check -p dear-imgui-bevy --examples --features render`.

## M6 - Example, Docs, And Closeout

- [x] DMV-070 [owner=codex] [deps=DMV-060] [scope=backends/dear-imgui-bevy/examples,backends/dear-imgui-bevy/README.md,docs/workstreams/bevy-docking-multi-viewport-v1]
  Goal: Add or update a Bevy example and README section that demonstrates `multi_viewport = true`
  and documents native/wasm support boundaries.
  Validation: `cargo +stable check -p dear-imgui-bevy --features render --example editor_shell` and
  `cargo +stable check -p dear-imgui-bevy --examples --features render`.
  Review: Confirm the example is usable as a product-facing proof rather than a hidden test harness.
  Evidence: example source, README commands, and verification log.
  Handoff: DONE 2026-05-23. The `editor_shell` product-facing example now requests
  `multi_viewport = true` when compiled with the `multi-viewport` feature, keeps the normal
  `render` gate available, and shows requested/supported/render-routing status in its Diagnostics
  panel. README now documents both the normal editor command and the native
  `render,multi-viewport` command, with wasm directed to the plain `render` path.

- [x] DMV-080 [owner=planner] [deps=DMV-070] [scope=docs/workstreams/bevy-docking-multi-viewport-v1]
  Goal: Close the lane or split any remaining platform-specific follow-on.
  Validation: `verify-rust-workstream` records fresh final gate evidence.
  Review: `review-workstream` has no blocking findings.
  Evidence: `EVIDENCE_AND_GATES.md`, `WORKSTREAM.json`, and `HANDOFF.md`.
  Handoff: DONE 2026-05-23. Lane closed. Native `render,multi-viewport` requested configs now ship
  the OS-window PlatformIO bridge, secondary window input/platform feedback, per-viewport render
  routing, product-facing example command, and README support matrix. Remaining limitations are
  platform-specific follow-ons: Bevy `0.19.0-rc.2` has no persistent minimized-window field,
  `wasm32-unknown-unknown` still rejects the `multi-viewport` feature at compile time, and mobile
  multi-window support needs a separate target gate. Later prelaunch work maps Bevy
  `WindowOccluded` events into minimized feedback for secondary viewport windows.
