# Bevy Native Backend Workstream — Evidence And Gates

Status: Active
Last updated: 2026-05-23

## Smallest Current Repro

Before implementation exists, the smallest proof is documentation consistency:

```bash
test -f docs/adr/0001-bevy-native-imgui-backend.md \
  -a -f docs/workstreams/bevy-native-backend/DESIGN.md \
  -a -f docs/workstreams/bevy-native-backend/TODO.md \
  -a -f docs/workstreams/bevy-native-backend/MILESTONES.md \
  -a -f docs/workstreams/bevy-native-backend/EVIDENCE_AND_GATES.md \
  -a -f docs/workstreams/bevy-native-backend/WORKSTREAM.json
```

## Gate Set

### Documentation / Planning Gate

```bash
test -f docs/adr/0001-bevy-native-imgui-backend.md \
  -a -f docs/workstreams/bevy-native-backend/DESIGN.md \
  -a -f docs/workstreams/bevy-native-backend/TODO.md \
  -a -f docs/workstreams/bevy-native-backend/MILESTONES.md \
  -a -f docs/workstreams/bevy-native-backend/EVIDENCE_AND_GATES.md \
  -a -f docs/workstreams/bevy-native-backend/WORKSTREAM.json
```

Proves that the workstream has durable authority docs before implementation begins.


### Bevy Target Train Gate

```bash
git -C repo-ref/bevy rev-list -n1 v0.19.0-rc.2
git -C repo-ref/bevy show v0.19.0-rc.2:Cargo.toml | grep -n '^version\|^rust-version'
git -C repo-ref/bevy show v0.19.0-rc.2:crates/bevy_render/Cargo.toml | grep -n 'wgpu = '
```

Expected first-proof target: Bevy `v0.19.0-rc.2` at `a389b928aee5906928a16a7d4e66cb02c7362901`, Rust `1.95.0`, WGPU `29.0.3`. This gate proves the implementation is tied to an explicit reference train rather than an implicit moving Bevy main.

### Core Lifecycle Gate

```bash
cargo nextest run -p dear-imgui-rs --test frame_lifecycle
```

Proves that core lifecycle/snapshot changes are validated without relying on Bevy.

### Extension Ecosystem Gate

```bash
cargo nextest run -p dear-implot -p dear-imnodes -p dear-node-editor -p dear-imguizmo
```

Package names may need adjustment to match actual crate names. This gate proves the Bevy lifecycle changes did not strand existing `Ui`-based extension crates.

### Bevy Backend Package Gate

```bash
cargo check -p dear-imgui-bevy --no-default-features
cargo check -p dear-imgui-bevy --features render
cargo nextest run -p dear-imgui-bevy
```

Proves the new backend crate compiles in its minimal and render-enabled modes and passes its own tests.

### Example Gates

```bash
cargo check -p dear-imgui-bevy --example simple
cargo check -p dear-imgui-bevy --example editor_shell
cargo check -p dear-imgui-bevy --example ecosystem
```

Proves the public usage surfaces for embedded UI, editor shell, and ecosystem composition remain valid.

### Broader Closeout Gate

Preferred when feasible:

```bash
cargo nextest run --workspace
```

If Bevy's MSRV or dependency weight makes the full workspace gate impractical, use a narrower documented gate covering changed core, extension, and backend crates, and explain why here with fresh evidence.

### Review Gate

Run `review-workstream` before accepting task or lane completion. Record blocking findings, missing gates, and residual risks in this file or link to the review note.

## Evidence Anchors

- `docs/adr/0001-bevy-native-imgui-backend.md`
- `docs/workstreams/bevy-native-backend/DESIGN.md`
- `docs/workstreams/bevy-native-backend/TODO.md`
- `docs/workstreams/bevy-native-backend/MILESTONES.md`
- `docs/workstreams/bevy-native-backend/WORKSTREAM.json`
- future code paths:
  - `backends/dear-imgui-bevy/`
  - `dear-imgui/src/context/`
  - `dear-imgui/src/render/`
  - extension adapter/example paths added by BEVY-040/BEVY-130

## Fresh Evidence Log

