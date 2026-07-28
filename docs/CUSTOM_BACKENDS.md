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
Context-owned `RenderedFrame`, and return request-bound texture feedback. Use a
move-only `FrameSnapshot` only when rendering must leave the UI thread.

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
    render::{RenderedFrame, RendererConsumer, RendererConsumerError},
};

# struct MyPlatformBackend;
# struct MyRendererBackend { _consumer: RendererConsumer }
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
#         Ok(Self { _consumer: context.create_renderer_consumer()? })
#     }
#     fn render(&mut self, _frame: RenderedFrame<'_>) -> Result<(), RendererConsumerError> {
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

// 5) Move the Context-borrowed lease into the renderer. A real backend must
// reconcile every texture result before reading dependent draw commands.
let frame = imgui.render();
renderer.render(frame).unwrap();
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

## Renderer Backend Template

A renderer backend owns one renderer consumer, all managed GPU resources, and each
`RenderedFrame` while it is reconciling and drawing that frame.

```rust,no_run
use std::collections::{HashMap, HashSet};

use dear_imgui_rs::{
    BackendFlags, Context, TextureId,
    render::{
        RenderedFrame, RendererConsumer, RendererConsumerError, SnapshotTextureId,
        TextureOp, TextureUploadIdentity,
    },
};

struct ManagedResource {
    texture_id: TextureId,
    upload: TextureUploadIdentity,
}

pub struct MyRendererBackend {
    consumer: Option<RendererConsumer>,
    textures: HashMap<SnapshotTextureId, ManagedResource>,
    destroyed: HashSet<SnapshotTextureId>,
    next_texture: usize,
}

impl MyRendererBackend {
    pub fn new(imgui: &mut Context) -> Result<Self, RendererConsumerError> {
        imgui
            .set_renderer_name("my-renderer")
            .expect("renderer name must not contain NUL bytes");

        let mut flags = imgui.io().backend_flags();
        flags.insert(BackendFlags::RENDERER_HAS_TEXTURES);
        flags.insert(BackendFlags::RENDERER_HAS_VTX_OFFSET);
        imgui.io_mut().set_backend_flags(flags);

        // A previous renderer must have completed its own reset transaction before a new
        // renderer consumer is attached to this Context.
        let consumer = imgui.create_renderer_consumer()?;
        Ok(Self {
            consumer: Some(consumer),
            textures: HashMap::new(),
            destroyed: HashSet::new(),
            next_texture: 1,
        })
    }

    pub fn render(
        &mut self,
        mut frame: RenderedFrame<'_>,
    ) -> Result<(), RendererConsumerError> {
        let consumer = self.consumer.as_ref().expect("renderer was shut down");
        if frame.context_id() != consumer.context_id() {
            return Err(RendererConsumerError::ForeignContext {
                expected: consumer.context_id(),
                actual: frame.context_id(),
            });
        }
        let mut feedback = Vec::with_capacity(frame.texture_requests().len());
        for request in frame.texture_requests() {
            match request.operation() {
                TextureOp::Create { format, width, height, row_pitch, pixels } => {
                    if self.destroyed.contains(&request.texture()) {
                        // This upload predates an accepted Destroy for the same opaque identity.
                        continue;
                    }
                    let upload = request.upload_identity().expect("create has an upload identity");
                    if let Some(existing) = self.textures.get(&request.texture())
                        && existing.upload == upload
                    {
                        feedback.push(
                            request
                                .uploaded(existing.texture_id)
                                .expect("create is an upload"),
                        );
                        continue;
                    }
                    // Allocate and upload a GPU texture from the owned request bytes.
                    let _ = (format, width, height, row_pitch, pixels);
                    let texture_id = TextureId::new(self.next_texture);
                    self.next_texture += 1;
                    if let Some(previous) = self.textures.insert(
                        request.texture(),
                        ManagedResource { texture_id, upload },
                    ) {
                        // Retire the replaced GPU resource before dropping its record.
                        let _ = previous;
                    }
                    feedback.push(request.uploaded(texture_id).expect("create is an upload"));
                }
                TextureOp::Update { format, width, height, rects } => {
                    if self.destroyed.contains(&request.texture()) {
                        continue;
                    }
                    let upload = request.upload_identity().expect("update has an upload identity");
                    let resource = &mut self.textures[&request.texture()];
                    if resource.upload == upload {
                        feedback.push(
                            request
                                .uploaded(resource.texture_id)
                                .expect("update is an upload"),
                        );
                        continue;
                    }
                    let texture_id = resource.texture_id;
                    // Upload the owned update rectangles to this GPU texture.
                    let _ = (texture_id, format, width, height, rects);
                    resource.upload = upload;
                    feedback.push(request.uploaded(texture_id).expect("update is an upload"));
                }
                TextureOp::Destroy => {
                    // Seal the identity before releasing the resource. Repeated Destroy requests
                    // remain successful, while late Create/Update requests cannot revive it.
                    self.destroyed.insert(request.texture());
                    if let Some(resource) = self.textures.remove(&request.texture()) {
                        // Destroy the GPU resource before acknowledging this request.
                        let _ = resource;
                    }
                    feedback.push(request.destroyed().expect("destroy is not an upload"));
                }
            }
        }

        // This validates Context, consumer generation, epoch, request kind, and revision.
        // It also updates draw-command TextureId values before the commands are read below.
        frame.reconcile_texture_feedback(feedback)?;

        for draw_list in frame.draw_data().draw_lists() {
            // Upload or bind draw_list vertex/index buffers.
            // For each draw command:
            // - bind the texture from the command's TextureId
            // - apply clip rect/scissor in framebuffer coordinates
            // - draw indexed triangles with the command's element count
            let _ = draw_list;
        }
        Ok(())
    }

    pub fn shutdown(&mut self, imgui: &mut Context) -> Result<(), RendererConsumerError> {
        let Some(consumer) = self.consumer.as_ref() else {
            return Ok(());
        };
        // Wait for backend GPU completion here, then validate the exact idle consumer while the
        // complete GPU map is still intact. If preparation fails, retain the consumer and retry
        // after outstanding work has completed; do not render new frames in between.
        let reset = imgui.prepare_renderer_texture_reset(consumer)?;
        self.textures.clear();
        let _invalidated = reset.commit();
        self.destroyed.clear();
        drop(self.consumer.take());
        imgui.poll_snapshot_completions()?;
        Ok(())
    }
}
```

