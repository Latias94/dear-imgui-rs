# Custom Backend Guide

This document is the starting point for downstream users who want to integrate
`dear-imgui-rs` with a platform, renderer, engine, or game framework that does
not already have a first-party crate in this workspace.

Prefer existing backend crates when they fit:

- platform input/windowing: `dear-imgui-winit` or `dear-imgui-sdl3`
- rendering: `dear-imgui-wgpu`, `dear-imgui-glow`, or `dear-imgui-ash`
- engine integration: `dear-imgui-bevy`

Write a custom backend when your application already owns the event loop,
windowing abstraction, swapchain, render graph, or texture allocator and cannot
use those crates directly.

## Backend Ownership Model

Keep these layers separate:

| Layer | Owns | Should not own |
| --- | --- | --- |
| `dear-imgui-rs` | safe `Context`, `Io`, frame lifecycle, widgets, draw data, texture descriptions | window handles, GPU devices, swapchains |
| platform backend | input events, display size, DPI, cursor, clipboard, IME, focus, optional viewports | GPU resource upload or draw commands |
| renderer backend | font/user texture upload, GPU pipeline state, draw command execution, optional renderer viewports | OS event translation |
| application / engine | event loop, lifecycle, device creation, swapchain acquisition, threading, packaging | hidden global backend ownership |
| `dear-imgui-sys::backend_shim` | selected self-contained official Dear ImGui C++ backend shims | framework-specific safe API or application packaging |

Framework-specific backends should live in their own backend crate or in the
application. Do not route a framework-specific feature through
`dear-imgui-rs`, and do not put framework-owned build logic into
`dear-imgui-sys`.

## Pick The Right Route

### Rust-native platform or renderer

Use this route for engines or crates that already expose Rust events and GPU
objects.

Examples:

- a custom winit-like event loop
- a render graph built on `wgpu`, Vulkan, OpenGL, DirectX, or Metal
- a game framework that exposes raw draw surfaces and input events

Use `dear-imgui-rs` APIs directly. Translate events into `Io`, consume a
Context-owned `PendingFrame`, return request-bound texture feedback, and draw
only the resulting `ReconciledFrame`. Use a move-only `FrameSnapshot` only when
rendering must leave the UI thread.

### Official Dear ImGui C++ backend

Use this route when the upstream Dear ImGui backend is the best integration
point.

Rules:

- Call upstream C++ backend functions only through repository-owned or
  crate-owned `extern "C"` wrapper symbols.
- Put self-contained official shims in `dear-imgui-sys` only when they depend
  only on Dear ImGui backend sources plus platform SDK headers/libraries.
- Put framework-specific official shims in the framework backend crate.
  `dear-imgui-sdl3` is the model for this ownership split.

### Application-only integration

If only one application needs the backend, keep it in the application first.
Promote it to a crate after the event, texture, and shutdown contracts are
stable enough to document.

## Minimal Frame Loop

Every integration has this shape:

