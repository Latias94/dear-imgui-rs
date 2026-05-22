# Bevy Native Backend Workstream

Status: Draft
Last updated: 2026-05-22

## Why This Lane Exists

`dear-imgui-rs` needs a Bevy integration that can serve two different users without splitting the ecosystem: game/application developers who want Dear ImGui inside an existing Bevy app, and tool builders who want to build standalone Bevy-powered editors with docking, scene viewports, plots, node graphs, gizmos, and diagnostic overlays.

The important architectural choice is that Bevy already owns the event loop, window state, WGPU device, render world, extraction boundary, camera ordering, and render target lifecycle. The backend must therefore be native to Bevy rather than wrapping the existing winit and WGPU backend crates.

## Relevant Authority

- ADRs:
  - `docs/adr/0001-bevy-native-imgui-backend.md`
- Existing docs:
  - `docs/workstreams/fearless-backend-refactor.md`
  - `docs/COMPATIBILITY.md`
- Reference repositories:
  - `repo-ref/bevy` — target Bevy source reference, with `v0.19.0-rc.2` as the first proof target.
  - `repo-ref/bevy_egui` — reference for ecosystem shape, not a direct implementation template.
- Existing crates:
  - `dear-imgui` — safe core API, context, IO, draw data, snapshot, texture feedback.
  - `backends/dear-imgui-wgpu` — renderer reference, but not the Bevy backend implementation path.
  - `extensions/dear-implot`, `extensions/dear-imnodes`, `extensions/dear-node-editor`, `extensions/dear-imguizmo` — ecosystem crates that must remain usable from Bevy UI systems.

## Problem

A thin Bevy plugin that calls `dear-imgui-winit` plus `dear-imgui-wgpu` would duplicate ownership already held by Bevy and would not fit Bevy's ECS and render architecture:

- Bevy owns winit and emits translated window/input messages through `bevy_window`, `bevy_input`, and `bevy_winit`.
- Bevy owns WGPU resources through `RenderDevice`, `RenderQueue`, `ViewTarget`, `ExtractedView`, and camera-driven render schedules.
- Bevy may run rendering through a separate `RenderApp` and supports pipelined rendering, so render data must cross an extraction boundary safely.
- Dear ImGui frame lifecycle is immediate-mode and context-oriented; Bevy UI code is schedule/system-oriented.
- Extension crates such as ImPlot, ImNodes, node editor, and ImGuizmo are built around `Ui` and extension-owned contexts; the Bevy backend must not strand them behind a separate integration surface.
- A standalone editor needs more than an overlay: render-to-texture scene viewports, texture interop, dockspace defaults, input capture policy, and multiple tool panels should compose in one frame.

## Target State

When this workstream closes, the repository should have a clear Bevy-native architecture and at least one vertical proof that validates the direction:

- `backends/dear-imgui-bevy` exists as the Bevy integration crate.
- The crate treats Bevy as the owner of winit, window state, WGPU device/queue, render targets, and camera render order.
- The main-world lifecycle exposes a Bevy-friendly frame API where input is collected, an ImGui frame is opened once, user systems and extension crates draw into that frame, and render output is closed once.
- The render-world path uses a thread-safe snapshot/feedback model rather than borrowing ImGui-owned draw pointers across Bevy's extract/render boundary.
- The renderer is implemented as a Bevy-native WGPU pipeline that prepares buffers, textures, bind groups, and draw commands from extracted snapshot data.
- Bevy `Handle<Image>` and render-to-texture outputs can be shown as ImGui textures.
- The default surface supports both embedded overlay usage and standalone editor shell usage without forking the backend.
- ImPlot, ImNodes, node editor, ImGuizmo, and other `Ui`-based extensions can be used inside `ImguiPrimaryContextPass` with the same ImGui context and frame.
- Remaining non-MVP features are explicitly split or deferred.

## In Scope

- Create the `dear-imgui-bevy` backend crate under `backends/`.
- Refactor `dear-imgui-rs` lifecycle APIs when needed to make Bevy ECS integration explicit and safe.
- Add an engine-friendly frame lifecycle abstraction that supports one frame across multiple Bevy systems.
- Use Bevy input/window messages to feed Dear ImGui IO.
- Add Bevy-native render pipeline resources and systems in `RenderApp`.
- Use `FrameSnapshot`, `TextureRequest`, and `TextureFeedback` as the cross-world render contract, extending them if Bevy requires missing data.
- Add Bevy texture interop for `Handle<Image>` and render-to-texture viewports.
- Provide examples for:
  - embedded debug UI overlay;
  - editor shell / dockspace / scene viewport;
  - ecosystem interop using at least ImPlot and one node/gizmo extension.
- Document which parts are stable API, experimental API, and Bevy-version-coupled internals.

## Out Of Scope

