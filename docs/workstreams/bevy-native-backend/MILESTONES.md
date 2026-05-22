# Bevy Native Backend Workstream — Milestones

Status: Draft
Last updated: 2026-05-22

## M0 — Scope And Evidence Freeze

Exit criteria:

- The Bevy-native backend decision is recorded in ADR form.
- Problem, target state, scope, non-goals, and closeout condition are explicit.
- The first supported Bevy target train is identified or explicitly left as a task.
- Initial task ledger and gate plan exist.

Primary evidence:

- `docs/adr/0001-bevy-native-imgui-backend.md`
- `docs/workstreams/bevy-native-backend/DESIGN.md`
- `docs/workstreams/bevy-native-backend/TODO.md`

## M1 — Core Lifecycle Proof

Exit criteria:

- `dear-imgui-rs` exposes or internally supports an engine-managed frame lifecycle.
- Multiple Bevy systems can draw into one ImGui frame without each owning frame begin/end.
- Snapshot and texture feedback contracts are confirmed or extended for render-world usage.
- Extension context requirements for ImPlot, ImNodes, node editor, and ImGuizmo are documented.

Primary gates:

- `cargo nextest run -p dear-imgui-rs <lifecycle-or-snapshot-tests>`
- targeted extension crate checks for any changed extension API.

## M2 — Bevy Backend Skeleton And Input

Exit criteria:

- `backends/dear-imgui-bevy` exists with clear experimental/version-coupled docs.
- Plugin registration and primary context resource/component shape compile.
- Bevy window/input messages feed Dear ImGui IO for the first primary-window proof.
- User UI systems run in `ImguiPrimaryContextPass` or the chosen equivalent schedule.

Primary gates:

- `cargo check -p dear-imgui-bevy --no-default-features`
- `cargo nextest run -p dear-imgui-bevy <input-or-lifecycle-tests>`

## M3 — Bevy-Native Renderer Proof

Exit criteria:

- Frame snapshots cross into `RenderApp` without borrowing ImGui-owned draw pointers.
- A Bevy-native WGPU pipeline prepares and renders Dear ImGui draw data.
- ImGui-managed texture requests receive render-world feedback and are applied on the main-world side.
- Bevy `Handle<Image>` user textures can be displayed in ImGui.

Primary gates:

- `cargo check -p dear-imgui-bevy --features render`
- targeted render/texture tests where possible.

## M4 — Examples And Ecosystem Composition

Exit criteria:

- A minimal embedded Bevy example demonstrates overlay UI.
- An editor shell example demonstrates dockspace plus a Bevy render-to-texture scene/game viewport.
- An ecosystem example demonstrates at least ImPlot plus one graph/gizmo extension in the same Bevy-managed ImGui frame.
- Docs explain when to use backend-only mode versus editor helper mode.

Primary gates:

- `cargo check -p dear-imgui-bevy --example simple`
- `cargo check -p dear-imgui-bevy --example editor_shell`
- `cargo check -p dear-imgui-bevy --example ecosystem`

## M5 — Closeout

Exit criteria:

- Gate set is recorded with fresh evidence.
- Workstream docs match shipped behavior.
- Remaining work is completed, explicitly deferred, or split into follow-on workstreams.
- `WORKSTREAM.json` status is updated.

Primary gates:

- targeted package gates for changed crates;
- broader workspace gate if feasible, otherwise a documented narrower closeout gate with rationale.