- 2026-05-22: Workstream opened and ADR/design/task/gate docs created. Implementation gates not yet run because implementation has not started.
- 2026-05-22: BEVY-011 target train initially pinned to local `repo-ref/bevy` tag `v0.19.0-rc.1`, then intentionally updated to `v0.19.0-rc.2` after the newer release candidate became available. Current target: commit `a389b928aee5906928a16a7d4e66cb02c7362901`, Bevy `0.19.0-rc.2`, Rust `1.95.0`, WGPU `29.0.3`; `repo-ref/bevy_egui` kept as Bevy `0.18` ergonomics reference only.


- 2026-05-22: BEVY-020 core lifecycle proof implemented and verified.
  - `cargo nextest run -p dear-imgui-rs --test frame_lifecycle` — PASS, 6 tests. Proves explicit engine-owned prepare/begin/multi-system UI/render/snapshot flow, managed texture request snapshot handoff, and pre-FFI render-without-frame guard.
  - `cargo fmt --all --check` — PASS.
  - `cargo check -p dear-imgui-rs` — PASS.
  - `cargo doc -p dear-imgui-rs --no-deps` — PASS with pre-existing rustdoc warnings outside the new lifecycle docs.
  - Note: `cargo nextest run -p dear-imgui-rs frame_lifecycle` matches only the crate-internal context test under nextest filtering; the authoritative BEVY-020 gate is the integration-test command above.

- 2026-05-22: BEVY-030 snapshot/texture feedback contract implemented and verified.
  - `cargo nextest run -p dear-imgui-rs --test snapshot_contract` — PASS, 5 tests. Proves `FrameSnapshot` preserves draw metadata, legacy texture bindings, managed texture bindings, standard sampler callbacks, managed create requests, optional draw-only snapshot mode, and tight texture update upload rectangles.
  - `cargo nextest run -p dear-imgui-rs --test frame_lifecycle` — PASS, 6 tests. Re-ran lifecycle gate after snapshot-contract updates.
  - `cargo nextest run -p dear-imgui-rs platform_io::tests::apply_texture_feedback_can_set_backend_user_data_and_clear_on_destroy --lib` — PASS. Proves `TextureFeedback` can carry renderer-owned backend user data and that `Destroyed` feedback clears TexID/backend user data on the UI thread.
  - `cargo check -p dear-imgui-rs` — PASS.
  - Review: no blocking BEVY-030 findings. Residual note: after submodule updates, stale `dear-imgui-sys` build artifacts can still expose old missing-symbol link errors until `cargo clean -p dear-imgui-sys`; this is build-cache hygiene, not a Bevy snapshot contract blocker.

- 2026-05-22: BEVY-040 extension ecosystem composition proof implemented and verified.
  - `cargo nextest run -p dear-imguizmo --test ecosystem_frame` — PASS, 1 test. Proves ImPlot, ImNodes, imgui-node-editor, and ImGuizmo can all draw through the same engine-managed `FrameToken`/`&Ui` before a single `render_snapshot()` handoff.
  - `cargo nextest run -p dear-implot -p dear-imnodes -p dear-node-editor -p dear-imguizmo` — PASS, 84 tests. Proves the targeted extension crates still pass with the shared-frame composition test included.
  - `cargo fmt --all --check` — PASS.
  - Review: no blocking BEVY-040 findings. Bevy should model extension-owned contexts/editors as ECS resources instead of creating isolated plugin frames.

- 2026-05-22: Fresh completion audit after `git submodule update --init --recursive`.
  - Current cimgui sources export `ImDrawList_AddLineH`, `ImDrawList_AddLineV`, and `ImGuiViewport_GetDebugName` in `dear-imgui-sys/third-party/cimgui/cimgui.cpp` and `cimgui.h`; pregenerated bindings also declare them.
  - `cargo clean -p dear-imgui-sys && cargo nextest run -p dear-imgui-rs direct_draw_inputs_validate_before_ffi --lib && cargo nextest run -p dear-imgui-rs platform_io::viewport::tests::main_viewport_exposes_debug_name --lib` — PASS. Proves the safe `add_line_h`/`add_line_v` and `Viewport::debug_name()` APIs link against the updated submodule after clearing stale sys artifacts.
  - `cargo nextest run -p dear-imgui-rs --test snapshot_contract` — PASS, 5 tests, including standard sampler callback preservation as `DrawCmdSnapshot::SetSamplerLinear` / `SetSamplerNearest`.
  - `cargo nextest run -p dear-imgui-rs platform_io::tests::apply_texture_feedback_can_set_backend_user_data_and_clear_on_destroy --lib` — PASS.
  - `cargo nextest run -p dear-imguizmo --test ecosystem_frame` — PASS, 1 test.
  - `cargo nextest run -p dear-imgui-rs --test frame_lifecycle` — PASS, 6 tests.
  - `cargo nextest run -p dear-implot -p dear-imnodes -p dear-node-editor -p dear-imguizmo` — PASS, 84 tests.
  - `cargo check -p dear-imgui-rs` — PASS.
  - `cargo fmt --all --check` — PASS.
  - `CHANGELOG.md` has no diff for this symbol investigation by design: the failure was stale local build state, not a safe API or release-note-worthy bug.