```rust,no_run
use dear_imgui_rs::{
    Condition, Context,
    render::{PendingFrame, RendererConsumerError, SynchronousRendererConsumer},
};

# struct MyPlatformBackend;
# struct MyRendererBackend { consumer: SynchronousRendererConsumer }
# struct MyWindow;
# struct MyEvent;
# impl MyPlatformBackend {
#     fn new(_: &mut Context) -> Self { Self }
#     fn handle_event(&mut self, _: &mut Context, _: &MyEvent) -> bool { false }
#     fn prepare_frame(&mut self, _: &mut Context, _: &MyWindow) {}
#     fn prepare_render(&mut self, _: &mut Context, _: &MyWindow) {}
# }
# impl MyRendererBackend {
#     fn new(context: &mut Context) -> Result<Self, RendererConsumerError> {
#         Ok(Self { consumer: context.create_synchronous_renderer_consumer()? })
#     }
#     fn consumer(&self) -> &SynchronousRendererConsumer { &self.consumer }
#     fn render(&mut self, _frame: PendingFrame<'_>) -> Result<(), RendererConsumerError> {
#         // Implement the request/reconcile/draw sequence in the complete template below.
#         todo!()
#     }
# }
# let mut imgui = Context::create();
# let mut platform = MyPlatformBackend::new(&mut imgui);
# let mut renderer = MyRendererBackend::new(&mut imgui).unwrap();
# let window = MyWindow;
# let event = MyEvent;

// 1) Feed OS/framework events before the frame.
platform.handle_event(&mut imgui, &event);

// 2) Update display size, framebuffer scale, delta time, cursor/IME state.
platform.prepare_frame(&mut imgui, &window);

// 3) Build UI.
let ui = imgui.frame();
ui.window("Tools")
    .size([360.0, 200.0], Condition::FirstUseEver)
    .build(|| {
        ui.text("Custom backend");
    });

// 4) Let the platform backend apply post-UI state such as cursor shape or IME.
platform.prepare_render(&mut imgui, &window);

// 5) Close the frame with this renderer's synchronous capability. PendingFrame
// exposes requests and requirements, but no draw data.
let pending = imgui.render(renderer.consumer());

// 6) Move the pending capability into the renderer. A real backend must return
// exactly one result per request before it can obtain drawable commands.
renderer.render(pending).unwrap();
```

An owning backend should register its callback state as a Context attachment.
That lets explicit backend shutdown and Context-first teardown enter the same
idempotent state machine. Attachment hooks return `Result`; during final Context
drop, every peer in the current phase is notified, but any error or panic aborts
the process before a later destructive phase can violate resource ordering. Keep
external prerequisites such as windows, devices, queues, and Vulkan instances
alive until teardown has released every backend resource.

## Platform Backend Template

A platform backend translates your framework's events into `Io` and updates
per-frame platform state.

```rust,no_run
use dear_imgui_rs::{
    BackendFlags, Context, Key, MouseButton,
};
use std::time::Instant;

pub struct MyPlatformBackend {
    last_frame: Instant,
}

pub struct MyWindowInfo {
    pub logical_size: [f32; 2],
    pub framebuffer_scale: [f32; 2],
}

#[derive(Clone, Copy, Debug)]
pub enum MyEvent {
    MouseMoved { x: f32, y: f32 },
    MouseButton { button: MouseButton, down: bool },
    MouseWheel { x: f32, y: f32 },
    Key { key: Key, down: bool },
    Text(char),
    Focus(bool),
}

impl MyPlatformBackend {
    pub fn new(imgui: &mut Context) -> Self {
        imgui
            .set_platform_name("my-platform")
            .expect("platform name must not contain NUL bytes");

        let mut flags = imgui.io().backend_flags();
        flags.insert(BackendFlags::HAS_MOUSE_CURSORS);
        imgui.io_mut().set_backend_flags(flags);

        Self {
            last_frame: Instant::now(),
        }
    }

    pub fn handle_event(&mut self, imgui: &mut Context, event: &MyEvent) -> bool {
        let io = imgui.io_mut();
        match *event {
            MyEvent::MouseMoved { x, y } => io.add_mouse_pos_event([x, y]),
            MyEvent::MouseButton { button, down } => io.add_mouse_button_event(button, down),
            MyEvent::MouseWheel { x, y } => io.add_mouse_wheel_event([x, y]),
            MyEvent::Key { key, down } => io.add_key_event(key, down),
            MyEvent::Text(ch) => io.add_input_character(ch),
            MyEvent::Focus(focused) => io.add_focus_event(focused),
        }

        // Return whether your application should stop processing the event.
        // Many integrations use io.want_capture_mouse() / io.want_capture_keyboard()
        // after translating the event to make this decision.
        false
    }

    pub fn prepare_frame(&mut self, imgui: &mut Context, window: &MyWindowInfo) {
        let now = Instant::now();
        let delta = now.duration_since(self.last_frame).as_secs_f32();
        self.last_frame = now;

        let io = imgui.io_mut();
        io.set_delta_time(delta.max(1.0 / 1000.0));
        io.set_display_size(window.logical_size);
        io.set_display_framebuffer_scale(window.framebuffer_scale);
    }

    pub fn prepare_render(&mut self, imgui: &mut Context) {
        let _io = imgui.io();
        // Update OS cursor shape, IME enablement, clipboard hooks, etc.
    }
}
```

