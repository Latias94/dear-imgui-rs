# Bevy Backend Prelaunch Refactor - Evidence And Gates

Status: Closed
Last updated: 2026-05-25

## Smallest Current Repro

The original pre-review gate compiled but appeared to hang during nextest discovery when it used the
shared root target directory:

```bash
cargo +stable nextest run -p dear-imgui-bevy --features render
cargo +stable nextest run -p dear-imgui-bevy --features render,multi-viewport
```

Observed state on 2026-05-25: both commands reached `Finished test profile`; child processes then
stayed at `dear_imgui_bevy --list --format terse` and `--ignored` until terminated. BPR-060
isolated this before closeout.

Closeout diagnosis: the backend tests themselves are not the blocker. The shared root
`target/debug/deps` directory was polluted/large enough that rustc startup and test discovery spent
minutes scanning dependency paths before any Bevy backend test started. All closeout gates below use
`CARGO_TARGET_DIR=target/bevy-backend-prelaunch`, and nextest discovery completes normally there.

## Gate Set

### Targeted Iteration Gates

```bash
cargo +stable nextest run -p dear-imgui-bevy render --features render
cargo +stable nextest run -p dear-imgui-bevy input --features render
cargo +stable nextest run -p dear-imgui-bevy texture --features render
```

### Package Gates

```bash
cargo +stable nextest run -p dear-imgui-bevy
cargo +stable nextest run -p dear-imgui-bevy --features render
cargo +stable nextest run -p dear-imgui-bevy --features render,multi-viewport
```

### Compile Gates

```bash
cargo +stable check -p dear-imgui-bevy --no-default-features
cargo +stable check -p dear-imgui-bevy --features render
cargo +stable check -p dear-imgui-bevy --target wasm32-unknown-unknown --no-default-features
cargo +stable check -p dear-imgui-bevy --target wasm32-unknown-unknown --features render
cargo +stable check -p dear-imgui-bevy --features render,multi-viewport,ecosystem --examples
```

### Hygiene Gates

```bash
cargo +stable fmt --all --check
git diff --check
```

### Review Gate

Closeout review completed on 2026-05-25. No blocking workstream-compliance, code-quality, or
missing-gate findings remain. Residual risks are recorded in `HANDOFF.md` and the closeout journal.

## Evidence Anchors

- `docs/workstreams/bevy-backend-prelaunch-refactor-v1/DESIGN.md`
- `docs/workstreams/bevy-backend-prelaunch-refactor-v1/TODO.md`
- `backends/dear-imgui-bevy/src/render.rs`
- `backends/dear-imgui-bevy/src/input.rs`
- `backends/dear-imgui-bevy/src/texture.rs`
- `backends/dear-imgui-bevy/README.md`
- `backends/dear-imgui-bevy/tests/`

## Evidence Log

### 2026-05-25 - Publication Readiness Rerun

Environment:

- `rustc 1.95.0 (59807616e 2026-04-14)`
- `cargo 1.95.0 (f2d3ce0bd 2026-03-21)`
- `cargo-nextest 0.9.115 (b8e0d5dcd 2025-12-15)`
- `CARGO_TARGET_DIR=target/bevy-backend-prelaunch`
- `wasm32-unknown-unknown` target installed before wasm checks.
- Commands were run serially to avoid cargo build-lock contention.

Results:

| Command | Result | Behavior proven |
| --- | --- | --- |
| `cargo nextest run -p dear-imgui-bevy` | PASS: 43 passed, 0 skipped | Base backend tests pass under the repository-pinned Rust 1.95 toolchain. |
| `cargo nextest run -p dear-imgui-bevy --features render` | PASS: 78 passed, 2 skipped | Render, input, texture, gamma, and lifecycle tests pass with the render feature enabled. |
| `cargo nextest run -p dear-imgui-bevy --features render,multi-viewport` | PASS: 101 passed, 2 skipped | Render plus multi-viewport tests pass, including secondary viewport/window and overlay-camera behavior. |
| `cargo check -p dear-imgui-bevy --no-default-features` | PASS | Core backend compiles without default features. |
| `cargo check -p dear-imgui-bevy --features render` | PASS | Native render feature compiles. |
| `cargo check -p dear-imgui-bevy --target wasm32-unknown-unknown --no-default-features` | PASS | Wasm core backend compiles; existing `dear-imgui` clipboard dead-code warnings remain. |
| `cargo check -p dear-imgui-bevy --target wasm32-unknown-unknown --features render` | PASS | Wasm render feature compiles; existing `dear-imgui` clipboard dead-code warnings remain. |
| `cargo check -p dear-imgui-bevy --features render,multi-viewport,ecosystem --examples` | PASS | Bevy examples compile across render, multi-viewport, and ecosystem feature surfaces. |
| `cargo clippy -p dear-imgui-bevy --all-targets --features render,multi-viewport,ecosystem --no-deps -- -D warnings` | PASS | Full Bevy backend target surface is clippy-clean; output contains only existing native binding build-script warnings. |
| `cargo fmt --all --check` | PASS | Workspace formatting remains clean after the publication readiness rerun. |
| `git diff --check` | PASS | No whitespace errors in the evidence update. |

