# Bevy Backend Prelaunch Refactor

Status: Closed
Last updated: 2026-05-25

## Why This Lane Exists

`dear-imgui-bevy` is still pre-release, so the backend can accept breaking API and architecture
changes before users depend on its current behavior. Review against Bevy's camera/render model and
issue history from `bevy_egui` and `bevy_mod_imgui` found several product-shaping risks that should
be fixed before publishing the backend as a reusable integration.

## Relevant Authority

- ADRs:
  - `docs/adr/0001-bevy-native-imgui-backend.md`
- Existing docs:
  - `backends/dear-imgui-bevy/README.md`
  - `docs/workstreams/bevy-native-backend/`
  - `docs/workstreams/bevy-docking-multi-viewport-v1/`
- Reference repositories:
  - `repo-ref/bevy`
  - `repo-ref/bevy_egui`
- External issue themes:
  - `vladbat00/bevy_egui`: native multi-viewport, input capture, picking, IME, wasm, color tests,
    render target format mismatches.
  - `jbrd/bevy_mod_imgui`: multiple windows/viewports, scale factor changes, shutdown panics,
    Bevy image texture registration, and image sampler handling.

## Problem

The backend currently works for the primary product examples but leaves important integration
contracts implicit:

- overlay rendering is routed to the highest-order camera per render target instead of an explicit
  backend-owned camera policy;
- Bevy `Camera.viewport` is not applied to the ImGui render pass, so split-screen and embedded
  viewport rendering can paint outside the camera's intended region;
- ImGui color correction is derived only from `TextureFormat::is_srgb()`, which may be wrong for
  Bevy `CompositingSpace::Srgb` main textures using linear `Rgba8Unorm` storage;
- Bevy `Image` texture interop ignores the image sampler contract;
- input capture exists only as a resource snapshot, without ergonomic run conditions for gameplay,
  camera controllers, picking, and editor tools;
- runtime support boundaries for wasm IME/clipboard/file drop/gamepad/picking are not explicit
  enough for release notes;
- a pre-review `cargo nextest` run compiled successfully but hung in test discovery, so CI stability
  needs direct verification before closeout.

## Target State

When this lane closes:

- ImGui overlay render targets are explicit and stable, with a documented fallback for simple apps.
- The renderer honors Bevy camera viewport state or rejects unsupported camera shapes explicitly.
- Color correction accounts for Bevy's target format and compositing space, with focused tests.
- Bevy image texture registration has a clear sampler contract backed by tests and docs.
- Public input capture helpers make it straightforward to gate gameplay, camera, picking, and
  editor systems.
- README support matrices document current native, wasm, clipboard, IME, gamepad, file-drop, and
  picking behavior honestly.
- The Bevy backend test discovery issue is either fixed or converted into a documented blocking
  follow-up with an isolated repro.
- Fresh gates are recorded in `EVIDENCE_AND_GATES.md`.

## In Scope

- Breaking API changes in `dear-imgui-bevy`.
- Refactoring render extraction, target selection, draw preparation, and public marker components.
- Refactoring `dear-imgui-rs` core APIs when the Bevy backend needs a more correct engine-facing
  contract.
- Tests and examples that prove the changed contracts.
- README and workstream evidence updates.

## Out Of Scope

- Polishing every native window-manager edge case in Dear ImGui multi-viewport mode.
- Implementing full wasm runtime clipboard/IME/file-drop support unless the required fix is small
  and local.
- Supporting mobile multi-window behavior.
- Replacing Dear ImGui's own input capture semantics with a new policy model.

## Starting Assumptions

| Assumption | Confidence | Evidence | Consequence if wrong |
| --- | --- | --- | --- |
| Bevy camera viewport handling must be mirrored by custom overlay render passes. | High | Bevy core 2D/3D pass code calls `set_camera_viewport`. | If wrong, the viewport fix may be unnecessary but still harmless for camera-limited render passes. |
| `CompositingSpace::Srgb` requires gamma-aware ImGui output even when the main texture format is `Rgba8Unorm`. | Medium | Bevy `ExtractedCamera` documents sRGB-encoded shader output for this path. | If wrong, color tests should reveal the expected behavior and the implementation should follow Bevy's current renderer. |
| Users expect registered Bevy images to keep their Bevy sampler semantics. | High | `bevy_mod_imgui` issue history and Bevy asset API conventions. | If supporting full sampler semantics is too expensive, the backend must expose and document a narrower sampler contract before release. |
| A single Dear ImGui context remains the right first release shape. | Medium | Current backend and examples are designed around one context. | If multi-context support is required for correctness, split a separate workstream rather than hiding it inside this lane. |

## Architecture Direction

Prefer explicit ownership over heuristics. The Bevy backend should expose a small marker/config
surface that says which camera receives the ImGui overlay for each render target. That surface can
keep the current highest-order behavior as a compatibility fallback for small applications, but
examples and docs should use the explicit API.

Renderer changes should follow Bevy's render-world model: extract all main-world policy into owned
data, prepare draw batches without raw Dear ImGui pointers, and render with Bevy's `RenderCommand`
invariants for viewport, color target, and pipeline specialization.

Input helpers should remain policy hints. The backend should not consume Bevy messages, but it can
provide run conditions and small predicates so downstream systems do not duplicate capture logic.

## Closeout Condition

This lane can close when:

- all TODO slices are done or deliberately split,
- `review-workstream` has no blocking findings,
- fresh verification gates are recorded,
- README reflects the shipped behavior,
- and `WORKSTREAM.json` status is updated.

## Closeout Summary - 2026-05-25

The lane is closed. The shipped backend now exposes `render::ImguiOverlayCamera` for explicit
overlay camera ownership while preserving the previous highest-order fallback when no active camera
on a render target is marked. Render extraction carries Bevy `Camera.viewport` into render-world
draw preparation; rendering applies the camera viewport and clips ImGui scissors to that physical
region without scaling the full ImGui framebuffer into split-screen cameras.

Gamma selection now considers both target texture format and Bevy `CompositingSpace::Srgb`.
Texture bind groups now carry a sampler binding, so registered Bevy images use the prepared
`GpuImage` sampler while managed ImGui textures still honor ImGui's standard linear/nearest sampler
callbacks. Input capture exposes predicate methods and `imgui_wants_*` run conditions as policy
hints. README and examples document the explicit camera marker, dedicated local target directory,
texture sampler behavior, and current runtime support boundaries.

BPR-060 was resolved as an environment/artifact isolation problem rather than a backend test
deadlock: shared root `target/debug/deps` was large enough to make rustc startup and nextest
discovery appear hung. The closeout gates complete reliably with
`CARGO_TARGET_DIR=target/bevy-backend-prelaunch`.