- Replacing `dear-imgui-winit`, `dear-imgui-wgpu`, or other first-party backend crates.
- Supporting Bevy versions older than the explicitly chosen target train in the first proof.
- Providing a full production editor in the first proof.
- Guaranteeing Dear ImGui docking multi-viewport OS-window support in the first proof.
- Rewriting all extension crates unless a Bevy lifecycle issue forces a small shared abstraction.
- Supporting non-WGPU Bevy render backends before the WGPU-native proof is complete.

## Starting Assumptions

| Assumption | Confidence | Evidence | Consequence if wrong |
| --- | --- | --- | --- |
| Bevy-native rendering is required for long-term editor viability. | High | Bevy owns `RenderApp`, `ExtractSchedule`, camera-driven `Core2d`/`Core3d`, `RenderDevice`, and `ViewTarget`. | If wrong, a wrapper backend could be cheaper, but it would still need clear ownership rules. |
| The first target should follow Bevy `v0.19.0-rc.2` architecture. | Medium | The local reference has tag `v0.19.0-rc.2`, WGPU `29.0.3`, and camera-driven schedules. | If release APIs shift, the workstream must pin a compatible Bevy version or split a migration task. |
| `FrameSnapshot` plus `TextureFeedback` is the right render-world contract. | High | The core crate already documents this as thread-safe render work and feedback application. | If missing fields are discovered, update the snapshot contract before building more backend code. |
| Extension crates can compose through a shared `Ui` borrow when frame lifecycle is explicit. | High | ImPlot, ImNodes, node editor, and ImGuizmo expose APIs around `dear_imgui_rs::Ui` and extension-owned contexts. | If an extension relies on hidden global state, add context binding/resource wrappers in the Bevy crate or extension crate. |
| A standalone editor can be built as a higher-level plugin on the same backend. | High | Bevy supports render-to-texture, camera targets, multiple windows, and UI schedules; ImGui supports dockspace and panels. | If editor needs new primitives, split them into `dear-imgui-bevy-editor` or optional features later. |
| The Bevy backend may need a higher MSRV than current core crates. | High | `repo-ref/bevy` declares Rust `1.95.0`; the workspace currently declares Rust `1.92`. | The backend may need to be excluded from default workspace gates or version-gated until the repo MSRV moves. |


## First Target Train And Compatibility Policy

The first implementation target is Bevy `v0.19.0-rc.2`, available in the checked-in `repo-ref/bevy` reference clone:

- Bevy package version: `0.19.0-rc.2`
- Reference tag: `v0.19.0-rc.2`
- Reference commit: `a389b928aee5906928a16a7d4e66cb02c7362901`
- Bevy MSRV: Rust `1.95.0`
- Bevy WGPU: `wgpu 29.0.3`
- Reference architecture: `RenderApp`, `ExtractSchedule`, camera-driven `Core2d` / `Core3d` schedules, `Core2dSystems` / `Core3dSystems`, `RenderDevice`, `RenderQueue`, `ExtractedView`, and `ViewTarget`.

`repo-ref/bevy_egui` remains a design reference for API ergonomics and ecosystem shape, but not a direct architecture target, because that repository is `bevy_egui 0.39.1` for Bevy `0.18.0` with `wgpu-types 27.0`.

Compatibility policy for the first proof:

- `dear-imgui-rs` core lifecycle and snapshot work must keep the workspace MSRV at Rust `1.92` unless there is a separate explicit MSRV decision.
- `dear-imgui-bevy` may be Bevy-version-coupled and may require Rust `1.95.0` while the root workspace remains on Rust `1.92`.
- If Cargo or CI cannot support the Bevy backend inside the default workspace gate while preserving core MSRV, the Bevy crate should be gated, excluded from broad workspace checks, or validated through a dedicated Bevy gate until the repository-wide MSRV is intentionally raised.
- Do not widen the first proof to Bevy `0.18` and `0.19` simultaneously. Add a follow-on compatibility task after the Bevy `0.19` shape is proven.

## Architecture Direction

### Ownership Boundaries

| Layer | Owns | Does not own |
| --- | --- | --- |
| `dear-imgui-rs` | ImGui context, IO APIs, frame lifecycle primitives, `Ui`, draw data, snapshots, texture requests and feedback. | Bevy schedules, Bevy render pipeline, Bevy texture handles. |
| `dear-imgui-bevy` | Bevy plugin surface, context resources/components, ECS schedules, input translation, Bevy-native renderer, Bevy image interop, editor helper features. | Winit event loop ownership, WGPU device ownership, generic non-Bevy renderer policy. |
| Bevy | Window lifecycle, input message production, WGPU device/queue, render graph/schedules, render targets, camera ordering. | Dear ImGui internal frame semantics. |
| Extension crates | Tool-specific `Ui` widgets and extension contexts such as plot/node/gizmo state. | Bevy scheduling and renderer integration unless an optional Bevy adapter is explicitly added. |