### 2026-05-25 - Closeout Gate Run

Environment:

- `CARGO_TARGET_DIR=target/bevy-backend-prelaunch`
- Commands were run serially to avoid cargo build-lock contention.

Results:

| Command | Result | Behavior proven |
| --- | --- | --- |
| `cargo +stable fmt --all --check` | PASS | Workspace formatting is clean after example and doc-adjacent code changes. |
| `cargo +stable clippy -p dear-imgui-bevy --all-targets --features render --no-deps -- -D warnings` | PASS | Render feature library/tests are clippy-clean; output contains only existing native binding build-script warnings. |
| `cargo +stable check -p dear-imgui-bevy --no-default-features` | PASS | Core backend compiles without default features. |
| `cargo +stable check -p dear-imgui-bevy --features render` | PASS | Native render feature compiles. |
| `cargo +stable check -p dear-imgui-bevy --target wasm32-unknown-unknown --no-default-features` | PASS | wasm core backend compiles; existing `dear-imgui` clipboard dead-code warnings remain. |
| `cargo +stable check -p dear-imgui-bevy --target wasm32-unknown-unknown --features render` | PASS | wasm render feature compiles; existing `dear-imgui` clipboard dead-code warnings remain. |
| `cargo +stable check -p dear-imgui-bevy --features render,multi-viewport,ecosystem --examples` | PASS | Render, multi-viewport, ecosystem examples compile with explicit `ImguiOverlayCamera` markers. |
| `cargo +stable nextest run -p dear-imgui-bevy` | PASS: 43 passed, 0 skipped | Base backend tests complete without discovery hang in the dedicated target dir. |
| `cargo +stable nextest run -p dear-imgui-bevy --features render` | PASS: 78 passed, 2 skipped | Render extraction, preparation, texture, gamma, sampler, input, and lifecycle tests complete. |
| `cargo +stable nextest run -p dear-imgui-bevy --features render,multi-viewport` | PASS: 101 passed, 2 skipped | Multi-viewport tests complete with secondary overlay cameras marked explicitly. |
| `cargo +stable clippy -p dear-imgui-bevy --all-targets --features render,multi-viewport,ecosystem --no-deps -- -D warnings` | PASS | Full feature/test/example surface is clippy-clean after example marker updates; output contains only existing native binding build-script warnings. |
| `git diff --check` | PASS | No whitespace errors in the final diff. |

### 2026-05-25 - Behavioral Evidence

- BPR-020: `render_extract_prefers_explicit_overlay_camera_over_higher_order_fallback` proves the
  explicit overlay marker wins over the fallback order heuristic. `render_extract_uses_one_overlay_camera_per_render_target`
  keeps one overlay pass per target. `render_extract_preserves_explicit_overlay_camera_viewport_for_render_pass`
  proves Bevy camera viewport extraction reaches prepared draws.
- BPR-020 viewport semantics: `camera_viewport_uniforms_use_logical_viewport_rect_without_scaling_imgui_coordinates`
  proves the renderer uses the camera viewport's logical rect instead of scaling the whole ImGui
  framebuffer into a split-screen viewport. `render_pass_scissor_intersects_draws_with_camera_viewport_without_scaling`
  proves draw scissors are clipped to the physical camera viewport.
- BPR-030: `gamma_helper_uses_srgb_for_srgb_targets_and_compositing_space` proves gamma is selected
  from target format and `CompositingSpace::Srgb`.
- BPR-040: `renderer_pipeline_resources_and_descriptors_are_installed` proves the texture bind
  group layout contains a sampler binding. `bevy_image_sampling_compatibility_accepts_filterable_float_2d_views`
  and `bevy_image_handles_register_as_stable_imgui_texture_ids_and_extract` cover Bevy image
  texture interop, while prepared draw sampler tests cover ImGui managed texture linear/nearest
  selection.
- BPR-050: `input_capture_predicates_and_run_conditions_expose_imgui_policy_hints` proves the
  public capture predicate methods and run conditions reflect `ImguiInputCapture`.

### Gates Not Run

- `cargo +stable nextest run --workspace` was intentionally not run. This lane is scoped to the
  Bevy backend, whose README already documents a dedicated backend CI lane because it uses a
  different Rust/Bevy release train than the rest of the workspace.
