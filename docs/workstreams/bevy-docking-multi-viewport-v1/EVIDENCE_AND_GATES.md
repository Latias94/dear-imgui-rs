# Bevy Docking Multi-Viewport Workstream - Evidence And Gates

Status: Closed
Last updated: 2026-05-24

## Smallest Current Repro

```bash
cargo +stable test -p dear-imgui-rs --features multi-viewport platform_io
```

This starts at the core boundary because the first risk is not whether Bevy can draw a window; it is
whether `dear-imgui-rs` exposes a safe enough engine-facing PlatformIO contract.

## Gate Set

### Core Platform Contract Gate

```bash
cargo +stable test -p dear-imgui-rs --features multi-viewport platform_io
```

Proves core PlatformIO callback storage, trampolines, and any new command/snapshot API work before
Bevy relies on them.

### Bevy Lifecycle Gate

```bash
cargo +stable nextest run -p dear-imgui-bevy multi_viewport --features render
cargo +stable nextest run -p dear-imgui-bevy viewport --features render
```

Use the narrowest available filter after tests are added. These prove callback-captured viewport
commands are applied by Bevy systems and that lifecycle cleanup is deterministic.

### Bevy Package Gate

```bash
cargo +stable nextest run -p dear-imgui-bevy --features render
```

Proves multi-viewport changes do not regress the existing Bevy backend behavior.

### Example Gate

```bash
cargo +stable check -p dear-imgui-bevy --features render --example editor_shell
cargo +stable check -p dear-imgui-bevy --examples --features render
```

Proves the product-facing example surface still compiles and eventually demonstrates native
multi-viewport support.

### Platform Gate

```bash
cargo +stable check -p dear-imgui-bevy --target wasm32-unknown-unknown --no-default-features
cargo +stable check -p dear-imgui-bevy --target wasm32-unknown-unknown --features render
```

Proves wasm support remains explicit. Full multi-viewport support is native-only unless a later task
adds a wasm-specific platform contract.

### Formatting And Static Checks

```bash
cargo +stable fmt --all --check
git diff --check
```

Proves formatting and patch hygiene before closeout.

### Review Gate

Run `review-workstream` before accepting task or lane completion. Record blocking findings, missing
gates, and residual risks here or link to the review note.

## Evidence Anchors

- `docs/workstreams/bevy-docking-multi-viewport-v1/DESIGN.md`
- `docs/workstreams/bevy-docking-multi-viewport-v1/TODO.md`
- `docs/workstreams/bevy-docking-multi-viewport-v1/MILESTONES.md`
- `docs/workstreams/bevy-docking-multi-viewport-v1/WORKSTREAM.json`
- `docs/workstreams/bevy-backend-product-followups-v1/JOURNAL/2026-05-23-bbp-030.md`
- `docs/adr/0001-bevy-native-imgui-backend.md`
- `dear-imgui/src/platform_io/`
- `dear-imgui/src/context/platform.rs`
- `dear-imgui/src/context/frame.rs`
- `dear-imgui/src/render/snapshot.rs`
- `backends/dear-imgui-bevy/src/lib.rs`
- `backends/dear-imgui-bevy/src/context.rs`
- `backends/dear-imgui-bevy/src/input.rs`
- `backends/dear-imgui-bevy/src/render.rs`
- `backends/dear-imgui-bevy/tests/`

## Notes

Support is not complete merely because `ConfigFlags::VIEWPORTS_ENABLE` is set. The backend must
prove OS-window lifecycle, input/focus feedback, and secondary-window render routing.

Any `dear-imgui-rs` core refactor must be justified by one of this lane's Bevy tasks and covered by
core tests. Avoid generic cleanup inside this lane unless it removes a concrete multi-viewport
blocker.

Fresh verification is required before marking a task, Codex goal, or lane complete.

## Verification Log

- 2026-05-23: DMV-010 scope freeze.
  - Opened a dedicated execution lane for real Bevy docking multi-viewport support after the product
    follow-up lane established only the safe boundary slice.
  - Recorded that core `dear-imgui-rs` refactoring is allowed when required by the Bevy
    multi-viewport contract.
  - No code gates were run for DMV-010 because it creates planning and evidence documents only.

- 2026-05-23: DMV-020 core PlatformIO contract.
  - Added core evidence that typed PlatformIO callbacks can use `Io::BackendPlatformUserData` to
    queue copied viewport lifecycle intent without storing raw viewport pointers.
  - Added crate-local test serialization for context-owning unit tests so the documented
    `platform_io` gate is stable under the default test harness concurrency.
  - Verification: `cargo +stable test -p dear-imgui-rs --features multi-viewport platform_io` -
    PASS, 25 unit tests matched in `src/lib.rs`; remaining integration test binaries had zero
    matched tests.

