# Compatibility Matrix

This document tracks compatibility across the workspace crates, upstream Dear ImGui, and key external dependencies. The root README shows the latest recommendations; this file keeps version history and compatibility policy.

For Apple-specific integration notes and the repository-owned iOS example
paths, see `docs/workstreams/apple-platform-support.md`.

## Versioning Policy

- Unified release train: all published `dear-*` crates in this workspace are versioned and released together under the same semver, so consumers can depend on a single minor across the board.
- Upcoming train: unified `v0.16.0-alpha.2`. Until it is published, test the candidate with a Git dependency on `main`; after publication, use an exact `=0.16.0-alpha.2` requirement rather than a broad crates.io `0.16` requirement.
- Current published prerelease: unified `v0.16.0-alpha.1` (use exact requirements such as `version = "=0.16.0-alpha.1"`).
- Current stable train: unified `v0.15.1` (use `version = "0.15"`).
- Previous train: unified `v0.14.1` (use `version = "0.14"`).
- Previous train: unified `v0.13.0` (use `version = "0.13"`).
- Previous train: unified `v0.12.0` (use `version = "0.12"`).
- Previous train: unified `v0.11.0` (use `version = "0.11"`).
- Previous train: unified `v0.10.4` (use `version = "0.10"`).
- Previous train: unified `v0.9.0` (use `version = "0.9"`).
- Previous train: unified `v0.8.0` (use `version = "0.8"`).
- Internal dependency constraints in this repo pin to the exact current prerelease (for example, `=0.16.0-alpha.2`). Mixing different release trains across our crates is unsupported.

## Release Candidate (0.16.0-alpha.2)

Core

| Crate           | Version | Upstream        | Notes                                     |
|-----------------|---------|-----------------|-------------------------------------------|
| dear-imgui-rs   | 0.16.0-alpha.2  | —               | Safe Rust API over dear-imgui-sys         |
| dear-imgui-sys  | 0.16.0-alpha.2  | ImGui v1.92.9b  | Docking branch via cimgui; three binding profiles |

Backends

| Crate             | Version | External deps           | Notes |
|-------------------|---------|-------------------------|-------|
| dear-imgui-wgpu   | 0.16.0-alpha.2  | wgpu = 30/29/28/27     | WGPU 30 default; native Winit/SDL3 multi-viewport; browser single-window |
| dear-imgui-glow   | 0.16.0-alpha.2  | glow = 0.17            | OpenGL 3.0+/ES 3.0+/WebGL 2 renderer; live sampler capability with restorative fallback |
| dear-imgui-ash    | 0.16.0-alpha.2  | ash = 0.38             | Native Vulkan renderer; shared Winit/SDL3 multi-viewport runtime |
| dear-imgui-winit  | 0.16.0-alpha.2  | winit = 0.30.13        | Winit platform backend |
| dear-imgui-sdl3   | 0.16.0-alpha.2  | sdl3 = 0.18.4, sdl3-sys 0.6 | SDL3 platform backend with optional official OpenGL3, SDLRenderer3, and SDLGPU3 renderers |
| dear-imgui-bevy   | 0.16.0-alpha.2  | Bevy = 0.19.0          | Bevy-native backend; default renderer and Bevy UI ordering, explicit advanced routes, Rust 1.95 minimum |

Utilities

| Crate     | Version | External deps | Notes |
|-----------|---------|---------------|-------|
| dear-app  | 0.16.0-alpha.2  | winit, wgpu 30 | Generation-aware application runtime |

Tooling

| Crate                    | Version | External deps | Notes |
|--------------------------|---------|---------------|-------|
| dear-imgui-build-support | 0.16.0-alpha.2  | ureq = 3.3    | Binding specification, build, package, and prebuilt helpers |

Extensions

| Crate               | Version | Requires dear-imgui-rs | Sys crate                    | Notes                                  |
|---------------------|---------|------------------------|------------------------------|----------------------------------------|
| dear-implot         | 0.16.0-alpha.2 | 0.16.0-alpha.2 | dear-implot-sys 0.16.0-alpha.2 | 2D plotting |
| dear-imnodes        | 0.16.0-alpha.2 | 0.16.0-alpha.2 | dear-imnodes-sys 0.16.0-alpha.2 | WASM-capable node editor |
| dear-node-editor    | 0.16.0-alpha.2 | 0.16.0-alpha.2 | dear-node-editor-sys 0.16.0-alpha.2 | Native-only; opt-in blueprints profile |
| dear-imguizmo       | 0.16.0-alpha.2 | 0.16.0-alpha.2 | dear-imguizmo-sys 0.16.0-alpha.2 | 3D gizmo |
| dear-file-browser   | 0.16.0-alpha.2 | 0.16.0-alpha.2 | — | State-owned ImGui UI + native dialogs |
| dear-implot3d       | 0.16.0-alpha.2 | 0.16.0-alpha.2 | dear-implot3d-sys 0.16.0-alpha.2 | 3D plotting |
| dear-imguizmo-quat  | 0.16.0-alpha.2 | 0.16.0-alpha.2 | dear-imguizmo-quat-sys 0.16.0-alpha.2 | Quaternion gizmo |
| dear-imgui-test-engine | 0.16.0-alpha.2 | 0.16.0-alpha.2 | dear-imgui-test-engine-sys 0.16.0-alpha.2 | UI automation and test runner |
| dear-imgui-reflect  | 0.16.0-alpha.2 | 0.16.0-alpha.2 | — | Session-owned reflection UI |

