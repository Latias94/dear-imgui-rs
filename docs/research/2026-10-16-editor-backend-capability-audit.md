# Editor backend capability audit

Date: 2026-10-16

Baseline: `391171d`, with the working tree's existing native submodule changes.

Scope: the Winit and WGPU backends, core frame/snapshot contracts, selected SDL3
interop paths, and `dear-app`. This is a source-level capability assessment, not
a claim that every platform path was exercised. Three independent Codex scouts
examined platform ownership, input integration, and frame scheduling. No Rust
implementation, Cargo build, test run, or GPU experiment was performed.

## Assessment

The stack already supports external WGPU images, native secondary windows,
mixed-DPI coordinates, IME, managed texture reconciliation, and detached core
draw snapshots. The principal editor gap is the public contract connecting
those capabilities: a host cannot obtain a durable, same-frame account of
which native window submitted draws using which external texture binding.

Two existing behavior defects should be fixed independently of the larger API
work. Beyond those, prioritize native window identity, event outcomes, pointer
ownership, and WGPU submission reporting. Add engine-controlled recording and
detached rendering only through explicit ownership boundaries.

Priorities below describe editor integration impact. A missing optional feature
is not evidence that the current supported rendering path is broken.

## Existing capabilities to preserve

| Area | Verified foundation |
| --- | --- |
| External images | Opaque `ExternalTextureId`; register, replace, unregister, and renderer-local stale-handle validation. See `backends/dear-imgui-wgpu/src/renderer/external_textures.rs:11` and `src/texture/cache.rs:26`. |
| Native rendering | Owning Winit/SDL3 WGPU routes with secondary surfaces, MSAA/depth attachments, zero-size suspension, surface recovery, and ordered teardown. See `backends/dear-imgui-wgpu/README.md:139`. |
| Secondary submission evidence | `WgpuViewportFrameReport` separately records render submission and presentation API calls. See `backends/dear-imgui-wgpu/src/renderer/multi_viewport_runtime/trace.rs:10`. |
| Frame ownership | `ContextId`, renderer consumers, ordered `SnapshotEpoch`, managed texture feedback, and attachment teardown already establish ownership. See `dear-imgui/src/render/frame.rs:18` and `src/render/snapshot.rs:56`. |
| Detached draw data | `FrameSnapshot::viewports()` and per-viewport draw commands are already owned and thread-safe. See `dear-imgui/src/render/snapshot.rs:265` and `dear-imgui/src/context/frame.rs:411`. |
| Stable viewport precedent | Bevy already uses `ImguiViewportInstanceId`, distinct from mutable ImGui routing IDs. See `backends/dear-imgui-bevy/src/viewport/protocol.rs:9`. |
| DPI and input recovery | Native desktop coordinate conversion, mixed-DPI monitors, IME positioning, multi-window focus reconciliation, input release on viewport destruction, and secondary-window redraw wakeups already exist. See `backends/dear-imgui-winit/src/multi_viewport/coordinates.rs:510`, `runtime.rs:848`, and `events.rs:21`. |
| GPU recovery | `dear-app` has `GpuGeneration` and generation-bound external texture handles. See `dear-app/src/application.rs:19`. |

## Behavior defects

### B1 — P1: ordinary input does not request a frame in dear-app Wait mode

Execution path:

1. `Runner::window_event` dispatches the event to `Runtime::handle_event` in
   `dear-app/src/runtime/runner.rs:592`.
2. The platform receives the full window event at `runner.rs:178` and queues
   Dear ImGui input. Main-window Winit input translation does not request redraw.
3. `Runtime::handle_event` only requests redraw for resize and DPI changes at
   `runner.rs:205`.
4. Rendering only runs for `RedrawRequested` at `runner.rs:617`.
5. `RedrawMode::Wait` only schedules a redraw for a pending surface retry at
   `runner.rs:656`.

Reproduction condition: after the initial frame, use `RedrawMode::Wait` with no
independent redraw source. Move/click/type in a normal UI widget. Input is
queued, but the runtime does not request the next frame; resize or an explicit
application redraw can make the queued change appear.

Fix in the application scheduler: request a frame for relevant input/state
changes, and make future application invalidation/deadlines explicit. Keep
frame-rate policy outside the low-level input translator. Add a regression
test for input -> frame request -> admitted frame, not just initial control
flow or surface retry behavior.

