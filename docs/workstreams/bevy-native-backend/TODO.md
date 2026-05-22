# Bevy Native Backend Workstream — TODO

Status: Active
Last updated: 2026-05-22

## Task Status Legend

- `TODO`: not started.
- `IN_PROGRESS`: actively being changed by one owner.
- `DONE`: implemented and verified with fresh evidence.
- `DONE_WITH_CONCERNS`: usable but has documented residual risk.
- `BLOCKED`: cannot proceed without decision or external change.
- `NEEDS_CONTEXT`: requires planner clarification before work starts.

## M0 — Scope And Evidence Freeze

- [x] BEVY-010 [owner=planner] [deps=none] [scope=docs/adr,docs/workstreams/bevy-native-backend]
  Goal: Freeze the native-Bevy direction, target state, non-goals, and initial evidence anchors.
  Validation: `DESIGN.md`, `MILESTONES.md`, `EVIDENCE_AND_GATES.md`, `WORKSTREAM.json`, and ADR exist and agree.
  Evidence: `docs/adr/0001-bevy-native-imgui-backend.md`, `docs/workstreams/bevy-native-backend/DESIGN.md`
  Handoff: Planner completed the initial lane opening; implementation starts at BEVY-020.

- [x] BEVY-011 [owner=planner] [deps=BEVY-010] [scope=docs/workstreams/bevy-native-backend,repo-ref/bevy,repo-ref/bevy_egui]
  Goal: Pin the first supported Bevy target train and record the exact API assumptions used by the proof.
  Validation: `DESIGN.md` records the selected Bevy version/commit assumptions and any MSRV consequence.
  Evidence: `docs/workstreams/bevy-native-backend/DESIGN.md`, `docs/workstreams/bevy-native-backend/EVIDENCE_AND_GATES.md`
  Handoff: DONE 2026-05-22. First proof target was intentionally updated to local `repo-ref/bevy` tag `v0.19.0-rc.2` commit `a389b928aee5906928a16a7d4e66cb02c7362901`, Bevy `0.19.0-rc.2`, Rust `1.95.0`, WGPU `29.0.3`; `repo-ref/bevy_egui` is Bevy `0.18` ergonomics reference only. If Bevy APIs shift, split a compatibility task instead of silently widening implementation scope.

## M1 — Core Lifecycle Proof

- [x] BEVY-020 [owner=codex] [deps=BEVY-010] [scope=dear-imgui/src/context,dear-imgui/src/render,dear-imgui/tests]
  Goal: Add or refactor Dear ImGui frame lifecycle APIs so an engine can open one frame, run many systems, close the frame, and snapshot output without exposing raw context misuse.
  Validation: `cargo nextest run -p dear-imgui-rs frame_lifecycle` or the nearest targeted lifecycle/snapshot test added by the task.
  Review: `review-workstream` before accepting completion.
  Evidence: `dear-imgui/tests/frame_lifecycle.rs`, `dear-imgui/src/context/frame.rs`, fresh `cargo nextest run -p dear-imgui-rs --test frame_lifecycle`.
  Handoff: DONE 2026-05-22. Added explicit engine-managed lifecycle API: `FramePrepareOptions`, `Context::prepare_frame`, `FrameLifecycleState`, `Context::begin_frame`, `FrameToken::{ui,lifecycle_state,render,render_snapshot}`, and `Context::frame_with_result`. Existing `Context::frame`/`render` remain compatible, with added pre-FFI guards against nested frames and render-without-frame misuse.