## 0.16 Architecture Contracts

The 0.16 train is not source-compatible with 0.15.x, and alpha.2 deliberately removes provisional alpha.1 APIs whose contracts were not sound enough to stabilize. The baseline is Dear ImGui v1.92.9b docking via cimgui, Rust 1.92 for the workspace, Rust 1.95 for the Bevy backend, WGPU 30 by default with explicit 29/28/27 routes, and Bevy 0.19. Alpha.2 migrations live in the `0.16.0-alpha.2` section of `CHANGELOG.md`; applications coming from 0.15.x must also apply the alpha.1 migrations.

The safe Rust layer intentionally breaks APIs that expose C++ lifecycle
protocols, wrong-context state, stale GPU handles, or platform-specific ABI
assumptions. Raw `*-sys` APIs remain the explicitly unsafe escape hatch, but
their pregenerated layout and native artifact profile must still match the
target.

### Runtime and feature matrix

| Route | 0.16 contract |
| --- | --- |
| Native core | Uses the target-selected `windows64` or `non-windows` pregenerated binding profile. |
| Native core build strategy | Source is the default. Enable `dear-imgui-rs/prebuilt` for verified release archives or `dear-imgui-rs/build-from-source` to force source; source wins when both are unified. |
| Native test engine | `test-engine` is source-only, implies `build-from-source`, and is excluded from prebuilt package profiles. |
| Native blueprint stack layout | Enable `dear-imgui-rs/stack-layout` directly or `dear-node-editor/blueprints`; this selects a distinct patched native artifact. |
| WASM core | Only `wasm32-unknown-unknown` is supported; it must explicitly enable `dear-imgui-rs/wasm` and use the `imgui-sys-v1` provider. WASI and Emscripten targets are rejected. |
| WASM stack layout / blueprints | Unsupported; `stack-layout` and `wasm` are rejected together. Use `dear-imnodes` for the WASM node-editor route. |
| WASM test engine / prebuilt | Unsupported; `test-engine` needs native source hooks and `prebuilt` contains native static libraries. |
| WGPU renderer | WGPU 30 is the default; 29, 28, and 27 are separate mutually exclusive features. Native Winit and SDL3 multi-viewport adapters are also mutually exclusive. |
| Glow renderer | Requires OpenGL 3.0+, OpenGL ES 3.0+, or WebGL 2. Sampler objects are selected from the live context; older desktop contexts use a temporary filter override that restores application texture parameters before the draw scope ends. |
| Ash renderer | Native Vulkan via Ash 0.38. Winit and SDL3 multi-viewport surface adapters are mutually exclusive and share one swapchain runtime. |
| Browser multi-viewport | Unsupported. Browser integrations render one main canvas. |
| Bevy default | `dear-imgui-bevy` enables `render` and `bevy-ui`; the primary Context automatically targets the unique eligible primary-window camera. |
| Bevy headless | `default-features = false` retains all Context schedules/lifecycle and primary-Context input/capture without RenderApp extraction; explicit multi-Context input routes require `render`. |
| Bevy render-only | `default-features = false, features = ["render"]` installs the renderer without Bevy UI ordering. |
| Bevy native multi-viewport | `multi-viewport` implies `render` and `bevy_winit`; each Context opts in explicitly. Secondary state is keyed by stable `ImguiViewportInstanceId`; the numeric `ViewportId` is only the current routing projection and may change during docking. Wayland falls back to host-window docking because global desktop client coordinates are unavailable; call `ImguiNativeViewportSupport::get(context_id)` for the per-Context runtime status. |
| Bevy WASM | `wasm` supports both default and headless builds; combining WASM with native `multi-viewport` is rejected. |

The six native safe extension crates forward build strategy through both the
core and their corresponding sys crate. Select the route on the safe crate:

| Safe extension | `prebuilt` | `build-from-source` | `wasm` |
| --- | --- | --- | --- |
| `dear-implot` | yes | yes | yes |
| `dear-implot3d` | yes | yes | yes |
| `dear-imnodes` | yes | yes | yes |
| `dear-imguizmo` | yes | yes | yes |
| `dear-imguizmo-quat` | yes | yes | yes |
| `dear-node-editor` | yes | yes | no, native-only |

When Cargo unifies `prebuilt` and `build-from-source`, source wins. Test Engine
is deliberately outside this table: it has no prebuilt or WASM route and always
enables the source-built core hooks.

Each Context owns its managed texture allocations and permits one non-cloneable
renderer-consumer generation. Synchronous renderers retain a
`SynchronousRendererConsumer`, consume a `PendingFrame<'ctx>`, return exactly one
request-bound outcome for every texture request, and receive the drawable
`ReconciledFrame<'ctx>`. Threaded or render-graph integrations retain a
`DetachedRendererConsumer`, move one pointer-free, non-cloneable `FrameSnapshot`
across the boundary, and consume it with `FrameSnapshot::commit`; dropping it
records an abandoned epoch rather than acknowledging destroy requests. Managed
texture retirement completes only after matching-generation destroy feedback
and the ordered completion watermark both permit reclamation.

Renderer resource maps keep a tombstone for every accepted Destroy identity
until a complete idle-consumer reset succeeds. A late Create or Update for a
tombstoned identity is ignored without GPU work or feedback, so out-of-order
snapshots cannot revive a released resource. Multi-Context `SharedFontAtlas`
use is legacy-only; managed atlas rendering requires exactly one registered
Context and a renderer-local opaque namespace. Use a separate atlas for each
independent managed renderer. Reusing a managed shared atlas also requires the
renderer to release its complete GPU texture map and commit the Context reset
before teardown; otherwise later Context registration fails closed until the
atlas is dropped and recreated.