- 2026-05-23: DMV-030 Bevy viewport lifecycle bridge.
  - Added `backends/dear-imgui-bevy/src/viewport.rs`, `ImguiViewportBridge`,
    `ImguiViewportCommand`, `ImguiViewportSnapshot`, `ImguiViewportWindow`, and the
    `multi-viewport` Cargo feature forwarding `dear-imgui-rs/multi-viewport`.
  - The bridge stores backend state in `BackendPlatformUserData`, installs PlatformIO lifecycle
    callbacks, only enqueues intent in C ABI callbacks, and applies queued commands from Bevy ECS to
    spawn/update/despawn secondary `Window` entities.
  - `ConfigFlags::VIEWPORTS_ENABLE` and `multi_viewport_supported` remain disabled until native
    status, input feedback, and secondary render routing are complete.
  - Verification:
    `cargo +stable nextest run -p dear-imgui-bevy viewport --features render,multi-viewport` -
    PASS, 6 tests run and 29 skipped by filter.

- 2026-05-23: DMV-040 PlatformIO enablement and status gating.
  - Added explicit `ImguiBackendStatus` fields for requested, feature-enabled, native-target,
    lifecycle-bridge, input-feedback, render-routing, and full-support state.
  - The native `multi-viewport` feature installs the lifecycle bridge only when
    `ImguiBackendConfig::multi_viewport` is requested. Default configs leave
    `BackendPlatformUserData` unset.
  - README documents the current support matrix: native without feature fails closed, native with
    feature has lifecycle bridge only, and wasm remains unsupported for the `multi-viewport`
    feature.
  - `ConfigFlags::VIEWPORTS_ENABLE` and `multi_viewport_supported` remain disabled until DMV-050
    and DMV-060 prove all-window input feedback and secondary render routing.
  - Verification:
    `cargo +stable nextest run -p dear-imgui-bevy multi_viewport --features render` - PASS, 2 tests
    run and 31 skipped by filter.
  - Verification:
    `cargo +stable nextest run -p dear-imgui-bevy viewport --features render,multi-viewport` -
    PASS, 8 tests run and 29 skipped by filter.
  - Verification:
    `cargo +stable check -p dear-imgui-bevy --target wasm32-unknown-unknown --features render` -
    PASS.
  - Verification:
    `cargo +stable check -p dear-imgui-bevy --target wasm32-unknown-unknown --features render,multi-viewport`
    - EXPECTED FAIL at `dear-imgui/src/platform_io.rs` compile-time unsupported-target gate.

- 2026-05-23: DMV-050 multi-window input and platform feedback.
  - Generalized input handling from `PrimaryWindow` to primary plus mapped
    `ImguiViewportWindow` secondary windows for mouse movement/leave, buttons, wheel, keyboard,
    touch, IME, and focus.
  - Secondary mouse/touch input now sends `add_mouse_viewport_event` and sets
    `BackendFlags::HAS_MOUSE_HOVERED_VIEWPORT`.
  - Platform cursor and IME feedback now applies to the Dear ImGui viewport named by
    `PlatformImeData.ViewportId`, falling back to the primary window only when no mapped secondary
    viewport is found.
  - `ImguiViewportBridge` now keeps ECS-owned window feedback snapshots for primary and secondary
    viewports and installs PlatformIO getter callbacks for window position, size, framebuffer scale,
    DPI scale, focus, and minimized state. Bevy `0.19.0-rc.2` has no observable minimized-window
    state, so minimized feedback returns `false`.
  - `ImguiBackendStatus::viewport_input_feedback_enabled` is true for native requested
    `multi-viewport` builds. `ConfigFlags::VIEWPORTS_ENABLE` and `multi_viewport_supported` remain
    false until DMV-060 proves secondary viewport render routing.
  - Verification:
    `cargo +stable nextest run -p dear-imgui-bevy input_secondary_viewport --features render,multi-viewport`
    - PASS, 1 test run and 39 skipped by filter.
  - Verification:
    `cargo +stable nextest run -p dear-imgui-bevy input_platform_feedback_updates_secondary --features render,multi-viewport`
    - PASS, 1 test run and 39 skipped by filter.
  - Verification:
    `cargo +stable nextest run -p dear-imgui-bevy multi_viewport --features render,multi-viewport`
    - PASS, 4 tests run and 36 skipped by filter.
  - Verification:
    `cargo +stable nextest run -p dear-imgui-bevy viewport --features render,multi-viewport` -
    PASS, 11 tests run and 29 skipped by filter.
  - Verification:
    `cargo +stable nextest run -p dear-imgui-bevy --features render,multi-viewport` - PASS, 38
    tests run and 2 skipped.

