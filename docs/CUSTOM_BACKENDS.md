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
idempotent state machine. Keep external prerequisites such as windows, devices,
queues, and Vulkan instances alive until that teardown has released every
backend resource.

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
use std::collections::HashMap;

use dear_imgui_rs::{
    BackendFlags, Context, TextureId,
    render::{
        RenderedFrame, RendererConsumer, RendererConsumerError, SnapshotTextureId,
        TextureOp,
    },
};

pub struct MyRendererBackend {
    consumer: RendererConsumer,
    textures: HashMap<SnapshotTextureId, TextureId>,
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

        let consumer = imgui.create_renderer_consumer()?;
        imgui.reset_renderer_texture_bindings(&consumer)?;
        Ok(Self {
            consumer,
            textures: HashMap::new(),
            next_texture: 1,
        })
    }

    pub fn render(
        &mut self,
        mut frame: RenderedFrame<'_>,
    ) -> Result<(), RendererConsumerError> {
        if frame.context_id() != self.consumer.context_id() {
            return Err(RendererConsumerError::ForeignContext {
                expected: self.consumer.context_id(),
                actual: frame.context_id(),
            });
        }
        let mut feedback = Vec::with_capacity(frame.texture_requests().len());
        for request in frame.texture_requests() {
            match request.operation() {
                TextureOp::Create { format, width, height, row_pitch, pixels } => {
                    // Allocate and upload a GPU texture from the owned request bytes.
                    let _ = (format, width, height, row_pitch, pixels);
                    let texture_id = TextureId::new(self.next_texture);
                    self.next_texture += 1;
                    self.textures.insert(request.texture(), texture_id);
                    feedback.push(request.uploaded(texture_id).expect("create is an upload"));
                }
                TextureOp::Update { format, width, height, rects } => {
                    let texture_id = self.textures[&request.texture()];
                    // Upload the owned update rectangles to this GPU texture.
                    let _ = (texture_id, format, width, height, rects);
                    feedback.push(request.uploaded(texture_id).expect("update is an upload"));
                }
                TextureOp::Destroy => {
                    if let Some(texture_id) = self.textures.remove(&request.texture()) {
                        // Destroy the GPU resource before acknowledging this request.
                        let _ = texture_id;
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
- On `Destroy`, free the GPU resource before returning `request.destroyed()`.
- Reconcile feedback before reading draw commands that depend on newly assigned texture IDs.
- During reset or shutdown, actually release renderer resources before calling
  `Context::reset_renderer_texture_bindings`.
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
- Keep the non-cloneable `RendererConsumer` on the UI thread. One Context permits one active
  consumer generation, and its first frame fixes that generation to synchronous or detached mode.

The Bevy backend is the best workspace example of this split.

## Multi-viewport Policy

Do not start with multi-viewport unless the single-window path is already
correct. Multi-viewport requires both platform and renderer support:

- The platform backend must create, move, resize, focus, title, and destroy OS
  windows requested by Dear ImGui.
- The renderer backend must render each viewport's draw data with the correct
  surface, framebuffer scale, and swapchain state.
- Backend user-data pointers must be cleared when viewports or renderer state
  are destroyed.

Install the owning platform runtime first and the owning renderer runtime
second, before any secondary platform window exists. A renderer owns only the
five `Renderer_*` slots and each viewport's renderer user data; it must not
replace `Platform_*` slots or foreign `RendererUserData`. Registration should
fail atomically when a slot is occupied. The runtime, not the caller, keeps
callback-visible state at a stable address. Explicit shutdown runs in reverse
ownership order: release renderer callbacks and GPU resources, then release
platform callbacks and windows. Context-first drop invokes those same ordered
attachment phases as a best-effort fallback.

For first-party patterns, compare `dear-imgui-winit`,
`dear-imgui-sdl3`, `dear-imgui-wgpu`, `dear-imgui-glow`, and
`dear-imgui-ash`.

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

The bulk clear methods are appropriate only when the caller owns the complete
corresponding table. A composable renderer shutdown must compare every
`Renderer_*` slot with its installed thunk and clear only matches; if another
backend replaced a slot, preserve it and its capability flag. Apply the same
ownership check before releasing viewport `RendererUserData`. The WGPU and Ash
helpers implement this conditional teardown and are the reference behavior.

The shim must be compiled with the native artifact. Builds that omit native C++
hooks cannot install these callbacks, and compatible prebuilts declare
`platform-io-aggregate-hooks`. The repository probe invokes all seven real C++
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