Winit owns platform windows and the complete `Platform_*` callback table.
WGPU, Glow, and Ash multi-viewport wrappers own callback-visible renderer state
in stable internal storage, so callers no longer pin a renderer or uphold an
unsafe address-stability contract. WGPU attachment is safe. Glow attachment
remains unsafe because Rust cannot prove current-GL-context and share-group
lineage. Ash attachment remains unsafe because Rust cannot prove that the raw
Vulkan instance, physical device, surface, queues, and queue-family indices
share the declared device lineage. Platform callbacks are installed first; renderer
callbacks are installed before any secondary window exists. Registration
refuses foreign callbacks or user data instead of overwriting them. Explicit
shutdown releases the renderer runtime before the platform runtime; Context-
first teardown invokes the same renderer-resource and platform-window phases.
Explicit Winit platform shutdown takes `&mut Context`, closes an open frame
while callbacks are still attached, and only then destroys platform windows.
Engine integrations can use idempotent `Context::end_frame()` before their own
detachment sequence. Main viewport checks use `Viewport::is_main()` rather than
an exported numeric ID.

The Bevy backend applies the same ownership model across the engine boundary. A main-thread registry serially drives App-owned private Context passes; each pass injects a lifetime-bound `ImguiFrame<'_, P>` that cannot be installed in an ordinary Bevy schedule. Frame mailboxes, extracted snapshots, render routes, input capture, renderer generations, managed textures, diagnostics, and native viewport state remain keyed by `ContextId`. Render and input routes are separate declarations, so image targets never acquire window input implicitly. Each frame snapshot carries the immutable route epoch and viewport metrics used to create it, while cursor and IME feedback are arbitrated by Context input ownership. Context retirement waits for both RenderWorld and ECS acknowledgements; `Drop` only transfers complete owners into an app-local retirement queue and never reaches into another Bevy world.

### 0.16 alpha.2 public API disposition

The following ledger is the release target for provisional 0.16 APIs. `keep` preserves the public
concept, `rename` gives an existing concept a lifecycle-accurate name, `replace` removes a misleading
contract in favor of the listed one, `unsafe` deliberately retains an explicit native prerequisite,
and `delete` removes a surface whose safe contract cannot be upheld. Provisional alpha APIs do not
receive compatibility aliases unless the ledger explicitly says otherwise.

| Area | Provisional or current surface | Disposition | Alpha.2 contract |
| --- | --- | --- | --- |
| Synchronous frame | `RenderedFrame<'ctx>` exposes draw data before reconciliation | rename + replace | `PendingFrame<'ctx>` owns the unresolved request lease and exposes no draw data. Reconciliation consumes it and returns `ReconciledFrame<'ctx>`, the only drawable capability. No alias is retained. |
| Reconciled frame | `ReconciledFrame` is only completion proof | keep + strengthen | It owns the frame's Context borrow and draw-data access after successful request-bound reconciliation. |
| Renderer consumer | One `RendererConsumer` selects synchronous or detached mode on first use | replace | Separate `SynchronousRendererConsumer` and `DetachedRendererConsumer` capabilities are selected when created. A generation cannot change modes, and the old type is deleted without an alias. |
| Detached snapshot | `FrameSnapshot::commit` permits omitted request outcomes and defers some validation | keep + replace contract | Every snapshot request receives exactly one `uploaded`, `destroyed`, `superseded`, or `retry` outcome. Snapshot-local duplicate, foreign, and malformed outcomes fail at submission; remaining Context-state failures use a fallible completion path. |
| Renderer reset | `RendererTextureReset` is a two-phase permit whose `commit` count is routinely ignored | keep + replace result | Preparation remains fallible and inert when dropped. Successful `commit` returns `()` unless a future result carries an actionable state. |
| Context activation | `SuspendedContext::activate` returns only the owner on failure | replace | Fallible activation returns a typed reason together with the still-owned Context. Panic convenience is separately and explicitly named. Scoped binding continues to restore the previously active Context. |
| Direct draw state | `DrawListTextNoPixelSnapToken` assumes stack-like drop order for directly restored state | delete | `with_text_no_pixel_snap` is the public scope and uses a private guard that remains correct for every Safe Rust drop order. |
| Native stack scopes | Table, style, font, clip, and other native push/pop tokens | keep selectively | Native stack-backed tokens retain explicit LIFO panic contracts; closure helpers are the canonical teaching path. Low-level table parity remains, with every phase-dependent panic documented. |
| Texture pixels | `TextureData::set_data` silently accepts the overlapping prefix | replace | Exact full replacement and explicit subresource update APIs validate dimensions, rectangle, row pitch, format, and byte length before changing pixels, revision, status, or dirty rectangles. |
| Font atlas | One atlas surface mixes managed texture ownership with legacy renderer-built operation | replace | Managed and legacy capabilities are distinct. Safe owned font data exists only for a loader-bound, format-bounded path with a proven read contract; borrowed bytes, compressed data, custom loaders, and unproven parser combinations remain unsafe. |
| Docking identity | Display strings also act as persistent window and docking identity | replace | `WindowKey` separates the displayed title from the stable Dear ImGui identity, and one `DockspaceBuilder` owns the normal dockspace configuration path. |
| WGPU viewport frames | Context-finalizing `render_context*` aliases and separately coordinated trace/prepared state | delete + replace | One route-owned prepared viewport-frame transaction reconciles textures, dispatches secondary viewports, and aggregates faults before yielding the main-frame capability. Main-surface acquisition, submission, and presentation remain application-owned. |
| Ash viewport frames | Separately coordinated trace, prepared state, fault report, and retirement state | replace | One prepared viewport-frame transaction preserves command-buffer lineage and carries retirement state until a covering fence is acknowledged. Unsafe command recording and attachment remain explicit. |
| Glow lifecycle | Public `new_frame` plus inconsistent owned/external teardown names | delete + rename | Rendering recreates lost renderer objects transactionally while the required GL context is current. Owned teardown is `shutdown`; external-context variants keep names and safety text that expose current-context and share-group requirements. |
| Winit platform ownership | Normal multi-viewport use coordinates a base platform and a separate runtime | replace | One attached platform owner can be upgraded into viewport ownership and lends event-loop-scoped operations without `Option::take` choreography. `ActiveEventLoop` scope remains explicit. |
| SDL3 callback handoff | Example code owns callback payload copying and deferred delivery | replace + unsafe | `dear-imgui-sdl3` owns all backend-consumed pointer-bearing payloads and deferred faults. One raw enqueue boundary remains unsafe because SDL lends the event union only for the callback duration. |
| `dear-app` entry levels | `run_ui` or the full `Application` trait | keep + add | `run_ui` remains the smallest adapter. A fallible, exit-capable `FrameContext` closure is the middle level, and `Application` remains the full lifecycle level. Exit is control flow; the first actual error remains primary and shutdown runs exactly once. |
| Bevy installation | Configuration validation occurs through `Plugin::build` | keep + add | `ImguiAppExt::try_install_imgui` is the App-aware fallible transaction. `ImguiPlugin` remains an explicit panic convenience adapter over the same validation. |
| Bevy retirement | Applications retry synchronous `ImguiContexts::remove` each frame | replace | The existing retirement queue owns asynchronous removal and emits one Context-generation-keyed completion. The synchronous escape hatch is renamed to state its retry semantics. |
| Host configuration | `AppConfig`, `ImguiPluginConfig`, `ImguiContextConfig`, and platform/backend configuration | keep separate | Types remain host-specific until ownership, defaults, and failure behavior are genuinely identical. Only lossless conversions are added. |