Platform checklist:

- Set `Context::set_platform_name`.
- Submit input through `Io::add_*` methods.
- Set `Io::set_display_size`, `Io::set_display_framebuffer_scale`, and
  `Io::set_delta_time` every frame.
- Set only the `BackendFlags` you truly support.
- If you store backend state in `BackendPlatformUserData`, clear it before the
  backend or window is destroyed.
- For IME, cursor, clipboard, and multi-viewport support, prefer matching the
  behavior of `dear-imgui-winit` or `dear-imgui-sdl3` before inventing new
  policy.

## Renderer Backend Reference

The executable
[`custom_renderer_headless.rs`](../dear-imgui/examples/custom_renderer_headless.rs)
is the canonical synchronous renderer reference. It has no windowing or GPU dependency and runs
the complete managed-texture contract:

```text
cargo run -j 1 -p dear-imgui-rs --example custom_renderer_headless
```

The example owns one `SynchronousRendererConsumer`, handles create, partial update, and destroy
requests with request-bound feedback, traverses only the resulting `ReconciledFrame`, rejects raw
callbacks before texture side effects, and performs the two-phase renderer reset during shutdown.
Use it as the starting implementation and replace only the CPU texture storage and recorded draw
operations with backend resources.

A synchronous renderer must preserve these invariants:

- Set `BackendFlags::RENDERER_HAS_TEXTURES` only when every `TextureRequest` receives exactly one
  `uploaded`, `destroyed`, `superseded`, or `retry` outcome.
- Retain exactly one `SynchronousRendererConsumer` for the renderer lifetime and reject a
  `PendingFrame` from another Context or consumer generation.
- Preflight `PendingFrame::draw_requirements` before applying texture side effects.
- Pair each resource with `TextureRequest::upload_identity`; identical retries reuse the existing
  binding, while a changed upload retires or updates the previous resource first.
- Seal a texture identity before acknowledging `Destroy`, and do not let late upload work revive
  it. Tombstones may be removed only after the matching completion watermark or a complete reset.
- Reconcile all feedback before reading draw commands because reconciliation assigns the effective
  `TextureId` used by those commands.
- Apply clip rectangles in framebuffer coordinates, honor vertex and index offsets, and either
  support raw callbacks with their documented unsafe contract or reject the frame during preflight.
- During shutdown, call `Context::prepare_renderer_texture_reset` while the complete resource map
  still exists. Release resources only after preparation succeeds, then commit the permit and drop
  the consumer. A failed preparation must leave the renderer intact and retryable.

For a detached render thread or render graph, use the separate move-only snapshot example described
below. Do not combine synchronous and detached consumer modes in one renderer instance.

## Threaded Or Render-Graph Backends

If rendering happens off the UI thread, do not send live `DrawData` references
across threads. Build a snapshot on the UI thread and send owned render work to
the renderer:

- Use `FrameSnapshot` when texture requests and viewport draw data need to cross
  threads.
- Keep snapshots move-only and call `FrameSnapshot::commit` exactly once after rendering. Dropping
  one uncommitted abandons its epoch and deliberately reissues unacknowledged destroys.
- Call `Context::poll_snapshot_completions` on the UI thread before creating later frames.
- Keep GPU resources in a renderer-owned map keyed by `SnapshotTextureId`; never retain a native
  `TextureData` pointer.
- Keep destroyed identities in the renderer until a complete idle-consumer reset; out-of-order
  snapshots may still carry an older upload after a later Destroy was processed.
- Keep the non-cloneable `DetachedRendererConsumer` on the UI thread. One Context permits one
  active consumer generation, and the capability kind is fixed when it is created.