- 2026-05-23: DMV-060 secondary viewport rendering.
  - Added a viewport-aware core render snapshot path. `FrameSnapshot` now carries per-viewport draw
    snapshots, `FrameSnapshot::from_platform_io` copies draw data from Dear ImGui platform
    viewports, and `FrameSnapshot::viewport_draw` exposes targeted lookup for engine backends.
  - Bevy end-frame rendering now snapshots platform viewport draw data before
    `update_platform_windows()`. Render extraction/preparation routes each viewport draw list to
    the matching Bevy window render target and keeps the primary overlay on the primary target.
  - `ImguiBackendStatus::viewport_render_routing_enabled` and
    `ImguiBackendStatus::multi_viewport_supported` now become true for native
    `render,multi-viewport` builds when `ImguiBackendConfig::multi_viewport` is requested.
  - Fixed shutdown ordering by calling Dear ImGui `DestroyPlatformWindows()` before clearing
    `BackendPlatformUserData` and PlatformIO handlers, so viewport backend markers are cleared by
    the installed destroy callback before context shutdown.
  - The viewport command bridge now merges same-drain `Create` plus immediate window update
    commands before Bevy `Commands` flushes the new `Window` component, so initial position/size
    updates are not dropped.
  - Verification:
    `cargo +stable test -p dear-imgui-rs --features multi-viewport platform_io_snapshot_captures_draw_data_per_viewport`
    - PASS, 1 matched core unit test and zero matched integration tests.
  - Verification:
    `cargo +stable nextest run -p dear-imgui-bevy render --features render,multi-viewport` - PASS,
    11 tests run and 30 skipped by filter.
  - Verification:
    `cargo +stable nextest run -p dear-imgui-bevy viewport_platform --features render,multi-viewport`
    - PASS, 2 tests run and 39 skipped by filter.
  - Verification:
    `cargo +stable nextest run -p dear-imgui-bevy --features render,multi-viewport` - PASS, 39
    tests run and 2 skipped.
  - Verification:
    `cargo +stable check -p dear-imgui-bevy --examples --features render` - PASS.
  - Verification:
    `cargo +stable fmt --all --check` - PASS.
  - Verification:
    `git diff --check` - PASS.

- 2026-05-23: DMV-080 closeout verification and review.
  - Fixed a final-gate regression discovered by `cargo +stable nextest run -p dear-imgui-bevy
    --features render`: render extraction now only treats `ImguiViewportWindow` mappings as Dear
    ImGui platform viewport targets when `ImguiBackendStatus::multi_viewport_supported` is true.
    Plain `render` builds continue to duplicate the primary overlay to multiple Bevy camera/window
    targets without pretending those targets are Dear ImGui OS-level viewports.
  - Added a plain-`render` regression test proving viewport-window mappings are ignored until full
    multi-viewport support is enabled, and kept the per-viewport projection routing test behind the
    `multi-viewport` feature.
  - Verification:
    `cargo +stable test -p dear-imgui-rs --features multi-viewport platform_io` - PASS, 26 matched
    core unit tests and zero matched integration tests.
  - Verification:
    `cargo +stable nextest run -p dear-imgui-bevy --features render` - PASS, 34 tests run and 2
    skipped.
  - Verification:
    `cargo +stable nextest run -p dear-imgui-bevy --features render,multi-viewport` - PASS, 39
    tests run and 2 skipped.
  - Verification:
    `cargo +stable check -p dear-imgui-bevy --examples --features render` - PASS.
  - Verification:
    `cargo +stable check -p dear-imgui-bevy --features render,multi-viewport --example editor_shell`
    - PASS.
  - Verification:
    `cargo +stable check -p dear-imgui-bevy --target wasm32-unknown-unknown --no-default-features`
    - PASS.
  - Verification:
    `cargo +stable check -p dear-imgui-bevy --target wasm32-unknown-unknown --features render` -
    PASS.
  - Verification:
    `cargo +stable check -p dear-imgui-bevy --target wasm32-unknown-unknown --features render,multi-viewport`
    - EXPECTED FAIL at `dear-imgui/src/platform_io.rs` with
    `The multi-viewport feature is not supported on wasm32 targets yet.`
  - Verification:
    `cargo +stable fmt --all --check` - PASS.
  - Verification:
    `git diff --check` - PASS.
  - Review: no blocking workstream-compliance or code-quality findings remain. Residual platform
    risks are documented as follow-ons rather than blockers: minimized-window feedback is not
    observable in Bevy `0.19.0-rc.2`, wasm rejects `multi-viewport` by design, and mobile
    multi-window support needs a target-specific lane.

