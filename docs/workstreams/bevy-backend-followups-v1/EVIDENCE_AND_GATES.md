# Bevy Backend Follow-Ups Workstream - Evidence And Gates

Status: Closed
Last updated: 2026-05-23

## Smallest Current Repro

```bash
cargo +stable nextest run -p dear-imgui-bevy --features render
```

## Gate Set

### Multi-Window And Input Gate

```bash
cargo +stable nextest run -p dear-imgui-bevy input
```

Proves the input follow-up slices still keep their primary-window policy honest and gives a home for
future multi-window and cursor/IME tests.

### Runtime Package Gate

```bash
cargo +stable nextest run -p dear-imgui-bevy --features render
```

Proves the renderer, texture interop, and runtime smoke path still compile and pass their backend
tests after follow-up changes land.

### Runtime GPU Harness Gate

```bash
DEAR_IMGUI_BEVY_GPU_HARNESS=1 cargo +stable test -p dear-imgui-bevy --features render --lib bevy_image_texture_bind_groups -- --ignored --nocapture
```

Proves the backend can drive a real native `RenderDevice` and `RenderAssets<GpuImage>` path when the
opt-in GPU harness is enabled, so stale or non-sampled Bevy images do not silently bypass the
texture bind-group path.

### Windowed Runtime Example Gate

```bash
cargo +stable check -p dear-imgui-bevy --features render --example windowed_overlay
```

Proves the persistent windowed runtime example keeps building against the current Bevy target train.

### Editor Helper Gate

```bash
cargo +stable check -p dear-imgui-bevy --features render --example editor_shell
```

Proves the editor-facing example still compiles while helper APIs are refined or split out.

### Example Surface Gate

```bash
cargo +stable check -p dear-imgui-bevy --examples --features render
```

Proves the shared helper surface keeps the full example set compiling after the example setup
boilerplate is centralized.

### Platform Gate

```bash
cargo +stable check -p dear-imgui-bevy --target wasm32-unknown-unknown --no-default-features
cargo +stable check -p dear-imgui-bevy --target wasm32-unknown-unknown --features render
```

Use the platform gate chosen by the wasm/mobile slice to prove the backend still has a viable
non-native path.

### Review Gate

Run `review-workstream` before accepting task or lane completion. Record blocking findings, missing
gates, and residual risks here or link to the review note.

## Evidence Anchors

- `docs/workstreams/bevy-backend-followups-v1/DESIGN.md`
- `docs/workstreams/bevy-backend-followups-v1/TODO.md`
- `docs/workstreams/bevy-backend-followups-v1/MILESTONES.md`
- `docs/workstreams/bevy-backend-followups-v1/WORKSTREAM.json`
- `backends/dear-imgui-bevy/src/lib.rs`
- `backends/dear-imgui-bevy/README.md`
- `backends/dear-imgui-bevy/src/helpers.rs`
- `backends/dear-imgui-bevy/src/input.rs`
- `backends/dear-imgui-bevy/src/render.rs`
- `backends/dear-imgui-bevy/tests/texture.rs`
- `backends/dear-imgui-bevy/tests/helpers.rs`
- `backends/dear-imgui-bevy/examples/runtime/windowed_overlay.rs`
- `backends/dear-imgui-bevy/examples/editor/editor_shell.rs`
- `backends/dear-imgui-bevy/examples/basic/simple.rs`
- `backends/dear-imgui-bevy/examples/ecosystem/ecosystem.rs`
- `backends/dear-imgui-bevy/examples/ecosystem/bevy_plot_controls.rs`

## Notes

Fresh verification is required before marking a task, Codex goal, or lane complete.
Keep the lane split if any follow-up grows into its own product boundary.

## Verification Log

- 2026-05-23: BBF-020 verified for the multi-window routing slice.
  - `cargo +stable nextest run -p dear-imgui-bevy input` — PASS, 9 tests. Proves the primary-window
    input policy and the existing non-primary filtering behavior remain intact.
  - `cargo +stable nextest run -p dear-imgui-bevy --features render render_extract` — PASS, 2 tests.
    Proves render extraction keeps both primary and secondary window camera targets and routes the
    prepared overlay draws to both cameras.
  - `cargo +stable nextest run -p dear-imgui-bevy --features render` — PASS, 24 tests. Proves the
    package-level render gate stayed green after the multi-window test landed.
  - `cargo +stable fmt --all --check` — PASS after the doc/test updates.
