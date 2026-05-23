# Bevy Runtime Productization Workstream — Evidence And Gates

Status: Closed
Last updated: 2026-05-23

## Shipped Proof Set

The lane now ships the runtime/editor proof set as dedicated Bevy `cargo +stable` gates:

```bash
cargo +stable check -p dear-imgui-bevy --features render --example windowed_overlay
cargo +stable check -p dear-imgui-bevy --features render --example editor_shell
cargo +stable nextest run -p dear-imgui-bevy --features render
```

`simple.rs` remains the minimal compile proof. `windowed_overlay.rs` is the persistent runtime
smoke app, and `editor_shell.rs` is the richer editor productization example.

## Gate Set

### Persistent Windowed Example Gate

```bash
cargo +stable check -p dear-imgui-bevy --features render --example windowed_overlay
```

Proves the real windowed example compiles against the selected Bevy target train.

### Runtime Renderer Harness Gate

```bash
DEAR_IMGUI_BEVY_GPU_HARNESS=1 cargo +stable test -p dear-imgui-bevy --features render --lib bevy_image_texture_bind_groups -- --ignored --nocapture
```

The harness is intentionally `#[ignore]` by default because it initializes a real native
Bevy/wgpu adapter. The default package gate below must show those tests as skipped rather than
silently passing through an early return.

### Backend Package Gate

```bash
cargo +stable nextest run -p dear-imgui-bevy --features render
```

Proves the backend package tests remain green with runtime productization changes.

### Example Gates

```bash
cargo +stable check -p dear-imgui-bevy --example simple
cargo +stable check -p dear-imgui-bevy --features render --example editor_shell
cargo +stable check -p dear-imgui-bevy --example ecosystem
```

Keep existing public examples compiling while the runtime/productized examples are added.

### Review Gate

Run `review-workstream` before accepting task or lane completion. Record blocking findings, missing
gates, and residual risks here or link to the review note.

## Evidence Anchors

- `docs/workstreams/bevy-runtime-productization/DESIGN.md`
- `docs/workstreams/bevy-runtime-productization/TODO.md`
- `docs/workstreams/bevy-runtime-productization/MILESTONES.md`
- `docs/workstreams/bevy-runtime-productization/WORKSTREAM.json`
- `backends/dear-imgui-bevy/examples/windowed_overlay.rs`
- runtime renderer harness path added by BRP-030
- editor helper/example paths updated by BRP-040

## Fresh Evidence Log

- 2026-05-23: BRP-010 opened the follow-on lane for persistent windowed runtime proof, runtime
  renderer harness coverage, and editor shell productization. Implementation gates not yet run for
  BRP-020+.
- 2026-05-23: BRP-020 persistent windowed runtime proof implemented and verified.
  - Added `backends/dear-imgui-bevy/examples/windowed_overlay.rs`.
  - Added a dev-only top-level `bevy = "=0.19.0-rc.2"` dependency with `2d` and `default_platform`
    features so the example can use `DefaultPlugins` and Bevy's normal windowed runner.
  - Updated `backends/dear-imgui-bevy/README.md` with the manual run command:
    `cargo +stable run -p dear-imgui-bevy --features render --example windowed_overlay`.
  - `cargo +stable check -p dear-imgui-bevy --features render --example windowed_overlay` — PASS.
  - Review: no blocking BRP-020 findings. The example is intentionally a runtime smoke app, not a
    deterministic CI execution gate, because it opens a real OS window and exits on Escape.
  - Status: BRP-020 DONE. Continue with BRP-030 runtime renderer harness.
- 2026-05-23: BRP-030 runtime renderer harness implemented and verified.
  - Added ignored opt-in harness tests in `backends/dear-imgui-bevy/src/render.rs`:
    `bevy_image_texture_bind_groups_use_real_render_assets_when_gpu_harness_is_enabled` and
    `bevy_image_texture_bind_groups_ignore_non_sampled_gpu_images_when_gpu_harness_is_enabled`.
  - The harness initializes Bevy renderer resources with `initialize_renderer`, constructs a real
    `PipelineCache`, creates `GpuImage` entries inside `RenderAssets<GpuImage>`, and calls the
    production Bevy image bind-group preparation path.
  - Positive path proves `TextureUsages::TEXTURE_BINDING` images register a real
    `TextureBinding::Legacy` bind group.
  - Negative/stale paths prove non-sampled GPU images and missing render assets do not leave stale
    bind groups.
  - `cargo +stable fmt --all --check` — PASS.
  - `cargo +stable nextest run -p dear-imgui-bevy --features render` — PASS: 21 passed, 2 skipped
    ignored GPU harness tests.
  - `DEAR_IMGUI_BEVY_GPU_HARNESS=1 cargo +stable test -p dear-imgui-bevy --features render --lib bevy_image_texture_bind_groups -- --ignored --nocapture`
    — PASS: 2 passed.
  - Status: BRP-030 DONE. Continue with BRP-040 editor shell productization.
- 2026-05-23: BRP-040 editor shell productization implemented and verified.
  - Converted `backends/dear-imgui-bevy/examples/editor_shell.rs` from a one-frame
    `ScheduleRunnerPlugin::run_once()` proof to a persistent windowed app using Bevy
    `DefaultPlugins`.
  - Added a live offscreen Bevy 2D scene rendered into a target `Handle<Image>`, registered through
    `ImguiBevyTextures`, and displayed as an ImGui scene viewport.
  - Added docked hierarchy, inspector, input-policy, and diagnostics panels with viewport zoom,
    playback state, selection state, capture flags, and frame/snapshot diagnostics.
  - Added `render::ImguiOverlayDisabled` for cameras that should not receive the global ImGui
    overlay pass, and used it on the editor scene camera to avoid feedback into the scene viewport.
  - Updated `tests/render_extract.rs` so disabled cameras are excluded from extracted overlay
    camera targets.
  - Updated `backends/dear-imgui-bevy/README.md` with the persistent editor shell behavior and
    `ImguiOverlayDisabled` guidance.
  - `cargo +stable fmt --all --check` — PASS.
  - `cargo +stable check -p dear-imgui-bevy --features render --example editor_shell` — PASS.
  - `cargo +stable test -p dear-imgui-bevy --features render render_extract_clones_snapshot_texture_requests_and_camera_targets -- --nocapture`
    — PASS: 1 passed.
  - `cargo +stable nextest run -p dear-imgui-bevy --features render` — PASS: 21 passed, 2 skipped
    ignored GPU harness tests.
  - Status: BRP-040 DONE. Continue with BRP-050 closeout.
- 2026-05-23: BRP-050 closeout completed.
  - `cargo +stable fmt --all --check` — PASS.
  - `cargo +stable check -p dear-imgui-bevy --features render --example windowed_overlay` — PASS.
  - `cargo +stable nextest run -p dear-imgui-bevy --features render` — PASS: 21 passed, 2 skipped
    ignored GPU harness tests.
  - `cargo +stable check -p dear-imgui-bevy --features render --example editor_shell` — PASS.
  - `cargo +stable test -p dear-imgui-bevy --features render render_extract_clones_snapshot_texture_requests_and_camera_targets -- --nocapture`
    — PASS: 1 passed.
  - `cargo +stable clippy -p dear-imgui-bevy --all-targets --features render --no-deps -- -D warnings`
    — PASS.
  - Status: Closed. The lane now ships the persistent windowed example, runtime renderer harness
    coverage, and richer editor shell surface.

## Notes

Fresh verification is required before marking a task, Codex goal, or lane complete. Do not claim a
runtime GPU path is proven from CPU-only renderer preparation tests.