### B2 — P2: single-window touch ownership survives focus loss

`WinitPlatformControl::note_single_touch` stores the active finger in
`backends/dear-imgui-winit/src/platform/input_events.rs:24`. The first-finger
state machine in `src/events.rs:172` rejects a new `Started` event while another
finger remains active. The single-window `Focused(false)` path at
`src/platform/input_events.rs:423` only sends Dear ImGui a focus event; it does
not clear the platform's `active_touch`.

Reproduction condition: finger 7 starts, the application loses focus without
receiving that finger's End/Cancelled event, focus returns, and finger 8 starts.
Dear ImGui can clear its button state while the platform still rejects finger 8.

Reuse `release_single_touch_in_current_context` from
`src/platform/input_events.rs:31` on single-window focus loss. Test the exact
sequence including a new finger after focus restoration. Multi-viewport input
already clears ownership during focus reconciliation at
`src/multi_viewport/runtime.rs:884`.

## Public contract gaps

### C1 — P1: durable native window identity and safe window access

The current numeric `Viewport::id()` is mutable during docking and explicitly
is not a persistent native allocation identity
(`dear-imgui/src/platform_io/viewport.rs:104`). Winit has validated internal
lookup functions, but `window_for_viewport` is crate-private
(`backends/dear-imgui-winit/src/multi_viewport.rs:113`). Public platform handles
are backend-defined raw values, not typed window capabilities.

The public viewport list is not a reliable historical lookup: hidden or
transitioning viewports can be absent while internal ownership remains live
(`backends/dear-imgui-winit/src/multi_viewport/registry.rs:320`). Native window
destruction also removes the current mapping.

Provide a window identity with Context/platform ownership and a never-reused
native allocation generation. Capture the current routing ID and native
`WindowId`/SDL ID alongside it when reporting work. Expose current lookup and
scoped window access through the platform owner. Historical identity must
remain useful after the actual window has been destroyed; an owned raw handle
or a retained `Arc<Window>` is not a substitute for that record.

Use the Bevy instance-ID model as precedent, without importing Bevy or merging
its platform runtime into Winit. SDL3 needs its own implementation of the same
identity semantics. Avoid creating a second general-purpose window registry.

### C2 — P1: window creation configuration and lifecycle observation

Secondary Winit windows use fixed `WindowAttributes` at
`backends/dear-imgui-winit/src/multi_viewport/callbacks/window.rs:119`, followed
by `create_window` at line 148. There is no application hook for icon, native
size limits, theme, application/class identity, or engine window-service
registration. `WinitPlatform::create_window_attributes` only returns generic
main-window defaults; the secondary path does not use it.

Add bounded configuration before creation and owned lifecycle notifications
after successful creation/before retirement. Preserve backend-owned geometry,
initial hidden state, callback publication, native policy, and teardown order.
User configuration must not silently override those invariants. Callback panic
or configuration failure needs the same containment as existing callbacks.

ImGui widget/window size constraints already exist; the missing capability is
native-window customization. SDL3 has the analogous fixed upstream creation
path and needs an explicit shim/configuration design if parity is required.

### C3 — P1: event results conflate routing, forwarding, and capture intent

`WinitPlatform::handle_event` and `handle_window_event` return an undocumented
`bool` (`backends/dear-imgui-winit/src/platform/input_events.rs:148`, line 190).
The result usually reflects current Context-wide capture intent, but:

- Main-window modifier updates return false at `input_events.rs:371`; secondary
  modifier updates return keyboard capture intent at `multi_viewport/events.rs:107`.
- `events.rs:16` can inject text while returning false if the key has no ImGui
  key mapping.
- A secondary close request sets `PlatformRequestClose` while returning false
  (`multi_viewport/events.rs:92`).

Return a small structured outcome that distinguishes native target ownership,
input/platform forwarding, capture intent, and redraw effects. Add a direct
`WindowId`-based entry for `ApplicationHandler::window_event`, so callers do not
need the private secondary `Window` or a synthetic full `Event` to route it.
Existing full-event routing is already capable of secondary dispatch.

Capture is a recent ImGui frame's intent, not proof of this event's final UI
hit. The engine should keep its own press/release recipient state and Scene/Game
input policy. The bundled FAQ explains the Enter-down/Enter-up focus transition
in `dear-imgui-sys/third-party/cimgui/imgui/docs/FAQ.md:204`.