### Lifecycle and ownership axes

These states are deliberately orthogonal; they are not collapsed into one cross-crate enum. Each
axis has one owner, and failure either rolls back to the listed retry state or transfers cleanup to
the listed terminal owner.

| Axis | Contract states | Unique owner and legal transition | Failure, retry, and terminal responsibility | Characterization evidence |
| --- | --- | --- | --- | --- |
| Native Context | `Alive -> Dropping -> NativeDestroyed` | `Context` owns the native allocation until teardown transfers only bounded cleanup capabilities to attachments. | Teardown is ordered and idempotent. After `NativeDestroyed`, no owner may dereference the old native pointer. | `dear-imgui/src/context/tests.rs` attachment and teardown tests |
| Context activation | `Suspended <-> Active`, with a scoped bound overlay | The owning Context or suspension error retains the allocation; a bound scope restores the exact previously active Context. | Foreign-active and open-frame conflicts return ownership and a typed reason; panic adapters must not be the only route. | Context binding and suspension tests in `dear-imgui/src/context/tests.rs` |
| Synchronous frame | `Idle -> InFrame -> PendingFrame -> ReconciledFrame -> Completed`, plus `Abandoned` | The Context frame borrow and synchronous consumer generation jointly own the epoch. Only reconciliation transfers pending work into drawable proof. | Dropping before reconciliation abandons the epoch and cannot enter renderer or platform callbacks. | `dear-imgui/tests/frame_lifecycle.rs` |
| Detached frame | `Snapshot -> Submitted -> Applied -> Drained`, plus `Abandoned` | A move-only snapshot owns one epoch until exactly one completion submission or drop. The Context applies submissions in generation order. | Invalid local outcomes reject submission; retries are reissued, superseded work advances without mutation, and reset waits for the completion watermark. | `dear-imgui/tests/snapshot_contract.rs` |
| Renderer consumer generation | `Absent -> ActiveSync` or `ActiveDetached -> Draining -> ResetPrepared -> Released` | One non-cloneable generation capability owns all request and feedback identities. Sync and detached modes are nominally distinct. | Dropping an uncommitted reset permit is inert. Outstanding epochs keep the generation draining and block replacement. | `frame_lifecycle.rs` reset tests and `snapshot_contract.rs` draining/watermark tests |
| Attachment graph | `PlatformAttached -> RendererAttached -> ReleasePrepared -> RendererReleased -> PlatformReleased -> Detached` | The Context owns the graph; typed platform and renderer runtimes own their respective attachment leases. | Renderer attachment blocks platform release. Failed preparation or teardown preserves the still-owned runtime for retry; Context-first teardown follows the same order. | Winit, SDL3, WGPU, Glow, and Ash lifecycle tests |
| Backend runtime | `Constructing -> Attached -> ShuttingDown -> Detached -> ResourceDropped` | The owning backend runtime publishes callbacks only after validation and retains callback-visible storage at a stable address. | Partial attach rolls back and returns the renderer where ownership was transferred. The first terminal contract fault is sticky through shutdown. | Backend multi-viewport contract tests; Ash attach-owner test |
| `dear-app` GPU generation | `Running(g) -> Recovering(g) -> Running(g+1)`, or `Failed -> Shutdown` | The runtime owns the main surface and GPU generation; application and ImGui Context identity survive recovery. | Old generation handles remain invalid. First real failure stays primary; shutdown runs once and is primary only when no earlier failure exists. | `dear-app/src/runtime/runner_tests.rs` and admission tests |
| Bevy Context retirement | `Ready -> Driving -> Teardown -> AwaitRenderWorld/AwaitViewportEcs -> Complete` | The main-world registry and existing `ImguiContextRetirements` queue own the Context generation until both worlds acknowledge it. | Acknowledgements may arrive in either order. Completion is generation-keyed and emitted once; `Drop` transfers ownership to retirement instead of destroying cross-world state. | `backends/dear-imgui-bevy/src/context/tests/lifecycle.rs` |