- [x] BEVY-030 [owner=codex] [deps=BEVY-020] [scope=dear-imgui/src/render,dear-imgui/src/platform_io,dear-imgui/tests]
  Goal: Audit and extend `FrameSnapshot`, `TextureRequest`, and `TextureFeedback` for Bevy render-world needs, including draw command, texture binding, sampler, and feedback application semantics.
  Validation: `cargo nextest run -p dear-imgui-rs snapshot` plus any new texture feedback tests.
  Review: `review-workstream` before accepting completion.
  Evidence: `dear-imgui/tests/snapshot_contract.rs`, `dear-imgui/src/render/snapshot.rs`, `dear-imgui/src/platform_io/textures.rs`, `dear-imgui/src/platform_io/tests.rs`, fresh `cargo nextest run -p dear-imgui-rs --test snapshot_contract`.
  Handoff: DONE 2026-05-22. Snapshot now proves draw metadata, legacy and managed texture bindings, standard sampler callbacks, managed create requests, draw-only snapshot mode, tight texture update uploads, and feedback application with TexID/backend-user-data cleanup. No Bevy-only core hack was added; Bevy render-world integration should consume `FrameSnapshot::draw`, `TextureRequest`, and apply `TextureFeedback` on the next UI-thread frame.

- [x] BEVY-040 [owner=codex] [deps=BEVY-020] [scope=extensions/dear-implot,extensions/dear-imnodes,extensions/dear-node-editor,extensions/dear-imguizmo,docs/workstreams/bevy-native-backend]
  Goal: Prove the extension ecosystem can share the Bevy-managed ImGui frame and identify any extension context/resource adapters needed for ImPlot, ImNodes, node editor, and ImGuizmo.
  Validation: targeted compile/tests for affected extension crates, e.g. `cargo nextest run -p dear-implot -p dear-imnodes -p dear-node-editor -p dear-imguizmo` if package names match.
  Review: `review-workstream` before accepting completion.
  Evidence: `extensions/dear-imguizmo/tests/ecosystem_frame.rs`, `extensions/dear-imguizmo/Cargo.toml`, fresh `cargo nextest run -p dear-implot -p dear-imnodes -p dear-node-editor -p dear-imguizmo`.
  Handoff: DONE 2026-05-22. A single engine-managed `FrameToken` can host ImPlot, ImNodes, imgui-node-editor, and ImGuizmo calls before one `render_snapshot()` close. No extension rewrite was required; Bevy should provide small ECS resources for extension contexts/editors and expose the same frame-scoped `&Ui` to user systems.

## M2 — Bevy Backend Skeleton And Input

- [x] BEVY-050 [owner=codex] [deps=BEVY-011,BEVY-020] [scope=Cargo.toml,backends/dear-imgui-bevy]
  Goal: Create the `dear-imgui-bevy` crate skeleton with feature flags, dependency policy, experimental docs, and minimal plugin registration.
  Validation: `cargo check -p dear-imgui-bevy --no-default-features` and the selected default feature check.
  Review: `review-workstream` before accepting completion.
  Evidence: `backends/dear-imgui-bevy/Cargo.toml`, `backends/dear-imgui-bevy/src/lib.rs`, `backends/dear-imgui-bevy/README.md`, `backends/dear-imgui-bevy/tests/plugin.rs`, fresh `cargo +stable check -p dear-imgui-bevy --no-default-features`, `cargo +stable check -p dear-imgui-bevy --features render`, and `cargo +stable nextest run -p dear-imgui-bevy`.
  Handoff: DONE 2026-05-22. Created the experimental crate skeleton, added it to the workspace, documented the dedicated Bevy/Rust gate policy, and installed a narrow `ImguiPlugin` that registers `ImguiBackendConfig`, `ImguiBackendStatus`, and a non-send `ImguiContext`. The crate now targets Bevy `0.19.0-rc.2` with exact pin anchors to keep the proof coherently on rc.2 until the next intentional target-train update. Continue with BEVY-060 input mapping; keep renderer extraction for BEVY-080+.