The Bevy backend demonstrates the engine-owned form of this split: a main-thread registry serially binds one Context at a time, a Context-keyed mailbox moves snapshots into the render world, renderer namespaces and completion acknowledgements remain isolated by Context, and routes freeze camera identity for each extraction epoch. Its internal ECS and renderer resources are deliberately private implementation storage rather than a public custom-backend extension API.

One managed renderer must own one Context-local texture namespace. Multiple Contexts may share a
`SharedFontAtlas` only while they use legacy rendering. Managed atlas rendering requires exactly
one registered Context and a committed renderer reset before Context teardown; use a separate
atlas for each independently managed renderer.

## Multi-viewport Policy

Do not start with multi-viewport unless the single-window path is already
correct. Multi-viewport requires both platform and renderer support:

- The platform backend must create, move, resize, focus, title, and destroy OS
  windows requested by Dear ImGui.
- The renderer backend must render each viewport's draw data with the correct
  surface, framebuffer scale, and swapchain state.
- Backend user-data pointers must be cleared when viewports or renderer state
  are destroyed.
- Identify the main viewport through `Viewport::is_main()`. Its numeric ID is an upstream-private
  implementation detail and is not a cross-Context identity.
- Give every native viewport a backend-owned stable instance identity scoped to its Context, and key engine-side windows, cameras, surfaces, commands, feedback, callback faults, and teardown acknowledgements by that identity. Treat the numeric `ViewportId` as a mutable routing projection: IDs may repeat across live Contexts and docking may change an ID while preserving the native viewport.

Install the owning platform runtime first and the owning renderer runtime
second, before any secondary platform window exists. Treat each backend's
published state as one exclusive lease, not as independent fields that happen
to have familiar values:

- A platform lease includes an exact non-zero `BackendPlatformUserData`
  identity, the exact published `BackendPlatformName` pointer, its capability
  bits, IME state, the complete `Platform_*` callback table, monitor storage,
  and the main viewport's platform data and handles.
- A renderer lease includes an exact non-zero `BackendRendererUserData`
  identity, the exact published `BackendRendererName` pointer, core renderer
  capability bits, standard draw callbacks, render-state and texture-limit
  metadata, the five `Renderer_*` slots when viewports are enabled, and every
  viewport's renderer user data.

Use stable, non-zero-sized Rust allocations for backend identity. Comparing a
backend name by string contents is not ownership: another backend can publish
the same bytes from a different allocation. Likewise, marker callbacks used as
sentinel values must have link-distinct implementations; identical empty
functions may be folded to one address by LTO or identical-code folding.

Registration must preflight the complete lease and fail atomically when any
field is occupied. Publish fallible native resources before the lease, or keep
them in an armed rollback transaction until every field commits. The runtime,
not the caller, keeps callback-visible state at a stable address.

Every safe Rust entry and every direct C callback must bind the owner Context
and validate the complete installed lease before dereferencing backend data,
mutating a viewport, acquiring a surface, recording GPU work, or presenting.
Contain callback panics at the ABI boundary. The first identity, callback, or
capability drift is terminal for that attachment: latch one typed fault, revoke
the advertised capability, enter shutdown, and reject later work even if the
raw values are written back. Allowing restored values to resume would create an
ABA ownership hole.

Explicit shutdown runs in reverse ownership order: quiesce renderer callbacks,
release GPU resources, clear exact renderer-owned fields, then destroy platform
windows and clear exact platform-owned fields. Compare pointer and function
identity before clearing fields so foreign replacements survive; never infer
ownership from equal name bytes. Remove capability bits claimed by a departing
owner even after drift, because leaving them advertised after its resources are
gone is itself invalid state. Integrated upstream backends may need to destroy
per-viewport renderer resources while their platform windows are destroyed, but
must keep those callbacks alive until the whole native window-destruction phase
completes and release global renderer state before platform-global state.
Context-first drop invokes the same phases as a fail-stop fallback.

