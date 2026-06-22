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

Use `dear-imgui-rs` APIs directly. Translate events into `Io`, render
`DrawData`, and update `TextureData` requests.

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
use dear_imgui_rs::{Condition, Context};

# struct MyPlatformBackend;
# struct MyRendererBackend;
# struct MyWindow;
# struct MyEvent;
# impl MyPlatformBackend {
#     fn new(_: &mut Context) -> Self { Self }
#     fn handle_event(&mut self, _: &mut Context, _: &MyEvent) -> bool { false }
#     fn prepare_frame(&mut self, _: &mut Context, _: &MyWindow) {}
#     fn prepare_render(&mut self, _: &mut Context, _: &MyWindow) {}
# }
# impl MyRendererBackend {
#     fn new(_: &mut Context) -> Self { Self }
#     fn render(&mut self, _: &mut dear_imgui_rs::render::DrawData) {}
# }
# let mut imgui = Context::create();
# let mut platform = MyPlatformBackend::new(&mut imgui);
# let mut renderer = MyRendererBackend::new(&mut imgui);
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

// 5) Render the draw data. Renderer backends should take mutable draw data so
// texture status/TexID feedback can be written back.
let draw_data = imgui.render();
renderer.render(draw_data);
```

Keep the ImGui context alive longer than every platform and renderer object that
stores raw Dear ImGui pointers or backend user-data pointers.

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

A renderer backend owns GPU resources and draws `DrawData`.

```rust,no_run
use dear_imgui_rs::{
    BackendFlags, Context, TextureData, TextureId, TextureStatus,
    render::DrawData,
};

pub struct MyRendererBackend {
    next_texture: u64,
}

impl MyRendererBackend {
    pub fn new(imgui: &mut Context) -> Self {
        imgui
            .set_renderer_name("my-renderer")
            .expect("renderer name must not contain NUL bytes");

        let mut flags = imgui.io().backend_flags();
        flags.insert(BackendFlags::RENDERER_HAS_TEXTURES);
        flags.insert(BackendFlags::RENDERER_HAS_VTX_OFFSET);
        imgui.io_mut().set_backend_flags(flags);

        Self { next_texture: 1 }
    }

    pub fn render(&mut self, draw_data: &mut DrawData) {
        self.update_textures(draw_data);

        for draw_list in draw_data.draw_lists() {
            // Upload or bind draw_list vertex/index buffers.
            // For each draw command:
            // - bind the texture from the command's TextureId
            // - apply clip rect/scissor in framebuffer coordinates
            // - draw indexed triangles with the command's element count
            let _ = draw_list;
        }
    }

    fn update_textures(&mut self, draw_data: &mut DrawData) {
        let mut textures = draw_data.textures_mut();
        while let Some(mut texture) = textures.next() {
            match texture.status() {
                TextureStatus::WantCreate => self.create_texture(&mut texture),
                TextureStatus::WantUpdates => self.update_texture(&mut texture),
                TextureStatus::WantDestroy => self.destroy_texture(&mut texture),
                TextureStatus::OK | TextureStatus::Destroyed => {}
            }
        }
    }

    fn create_texture(&mut self, texture: &mut TextureData) {
        let id = TextureId::new(self.next_texture);
        self.next_texture += 1;

        // Allocate a GPU texture from texture.format(), texture.width(),
        // texture.height(), and texture pixel data.

        texture.set_tex_id(id);
        texture.set_status(TextureStatus::OK);
    }

    fn update_texture(&mut self, texture: &mut TextureData) {
        // Upload full texture data or update rects to the existing GPU texture.
        texture.set_status(TextureStatus::OK);
    }

    fn destroy_texture(&mut self, texture: &mut TextureData) {
        // Free the GPU resource for texture.tex_id() or texture.backend_user_data().
        texture.set_status(TextureStatus::Destroyed);
    }
}
```

Renderer checklist:

- Set `Context::set_renderer_name`.
- Set `BackendFlags::RENDERER_HAS_TEXTURES` only if `DrawData::textures_mut`
  requests are actually handled.
- Set `BackendFlags::RENDERER_HAS_VTX_OFFSET` if draw commands can use vertex
  offsets.
- On `WantCreate`, create the GPU texture, set `TextureData::set_tex_id`, then
  set `TextureStatus::OK`.
- On `WantUpdates`, upload the requested pixel data or update regions, then set
  `TextureStatus::OK`.
- On `WantDestroy`, free the GPU resource and set `TextureStatus::Destroyed`.
- Preserve or restore application GPU state unless the backend contract says the
  caller must reset state after rendering.
- Clip/scissor in framebuffer coordinates, not logical window coordinates.

## Optional Unified Traits

`dear-imgui-rs` exposes `ImGuiPlatform` and `ImGuiRenderer` as small common
traits. They are useful for application-local abstraction, but first-party
backend crates may expose richer APIs when their framework needs concrete
window, device, queue, command encoder, or render-pass types.

```rust,no_run
use dear_imgui_rs::{ImGuiRenderer, render::DrawData};

pub struct MyRenderer;

#[derive(Debug, thiserror::Error)]
#[error("renderer error")]
pub struct MyRendererError;

impl ImGuiRenderer for MyRenderer {
    type Error = MyRendererError;

    fn init(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn render(&mut self, _draw_data: &mut DrawData) -> Result<(), Self::Error> {
        Ok(())
    }
}
```

Use these traits when they make your application cleaner. Do not force them
when the backend naturally needs a concrete render pass or command buffer.

## Threaded Or Render-Graph Backends

If rendering happens off the UI thread, do not send live `DrawData` references
across threads. Build a snapshot on the UI thread and send owned render work to
the renderer:

- Use `FrameSnapshot` when texture requests and viewport draw data need to cross
  threads.
- Apply `TextureFeedback` back on the UI thread through `PlatformIo` before the
  next frame.
- Keep raw GPU handles in renderer-owned maps; keep Dear ImGui-side texture
  state in `TextureData`.

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

For first-party patterns, compare `dear-imgui-winit`,
`dear-imgui-sdl3`, `dear-imgui-wgpu`, `dear-imgui-glow`, and
`dear-imgui-ash`.

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
- Starting with multi-viewport before single-window lifecycle, resize, and
  texture cleanup are correct.
- Hiding application-owned lifecycle work inside a backend crate, especially on
  Android, iOS, and engine render graphs.