### C4 — P1: pointer control is not coordinated with engine capture

Winit raw device handling is deliberately empty
(`backends/dear-imgui-winit/src/events.rs:224`). Raw motion has no native window
target and must not be blindly converted to ImGui absolute pointer input.
However, the stack lacks a scoped way to coordinate right-button camera flight
or Game-view pointer lock with ImGui's cursor updates, window focus, and window
destruction.

`NO_MOUSE_CURSOR_CHANGE` already permits application cursor ownership globally
(`src/platform/window_state.rs:297`). Build on that behavior with an explicit,
optional pointer-control capability bound to the exact native window. Define
grab failure, visibility ownership, restoration, and focus/destruction release.
The host retains raw motion routing and camera logic.

`dear-app::Application::event` only exposes main-window `WindowEvent`
(`dear-app/src/application.rs:275`), and its Runner has no application-facing
`device_event` path. Add that capability only if the convenience runtime is to
support this editor mode.

### C5 — P1: WGPU reports do not connect native targets to texture bindings

`WgpuViewportFrameReport` contains only sorted secondary viewport ID sets
(`backends/dear-imgui-wgpu/src/renderer/multi_viewport_runtime/trace.rs:10`).
It cannot identify the external image sampled by a submitted draw.

Track external texture use after validation, nonzero element-count filtering,
and framebuffer clipping in `src/renderer/draw.rs:200`. Associate the result
with the captured native target, UI frame/epoch, and submission outcome. This
is a many-to-many relation. Application asset IDs remain in the application's
mapping; the renderer must not invent engine asset identity.

Distinguish dependency use from visible pixels. Passing scissor tests and
submitting draws does not prove the image was unobscured or ultimately scanned
out. Raw callbacks can issue arbitrary GPU draws; mark their dependencies as
unknown or require explicit annotation instead of claiming complete inference.

Replacing a view keeps the same `ExternalTextureId`
(`src/texture/cache.rs:35`). Reports need a binding revision or equivalent
captured view identity to distinguish resize/double-buffer replacement. A
binding revision is not a content revision: engines must supply their own
content frame identity if they need it.

### C6 — P1: partial completion and skipped viewport outcomes are unavailable

`prepare_reconciled` pumps secondary windows, finishes the trace, and then
returns a pending fault (`src/renderer/multi_viewport_runtime/runtime.rs:1251`).
`finish_route_preparation` also returns only faults when a platform error is
present (`runtime.rs:261`). Successful work on other windows is not returned in
those failure cases. Normal zero-size, timeout, occlusion, and surface-recovery
skips are absent from the report rather than represented with a reason.

Return partial observations alongside faults, while keeping drawable prepared
capabilities unavailable after an invalid transaction. Preserve per-window
outcomes without changing the current fail-closed callback ownership behavior.

The main viewport has a separate boundary: `render_main` only records into an
application-owned render pass. Submission and presentation must be explicitly
reported by the host or remain unknown. Do not infer them from successful draw
recording.

### C7 — P1/P2: engine-controlled submission and detached WGPU consumption

The convenience route's `prepare` submits and presents secondary windows
internally (`src/renderer/multi_viewport_runtime/callbacks.rs:748`, line 777).
It works when producers of external images submit first, as the example does
in `examples/support/wgpu_multi_viewport_runtime.rs:580`. If the engine leaves
the current scene producer in an unsubmitted encoder until after `prepare`,
secondary windows can sample previous contents.

Provide an advanced recording path that makes producer -> UI -> presentation
ordering explicit, retaining the convenience route. Avoid parallel overlapping
APIs that bypass managed-texture reconciliation or native lifetime validation.
Do not solve queue ordering with a CPU wait every frame.

Core detached snapshots are already available. WGPU's public renderer still
consumes `PendingFrame`/`ReconciledFrame` and is deliberately not `Send`/`Sync`
(`src/renderer/render.rs:73`, `tests/integration_test.rs:59`). A render-thread
integration needs a detached consumer/draw executor and separately retained
native render targets; it cannot be created by making the live Context renderer
`Send`. Reuse existing texture feedback and epoch ordering rather than building
a second snapshot protocol. Preserve the detached protocol's rejection of
arbitrary live raw callbacks (`dear-imgui/src/render/snapshot.rs:703`).

