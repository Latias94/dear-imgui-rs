# Bevy Runtime Productization Workstream — Evidence And Gates

Status: Active
Last updated: 2026-05-23

## Smallest Current Repro

The current gap is that existing examples compile as one-frame proofs but do not prove a normal
windowed Bevy runtime app:

```bash
cargo +stable run -p dear-imgui-bevy --example simple
```

This exits after one update because it uses `ScheduleRunnerPlugin::run_once()`.

## Gate Set

### Persistent Windowed Example Gate

```bash
cargo +stable check -p dear-imgui-bevy --features render --example windowed_overlay
```

Proves the real windowed example compiles against the selected Bevy target train.

### Runtime Renderer Harness Gate

```bash
cargo +stable nextest run -p dear-imgui-bevy --features render <runtime-renderer-filter>
```

If the harness requires a real GPU/device that is not stable in default CI, record an opt-in command
and keep a deterministic package gate below as the default gate.

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

## Notes

Fresh verification is required before marking a task, Codex goal, or lane complete. Do not claim a
runtime GPU path is proven from CPU-only renderer preparation tests.