### PlatformIO callback ABI

Most cimgui functions are ordinary C ABI calls. Seven `ImGuiPlatformIO` slots are different because the underlying Dear ImGui C++ callback type passes or returns `ImVec2`/`ImVec4` aggregates by value: `Platform_SetWindowPos`, `Platform_GetWindowPos`, `Platform_SetWindowSize`, `Platform_GetWindowSize`, `Platform_GetWindowFramebufferScale`, `Platform_GetWindowWorkAreaInsets`, and `Renderer_SetWindowSize`.

Version 0.16 installs repository-owned C++ thunks in those slots and exposes only pointer/out-parameter C callbacks at the Rust boundary. This is required even when a callback appears to work on one compiler: MSVC, MinGW, and Clang may lower small aggregates differently. Windows CI invokes the real C++ slots for both MSVC `/MD` and `/MT`.

First-party platform and renderer backends now claim an exact Context-bound identity plus their complete callback, capability, main-viewport, monitor, draw-marker, and renderer-metadata contract. Each Rust and direct C entry validates that contract before native or GPU work; the first drift or callback panic is sticky, revokes the advertised capability, and enters terminal teardown even if raw values are later restored. Teardown compares exact data, name-pointer, and function identity so a foreign replacement, including the same name bytes from another allocation, is not cleared. Winit and SDL3 own their complete platform layer, while WGPU, Glow, Ash, SDL3, and Bevy apply the same rule to core and viewport renderer state.

### State-owner migrations

| 0.15 shape | 0.16 owner | Contract |
| --- | --- | --- |
| Global/thread-local reflection settings and `Ui::input_reflect` | `ReflectSession` plus `ui.inspector(&session)` | The session owns settings and map drafts; the one-frame inspector owns response and logical field paths. |
| File browser draw methods borrow `&dyn FileSystem` | `FileDialogState` owns a blocking or background filesystem capability | Workers cannot outlive borrowed caller state; `FileSystem::visit_dir` streams entries and cooperates with cancellation through `ScanVisit`. |
| Incremental/synchronous scan presets | `ScanPolicy::Blocking` or native `ScanPolicy::Background` | Background mode requires `Arc<dyn FileSystem + Send + Sync>` and never silently downgrades; browser/JS adapters stay on the caller thread. |
| Raw monitor vector access | `PlatformIo::set_monitors` | Dear ImGui's allocator owns the replacement vector storage. |
| Public Bevy backend config/status resources, `ImguiUi`, camera markers, helper setup, manual begin/end schedules, or viewport command queues | `ImguiPlugin` / `ImguiPluginConfig`, private `ImguiPass<P>` handles, frame-scoped `ImguiFrame<'_, P>`, `ImguiContextConfig`, and explicit `ImguiRenderRoute` / `ImguiInputRoute` declarations | Register UI systems with `ImguiAppExt::add_imgui_system`; the default single-camera path needs no marker, while advanced ownership is declared by Context and camera identity and renderer/pass/callback storage stays private. |
| Manual Bevy texture IDs or best-effort Context extraction | `ImguiTexture` RAII leases and retryable `ImguiContexts::remove` | Image mappings remain alive through in-flight snapshots, and Context removal waits for render-world plus viewport-ECS acknowledgement while unrelated Contexts continue. |
| Public Bevy input state/systems, writable capture fields, or directly copied viewport markers | Read-only `ImguiInputCapture` queries/run conditions and borrowed `ImguiViewportWindow` / `ImguiViewportCamera` accessors | The plugin owns input translation and marker creation; `aggregate()` returns a copyable capture snapshot while marker identity is read through `context_id()` / `viewport_id()`. |

### dear-app migration

`dear-app` now owns the Winit/WGPU runtime instead of exposing its internal
generations as a builder/callback protocol:

| 0.15 API | 0.16 API |
| --- | --- |
| `AppBuilder`, `RunnerConfig`, `RunnerCallbacks` | `AppConfig` plus one state-owning `Application` value |
| `run_simple`, `run_with_callbacks` | `dear_app::run(config, application)` for lifecycle-aware state, or `dear_app::run_ui(config, closure)` for UI-only state |
| `FrameContext<'ui, 'runtime>` | `FrameContext<'frame>` with one caller-visible frame lifetime |
| `enable` / `auto_dockspace` boolean combinations and a default full-viewport dockspace | Explicit `DockingConfig::{Disabled,ApplicationManaged,FullViewport}` modes; use `full_viewport()` or `application_managed()` constructors |
| Application cleanup distributed across callbacks | `Application::shutdown` runs exactly once before add-ons and the stable ImGui context are torn down |
| GPU rebuild could replace application/context state | The application, main window, and ImGui context survive; only generation-scoped GPU resources are replaced |