### Frame Lifecycle Model

The Bevy backend should make the frame boundary explicit:

```text
PreUpdate / input systems
  Bevy messages -> ImGui IO

ImguiBeginFrame
  apply pending texture feedback
  update display size / framebuffer scale / delta time
  start exactly one Dear ImGui frame per context

ImguiPrimaryContextPass
  user systems receive a frame-scoped Ui access point
  extension crates draw into the same Ui

ImguiEndFrame / PostUpdate
  render the context
  create FrameSnapshot
  store output/capture flags for Bevy input policy

ExtractSchedule
  move snapshots and texture/user-image maps into RenderApp

Render/Core2d/Core3d
  prepare buffers/textures/bind groups
  draw ImGui after scene post-process and before upscaling/presentation
```

The important invariant is: user systems draw into an already-open frame; they do not call `Context::frame()` or `Context::render()` themselves.

### ECS Surface Shape

The first public surface should stay narrow:

```rust
App::new()
    .add_plugins(DefaultPlugins)
    .add_plugins(ImguiPlugin::default())
    .add_systems(ImguiPrimaryContextPass, ui_system)
    .run();

fn ui_system(mut contexts: ImguiContexts) {
    let ui = contexts.primary_ui_mut()?;
    ui.window("Tools").build(|| {
        ui.text("Hello from Dear ImGui");
    });
}
```

Extension crates should compose in the same system or adjacent systems inside the same pass:

```rust
fn tools_ui(mut contexts: ImguiContexts, plots: Res<EditorPlotContext>) {
    let ui = contexts.primary_ui_mut()?;
    let plot_ui = plots.get_plot_ui(ui);
    // ImPlot / ImNodes / ImGuizmo calls happen here in the same ImGui frame.
}
```

### Render Architecture

The Bevy renderer should not instantiate `dear-imgui-wgpu::WgpuRenderer`. Instead it should:

- define Bevy render resources for pipeline, bind group layouts, samplers, texture maps, and per-target buffers;
- specialize pipelines by Bevy target format, HDR/compositing state, MSAA expectations, and bindless availability;
- convert extracted `FrameSnapshot` draw lists into Bevy GPU buffers;
- handle `TextureRequest` in render world and emit `TextureFeedback` for the next main-world frame;
- support Bevy `Handle<Image>` and render-to-texture views as user textures;
- render after camera post-processing and before final upscaling/presentation.

For Bevy `0.19-dev`, prefer camera-driven schedules over old-style render graph nodes:

```text
Core2d: after Core2dSystems::PostProcess, before upscaling
Core3d: after Core3dSystems::PostProcess, before upscaling
```

If the target Bevy version reintroduces or requires graph nodes, the same extracted render data should still be usable behind a different scheduling adapter.

### Editor Mode

Editor support should be a composition layer, not a separate backend:

- `ImguiPlugin`: core integration.
- `ImguiEditorPlugin`: optional defaults for dockspace, scene viewport, panels, and editor input policy.
- Editor viewports should use Bevy cameras rendering into `Image` handles, then register those handles as ImGui textures.
- Gizmo and node editor crates should use the same `Ui` frame and extension contexts.

### Ecosystem Interop Requirements

The backend must preserve a single shared ImGui frame so these can be used together:

- `dear-implot` plots in diagnostics/profiler panels.
- `dear-imnodes` or `dear-node-editor` graph editors for visual scripting/material graphs.
- `dear-imguizmo` scene viewport transform controls.
- `dear-imgui-reflect` inspector/editor panels.
- Future extension crates that only require `&Ui` and optional extension contexts.

This means the Bevy API should avoid creating isolated per-plugin frames. Instead, it should expose a frame-scoped access object and optional resources for extension contexts.

### Naming And Packaging

- Crate path: `backends/dear-imgui-bevy`
- Crate name: `dear-imgui-bevy`
- Rust import: `dear_imgui_bevy`
- First status: experimental / Bevy-version-coupled

A separate repository is deferred until the backend API is proven. A later `dear-imgui-bevy-editor` crate or feature may be split if editor helpers grow independently from the backend.

## Closeout Condition

This lane can close when:

- `dear-imgui-bevy` has a documented Bevy-native backend design and a working proof slice;
- any required `dear-imgui-rs` lifecycle/snapshot changes are implemented or split into explicit follow-ons;
- at least one embedded example and one editor-oriented example compile and demonstrate the intended model;
- at least one ecosystem interop example uses an extension crate inside the Bevy ImGui pass;
- targeted gates and broader gates are recorded with fresh evidence;
- remaining multi-window, docking viewport, wasm/mobile, and editor-product scope is completed, deferred, or split.