- [x] BEVY-060 [owner=codex] [deps=BEVY-050] [scope=backends/dear-imgui-bevy/src/input.rs,backends/dear-imgui-bevy/src/lib.rs]
  Goal: Translate Bevy window, mouse, keyboard, touch, focus, DPI, and IME messages into Dear ImGui IO for a primary window/context.
  Validation: targeted input unit tests in `dear-imgui-bevy` plus `cargo nextest run -p dear-imgui-bevy input`.
  Review: `review-workstream` before accepting completion.
  Evidence: `backends/dear-imgui-bevy/src/input.rs`, `backends/dear-imgui-bevy/tests/input.rs`, `backends/dear-imgui-bevy/README.md`, fresh `cargo +stable nextest run -p dear-imgui-bevy input`, `cargo +stable nextest run -p dear-imgui-bevy`, `cargo +stable check -p dear-imgui-bevy --no-default-features`, `cargo +stable check -p dear-imgui-bevy --features render`, and `cargo fmt --all --check`.
  Handoff: DONE 2026-05-22. Added primary-window input mapping in `input::ImguiInputSystems`, including window size/DPI, mouse position/buttons/wheel/leave, keyboard keys/modifiers/text, focus-loss sticky input release, first-touch-to-mouse translation, IME commit/enable/disable state, non-primary window filtering, and documented capture policy. Multi-window routing, cursor icon feedback, and platform IME positioning remain later follow-ups rather than BEVY-060 scope.

- [x] BEVY-070 [owner=codex] [deps=BEVY-060] [scope=backends/dear-imgui-bevy/src/context.rs,backends/dear-imgui-bevy/src/schedule.rs]
  Goal: Add Bevy ECS resources/components and schedules that open a frame once, expose frame-scoped UI access to systems, and close/snapshot the frame once.
  Validation: `cargo nextest run -p dear-imgui-bevy lifecycle` and a minimal compile example.
  Review: `review-workstream` before accepting completion.
  Evidence: `backends/dear-imgui-bevy/src/context.rs`, `backends/dear-imgui-bevy/src/schedule.rs`, `backends/dear-imgui-bevy/tests/lifecycle.rs`, `backends/dear-imgui-bevy/README.md`, fresh `cargo +stable nextest run -p dear-imgui-bevy lifecycle`, `cargo +stable nextest run -p dear-imgui-bevy`, `cargo +stable check -p dear-imgui-bevy --no-default-features`, `cargo +stable check -p dear-imgui-bevy --features render`, `cargo +stable clippy -p dear-imgui-bevy --all-targets --features render --no-deps -- -D warnings`, and `cargo fmt --all --check`.
  Handoff: DONE 2026-05-22. Added `ImguiBeginFrame`, `ImguiPrimaryContextPass`, `ImguiEndFrame`, `ImguiContexts`, `ImguiFrameState`, and `ImguiFrameOutput`. `ImguiPlugin` now inserts the schedules after `PreUpdate`, opens one primary-window frame per `App::update()`, lets multiple user systems draw through the same `&Ui`, and renders/snapshots once at frame end. Extension crates should compose by using the shared `&Ui` from `ImguiContexts`; render extraction remains BEVY-080.

## M3 — Bevy-Native Renderer Proof

- [x] BEVY-080 [owner=codex] [deps=BEVY-030,BEVY-050] [scope=backends/dear-imgui-bevy/src/render]
  Goal: Implement Bevy render-world extraction resources for frame snapshots, texture requests, and per-camera render target association.
  Validation: `cargo +stable nextest run -p dear-imgui-bevy --features render render_extract`.
  Review: `review-workstream` before accepting completion.
  Evidence: `backends/dear-imgui-bevy/src/render.rs`, `backends/dear-imgui-bevy/tests/render_extract.rs`, fresh `cargo +stable nextest run -p dear-imgui-bevy --features render render_extract`, `cargo +stable nextest run -p dear-imgui-bevy --features render`, `cargo +stable check -p dear-imgui-bevy --features render`, and Bevy rc.2 dependency-tree pin check.
  Handoff: DONE 2026-05-22. Added `render::ImguiExtractedRenderFrame` and `render::ImguiCameraTarget`, installed extraction into Bevy `RenderApp` / `ExtractSchedule`, cloned `FrameSnapshot` plus texture requests across the boundary, and recorded active camera normalized render targets sorted by camera order. Raw ImGui draw data remains on the main/UI thread. Continue with BEVY-090 for the Bevy-native WGPU pipeline.