Use `run_ui` when a closure-captured state value only needs `&Ui`. Implement `Application::frame` and opt into the lifecycle hooks you need: `configure_imgui`, `initialized`, `event`, `prepare_frame`, `gpu_lost`, `gpu_recreated`, and `shutdown`. Rebuild application GPU resources from `gpu_recreated`. External texture handles carry a `GpuGeneration` and deliberately reject use after their generation is replaced.

### Binding and prebuilt provenance

Native bindings are checked in as two intentional ABI profiles:

- `windows64`: x86_64/aarch64 Windows MSVC and x86_64 Windows GNU
- `non-windows`: x86_64/aarch64/i686/armv7 Linux, x86_64/aarch64 macOS,
  x86_64/aarch64 iOS device/simulator, and x86/x86_64/aarch64/armv7 Android
  routes

The profiles are not interchangeable: Dear ImGui internal layout differs on
Windows even where aggregate sizes happen to match. WASM uses a third,
import-style binding artifact. `xtask verify-bindings` regenerates and compares
all supported profiles; arbitrary bindgen clang-argument overrides are rejected
for canonical artifacts.

The binding generator contract, formatter, allow/block lists, enum normalization, header shims, opaque types, provider name, and exact compatibility target facts all participate in the deterministic binding-spec hash. `dear-imgui-build-support` will ship on the same 0.16.0-alpha.2 train as every other publishable crate.

Source identity is package data rather than repository state. The exact 40-hex
cimgui and nested Dear ImGui revisions live in
`[package.metadata.dear-imgui-sources]`, survive `cargo package`, and are read by
normal builds and artifact packaging without invoking Git. Maintainer update,
pre-publish, and CI workflows are the only layers that compare those values to
submodule `HEAD`; they reject staged, unstaged, and untracked changes in either
source tree.

A `dear-imgui-sys` core native prebuilt is accepted only when `manifest.txt` exactly matches the
requested artifact profile:

- crate name and version
- target triple, static link type, and MSVC CRT (`md` or `mt`)
- normalized core artifact features, including `wchar32`,
  `platform-io-aggregate-hooks-v2`, `safe-demo-font-boundary-v1`, and optional
  `stack-layout` or `freetype`
- the exact 40-hex cimgui and nested Dear ImGui source revisions
- the deterministic binding-spec hash

Missing, duplicate, mismatched, or unknown manifest fields reject the artifact.
This applies equally to release downloads and explicit `IMGUI_SYS_LIB_DIR` or
`IMGUI_SYS_PREBUILT_URL` inputs. The independent `stack-layout` and `freetype`
feature dimensions produce four exact combinations: normal, freetype,
stack-layout, and stack-layout + freetype. Each has a distinct archive name,
cache identity, and manifest, so no build can silently consume another
combination.

High-level users select this route through `dear-imgui-rs/prebuilt`; direct sys
consumers use `dear-imgui-sys/prebuilt`. `test-engine` always forces a source
build and cannot be emitted by the package binary, even if Cargo also unifies
`prebuilt`.

### Raw binding migration

Version 0.16 deliberately narrows the raw surface where C ABI portability cannot be
proven:

- `IMGUI_VERSION` is renamed to `BINDING_VERSION`; there is no compatibility
  alias, so consumers must update the constant name.
- cimgui functions whose signatures contain C/C++ `va_list` are not generated.
  Rust callers should use the corresponding non-`V` variadic wrapper or format
  text before crossing the FFI boundary.
- `ImGuiDockNode` is pointer-only and opaque in generated bindings. Its private
  C++ layout is not a cross-target Rust ABI contract.

These changes are intentionally breaking. They prevent a binding generated on
one compiler/target from claiming compatibility with a different C++ ABI.

## History

Release Train 0.15 (previous)

- Latest patch: 0.15.1 (use `version = "0.15"`).
- All crates in this train share the 0.15 minor across the workspace.
- Core + backends aligned with Dear ImGui v1.92.8 (docking) via cimgui.
- Safe `Ui` APIs and scoped cleanup tokens bind operations to the receiver
  ImGui context and restore the previously current context, closing the
  multi-context lifecycle hole fixed for #31.
- The experimental Bevy backend ships on the same release train, targeting Bevy `0.19.0`.
- Normal source builds use checked-in pregenerated bindings by default; LLVM/libclang is only required for explicit binding regeneration.
- External dependencies baseline: wgpu 29, winit 0.30.13, glow 0.17, sdl3 0.18.4.
- Minimum supported Rust: 1.92 (workspace baseline).

Release Train 0.14 (previous)

- Latest patch: 0.14.1 (use `version = "0.14"`).
- All crates in this train share the 0.14 minor across the workspace.
- Core + backends aligned with Dear ImGui v1.92.8 (docking) via cimgui.
- The experimental Bevy backend ships on the same release train, targeting Bevy `0.19.0`.
- Normal source builds use checked-in pregenerated bindings by default; LLVM/libclang is only required for explicit binding regeneration.
- External dependencies baseline: wgpu 29, winit 0.30.13, glow 0.17, sdl3 0.18.4.
- Minimum supported Rust: 1.92 (workspace baseline).

Release Train 0.13 (previous)