- 2026-05-22: BEVY-050 Bevy backend crate skeleton implemented and verified.
  - `backends/dear-imgui-bevy` was added as an experimental workspace crate with `rust-version = "1.95.0"` because the first Bevy target train requires Rust `1.95.0` while the root workspace remains on Rust `1.92`.
  - The crate provides `ImguiPlugin`, `ImguiBackendConfig`, `ImguiBackendStatus`, non-send `ImguiContext`, a `render` feature placeholder, module docs, README gate policy, and plugin integration tests.
  - Bevy app/ECS dependencies are exact-pinned to `0.19.0-rc.2`; additional direct pin anchors keep Bevy app/ECS transitive crates from drifting to newer prereleases.
  - `cargo +stable tree -p dear-imgui-bevy --no-default-features | grep -E 'bevy(_|-)'` plus `! grep -v '0.19.0-rc.2' /tmp/dear-imgui-bevy-tree.txt` — PASS. Proves every Bevy app/ECS dependency-tree entry remains on Bevy `0.19.0-rc.2`.
  - `cargo +stable check -p dear-imgui-bevy --no-default-features` — PASS.
  - `cargo +stable check -p dear-imgui-bevy --features render` — PASS.
  - `cargo +stable nextest run -p dear-imgui-bevy` — PASS, 2 tests. Proves `ImguiPlugin` registers the minimal Bevy resources and preserves caller-provided config/context.
  - `cargo fmt --all --check` — PASS.
  - `cargo check -p dear-imgui-rs && cargo nextest run -p dear-imgui-rs --test snapshot_contract && cargo fmt --all --check` — PASS after BEVY-050. Proves the new Bevy workspace member did not break the core snapshot gate under the root Rust `1.92.0` toolchain.
  - Broader `cargo nextest run --workspace` was not run for BEVY-050 because this task intentionally introduces a Rust `1.95.0` Bevy-coupled crate into a workspace whose root `rust-toolchain.toml` still pins Rust `1.92.0`; the dedicated `cargo +stable` Bevy gates above are the authoritative BEVY-050 gates.

- 2026-05-22: Target train follow-up updated BEVY-050 from Bevy `0.19.0-rc.1` to the newer Bevy `0.19.0-rc.2`.
  - `git -C repo-ref/bevy fetch origin tag v0.19.0-rc.2` — PASS.
  - `git -C repo-ref/bevy rev-list -n1 v0.19.0-rc.2` — `a389b928aee5906928a16a7d4e66cb02c7362901`.
  - `git -C repo-ref/bevy show v0.19.0-rc.2:Cargo.toml | grep -n '^version\|^rust-version'` — Bevy `0.19.0-rc.2`, Rust `1.95.0`.
  - `git -C repo-ref/bevy show v0.19.0-rc.2:crates/bevy_render/Cargo.toml | grep -n 'wgpu = '` — WGPU `29.0.3`.
  - `cargo +stable tree -p dear-imgui-bevy --no-default-features | grep -E 'bevy(_|-)'` plus `! grep -v '0.19.0-rc.2' /tmp/dear-imgui-bevy-tree.txt` — PASS. Proves every Bevy app/ECS crate in the BEVY-050 dependency tree is on rc.2.
  - `cargo +stable check -p dear-imgui-bevy --no-default-features` — PASS.
  - `cargo +stable check -p dear-imgui-bevy --features render` — PASS.
  - `cargo +stable nextest run -p dear-imgui-bevy` — PASS, 2 tests.
  - `cargo fmt --all --check` — PASS.

