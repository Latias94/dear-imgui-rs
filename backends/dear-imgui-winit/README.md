# dear-imgui-winit

Winit platform backend for the `dear-imgui-rs` Rust crate. It wires winit input/events,
cursor handling and DPI awareness into Dear ImGui. Inspired by
`imgui-rs/imgui-winit-support`.

## Compatibility

| Item          | Version |
|---------------|---------|
| Crate         | 0.20.0  |
| dear-imgui-rs | 0.20.0  |
| winit         | 0.30.13 |

See also: [docs/COMPATIBILITY.md](https://github.com/Latias94/dear-imgui-rs/blob/main/docs/COMPATIBILITY.md) for the full workspace matrix.

## Quick Start

### Native window registration queries

`window_snapshots()` observes the main window and every live secondary registration, including
hidden viewports. `window_snapshot(window_id)` resolves Winit event routing, while
`window_snapshot_for_instance(instance_id)` resolves one exact native registration.

```rust,no_run
# use dear_imgui_winit::{WinitPlatform, WinitPlatformError};
# fn inspect_windows(platform: &WinitPlatform) -> Result<(), WinitPlatformError> {
for snapshot in platform.window_snapshots()? {
    let registration = snapshot.target.instance_id;
    let current_viewport = snapshot.target.viewport_id;
    platform.with_window(registration, |window| {
        window.request_redraw();
    })?;
    // Store snapshot in history without retaining the native Window.
    let _ = (snapshot, current_viewport);
}
# Ok(())
# }
```

The registration identity survives viewport routing-ID changes and main-window multi-viewport
enable/disable cycles. Detach/reattach creates a new identity, even for the same host window.
Logical editor viewports may survive native-window replacement, so keep their identity separate.
Snapshots own only metadata and do not extend window or Context lifetimes. `with_window` validates
the current owner and lends the native window for one closure; its borrow cannot escape.
Queries never drain deferred callback faults.

### Secondary creation policy and native observations

With `multi-viewport`, install `WinitViewportWindowConfig` before creating secondary windows:

```rust,no_run
# use dear_imgui_winit::{WinitPlatform, WinitPlatformError, WinitViewportWindowConfig};
# use winit::{dpi::PhysicalSize, window::Theme};
# fn configure(platform: &mut WinitPlatform) -> Result<(), WinitPlatformError> {
platform.set_viewport_window_config(WinitViewportWindowConfig {
    min_inner_size: Some(PhysicalSize::new(320, 180)),
    max_inner_size: Some(PhysicalSize::new(3840, 2160)),
    theme: Some(Theme::Dark),
    ..Default::default()
})?;
platform.set_viewport_window_configurator(|info, config| {
    // Tailor a future registration using detached viewport and native-owner metadata.
    let _ = info.target;
    config.resizable = true;
    Ok(())
})?;
# Ok(())
# }
```

The bounded policy supports icon, physical client-size limits, theme, resizability and native
transparency. It cannot replace backend geometry, hidden initial visibility, native sidecars or
owner pointers. Policy changes affect future windows. Invalid limits and configurator errors
reject creation before native allocation; configurator panics are contained at the callback
boundary. A reserved instance identity is never reused after failure.

`drain_window_lifecycle_events()` returns `Created(snapshot)` after publication and
`Retired(target)` once when registry access ends. Main-window MV toggles and routing-ID remaps
emit no additional registration events. Retirement does not imply that GPU work or a retained
renderer window lease has finished. Drain this metadata queue regularly; callback faults have a
separate queue.

Snapshots include `native_state` and `capabilities`. Optional native state stays `None` when
Winit cannot query it; requested transparency and icon settings are never presented as observed
native facts. Policy support distinguishes `Dynamic`, `CreationOnly`, and `Unsupported`.
X11 transparency is creation-only, and its visibility/theme/resizability observations remain
unknown where Winit lacks native queries. Wayland desktop positioning and secondary viewports
remain unsupported.

Windows applies Dear ImGui `ParentViewportId` using top-level native ownership, including
dynamic owner changes, and advertises `HAS_PARENT_VIEWPORT`. Owner cycles and unavailable
registrations are rejected. Renderer work using an owned window must retain its owner chain
until presentation/discard. macOS/X11 report native owner policy as unsupported and leave the
default main-parent hint unapplied; explicit secondary-parent requests return a typed error.
Per-window icons are unsupported on macOS.

Minimal flow with winit 0.30 ApplicationHandler-style loops:

```rust,no_run
use dear_imgui_rs::{Context, Condition};
use dear_imgui_winit::{WinitPlatform, HiDpiMode};
use winit::{event::WindowEvent, event_loop::{ActiveEventLoop, EventLoop}, window::WindowId};

struct App { /* ... */ }

impl winit::application::ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) { /* create window + ImGui + WinitPlatform */ }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let window = /* get your window */;
        // 1) forward the window-local event to ImGui
        self.imgui
            .platform
            .handle_window_event(&mut self.imgui.context, &window, &event)
            .expect("Winit platform contract changed");

        match event {
            WindowEvent::RedrawRequested => {
                // 2) per-frame prep
                self.imgui
                    .platform
                    .prepare_frame(&mut self.imgui.context, &window)
                    .expect("Winit platform contract changed");
                let ui = self.imgui.context.frame();

                // 3) build UI
                ui.window("Hello").size([400.0, 300.0], Condition::FirstUseEver).build(|| {
                    ui.text("ImGui + winit");
                });

                // 4) update OS cursor from UI
                self.imgui
                    .platform
                    .prepare_render(&ui, &window)
                    .expect("Winit platform contract changed");

                // 5) close the frame with your renderer's synchronous consumer, then let that
                // renderer reconcile every texture request before it reads draw commands
                let pending = self
                    .imgui
                    .context
                    .render(
                        self.renderer
                            .renderer_consumer()
                            .expect("renderer must remain attached"),
                    );
                self.renderer.render(pending /*, target-specific arguments */);
            }
            _ => {}
        }
    }
}
```

APIs of interest:
- `WinitPlatform::new(&mut Context) -> Result<WinitPlatform, WinitPlatformError>`
- `WinitPlatform::attach_window(Arc<Window>, HiDpiMode, &mut Context) -> Result<(), WinitPlatformError>` — the platform retains the exact shared window allocation until detach or Context teardown, so IME callbacks cannot outlive it
- `WinitPlatform::set_hidpi_mode(HiDpiMode) -> Result<(), WinitPlatformError>` — configure primary-window scaling before attaching multi-viewport support
- `WinitPlatform::handle_window_event(&mut Context, &Window, &WindowEvent) -> Result<bool, WinitPlatformError>` — for `ApplicationHandler::window_event`
- `WinitPlatform::handle_event(&mut Context, &Window, &Event<T>) -> Result<bool, WinitPlatformError>` — for closure-style `EventLoop::run`; events for another `WindowId` return `Ok(false)` without being dispatched
- `WinitPlatform::prepare_frame(&mut Context, &Window) -> Result<(), WinitPlatformError>` — updates timing and native platform state before `Context::frame`
- `WinitPlatform::prepare_render(&Ui, &Window) -> Result<(), WinitPlatformError>` — updates OS cursor and IME state after UI construction
- `WinitPlatform::detach_window(&mut Context) -> Result<Arc<Window>, WinitPlatformError>` — clears winit-owned IME hooks before a window is destroyed while the context remains alive

## DPI / HiDPI

`HiDpiMode` controls how the backend derives the framebuffer scale:
- `Default`: use winit’s `window.scale_factor()` directly.
- `Rounded`: round the winit factor to the nearest integer to avoid blurry scaling.
- `Locked(f64)`: force a custom factor (e.g. 1.0).

Choose the mode before calling `WinitPlatform::enable_viewports`. While native viewports are
enabled, `set_hidpi_mode` and `attach_window` return
`WinitPlatformError::RuntimeConfigurationLocked` without modifying the main-window or coordinate
state. This preserves the single platform-native desktop coordinate model shared by primary and
secondary viewports.

When DPI changes (`ScaleFactorChanged`), the backend updates `io.display_size` and
`io.display_framebuffer_scale`. Single-window mode also adjusts the stored mouse position to keep
the pointer location consistent across scales; a live multi-viewport runtime already receives
absolute mouse positions in its native desktop coordinate space.

Helpers are provided if you pass winit logical values around and need the same
coordinates ImGui uses:
- `scale_size_from_winit(&Window, LogicalSize<f64>) -> LogicalSize<f64>`
- `scale_pos_from_winit(&Window, LogicalPosition<f64>) -> LogicalPosition<f64>`
- `scale_pos_for_winit(&Window, LogicalPosition<f64>) -> LogicalPosition<f64>`

## Input & IME

- Keyboard: press/release is mapped to `dear_imgui_rs::Key`. When `event.text`
  is present on key press, characters are injected via `io.add_input_character`.
  Coverage includes letters/digits, punctuation (',.-/;=[]\\`), function and lock keys,
  and numpad (0-9, decimal/divide/multiply/subtract/add/equal/enter).
- Mouse: buttons, position, wheel. `PixelDelta` retains fractional movement after scaling to logical scroll units; `LineDelta` uses the provided values.
- Modifiers: `ModifiersChanged` updates aggregate modifier state; physical key events preserve left/right identity.
- IME: preedit is ignored (no transient injection); committed text is appended.

### Touch

Basic touch-to-mouse translation is provided:
- First active finger controls the pointer and Left mouse button.
- Started -> set position + press LMB; Moved -> update position; End/Cancelled -> release LMB.
- Losing window focus releases the active touch, so a missing End/Cancelled event cannot block a later finger.

### IME integration

- IME is **auto-managed** by default: `prepare_render` inspects
  `ui.io().want_text_input()` and toggles `Window::set_ime_allowed(...)`
  accordingly. This means IME (and soft keyboards on mobile) are only enabled
  while text widgets are active.
- You can temporarily override the state with
  `WinitPlatform::set_ime_allowed(bool)`. Auto-management may adjust
  it again on subsequent frames unless you disable it.
- To fully opt out and manage IME yourself, call
  `WinitPlatform::set_ime_auto_management(false)`.
- The backend tracks IME enabled/disabled state internally and exposes it
  through `WinitPlatform::ime_enabled()`.
- If the window is destroyed before the ImGui context, call
  `WinitPlatform::detach_window(&mut context)` first. The returned `Arc<Window>` is the
  platform-owned main window and proves that native IME state has been detached.

## Cursor Handling

`prepare_render(&Ui, &Window)` updates the OS cursor from `ui.mouse_cursor()`.
Changes are cached to avoid redundant OS calls. If `ConfigFlags::NO_MOUSE_CURSOR_CHANGE`
is set, OS cursor updates are skipped. `Io::set_mouse_draw_cursor(true)` enables Dear ImGui's
software cursor; the default uses the OS cursor.

In single-window mode, `prepare_frame` honors `io.want_set_mouse_pos()` by setting the window-local
OS cursor position. Native multi-viewport does not advertise `HAS_SET_MOUSE_POS`, because Winit has
no portable global desktop cursor-position setter.

### Software Cursor

You can force Dear ImGui to draw the cursor by enabling the software cursor:

```rust
// Option 1: via Io directly
imgui_ctx.io_mut().set_mouse_draw_cursor(true);

// Option 2: helper on the platform
platform
    .set_software_cursor_enabled(&mut imgui_ctx, true)
    .expect("Winit platform contract changed");
```

When software cursor is enabled:
- The platform hides the OS cursor.
- Dear ImGui emits cursor geometry in draw data; ensure your renderer renders the draw lists every frame.

## Backend Flags

This backend sets (when appropriate):
- `BackendFlags::HAS_MOUSE_CURSORS`
- `BackendFlags::PLATFORM_HAS_VIEWPORTS` while viewports are enabled on `WinitPlatform`
- `BackendFlags::HAS_MOUSE_HOVERED_VIEWPORT` on Windows while viewports are enabled

For diagnostics, the backend also sets `BackendPlatformName` to `"dear-imgui-winit {version}"`.

## Multi-Viewport (Experimental)

Multi-viewport support is available behind the `multi-viewport` feature and is
**experimental**. It follows the upstream backend split:

- The winit platform layer (`dear-imgui-winit/multi-viewport`) owns OS windows and
  routes events for secondary viewports.
- A renderer backend must also opt into viewports (e.g. `dear-imgui-wgpu/multi-viewport-winit`)
  to create per-viewport render targets and draw them.

`WinitPlatform` is the sole public platform owner. Its Context attachment retains the main
`Arc<Window>`, every secondary window, and the exact callback generation it installs; moving the
Rust wrapper does not move callback-visible state. First-party renderer routes retain an internal,
non-clone adapter for that exact generation, so renderer preparation and deferred platform-fault
collection form one transaction. Winit's `ActiveEventLoop` is available to native callbacks only
inside a non-escaping closure. A first-party renderer route owns that scope, secondary rendering,
and fault aggregation as one prepared-frame transaction. See
[`multi_viewport_wgpu`](../../examples/02-docking/multi_viewport_wgpu.rs) or
[`multi_viewport_ash`](../../examples/02-docking/multi_viewport_ash.rs) for the complete frame and
shutdown order. Do not call Dear ImGui's platform-window phases separately when using those routes.

Callback panics and window-creation failures are contained before returning
through C++. For custom renderer integrations only, `with_event_loop` returns a
`WinitViewportAttempt` that keeps the callback output and every deferred `WinitPlatformError` as
parallel values; a callback `Result` is never discarded by a later platform failure. Such
integrations may call `drain_viewport_faults()` after a native pass that is not already wrapped by
`with_event_loop`. First-party routes perform both steps automatically. Ordinary `handle_event` and
owner methods surface the oldest pending fault before entering Winit. Explicit shutdown is
idempotent and reports cleanup failures. A failed viewport remains marked for closure after Dear
ImGui clears callback request flags, and the failure is retired only when that exact native
viewport ownership generation is destroyed or replaced.
Dropping the owner without the mutable Context leaves native cleanup with the Context attachment,
so teardown still passes through the core open-frame normalization path.
Explicit shutdown returns `WinitPlatformError::RendererShutdownRequired` while a renderer callback
or viewport renderer state is still installed, preventing Winit-owned viewport data from being
passed to an unknown renderer callback. Context-owned teardown enforces this renderer-before-platform
order automatically. The Context attachment graph also returns
`WinitPlatformError::PlatformAttachmentRelease(RendererActive)` when platform or
viewport shutdown finds an active renderer attachment, before an open frame or native
state changes. Shut down the owning renderer runtime and retry the same Winit shutdown call.

The runtime only advertises callbacks it can implement. In particular, winit has no portable
per-window opacity API, so it does not install `Platform_SetWindowAlpha`; enabling transparent
docking payloads therefore fails core capability validation instead of silently accepting a no-op.
Unimplemented foreign callbacks, including alpha, work-area-inset, and Vulkan-surface hooks, stay
outside Winit's callback lease and may change without faulting the runtime.
Native move, resize, and DPI notifications are treated as authoritative platform feedback, while
touch events from secondary windows use the same touch-to-mouse path as the primary window.
`NO_INPUTS` viewports pass hit tests through so drag targets behind them remain reachable. Windows
does this with `WM_NCHITTEST`, discovers the hovered native window from the global pointer every
frame, and raises a moving viewport above its docking target without activating it or changing
compositor window styles. Mouse capture is transferred to the main window if the viewport that
started a drag is destroyed. Other desktop targets use Winit cursor hit testing.
Windows and X11 also honor `NO_TASK_BAR_ICON` at creation. Multi-viewport geometry follows the
native desktop coordinate space required by Dear ImGui: Windows and X11 use physical virtual-desktop
pixels with `FramebufferScale = (1, 1)`, while macOS uses Cocoa points with its backing scale.
`DpiScale` remains separate from framebuffer scale, so mixed-DPI monitor layouts are supported
without overlapping or discontinuous monitor rectangles. The monitor publication is refreshed at
the frame-preparation boundary and preserves the prior PlatformIO owner during teardown. Call
`WinitPlatform::monitor_publication_report()` to inspect the installed detached snapshots and each
snapshot's `WorkAreaProvenance`. A transient collection failure retains the previous complete
transaction and reports `RetainedAfterCollectionFailure`; initial attachment fails instead of
publishing a main-window rectangle as a monitor. A multi-monitor batch whose primary identity
cannot be proven is likewise rejected rather than promoting an arbitrary candidate. Runtime
attachment invalidates mouse coordinates cached in the single-window coordinate space; shutdown
restores the base logical display metrics and invalidates the desktop-space cache before returning
control to the single-window backend. On Windows, decorated client positions are converted with the
target viewport DPI rather than the source monitor's current frame size.

Multi-viewport requires `HiDpiMode::Default`. `Rounded` and `Locked` remap the primary-window
coordinate space, while secondary windows use Winit's native desktop coordinates; mixing those
models is rejected instead of publishing inconsistent input and window geometry.

Native Linux multi-viewport support is X11-only. Runtime construction rejects Wayland before any
PlatformIO state is published because Wayland cannot provide the desktop-space window positioning
required by Dear ImGui. Runtime construction likewise rejects non-desktop targets; the supported
window systems are Windows, macOS, and Linux/X11. Windows and macOS honor
`NO_FOCUS_ON_APPEARING` by creating secondary windows inactive and deciding focus from the final
flags at show time. The macOS path uses AppKit's non-activating `orderFront` operation instead of
Winit's activating `set_visible(true)` implementation. Linux/X11 accepts that flag without rejecting the viewport, but its window
manager controls the final focus behavior. `NO_FOCUS_ON_CLICK` is accepted as a platform policy:
Windows installs a native `WM_MOUSEACTIVATE` hook that returns `MA_NOACTIVATE`, while platforms
without an equivalent Winit hook treat it as best effort. Live decoration and top-most changes are
synchronized; Windows also updates taskbar visibility live, while X11 rejects a live
`NO_TASK_BAR_ICON` transition because its window type can only be selected at creation. Platforms
without a Winit taskbar API reject `NO_TASK_BAR_ICON` at creation instead of silently ignoring it.

Current support matrix:

- **winit + WGPU**: experimental native multi-viewport, exercised by the
  `multi_viewport_wgpu` example.
  - Enabled on Windows, macOS, and Linux/X11. The release gate runs a real Linux secondary-window and GPU-
    surface lifecycle under Xvfb with Mesa/Lavapipe; missing display or software-GPU
    infrastructure fails that gate.
  - Example:
    `cargo run -p dear-imgui-examples --bin multi_viewport_wgpu --features multi-viewport`
- **winit + OpenGL (glow/glutin)**: no official multi-viewport stack yet.
  If you need multi-viewport OpenGL today, use the SDL3 routes
  (`sdl3_opengl_multi_viewport` or `sdl3_glow_multi_viewport`).

The repository-wide route evidence matrix separates callback-safety and shared-runtime tests from
native route smoke evidence. It currently grades Winit + WGPU as an `A` route and keeps the
Winit + Glow combination out of the published native route inventory; see
[`docs/renderer-route-evidence.md`](../../docs/renderer-route-evidence.md) before treating an
example or a shared unit test as a complete platform/renderer validation.

## Notes & Differences vs imgui-rs

This crate targets the `dear-imgui-rs` bindings in this repository and its API
surface. It’s intentionally separate from `imgui-rs/imgui-winit-support`, though
many behaviors are aligned for familiarity.

Known limitations:
- Clipboard integration and gamepad navigation remain application-owned.
- Native multi-viewport does not support Wayland or a portable global cursor-position setter.