- All crates unified to 0.13.0 across the workspace (use `version = "0.13"`).
- Core + backends aligned with Dear ImGui v1.92.8 (docking) via cimgui.
- `dear-node-editor` / `dear-node-editor-sys` are native-only in the first integration phase and coexist with the existing `dear-imnodes` wasm-capable node editor.
- `dear-imgui-sys` includes the stack layout ABI used by the node-editor blueprints example; prebuilts must declare `features=stack-layout`.
- External dependencies baseline: wgpu 29, winit 0.30.13, glow 0.17, sdl3 0.18.
- Minimum supported Rust: 1.92 (workspace baseline).

Release Train 0.12 (previous)

- All crates unified to 0.12.0 across the workspace (use `version = "0.12"`).
- Core + backends aligned with Dear ImGui v1.92.8 (docking) via cimgui.
- `dear-imgui-build-support` ships on the same `0.12.x` train as the published workspace crates.
- External dependencies baseline: wgpu 29, winit 0.30.13, glow 0.17, sdl3 0.17.
- Minimum supported Rust: 1.92 (workspace baseline).

Release Train 0.11 (previous)

- All crates unified to 0.11.0 across the workspace (use `version = "0.11"`).
- Core + backends aligned with Dear ImGui v1.92.7 (docking) via cimgui.
- `dear-imgui-build-support` moved into the unified release train.
- External dependencies baseline: wgpu 29, winit 0.30.13, glow 0.17, sdl3 0.17.
- Minimum supported Rust: 1.92 (workspace baseline).

Release Train 0.10 (previous)

- All crates unified to 0.10.4 across the workspace (use `version = "0.10"`).
- Core + backends aligned with Dear ImGui v1.92.6 (docking) via cimgui.
- External dependencies baseline: wgpu 29, winit 0.30.12, glow 0.16, sdl3 0.17.
- Minimum supported Rust: 1.92 (workspace baseline).

Release Train 0.9 (previous)

- All crates unified to 0.9.0 across the workspace (use `version = "0.9"`).
- External dependencies baseline: wgpu 28, winit 0.30.12, glow 0.16, sdl3 0.17.
- Minimum supported Rust: 1.92 (required by `wgpu` 28).

Release Train 0.8 (previous)

- Planned changes (subject to adjustment before release):
  - Core + backends remain aligned with Dear ImGui v1.92.5 and the same wgpu/winit/glow/sdl3 baselines.
  - Import-style WASM support (via `imgui-sys-v0` provider) for selected extension crates:
    - `dear-implot` / `dear-implot-sys`: 2D plotting on wasm.
    - `dear-imnodes` / `dear-imnodes-sys`: node editor on wasm.
    - `dear-imguizmo` / `dear-imguizmo-sys`: 3D gizmo on wasm.
    - `dear-imguizmo-quat` / `dear-imguizmo-quat-sys`: quaternion gizmo on wasm.
    - `dear-implot3d` / `dear-implot3d-sys`: 3D plotting on wasm.
  - New/updated `xtask` flows for building the wasm demo and import-style provider:
    - `wasm-bindgen-*` commands for core + extensions (ImPlot, ImPlot3D, ImNodes, ImGuizmo, ImGuIZMO.quat).
    - `web-demo [features]` to toggle which extensions are compiled into the web demo.
    - `build-cimgui-provider` to build the shared `imgui-sys-v0` provider (Emscripten).

Release Train 0.7 (previous)

- All crates unified to 0.7.0 across the workspace (use `version = "0.7"`).
- External dependencies baseline: wgpu 27, winit 0.30.12, glow 0.16, sdl3 0.16.
- Patch note: `dear-imgui-rs` has a core-only patch release at 0.7.1; other workspace crates remain at 0.7.0.

Release Train 0.6 (previous)

- All crates unified to 0.6.0 across the workspace
  - Core: dear-imgui-rs 0.6.0, dear-imgui-sys 0.6.0
  - Backends: dear-imgui-wgpu 0.6.0, dear-imgui-glow 0.6.0, dear-imgui-winit 0.6.0
  - Utilities: dear-app 0.6.0
  - Extensions: dear-implot 0.6.0, dear-imnodes 0.6.0, dear-imguizmo 0.6.0, dear-implot3d 0.6.0, dear-imguizmo-quat 0.6.0, dear-file-browser 0.6.0, dear-imgui-reflect 0.6.0
  - Sys crates: dear-implot-sys 0.6.0, dear-imnodes-sys 0.6.0, dear-imguizmo-sys 0.6.0, dear-implot3d-sys 0.6.0, dear-imguizmo-quat-sys 0.6.0
- dear-imgui-sys 0.6.x binds Dear ImGui v1.92.5 (docking) via cimgui
- New features:
  - New drag/drop flag and style color for improved drop target customization
  - Inherited all bug fixes and behavior changes from Dear ImGui v1.92.5
- External dependencies baseline: wgpu 27, winit 0.30.12, glow 0.16, sdl3 0.16
- Upgrade: change `version = "0.5"` to `version = "0.6"` in your Cargo.toml for all `dear-*` crates

### Backend & multi-viewport support notes (0.6.x)