- 2026-05-22: BEVY-060 primary-window input mapping implemented and verified.
  - `cargo +stable nextest run -p dear-imgui-bevy input` — PASS, 9 tests. Proves primary-window window metrics/DPI, mouse move/button/wheel/leave, keyboard key/modifier/text, focus-loss sticky input release, `KeyboardFocusLost`, touch-to-mouse, IME enable/commit/disable semantics, non-primary filtering, and key mapping coverage.
  - `cargo +stable nextest run -p dear-imgui-bevy` — PASS, 11 tests. Re-ran the full backend package test set with BEVY-060 included.
  - `cargo +stable check -p dear-imgui-bevy --no-default-features` — PASS.
  - `cargo +stable check -p dear-imgui-bevy --features render` — PASS.
  - `cargo +stable tree -p dear-imgui-bevy --no-default-features | grep -E 'bevy(_|-)'` plus `! grep -v '0.19.0-rc.2' /tmp/dear-imgui-bevy-tree.txt` — PASS, 47 Bevy dependency-tree entries all on `0.19.0-rc.2`.
  - `cargo fmt --all --check` — PASS after rustfmt import-order cleanup.
  - `cargo +stable clippy -p dear-imgui-bevy --all-targets --features render --no-deps -- -D warnings` — PASS for the backend crate. The broader form without `--no-deps` is not a BEVY-060 gate and currently trips a pre-existing `dear-imgui-sys/build.rs` `clippy::collapsible_if` warning under `-D warnings`; BEVY-060 did not edit that build script.
  - Review: no blocking BEVY-060 findings. Residual follow-ups are intentionally outside primary-window scope: multi-window routing, cursor icon feedback, and platform IME positioning.

- 2026-05-22: BEVY-070 ECS frame lifecycle scheduling implemented and verified.
  - `cargo +stable nextest run -p dear-imgui-bevy lifecycle` — PASS, 2 tests. Proves `ImguiPrimaryContextPass` opens user systems inside an already-open primary frame, two sequential user systems share the same frame-scoped `&Ui`, frame indices advance once per `App::update()`, UI access is unavailable outside the pass, and `ImguiEndFrame` stores a `FrameSnapshot`.
  - `cargo +stable nextest run -p dear-imgui-bevy input` — PASS, 9 tests. Re-ran the BEVY-060 input gate after lifecycle schedules were inserted; input tests now drive `PreUpdate` directly to keep the input-only boundary explicit.
  - `cargo +stable nextest run -p dear-imgui-bevy` — PASS, 13 tests. Re-ran the full backend package set with BEVY-060 and BEVY-070 together.
  - `cargo +stable check -p dear-imgui-bevy --no-default-features` — PASS.
  - `cargo +stable check -p dear-imgui-bevy --features render` — PASS.
  - `cargo +stable clippy -p dear-imgui-bevy --all-targets --features render --no-deps -- -D warnings` — PASS for the backend crate.
  - `cargo fmt --all --check` — PASS.
  - Review: no blocking BEVY-070 findings. Renderer extraction, render-world resource transfer, and texture feedback application are intentionally BEVY-080+.