- 2026-05-23: BBF-030 verified for the cursor / IME / focus feedback slice.
  - `cargo +stable fmt --all --check` — PASS after the context/input/test/doc updates.
  - `cargo +stable nextest run -p dear-imgui-bevy input` — PASS, 12 tests run and 4 skipped by the
    filter. Proves primary-window input policy remains intact and the new cursor/IME platform
    feedback tests pass.
  - `cargo +stable nextest run -p dear-imgui-bevy --features render` — PASS, 27 tests run and 2
    skipped ignored GPU harness tests. Proves the render feature package gate stayed green after
    the lifecycle-level feedback sync landed.
  - `cargo +stable check -p dear-imgui-bevy --features render --example windowed_overlay` — PASS.
    Proves the windowed runtime example still builds against the updated frame lifecycle.
  - `cargo +stable check -p dear-imgui-bevy --features render --example editor_shell` — PASS. Proves
    the editor-facing example still builds against the updated frame lifecycle.
- 2026-05-23: BBF-040 verified for the platform follow-up slice.
  - `cargo +stable check -p dear-imgui-bevy --target wasm32-unknown-unknown --no-default-features`
    — PASS. Proves the backend core compiles on wasm without the render feature.
  - `cargo +stable check -p dear-imgui-bevy --target wasm32-unknown-unknown --features render`
    — PASS. Proves the render feature set also compiles on wasm, so the current support matrix can
    be documented as a single wasm gate rather than a split native-only warning.
  - `backends/dear-imgui-bevy/src/lib.rs` and `backends/dear-imgui-bevy/README.md` now document the
    wasm gate and the current mobile follow-on boundary.
- 2026-05-23: BBF-050 verified for the runtime smoke slice.
  - `cargo +stable nextest run -p dear-imgui-bevy --features render` — PASS, 27 tests run and 2
    skipped. Proves the package-level render gate stayed green after the runtime harness proof.
  - `DEAR_IMGUI_BEVY_GPU_HARNESS=1 cargo +stable test -p dear-imgui-bevy --features render --lib
    bevy_image_texture_bind_groups -- --ignored --nocapture` — PASS, 2 tests. Proves a real native
    Bevy `RenderDevice` and `RenderAssets<GpuImage>` path can prepare and reject texture bind groups
    under the opt-in GPU harness.
  - `cargo +stable check -p dear-imgui-bevy --features render --example windowed_overlay` and
    `cargo +stable check -p dear-imgui-bevy --features render --example editor_shell` were not
    rerun here because BBF-050 did not modify example sources; the task-specific runtime harness
    evidence came from the package gate plus the GPU harness.
- 2026-05-23: BBF-060 verified for the editor/helper surface slice.
  - `cargo +stable nextest run -p dear-imgui-bevy configure_example_context` — PASS, 1 test.
    Proves the shared example context helper applies the expected example defaults and toggles
    docking deterministically.
  - `cargo +stable check -p dear-imgui-bevy --examples --features render` — PASS. Proves the shared
    helper keeps all backend examples compiling after the repeated setup boilerplate moved into one
    public API.
  - `cargo +stable nextest run -p dear-imgui-bevy --features render` — PASS, 28 tests run and 2
    skipped. Proves the runtime package gate stayed green after the helper surface refactor.
- 2026-05-23: editor_shell dock layout seeded as a split editor surface instead of a single tab
  stack.
  - `cargo +stable fmt --all --check` — PASS after the editor shell layout and doc updates.
  - `cargo +stable check -p dear-imgui-bevy --features render --example editor_shell` — PASS.
    Proves the editor example still builds after the dock layout is seeded programmatically.
  - `cargo +stable nextest run -p dear-imgui-bevy --features render` — PASS, 28 tests run and 2
    skipped, rerun after the final dock position adjustment. Proves the package-level render gate
    stayed green after the editor shell layout change.
- 2026-05-23: BBF-070 closeout verified.
  - `cargo +stable fmt --all --check` — PASS.
  - `cargo +stable check -p dear-imgui-bevy --features render --example editor_shell` — PASS.
  - `cargo +stable nextest run -p dear-imgui-bevy --features render` — PASS, 28 tests run and 2
    skipped.
  - Review: no blocking findings in the current lane review.
  - Result: the five follow-up targets are complete and the lane is closed. Future follow-up work
    should split into a new workstream.
- 2026-05-23: Pre-commit resume verification.
  - `cargo +stable fmt --all --check` — PASS.
  - `cargo +stable check -p dear-imgui-bevy --examples --features render` — PASS. Proves the
    centralized example context helper still keeps the full Bevy backend example surface compiling,
    including the docked editor shell.
  - `cargo +stable nextest run -p dear-imgui-bevy --features render` — PASS, 28 tests run and 2
    skipped. Proves the render-feature package gate remains green before staging this closed lane.