Retain `ContextAttachmentLease::handle()` with the platform owner. Before closing
an open frame, clearing callbacks, or destroying native windows, call
`Context::prepare_platform_attachment_release(&handle)`. An active renderer
attachment rejects preparation without mutating the Context; shut down the
renderer and retry. Perform platform cleanup through the permit's `context_mut()`
and call `commit()` only after cleanup succeeds. Dropping an uncommitted permit
keeps the exact platform generation attached for a retry. Do not release native
platform state first and then rely on `ContextAttachmentLease::detach()` to
detect a dependency; that check is intentionally too late for transactional
shutdown. `detach()` now returns `Result<bool, ContextAttachmentDetachError>`:
an error means the attachment is still live, so native cleanup must not continue.
Context-owned teardown already applies renderer-before-platform order.

An engine integration must not mutate ECS or a render world from `Drop`. Transfer the complete Context owner, callback backing storage, renderer consumer, in-flight mailbox, and release leases into an app-local retirement queue. The engine schedule then quiesces new frames, waits for render-world and viewport-entity acknowledgements, clears exact native fields, and finally destroys the Context. If the engine executor is already gone, retaining or intentionally leaking the complete owner is safer than releasing callback or GPU state early.

For first-party patterns, compare `dear-imgui-winit`,
`dear-imgui-sdl3`, `dear-imgui-wgpu`, `dear-imgui-glow`,
`dear-imgui-ash`, and `dear-imgui-bevy`.

### Aggregate callback ABI

Seven pinned `ImGuiPlatformIO` callbacks cross C++ with an aggregate passed or
returned by value. A direct Rust `extern "C"` callback is not a portable match
for a C++ callback slot here, especially with the MSVC x64 aggregate ABI. Never
write these generated struct fields directly and never transmute a Rust
function pointer into their types.

`dear-imgui-rs` routes all seven through the repository-owned C++ shim. The C++
thunk has the exact Dear ImGui callback type; its Rust-facing side uses only
pointers or out parameters:

| Dear ImGui slot | Rust raw setter | Rust-facing callback argument |
| --- | --- | --- |
| `Platform_SetWindowPos` | `set_platform_set_window_pos_raw` | `*const ImVec2` |
| `Platform_GetWindowPos` | `set_platform_get_window_pos_raw` | `*mut ImVec2` out parameter |
| `Platform_SetWindowSize` | `set_platform_set_window_size_raw` | `*const ImVec2` |
| `Platform_GetWindowSize` | `set_platform_get_window_size_raw` | `*mut ImVec2` out parameter |
| `Platform_GetWindowFramebufferScale` | `set_platform_get_window_framebuffer_scale_raw` | `*mut ImVec2` out parameter |
| `Platform_GetWindowWorkAreaInsets` | `set_platform_get_window_work_area_insets_raw` | `*mut ImVec4` out parameter |
| `Renderer_SetWindowSize` | `set_renderer_set_window_size_raw` | `*const ImVec2` |

The three `ImVec2`-returning getters use the same raw signature:

```rust,ignore
unsafe extern "C" fn(
    viewport: *mut dear_imgui_sys::ImGuiViewport,
    out_value: *mut dear_imgui_sys::ImVec2,
)
```

The three `ImVec2` input callbacks use this pointer signature:

```rust,ignore
unsafe extern "C" fn(
    viewport: *mut dear_imgui_sys::ImGuiViewport,
    value: *const dear_imgui_sys::ImVec2,
)
```

The work-area getter is identical to the getter form except that its out
parameter is `*mut dear_imgui_sys::ImVec4`. The pointers are valid only for the
duration of the callback, and callbacks must not unwind.

Prefer the typed `PlatformIo::set_platform_*` and
`PlatformIo::set_renderer_set_window_size` methods unless a backend genuinely
needs sys pointers. The typed input setters still install the C++ thunk, so
their by-value `ImVec2` callbacks never occupy the C++ slot directly; typed
getters retain the out-parameter form. Installation must target the active
context's `PlatformIo`. `clear_platform_handlers`,
`clear_renderer_handlers`, and `Context` destruction clear both Rust callback
registries and the shim's per-`PlatformIo` storage.

