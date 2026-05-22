# Bevy Native Backend Workstream — Handoff

Status: Active
Last updated: 2026-05-23

## Current State

The workstream is open. The Bevy-native direction is recorded in ADR `docs/adr/0001-bevy-native-imgui-backend.md`. Design, milestones, task ledger, and gate plan exist under `docs/workstreams/bevy-native-backend/`.

Implementation has completed the first core proof slice and the backend skeleton:

- BEVY-020 lifecycle: explicit engine-managed Dear ImGui frame APIs.
- BEVY-030 snapshot/texture feedback: render-world handoff contract, including sampler callback preservation and texture feedback cleanup.
- BEVY-040 extension ecosystem: ImPlot, ImNodes, imgui-node-editor, and ImGuizmo compose inside one shared `FrameToken`/`&Ui`.
- BEVY-050 backend skeleton: `backends/dear-imgui-bevy` exists with a narrow plugin/resource surface and explicit Bevy/Rust gate policy.
- BEVY-060 input: primary-window Bevy window/input/IME messages feed Dear ImGui IO through `input::ImguiInputSystems`, with targeted tests and documented capture policy.
- BEVY-070 lifecycle: Bevy schedules open one primary ImGui frame, run user systems in `ImguiPrimaryContextPass` with shared `ImguiContexts`, and close/snapshot once per update.
- BEVY-080 render extraction: the render feature installs extraction into Bevy `RenderApp` / `ExtractSchedule`, clones `ImguiFrameOutput` snapshots into `render::ImguiExtractedRenderFrame`, preserves texture requests through the owned `FrameSnapshot`, and records active camera normalized render targets.
- BEVY-090 renderer proof: the render app now prepares extracted snapshots into Bevy-native WGPU batches, uploads vertex/index/uniform buffers, specializes an ImGui overlay pipeline, populates managed texture bind groups from snapshot texture requests, falls back to a 1x1 white texture for missing bindings, queues per-camera pipelines, and inserts Core2d/Core3d overlay passes.
- BEVY-100 texture interop: managed texture feedback is queued from render-world systems and applied on the main/UI thread before the next `ImguiBeginFrame`; Bevy `Handle<Image>` values register through `ImguiBevyTextures`, extract into render world, and resolve through `RenderAssets<GpuImage>` as legacy ImGui texture bind groups.
- BEVY-110 simple example: `backends/dear-imgui-bevy/examples/simple.rs` demonstrates installing `ImguiPlugin`, creating a primary window entity, and drawing overlay UI from `ImguiPrimaryContextPass`.

Submodule/symbol investigation is closed: after `git submodule update --init --recursive`, current cimgui exports `ImDrawList_AddLineH/V` and `ImGuiViewport_GetDebugName`; clean rebuild links safe APIs successfully. No CHANGELOG entry was added because the failure was stale local build state.

## Active Task

- Task ID: BEVY-120
- Owner: codex
- Files: `backends/dear-imgui-bevy/examples/editor_shell.rs`, `backends/dear-imgui-bevy/README.md`, docs as needed
- Validation: `cargo +stable check -p dear-imgui-bevy --example editor_shell` plus relevant backend/example gates.
- Status: TODO
- Evidence: BEVY-110 completed with `cargo +stable check -p dear-imgui-bevy --example simple`, `cargo +stable fmt --all --check`, and `python3 -m json.tool docs/workstreams/bevy-native-backend/WORKSTREAM.json`.

## Decisions Since Last Update