Renderer checklist:

- Set `Context::set_renderer_name`.
- Create exactly one `RendererConsumer` and retain it for the renderer's lifetime.
- Set `BackendFlags::RENDERER_HAS_TEXTURES` only if every `TextureRequest` is handled and
  reconciled with request-bound feedback.
- Set `BackendFlags::RENDERER_HAS_VTX_OFFSET` if draw commands can use vertex
  offsets.
- On `Create` or `Update`, upload the owned bytes and return `request.uploaded(texture_id)`.
- Pair each resource with `request.upload_identity()`: return its existing ID for an identical
  retry, and retire or update the old GPU resource before accepting a changed identity.
- On `Destroy`, free the GPU resource before returning `request.destroyed()`.
- Record the destroy epoch before processing `Destroy`; ignore late `Create` or `Update` for that
  identity without allocating a resource or returning upload feedback.
- Reconcile feedback before reading draw commands that depend on newly assigned texture IDs.
- During reset or shutdown, call `Context::prepare_renderer_texture_reset` while the complete GPU
  map is still intact. Release the map only after preparation succeeds, then commit the permit.
- A managed `SharedFontAtlas` becomes reusable only after that reset commit. Dropping its Context
  first preserves the native binding and makes later Context registration return
  `SharedFontAtlasRendererReleasePending`; after releasing external GPU resources, drop and
  recreate the atlas rather than transferring the old renderer namespace.
- Keep explicit shutdown retryable. Failed preparation must retain the same consumer, renderer
  owner, and GPU map so outstanding epochs can finish before the caller retries.
- Prune a tombstone only after `SnapshotCompletionProgress::watermark()` reaches its destroy epoch,
  or after the matching consumer is idle and a complete reset succeeds. One accepted Destroy
  feedback is not proof that every older request has drained.
- Preserve or restore application GPU state unless the backend contract says the
  caller must reset state after rendering.
- Clip/scissor in framebuffer coordinates, not logical window coordinates.

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
- Keep the non-cloneable `RendererConsumer` on the UI thread. One Context permits one active
  consumer generation, and its first frame fixes that generation to synchronous or detached mode.

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
- Key every engine-side window, camera, surface, command, feedback, callback fault, and teardown acknowledgement by `(ContextId, ViewportId)`. Numeric viewport IDs may repeat across live Contexts.

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
`platform-io-aggregate-hooks-v2`. The repository probe invokes all seven real C++
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