- [ ] BEVY-090 [owner=unassigned] [deps=BEVY-080] [scope=backends/dear-imgui-bevy/src/render]
  Goal: Implement the Bevy-native WGPU pipeline: shader, specialized pipeline, buffers, bind groups, texture map, and draw pass inserted into Bevy camera rendering.
  Validation: `cargo check -p dear-imgui-bevy --features render` plus targeted renderer tests where possible.
  Review: `review-workstream` before accepting completion.
  Evidence: render pipeline code, shader, and compile gates.
  Handoff: Prefer Bevy `Core2d`/`Core3d` camera-driven schedules for Bevy `0.19-dev`; document any compatibility shim if needed.

- [ ] BEVY-100 [owner=unassigned] [deps=BEVY-090] [scope=backends/dear-imgui-bevy/src/texture.rs,backends/dear-imgui-bevy/src/render]
  Goal: Close the texture loop for ImGui-managed textures and Bevy user images, including `Handle<Image>` registration and render-to-texture viewport display.
  Validation: `cargo nextest run -p dear-imgui-bevy texture` plus compile-check examples using `Handle<Image>`.
  Review: `review-workstream` before accepting completion.
  Evidence: texture interop tests and example code.
  Handoff: This is the bridge that enables editor scene/game viewports; do not defer user textures unless the renderer proof is split.

## M4 — Examples And Ecosystem Composition

- [ ] BEVY-110 [owner=unassigned] [deps=BEVY-070,BEVY-090] [scope=examples,backends/dear-imgui-bevy/examples]
  Goal: Add an embedded Bevy example showing Dear ImGui overlay usage inside a normal Bevy app.
  Validation: `cargo check -p dear-imgui-bevy --example simple` or matching example gate.
  Review: `review-workstream` before accepting completion.
  Evidence: simple example and README usage snippet.
  Handoff: Keep this example minimal; editor features belong in BEVY-120.

- [ ] BEVY-120 [owner=unassigned] [deps=BEVY-100,BEVY-110] [scope=backends/dear-imgui-bevy/examples,docs]
  Goal: Add an editor-oriented example with dockspace, scene/game viewport via Bevy render-to-texture, and documented editor input policy.
  Validation: `cargo check -p dear-imgui-bevy --example editor_shell` or matching example gate.
  Review: `review-workstream` before accepting completion.
  Evidence: editor shell example and docs.
  Handoff: This is not a full editor product; split inspector/assets/console into follow-ons if needed.

- [ ] BEVY-130 [owner=unassigned] [deps=BEVY-040,BEVY-120] [scope=backends/dear-imgui-bevy/examples,extensions]
  Goal: Add an ecosystem composition example using at least ImPlot and one graph/gizmo extension in the same Bevy-managed ImGui frame.
  Validation: `cargo check -p dear-imgui-bevy --example ecosystem` and targeted extension crate checks.
  Review: `review-workstream` before accepting completion.
  Evidence: ecosystem example and adapter docs.
  Handoff: The example should prove shared context/frame composition, not just compile separate crates.

## M5 — Closeout

- [ ] BEVY-140 [owner=planner] [deps=BEVY-110,BEVY-130] [scope=docs/workstreams/bevy-native-backend,docs,CHANGELOG.md]
  Goal: Finalize docs, record fresh gates, split remaining risks, and decide whether the lane closes or continues into multi-window/editor-product follow-ons.
  Validation: `verify-rust-workstream` records final fresh evidence.
  Review: `review-workstream` has no blocking findings.
  Evidence: `EVIDENCE_AND_GATES.md`, `WORKSTREAM.json`, closeout notes.
  Handoff: Split docking multi-viewport, wasm/mobile, and full editor product into follow-on workstreams if they remain open.

## Parallelization Notes

Safe parallel work after BEVY-020 lands:

- BEVY-030 and BEVY-040 can run in parallel if they do not edit the same core files.
- BEVY-060 input and BEVY-080 render extraction can run in parallel after the crate skeleton exists, but renderer work depends on the snapshot contract.
- Example tasks should wait for the relevant API slices to settle.

Unsafe parallel work:

- Do not run multiple workers on `dear-imgui/src/context/*` lifecycle changes at the same time.
- Do not let one worker rewrite extension APIs while another writes Bevy ecosystem examples without a shared adapter contract.