- 2026-05-24: Post-closeout detached Hierarchy regression probes.
  - Added `DEAR_IMGUI_BEVY_VIEWPORT_PROBE_DOCK_BACK=1` to the `editor_shell` probe. The probe now
    forces `Hierarchy` into a secondary platform viewport, then forces it back into the main
    dockspace and exits with an error if secondary viewport windows, overlay cameras, or post-dock
    interaction are stale.
  - Fixed Bevy input mapping to match the winit backend contract: primary-window mouse events now
    report the main Dear ImGui viewport via `Io::add_mouse_viewport_event`, while cursor leave clears
    the hovered viewport to `Id::default()`.
  - Tightened the runtime interaction probe after finding that the earlier
    `hierarchy-click-not-verified` result was a probe timing bug: `is_item_clicked()` fires on the
    press frame, so the probe now records `hierarchy-click-observed` when the target item receives the
    click and verifies selection after release.
  - The final dock-back runtime probe produced `dock-back-requested frame=75`, `secondary=[] cameras=[]`,
    `dock-back-clean frame=120`, `hierarchy-click-observed`, `hierarchy-click-verified frame=123`, and
    exited successfully at frame 150.
  - Verification:
    `DEAR_IMGUI_BEVY_VIEWPORT_PROBE=1 DEAR_IMGUI_BEVY_VIEWPORT_PROBE_DOCK_BACK=1 DEAR_IMGUI_BEVY_VIEWPORT_PROBE_FRAMES=150 CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_DEV_SPLIT_DEBUGINFO=off cargo +stable run -p dear-imgui-bevy --example editor_shell --no-default-features --features render,multi-viewport`
    - PASS.
  - Verification:
    `cargo +stable test -p dear-imgui-bevy --test input --no-default-features --features multi-viewport`
    - PASS, 15 tests passed including
    `primary_window_input_reports_main_hovered_viewport_when_viewports_are_enabled`.
  - Verification:
    `cargo +stable test -p dear-imgui-bevy --test viewport viewport_orphaned_secondary_overlay_camera_is_despawned_after_dock_back --no-default-features --features render,multi-viewport`
    - PASS.
  - Verification:
    `cargo +stable test -p dear-imgui-bevy --test viewport viewport_platform_monitors_use_real_monitor_space_not_primary_window_space --no-default-features --features multi-viewport`
    - PASS.
  - Verification:
    `cargo +stable check -p dear-imgui-bevy --example editor_shell --no-default-features --features render,multi-viewport`
    - PASS.
  - Verification:
    `cargo +stable fmt --check --all` - PASS.
  - Follow-up runtime check:
    `DEAR_IMGUI_BEVY_VIEWPORT_PROBE=1 DEAR_IMGUI_BEVY_VIEWPORT_PROBE_MOVE_PRIMARY=1 DEAR_IMGUI_BEVY_VIEWPORT_PROBE_FRAMES=90 CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_DEV_SPLIT_DEBUGINFO=off cargo +stable run -p dear-imgui-bevy --example editor_shell --no-default-features --features render,multi-viewport`
    - PASS; after moving the primary window, the detached Hierarchy viewport kept
    `window_pos=[1421.0, 96.0]:delta=[0.0, 0.0]`.
  - Follow-up runtime check:
    `DEAR_IMGUI_BEVY_VIEWPORT_PROBE=1 DEAR_IMGUI_BEVY_VIEWPORT_PROBE_CLOSE_PRIMARY=1 DEAR_IMGUI_BEVY_VIEWPORT_PROBE_FRAMES=90 CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_DEV_SPLIT_DEBUGINFO=off cargo +stable run -p dear-imgui-bevy --example editor_shell --no-default-features --features render,multi-viewport`
    - PASS; after primary close, Bevy logged `Closing window 254v0` for the detached Hierarchy window,
    then `No windows are open, exiting`.
  - Session recovery fresh verification:
    Recovered session `019e53ec-f1f8-7c43-8c1d-b6c5963e04b6` into a new active Codex goal, then
    re-verified the current worktree instead of relying on prior transcript output.
  - Verification:
    `git diff --check` - PASS.
  - Verification:
    `cargo +stable fmt --check --all` - PASS.
  - Verification:
    `cargo +stable test -p dear-imgui-bevy --test input --no-default-features --features multi-viewport`
    - PASS, 15 tests passed including
    `primary_window_input_reports_main_hovered_viewport_when_viewports_are_enabled`.
  - Verification:
    `cargo +stable check -p dear-imgui-bevy --example editor_shell --no-default-features --features render,multi-viewport`
    - PASS.
  - Verification:
    `cargo +stable nextest run -p dear-imgui-bevy --test viewport viewport_orphaned_secondary_overlay_camera_is_despawned_after_dock_back --no-default-features --features render,multi-viewport`
    - PASS, 1 test run and 12 skipped.
  - Verification:
    `cargo +stable nextest run -p dear-imgui-bevy --test viewport viewport_platform_monitors_use_real_monitor_space_not_primary_window_space --no-default-features --features multi-viewport`
    - PASS, 1 test run and 9 skipped.
  - Runtime probe:
    `DEAR_IMGUI_BEVY_VIEWPORT_PROBE=1 DEAR_IMGUI_BEVY_VIEWPORT_PROBE_DOCK_BACK=1 DEAR_IMGUI_BEVY_VIEWPORT_PROBE_FRAMES=150 CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_DEV_SPLIT_DEBUGINFO=off cargo +stable run -p dear-imgui-bevy --example editor_shell --no-default-features --features render,multi-viewport`
    - PASS; the probe logged a detached `Hierarchy` secondary window and camera, then
    `dock-back-requested frame=75`, `secondary=[] cameras=[]`, `dock-back-clean frame=120`,
    `hierarchy-click-observed`, and `hierarchy-click-verified frame=123`.
  - Runtime probe:
    `DEAR_IMGUI_BEVY_VIEWPORT_PROBE=1 DEAR_IMGUI_BEVY_VIEWPORT_PROBE_MOVE_PRIMARY=1 DEAR_IMGUI_BEVY_VIEWPORT_PROBE_FRAMES=90 CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_DEV_SPLIT_DEBUGINFO=off cargo +stable run -p dear-imgui-bevy --example editor_shell --no-default-features --features render,multi-viewport`
    - PASS; after moving the primary window to `[24, 96]`, the detached `Hierarchy` viewport kept
    `window_pos=[1421.0, 96.0]:delta=[0.0, 0.0]`.
  - Runtime probe:
    `DEAR_IMGUI_BEVY_VIEWPORT_PROBE=1 DEAR_IMGUI_BEVY_VIEWPORT_PROBE_CLOSE_PRIMARY=1 DEAR_IMGUI_BEVY_VIEWPORT_PROBE_FRAMES=90 CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_DEV_SPLIT_DEBUGINFO=off cargo +stable run -p dear-imgui-bevy --example editor_shell --no-default-features --features render,multi-viewport`
    - PASS; after requesting the primary close, Bevy logged `Closing window 254v0` for the detached
    `Hierarchy` window and then `No windows are open, exiting`.
  - This fresh evidence covers the user-reported detached `Hierarchy` failure modes: secondary
    viewport render isolation, primary-close cleanup, dock-back state and interaction cleanup, and
    primary-window movement coordinate stability.

