# Bevy Backend Product Follow-Ups Workstream - Evidence And Gates

Status: Split
Last updated: 2026-05-23

## Smallest Current Repro

```bash
cargo +stable check -p dear-imgui-bevy --examples --features render
```

## Gate Set

### Example Catalog Gate

```bash
cargo +stable fmt --all --check
cargo +stable check -p dear-imgui-bevy --examples --features render
```

Proves the categorized example source layout still matches Cargo's declared example targets and
the full Bevy example surface keeps compiling.

### Runtime Package Gate

```bash
cargo +stable nextest run -p dear-imgui-bevy --features render
```

Proves backend tests remain green after product-facing changes.

### Editor Example Gate

```bash
cargo +stable check -p dear-imgui-bevy --features render --example editor_shell
```

Proves the editor-facing example keeps compiling while product slices are added.

### Platform Gate

```bash
cargo +stable check -p dear-imgui-bevy --target wasm32-unknown-unknown --no-default-features
cargo +stable check -p dear-imgui-bevy --target wasm32-unknown-unknown --features render
```

Proves the current backend support matrix still includes the documented wasm compile path.

### Review Gate

Run `review-workstream` before accepting task or lane completion. Record blocking findings, missing
gates, and residual risks here or link to the review note.

## Evidence Anchors

- `docs/workstreams/bevy-backend-product-followups-v1/DESIGN.md`
- `docs/workstreams/bevy-backend-product-followups-v1/TODO.md`
- `docs/workstreams/bevy-backend-product-followups-v1/MILESTONES.md`
- `docs/workstreams/bevy-backend-product-followups-v1/WORKSTREAM.json`
- `backends/dear-imgui-bevy/Cargo.toml`
- `backends/dear-imgui-bevy/README.md`
- `backends/dear-imgui-bevy/examples/`
- `backends/dear-imgui-bevy/src/`

## Notes

Fresh verification is required before marking a task, Codex goal, or lane complete. Keep docking
multi-viewport work separate from the already-shipped render-target multi-window routing unless the
slice explicitly implements Dear ImGui platform windows.

## Verification Log

- 2026-05-23: BBP-010 scope freeze.
  - New lane created for the user's selected follow-up items 1, 2, 3, and 5 after the closed
    `bevy-backend-followups-v1` lane.
- 2026-05-23: BBP-020 example catalog implemented and verified.
  - Moved the Bevy backend examples into purpose-based folders:
    `examples/basic/simple.rs`, `examples/runtime/windowed_overlay.rs`,
    `examples/ecosystem/ecosystem.rs`, `examples/ecosystem/bevy_plot_controls.rs`, and
    `examples/editor/editor_shell.rs`.
  - Updated `backends/dear-imgui-bevy/Cargo.toml` so public Cargo example names remain unchanged.
  - Updated `backends/dear-imgui-bevy/README.md` with an example index and run commands grouped by
    basic, runtime, ecosystem, and editor use cases.
  - Updated current evidence anchors in the closed Bevy workstream docs so navigation points to the
    categorized source paths; historical journal entries still describe the files as they were first
    added.
  - `cargo +stable fmt --all --check` - PASS.
  - `cargo +stable check -p dear-imgui-bevy --examples --features render` - PASS. Proves Cargo's
    declared example paths match the moved files and the full Bevy backend example surface still
    compiles with the render feature enabled.
  - Broader `cargo +stable nextest run -p dear-imgui-bevy --features render` was not rerun for this
    task because BBP-020 changed example layout, Cargo example paths, and docs only; it did not
    change backend runtime behavior or tests.
- 2026-05-23: BBP-030 first docking multi-viewport boundary slice implemented and verified.
  - Audited the current Dear ImGui safe API and found PlatformIO callback support exists in
    `dear-imgui` behind the `multi-viewport` feature, but `dear-imgui-bevy` does not yet bridge
    those callbacks to Bevy-owned OS window lifecycle systems.
  - Added `ImguiBackendStatus::multi_viewport_requested` and
    `ImguiBackendStatus::multi_viewport_supported` so a `multi_viewport = true` configuration is
    observable instead of being a silent no-op.
  - Documented that the backend deliberately does not set `ConfigFlags::VIEWPORTS_ENABLE` until
    Bevy window creation, destruction, focus, size, DPI, and per-window render routing are wired.
  - Added focused tests proving default config reports no request/support, custom config records the
    request, and lifecycle execution does not falsely advertise Dear ImGui OS-level viewports.
  - `cargo +stable nextest run -p dear-imgui-bevy plugin lifecycle --features render` - PASS, 5
    tests run and 26 skipped by filter.
  - Status: DONE_WITH_CONCERNS. This completes the smallest safe BBP-030 slice, but not the full
    OS-window implementation. The remaining implementation should be split around PlatformIO
    command queuing, Bevy window entity mapping, and secondary-window render routing.
- 2026-05-23: BBP-040 editor product slice implemented and verified.
  - Reworked `editor_shell` so the sample scene seeds an actual Bevy ECS hierarchy rooted at
    `EditorSceneRoot` instead of rendering static hierarchy labels.
  - Added hierarchy selection backed by Bevy `Entity` IDs, plus inspector buffers for selected
    `Name` and `Transform` data.
  - The inspector can rename the selected entity and write translation, rotation, and scale back
    through Bevy `Commands`; animated scene objects also show their motion component data.
  - Added focused example unit tests for selection resync and transform-to-inspector buffer sync.
  - `cargo +stable fmt --all --check` - PASS.
  - `cargo +stable check -p dear-imgui-bevy --features render --example editor_shell` - PASS.
  - `cargo +stable test -p dear-imgui-bevy --features render --example editor_shell` - PASS, 2
    tests run.
  - Broader `cargo +stable nextest run -p dear-imgui-bevy --features render` was not rerun for this
    task because BBP-040 is confined to the editor example surface and its focused example tests plus
    editor example compile gate cover the changed behavior.
- 2026-05-23: BBP-050 platform and CI stabilization implemented and verified.
  - Added a dedicated `bevy-backend` GitHub Actions job in `.github/workflows/ci.yml` that runs the
    Bevy backend format, example, render test, and wasm checks on Ubuntu.
  - Updated `backends/dear-imgui-bevy/README.md` to say the Bevy backend gates belong to a dedicated
    CI lane rather than the root workspace workflow.
  - `cargo +stable fmt --all --check` - PASS.
  - `cargo +stable nextest run -p dear-imgui-bevy --features render` - PASS, 29 tests run and 2
    skipped.
  - `cargo +stable check -p dear-imgui-bevy --examples --features render` - PASS.
  - `cargo +stable check -p dear-imgui-bevy --target wasm32-unknown-unknown --no-default-features`
    - PASS.
  - `cargo +stable check -p dear-imgui-bevy --target wasm32-unknown-unknown --features render` -
    PASS.
  - `git diff --check` - PASS.
  - `ruby -e "require 'yaml'; YAML.load_file('.github/workflows/ci.yml'); puts 'workflow ok'"` -
    PASS.
- 2026-05-23: BBP-060 split real docking multi-viewport implementation.
  - Opened `docs/workstreams/bevy-docking-multi-viewport-v1/` as the dedicated execution lane for
    actual Dear ImGui docking multi-viewport OS-window support.
  - The split lane starts at the core PlatformIO contract and explicitly permits `dear-imgui-rs`
    refactoring when Bevy evidence requires a safer engine-managed API.
  - No code gates were rerun for this split record; the previous BBP-050 gates remain the latest
    product-follow-up implementation evidence.