- Winit + WGPU:
  - Multi-viewport support has experimental code paths in `dear-imgui-winit` + `dear-imgui-wgpu`, but is **not supported** in 0.6.x.
  - The `multi_viewport_wgpu` example is provided strictly as a **testbed** and is known to be unstable on some platforms (especially macOS/winit).
  - Do not rely on winit + WGPU multi-viewport for production use in this release train.
  - SDL3 + OpenGL3:
    - Supported via `dear-imgui-sdl3` (C++ `imgui_impl_sdl3.cpp` + `imgui_impl_opengl3.cpp`).
    - Multi-viewport: **supported** using the upstream SDL3 + OpenGL3 backend behaviour.
    - Example: `sdl3_opengl_multi_viewport` (`cargo run -p dear-imgui-examples --bin sdl3_opengl_multi_viewport --features multi-viewport,sdl3-opengl3`).
  - SDL3 + WGPU:
    - Supported via SDL3 platform backend (`dear-imgui-sdl3`) + Rust WGPU renderer (`dear-imgui-wgpu`).
    - A single-window example is provided: `sdl3_wgpu` (`cargo run -p dear-imgui-examples --bin sdl3_wgpu --features sdl3-platform`).
    - Multi-viewport for WebGPU remains **disabled** on this route, matching upstream `imgui_impl_wgpu` which currently does not implement multi-viewport.

Release Train 0.5 (previous)

- All crates unified to 0.5.0 across the workspace
  - Core: dear-imgui-rs 0.5.0, dear-imgui-sys 0.5.0
  - Backends: dear-imgui-wgpu 0.5.0, dear-imgui-glow 0.5.0, dear-imgui-winit 0.5.0
  - Utilities: dear-app 0.5.0
  - Extensions: dear-implot 0.5.0, dear-imnodes 0.5.0, dear-imguizmo 0.5.0, dear-implot3d 0.5.0, dear-imguizmo-quat 0.5.0, dear-file-browser 0.5.0
  - Sys crates: dear-implot-sys 0.5.0, dear-imnodes-sys 0.5.0, dear-imguizmo-sys 0.5.0, dear-implot3d-sys 0.5.0, dear-imguizmo-quat-sys 0.5.0
- dear-imgui-sys 0.5.x binds Dear ImGui v1.92.4 (docking) via cimgui
- New features:
  - Added `StyleColor::UnsavedMarker` for marking unsaved documents/windows
  - Inherited all bug fixes from Dear ImGui v1.92.4
- External dependencies baseline: wgpu 27, winit 0.30.12, glow 0.16
- Upgrade: change `version = "0.4"` to `version = "0.5"` in your Cargo.toml for all `dear-*` crates

Release Train 0.4 (previous)

- All crates unified to 0.4.0 across the workspace
  - Core: dear-imgui-rs 0.4.0, dear-imgui-sys 0.4.0
  - Backends: dear-imgui-wgpu 0.4.0, dear-imgui-glow 0.4.0, dear-imgui-winit 0.4.0
  - Extensions: dear-implot 0.4.0, dear-imnodes 0.4.0, dear-imguizmo 0.4.0, dear-implot3d 0.4.0, dear-imguizmo-quat 0.4.0
  - Sys crates: dear-implot-sys 0.4.0, dear-imnodes-sys 0.4.0, dear-imguizmo-sys 0.4.0, dear-implot3d-sys 0.4.0, dear-imguizmo-quat-sys 0.4.0
- dear-imgui-sys 0.4.x binds Dear ImGui v1.92.3 (docking) via cimgui
- External dependencies baseline: wgpu 27, winit 0.30.12, glow 0.16
- Upgrade: change `version = "0.3"` to `version = "0.4"` in your Cargo.toml for all `dear-*` crates

Release Train 0.3 (previous)

- BREAKING: Main crate renamed from `dear-imgui` to `dear-imgui-rs` (v0.3.0)
- All crates unified to 0.3.0 across the workspace
  - Core: dear-imgui-rs 0.3.0, dear-imgui-sys 0.3.0
  - Backends: dear-imgui-wgpu 0.3.0, dear-imgui-glow 0.3.0, dear-imgui-winit 0.3.0
  - Extensions: dear-implot 0.3.0, dear-imnodes 0.3.0, dear-imguizmo 0.3.0, dear-implot3d 0.3.0, dear-imguizmo-quat 0.3.0
  - Sys crates: dear-implot-sys 0.3.0, dear-imnodes-sys 0.3.0, dear-imguizmo-sys 0.3.0, dear-implot3d-sys 0.3.0, dear-imguizmo-quat-sys 0.3.0
- dear-imgui-sys 0.3.x binds Dear ImGui v1.92.3 (docking) via cimgui.
- dear-imgui-rs 0.3.x layers a safe API over the 0.3.x sys crate.
- External dependencies: wgpu 26, winit 0.30.12, glow 0.16
  - dear-file-browser (preview): optional features `imgui` (pure UI) and `native-rfd` (rfd backend) enabled by default. On wasm32, prefer `native-rfd` (Web File Picker). The ImGui UI enumerates the filesystem via `std::fs` and cannot list local files in the browser environment.

Release Train 0.2 (deprecated, yanked)

- dear-imgui-sys 0.2.x binds Dear ImGui v1.92.3 (docking) via cimgui.
- dear-imgui 0.2.x layers a safe API over the 0.2.x sys crate.
- Backends: wgpu (26), winit (0.30.12), glow (0.16).
- Extensions: dear-implot 0.2.x (with -sys 0.2.x), dear-imnodes 0.1.x, dear-imguizmo 0.1.x — all depend on dear-imgui/dear-imgui-sys 0.2.x.

## Upgrade Guidelines

- When bumping dear-imgui-sys to a new upstream ImGui, bump all -sys extensions (implot/imnodes/imguizmo) in lockstep and verify bindgen output/ABI.
- When bumping dear-imgui, check backends/extensions for API surface changes and update versions accordingly.
- Backend external deps (wgpu/winit/glow) often introduce breaking changes; track and bump backend crates even if core didn’t change.