`dear-app` can already render scene textures via `GpuApi::device()` and
`queue()` and submit before its overlay (`dear-app/src/application.rs:590`,
`runtime/surface.rs:156`). It lacks a shared-encoder hook, not the ability to
render scene content.

### C8 — P2: window state and platform capabilities are not exposed together

Core viewport geometry, framebuffer scale, DPI, flags, and Winit monitor
publication already exist. What is missing is a small platform-owned snapshot
for native window state and supported policy transitions. Preserve unknown
visibility/minimized/occlusion state; do not silently turn unavailable facts
into an authoritative visible/renderable answer.

Existing internal sources include focus/minimized callbacks at
`backends/dear-imgui-winit/src/multi_viewport/callbacks/window.rs:464` and line
479, and taskbar policy support at `src/multi_viewport/callbacks.rs:180`.
Expose only facts that the engine can use to schedule or explain work. Native
visibility is not equivalent to successful surface acquisition or pixel visibility.

Winit does not implement ImGui's OS parent/owner relationship and correctly
does not advertise `HAS_PARENT_VIEWPORT` (`src/platform/ownership.rs:37`). SDL3
implements it on supported platforms (`imgui_impl_sdl3.cpp:599`, line 1105).
Treat native owner behavior as a separate platform-gated feature; do not use an
embedded child-window API as a substitute.

### C9 — P2: optional editor services need clear ownership

- Winit forwards borrowed events, so the application already retains file-drop
  events. Add examples and appropriate invalidation, not an asset-management
  system inside the platform backend. Secondary automatic redraw currently
  excludes file hover/drop events (`multi_viewport/events.rs:21`).
- SDL3 callback handoff intentionally reduces unsupported pointer payloads to
  inert summaries. A dropped file loses its path, target, and coordinates after
  drain (`backends/dear-imgui-sdl3/src/callback_events.rs:123`, line 923).
  Offer bounded owned drop summaries or document a separate application queue;
  event-pump mode does not have this ownership problem.
- Winit/dear-app do not install a portable system clipboard backend. Windows
  can use ImGui's Win32 defaults; platforms using the fallback get an internal
  ImGui clipboard unless the application installs one. Core already exposes
  `ClipboardBackend` (`dear-imgui/src/context/clipboard.rs:45`); the fallback is
  explicit in the vendored `imgui.cpp:22021`. Prefer optional adapter wiring.
- The default WGPU image pipeline is a filterable, single-sampled 2D float
  texture (`backends/dear-imgui-wgpu/src/render_resources.rs:95`). Depth,
  integer-ID, cube/3D, unfilterable formats, and HDR previews require an engine
  conversion pass or explicit draw extension. Ordinary array layers/mips can
  use compatible 2D views. Keep asset preview interpretation outside ImGui.

### C10 — P2: dear-app has a deliberately limited host contract

`dear-app` rejects `VIEWPORTS_ENABLE` at startup and after application callbacks
(`dear-app/src/runtime/runner.rs:49`, `runtime/state.rs:181`). This is a documented
single-window recovery boundary, not evidence that Winit or WGPU lack secondary
windows. If the convenience runtime becomes an editor host, compose the existing
owning routes and extend recovery/close transactions rather than implementing
another multi-viewport runtime.

The private event proxy currently carries GPU faults/loss
(`dear-app/src/runtime/state.rs:207`). Applications can call `request_redraw`
through a window borrowed in a hook, but there is no narrow public wake handle
or application-defined next-frame deadline. Add those capabilities for background
asset loading, file watching, timers, and UI animations in Wait mode. Merge
invalidation with surface retry deadlines, and avoid waking continuously just
because an animation or job exists.

Core detached completion uses an mpsc channel and already exposes
`Context::poll_snapshot_completions`
(`dear-imgui/src/context/snapshot_hub.rs:635`). A host can wake its own event loop
after a completion. Keep that scheduler policy in the host; do not introduce a
Winit dependency or generic event-loop framework into the core.

## Refactoring boundaries