A backend that wraps an existing aggregate callback must keep a pure pass-through
in C++ or use the repository-owned C++ invocation bridge for the saved native
callback; Rust must not invoke the captured by-value slot directly. The shared
C++ shim address is not sufficient callback identity: a replacement installed
through the pointer setter keeps that same address while changing the shim's
stored pointer callback. Ownership-aware teardown must snapshot and restore
that stored callback as well as the raw slot.

The bulk clear methods are appropriate only when the caller owns the complete
corresponding table. A composable renderer shutdown must compare every
`Renderer_*` slot with its installed thunk and clear only matches. If another
backend replaced a slot, preserve that callback pointer, but revoke the departing
runtime's `RENDERER_HAS_VIEWPORTS` capability immediately; a replacement backend
must claim and advertise its own complete contract. Apply the same ownership
check before releasing viewport `RendererUserData`. Core renderer identity,
capabilities, draw callbacks, and metadata must pass the same check before any
viewport callback starts GPU work. WGPU, Glow, Ash, SDL3, and Bevy implement
this conditional teardown and sticky fail-closed behavior.

The shim must be compiled with the native artifact. Builds that omit native C++
hooks cannot install these callbacks, and compatible prebuilts declare
`platform-io-aggregate-hooks-v3`. The repository probe invokes all seven real C++
slots and runs on MSVC with both dynamic (`/MD`) and static (`/MT`) CRT profiles;
use an equivalent ABI probe when maintaining an out-of-tree native artifact.

## Build Script And Native Sources

Only add a `build.rs` when the backend compiles native code or must discover
native headers/libraries.

If you compile official Dear ImGui backend C++ sources:

- get upstream backend paths from `dear-imgui-sys` cargo metadata
- wrap C++ entry points in crate-owned `extern "C"` symbols
- do not expose upstream C++ names as your Rust ABI
- keep framework-specific include discovery in the backend crate

If a native dependency only needs include roots, prefer the shared helpers in
`dear-imgui-build-support` instead of open-coding pkg-config, vcpkg, and env-var
search order.

## Tests And Examples

Minimum useful coverage for a new backend:

- a compile-check example that creates a context, feeds one frame, and renders
  an empty draw list
- unit tests for event translation where the input API is pure Rust
- unit tests for texture status transitions
- feature-gated checks for every renderer/platform combination the crate
  exposes
- a documented smoke path for platform-specific packaging that cannot run in
  regular CI
- engine integrations should compile and test headless, default, render-only, native multi-viewport, and supported WASM profiles independently
- engine renderers should include a GPU readback test that composes the UI with custom post-processing, supported MSAA modes, HDR/LDR targets, and the engine's own UI layer

Before publishing a first-party backend crate, document:

- supported external crate versions
- single-window support level
- texture support level
- multi-viewport support level
- which object owns shutdown
- whether users may mix manual functions with RAII owner types

## Common Failure Modes

- Rendering without handling `TextureStatus::WantCreate`, which leaves the font
  atlas unbuilt.
- Setting `RENDERER_HAS_TEXTURES` before texture requests are fully handled.
- Feeding logical coordinates to renderer scissors instead of framebuffer
  coordinates.
- Keeping stale `BackendPlatformUserData`, `BackendRendererUserData`, or
  texture backend user-data after a window, renderer, or texture is destroyed.
- Acknowledging `TextureOp::Destroy` when CPU command recording finishes rather
  than after the GPU can no longer reference the resource.
- Dropping a renderer consumer before detached epochs have committed or been
  abandoned, then trying to attach another consumer while the old generation is
  still draining.
- Starting with multi-viewport before single-window lifecycle, resize, and
  texture cleanup are correct.
- Hiding application-owned lifecycle work inside a backend crate, especially on
  Android, iOS, and engine render graphs.