- 2026-05-22: BEVY-080 Bevy RenderApp extraction implemented and verified.
  - `cargo +stable nextest run -p dear-imgui-bevy --features render render_extract` — PASS, 1 test. Proves `ImguiFrameOutput` snapshots are cloned into render-world `ImguiExtractedRenderFrame`, managed texture requests remain present in the owned `FrameSnapshot`, active cameras are associated with normalized render targets, inactive cameras are skipped, and the extracted frame is not a main-world resource.
  - `cargo +stable nextest run -p dear-imgui-bevy --features render` — PASS, 14 tests. Re-ran the backend package set with render extraction enabled.
  - `cargo +stable nextest run -p dear-imgui-bevy` — PASS, 13 tests. Re-ran the default backend package set without the render feature.
  - `cargo +stable check -p dear-imgui-bevy --no-default-features` — PASS.
  - `cargo +stable check -p dear-imgui-bevy --features render` — PASS.
  - `cargo +stable clippy -p dear-imgui-bevy --all-targets --features render --no-deps -- -D warnings` — PASS for the backend crate.
  - `cargo +stable fmt --all --check` — PASS.
  - `cargo +stable tree -p dear-imgui-bevy --features render | grep -E 'bevy(_|-)'` plus `! grep -v '0.19.0-rc.2' /tmp/dear-imgui-bevy-render-tree.txt` — PASS, 154 Bevy dependency-tree entries all on `0.19.0-rc.2`.
  - Note: `cargo +stable nextest run -p dear-imgui-bevy render_extract` without `--features render` intentionally discovers zero tests because `tests/render_extract.rs` is gated behind the `render` feature; BEVY-080 evidence uses the render-enabled command above.
  - Review: no blocking BEVY-080 findings. Residual risk for BEVY-090: renderer integration may need to map the stored main-world camera entity to Bevy render entities / `ExtractedView`; this does not block extraction proof completion.
  - Final verification rerun before completion: `cargo +stable nextest run -p dear-imgui-bevy --features render render_extract && cargo +stable check -p dear-imgui-bevy --features render && cargo +stable fmt --all --check && python3 -m json.tool docs/workstreams/bevy-native-backend/WORKSTREAM.json >/tmp/workstream-json-check.txt` — PASS.

- 2026-05-22: BEVY-090 first renderer-preparation slice implemented and verified.
  - `cargo +stable nextest run -p dear-imgui-bevy --features render renderer_prepare` — PASS, 1 test. Proves render-world preparation flattens extracted `FrameSnapshot` data into renderer-ready vertices, indices, per-camera draw commands, texture bindings, scissor rectangles, and validates the local ImGui WGSL entry points plus vertex layout.
  - `cargo +stable nextest run -p dear-imgui-bevy --features render renderer_pipeline` — PASS, 1 test. Proves the embedded shader asset, specialized pipeline descriptor, bind group layouts, texture-bind-group map resource, queued-pipeline resource, and render-world pipeline resources are installed.
  - `cargo +stable check -p dear-imgui-bevy --features render` — PASS.
  - `cargo +stable nextest run -p dear-imgui-bevy --features render` — PASS, 16 tests. Re-ran the backend package set with BEVY-090 preparation and pipeline scaffold included.
  - `cargo +stable nextest run -p dear-imgui-bevy` — PASS, 13 tests. Re-ran the default backend package set without the render feature after the pipeline scaffold landed.
  - `cargo +stable check -p dear-imgui-bevy --no-default-features` — PASS.
  - `cargo +stable clippy -p dear-imgui-bevy --all-targets --features render --no-deps -- -D warnings` — PASS for the backend crate.
  - `cargo +stable fmt --all --check` — PASS.
  - Status: BEVY-090 remains IN_PROGRESS. The next required slice is concrete texture-map population / fallback handling and any final camera/render-target integration polish before BEVY-100.

- 2026-05-23: BEVY-090 Bevy-native WGPU renderer proof completed and verified.
  - Added managed texture bind-group population for `TextureOp::Create`, row updates for `TextureOp::Update`, removal for `TextureOp::Destroy`, and a 1x1 white fallback bind group in `ImguiPipelineGpuResources` for draw commands whose texture binding is not yet registered.
  - The overlay pass now draws prepared commands with registered texture bind groups or the fallback bind group, while still consuming only owned `FrameSnapshot` / `ImguiPreparedRenderFrame` data from the Bevy render world.
  - `cargo +stable nextest run -p dear-imgui-bevy --features render renderer` — PASS, 2 tests. Proves renderer preparation and pipeline descriptors/resources remain wired after texture-map/fallback completion.
  - `cargo +stable check -p dear-imgui-bevy --features render` — PASS.
  - `cargo +stable nextest run -p dear-imgui-bevy --features render` — PASS, 18 tests. Re-ran the render-enabled backend package set with texture conversion unit tests included.
  - `cargo +stable nextest run -p dear-imgui-bevy` — PASS, 13 tests.
  - `cargo +stable check -p dear-imgui-bevy --no-default-features` — PASS.
  - `cargo +stable clippy -p dear-imgui-bevy --all-targets --features render --no-deps -- -D warnings` — PASS for the backend crate.
  - `cargo +stable fmt --all --check` — PASS.
  - Review: no blocking BEVY-090 findings. Residual risk is intentionally moved to BEVY-100: applying texture feedback on the main/UI context and exposing Bevy `Handle<Image>` user textures.
  - Status: BEVY-090 DONE. BEVY-100 remains responsible for applying texture feedback back to the main/UI context and exposing Bevy `Handle<Image>` user textures.

