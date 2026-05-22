# Bevy Native Backend Workstream — Handoff

Status: Active
Last updated: 2026-05-22

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

Submodule/symbol investigation is closed: after `git submodule update --init --recursive`, current cimgui exports `ImDrawList_AddLineH/V` and `ImGuiViewport_GetDebugName`; clean rebuild links safe APIs successfully. No CHANGELOG entry was added because the failure was stale local build state.

## Active Task

- Task ID: BEVY-090
- Owner: unassigned
- Files: `backends/dear-imgui-bevy/src/render` or equivalent render extraction modules
- Validation: `cargo +stable check -p dear-imgui-bevy --features render` plus targeted renderer tests where possible.
- Status: TODO
- Evidence: BEVY-080 passed `cargo +stable nextest run -p dear-imgui-bevy --features render render_extract` (1 test), `cargo +stable nextest run -p dear-imgui-bevy --features render` (14 tests), `cargo +stable nextest run -p dear-imgui-bevy` (13 tests), `cargo +stable check -p dear-imgui-bevy --no-default-features`, `cargo +stable check -p dear-imgui-bevy --features render`, `cargo +stable clippy -p dear-imgui-bevy --all-targets --features render --no-deps -- -D warnings`, and the Bevy rc.2 dependency-tree pin check.

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

## Blockers / Constraints

- Bevy `v0.19.0-rc.2` at `repo-ref/bevy` commit `a389b928aee5906928a16a7d4e66cb02c7362901` advertises Rust `1.95.0`, higher than this workspace's root Rust `1.92.0` toolchain. Use dedicated `cargo +stable` Bevy gates until the repository-wide MSRV changes or CI adds a Rust 1.95+ Bevy lane.
- BEVY-080 intentionally did not implement the WGPU draw pipeline, GPU texture feedback application, Bevy `Handle<Image>` user textures, examples, multi-window routing, cursor icon feedback, or platform IME positioning. Keep those in BEVY-090+ or split follow-ups.

## Next Recommended Action

1. Start BEVY-090: implement the Bevy-native WGPU pipeline that consumes `ImguiExtractedRenderFrame`.
2. Preserve the BEVY-080 invariant that renderer code consumes owned `FrameSnapshot` data, not raw ImGui draw pointers.
3. Then run BEVY-100 for texture feedback and Bevy `Handle<Image>` user-texture interop.