- `dear-imgui-bevy` is Bevy-native rather than a wrapper over `dear-imgui-winit` plus `dear-imgui-wgpu`.
- The backend lives in this repository first at `backends/dear-imgui-bevy`.
- The Bevy backend should preserve ecosystem composition by exposing the same frame-scoped `&Ui` to user systems and extension adapters.
- Engine-managed frame lifecycle is a core API concern, not a backend-local hack.
- Render-world integration should use snapshot and feedback contracts rather than raw ImGui draw data pointers.
- `ImguiContext` is a Bevy non-send resource because `dear_imgui_rs::Context` is not `Send`/`Sync` and Dear ImGui has current-context global state.
- BEVY-050 now targets Bevy app/ECS `0.19.0-rc.2` with exact pin anchors. The previous rc.1 skeleton showed that prerelease transitive crates can drift to a newer release candidate; exact pin anchors keep the proof coherently on rc.2 until we intentionally move again.
- BEVY-060 keeps input primary-window-only. The backend queues Dear ImGui IO events but does not consume or delete Bevy messages; gameplay/editor systems should read `ImguiInputCapture` / `io.want_capture_*` as policy hints.
- `Ime::Commit` queues committed characters without flipping `ImguiInputState::ime_enabled`; only explicit `Ime::Enabled` / `Ime::Disabled` messages update that state.
- BEVY-070 exposes user UI through `ImguiContexts` in `ImguiPrimaryContextPass`. User systems should not call `Context::frame()` or `Context::render()` directly.
- BEVY-070 snapshots the frame into `ImguiFrameOutput` on the main world. BEVY-080 now extracts that snapshot into Bevy render-world resources without borrowing raw ImGui draw-data pointers.
- BEVY-080 stores extracted frames only in Bevy `RenderApp`; tests assert `ImguiExtractedRenderFrame` is not a main-world resource. Extraction is installed in both plugin `build` and `finish` so it can attach when the render sub-app already exists or becomes available later in plugin setup.
- BEVY-090 keeps the renderer Bevy-native: it consumes owned `FrameSnapshot` / prepared data, never `dear-imgui-wgpu::WgpuRenderer`, and never borrows raw ImGui draw data across the main/render-world boundary. Missing texture bindings render through a white fallback bind group; BEVY-100 should replace that stopgap for real user-image interop where appropriate.
- BEVY-100 uses a cloned `ImguiTextureFeedbackQueue` resource to share a mutex-backed feedback list between Bevy main and render worlds. Render systems push `TextureFeedback`; the next main-world begin-frame drains and applies it via `PlatformIo::apply_texture_feedback`.
- BEVY-100 treats Bevy images as legacy ImGui texture ids derived from `AssetId<Image>`. `ImguiBevyTextures::register(&Handle<Image>)` is idempotent and render-world code resolves the extracted asset id through `RenderAssets<GpuImage>`.
- BEVY-110 intentionally uses `ScheduleRunnerPlugin::run_once()` and split Bevy crates instead of `DefaultPlugins` to keep the backend crate's dependency surface aligned with the current exact-pinned proof. A fully windowed/editor shell belongs in BEVY-120.

## Blockers / Constraints

- Bevy `v0.19.0-rc.2` at `repo-ref/bevy` commit `a389b928aee5906928a16a7d4e66cb02c7362901` advertises Rust `1.95.0`, higher than this workspace's root Rust `1.92.0` toolchain. Use dedicated `cargo +stable` Bevy gates until the repository-wide MSRV changes or CI adds a Rust 1.95+ Bevy lane.
- BEVY-100 tests do not instantiate a real WGPU `RenderDevice`, so direct GPU bind-group creation for `RenderAssets<GpuImage>` is compile- and integration-path covered but not runtime-rendered in CI yet. Keep screenshot/runtime verification for examples or a future renderer harness.
- Multi-window routing, cursor icon feedback, and platform IME positioning remain outside this lane unless split follow-ups choose to take them.

## Next Recommended Action

1. Start BEVY-120 with an editor-oriented example that demonstrates dockspace-style layout and a Bevy render-target `Handle<Image>` displayed through `ImguiBevyTextures`.
2. Keep full editor product features such as inspector/assets/console as follow-ons unless they are needed for the proof.
3. Re-run `cargo +stable check -p dear-imgui-bevy --example editor_shell` before marking BEVY-120 done.