| Layer | Owns | Should not absorb |
| --- | --- | --- |
| Core | Context/frame/epoch identity, owned draw data, managed texture feedback, platform-neutral instance semantics if genuinely shared | Winit/SDL types, engine assets, native event loop or render graph |
| Platform backend | Native allocation identity, current routing map, scoped window access, configuration, lifecycle/state facts, event outcomes, pointer ownership coordination | Game input policy, asset import, scene cameras, GPU submission |
| WGPU | Texture binding revisions, validated draw dependencies, recording/submission outcomes, GPU resource retirement | Native window ownership, engine content-frame identity, proof of physical visibility |
| Host/engine | Scene/Game image rectangles and picking, input recipient policy, scene producer ordering, render graph, asset IDs, async invalidation | Reimplementing backend native ownership through raw pointers |
| dear-app | Optional convenience scheduling, device events, wake handles/deadlines, GPU recovery, eventual multi-window orchestration | A mandatory engine runtime or a replacement for custom host loops |

This follows the repository's existing boundaries in
`docs/workstreams/fearless-backend-refactor.md:267` and the independent Bevy/Winit
ownership decisions in `docs/adr/0001-bevy-native-imgui-backend.md` and
`docs/adr/0002-share-winit-native-host-capabilities.md`.

There is no reason to replace already-working IME, DPI, managed-texture, or
attachment state machines wholesale. Rework the missing seams, retire redundant
raw-handle interpretation, and keep one authoritative owner per resource.

## Delivery order and verification

| Step | Scope | Acceptance evidence |
| --- | --- | --- |
| 1 | B1/B2 and Winit README corrections | Wait input causes a frame request; focus loss permits a new touch; existing input/recovery tests remain valid. README no longer claims quantized pixel scrolling or synthetic left/right modifiers. |
| 2 | C1/C2/C3: native identity, scoped lookup, event outcome, bounded window configuration | Undock/merge/remap preserves allocation identity; destroy/recreate changes it; main/secondary routing agree; callbacks cannot retain live Context/window borrows or violate publication order. |
| 3 | C5/C6: texture use and complete reports | Two native windows use different engine textures, one texture appears in both, clipped images are excluded, view replacement is distinguishable, partial failure preserves prior observations, skips have explicit reasons. |
| 4 | C4: optional engine pointer control | Camera capture in either native window; failure reporting; restore on escape/focus loss/window close; no UI cursor overwrite; press/release recipient policy demonstrated. |
| 5 | C7: controlled recording, then detached execution | Producer commands precede every UI consumer; main acquisition can fail without erasing secondary results; delayed GPU submissions cannot observe overwritten uploads; detached texture feedback survives frame drop and out-of-order completion. |
| 6 | C8/C9 and optional dear-app expansion | Supported native owner/state features have platform-specific evidence; file-drop lifetime is owned; convenience scheduling supports explicit invalidation without forcing busy polling. |

Use existing native smoke entry points and route evidence classification from
`docs/renderer-route-evidence.md`. Add actual runtime observations for new
window/texture behavior; source-shape scanners are not a substitute. Keep CPU
regression tests small and meaningful. Use `cargo fmt` and focused, serial
`cargo nextest` runs when implementing; do not build the whole workspace or run
parallel Cargo jobs by default.

WGPU upload resources are currently isolated per render epoch
(`backends/dear-imgui-wgpu/src/frame_resources.rs:226`). An explicit submission
contract could later permit safe reuse. Treat that as a measured optimization,
not a reason to weaken isolation before completion is observable.

## Evidence limits and external references

- This audit did not modify Rust code or execute existing tests. Named tests
  are source evidence only. The two behavior defects need regression tests and
  runtime confirmation during implementation.
- Native submodules already had working-tree differences. They were inspected
  as present and were not updated, reset, staged, or otherwise changed.
- Winit 0.30.13 exposes window attributes and cursor grab itself, with
  platform-dependent support. The identified gap is integration and ownership,
  not an assertion that Winit lacks those underlying APIs:
  [Winit Window documentation](https://docs.rs/winit/0.30.13/winit/window/struct.Window.html).
- WGPU 30 `Queue::present` schedules presentation; `on_submitted_work_done`
  reports GPU work completion. Neither is proof that an image was visible to a
  user. Report stage-specific facts:
  [WGPU Queue documentation](https://docs.rs/wgpu/30.0.1/wgpu/struct.Queue.html).
- Wayland global-desktop multi-viewport positioning and portable Winit window
  opacity remain explicit platform limits; they are not repaired by adding a
  generic Rust window facade.