- 2026-05-23: DMV-070 example, docs, and product-facing proof.
  - Updated `backends/dear-imgui-bevy/examples/editor/editor_shell.rs` so the existing editor shell
    requests `ImguiBackendConfig { multi_viewport: cfg!(feature = "multi-viewport"), .. }`. The
    normal `render` example gate remains valid, while native `render,multi-viewport` builds now use
    the real OS-level viewport path.
  - The editor Diagnostics panel now reports `multi_viewport_requested`,
    `multi_viewport_supported`, and viewport render routing status from `ImguiBackendStatus`, so
    users can see whether the product-facing example is running the native multi-viewport backend.
  - Updated `backends/dear-imgui-bevy/README.md` with both the normal editor shell command and the
    native `render,multi-viewport` command, and documented that wasm should use the plain `render`
    path because the `multi-viewport` feature is native-only today.
  - Verification:
    `cargo +stable check -p dear-imgui-bevy --features render --example editor_shell` - PASS.
  - Verification:
    `cargo +stable check -p dear-imgui-bevy --features render,multi-viewport --example editor_shell`
    - PASS.
  - Verification:
    `cargo +stable check -p dear-imgui-bevy --examples --features render` - PASS.
  - Verification:
    `cargo +stable fmt --all --check` - PASS.
  - Verification:
    `git diff --check` - PASS.