- 2026-05-23: BEVY-100 texture feedback and Bevy user-image interop implemented and verified.
  - Added `ImguiTextureFeedbackQueue`, installed it in the main-world lifecycle, shared it with the render world, and applied queued `TextureFeedback` through `PlatformIo::apply_texture_feedback` before each new `ImguiBeginFrame`.
  - Added `ImguiBevyTextures` for stable, idempotent `Handle<Image>` to Dear ImGui `TextureId` registration, plus render-world extraction into `ImguiExtractedBevyTextures`.
  - Render bind-group preparation now queues managed texture feedback after create/update/destroy, preserves managed texture bindings, registers a legacy TexID alias for created managed textures, and resolves extracted Bevy image registrations through `RenderAssets<GpuImage>`.
  - `cargo +stable nextest run -p dear-imgui-bevy --features render texture` — PASS, 6 tests. Proves managed texture requests cross the render boundary, render-world feedback is applied on the next main-world frame, and Bevy image handles register/extract as stable ImGui texture ids.
  - `cargo +stable nextest run -p dear-imgui-bevy --features render` — PASS, 21 tests.
  - `cargo +stable nextest run -p dear-imgui-bevy` — PASS, 13 tests.
  - `cargo +stable check -p dear-imgui-bevy --features render` — PASS.
  - `cargo +stable check -p dear-imgui-bevy --no-default-features` — PASS.
  - `cargo +stable clippy -p dear-imgui-bevy --all-targets --features render --no-deps -- -D warnings` — PASS.
  - `cargo +stable fmt --all --check` — PASS.
  - Review: no blocking BEVY-100 findings. Residual risk: current tests do not instantiate a real `RenderDevice`, so Bevy image bind-group creation is compile- and integration-path covered but not screenshot/runtime-rendered until the example harnesses in BEVY-110/120.
  - Status: BEVY-100 DONE. Continue with BEVY-110 simple embedded Bevy example.

- 2026-05-23: BEVY-110 simple embedded Bevy example implemented and verified.
  - Added `backends/dear-imgui-bevy/examples/simple.rs`, a minimal one-frame Bevy app that installs `ScheduleRunnerPlugin::run_once`, `ImguiPlugin`, creates a primary window entity, initializes the ImGui context, and draws an overlay in `ImguiPrimaryContextPass`.
  - Updated the backend README with the simple example command.
  - `cargo +stable check -p dear-imgui-bevy --example simple` — PASS.
  - `cargo +stable fmt --all --check` — PASS.
  - `python3 -m json.tool docs/workstreams/bevy-native-backend/WORKSTREAM.json` — PASS.
  - Review: no blocking BEVY-110 findings. The example is intentionally headless/single-frame to preserve the crate's narrow exact-pinned Bevy dependency surface; BEVY-120 owns the editor-shell/render-to-texture depth.
  - Status: BEVY-110 DONE. Continue with BEVY-120 editor shell example.

- 2026-05-23: BEVY-120 editor shell example implemented and verified.
  - Added `backends/dear-imgui-bevy/examples/editor_shell.rs`, an editor-oriented render-feature example that creates a Bevy `Image` render target, registers it through `ImguiBevyTextures`, renders it in a dockspace-driven ImGui shell, and documents the input-routing policy for editor tools.
  - Updated the backend README with the editor shell run command and marked the example as `required-features = ["render"]`.
  - `cargo +stable check -p dear-imgui-bevy --features render --example editor_shell` — PASS.
  - `cargo +stable fmt --all` — PASS.
  - Review: no blocking BEVY-120 findings. The example proves the render-target and input-policy surfaces without expanding into a full editor product; BEVY-130 owns ecosystem composition in the same Bevy-managed frame.
  - Status: BEVY-120 DONE. Continue with BEVY-130 ecosystem example.

## Notes

Fresh verification is required before marking a task, Codex goal, or lane complete. Do not mark renderer or lifecycle tasks done based only on compile success if the task changes runtime frame ordering or texture feedback semantics.
