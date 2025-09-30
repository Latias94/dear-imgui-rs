# dear-imgui-winit

Winit platform backend for the `dear-imgui` Rust crate. It wires winit input/events,
cursor handling and DPI awareness into Dear ImGui. Inspired by
`imgui-rs/imgui-winit-support`.

<p align="center">
  <img src="https://raw.githubusercontent.com/Latias94/dear-imgui/main/screenshots/game-engine-docking.png" alt="Docking (winit)" width="75%"/>
  <br/>
</p>

## Compatibility

| Item          | Version |
|---------------|---------|
| Crate         | 0.3.x   |
| dear-imgui-rs | 0.3.x   |
| winit         | 0.30.12 |

See also: [docs/COMPATIBILITY.md](https://github.com/Latias94/dear-imgui/blob/main/docs/COMPATIBILITY.md) for the full workspace matrix.

## Quick Start

Minimal flow with winit 0.30 ApplicationHandler-style loops:

```rust,no_run
use dear_imgui_rs::{Context, Condition};
use dear_imgui_winit::{WinitPlatform, HiDpiMode};
use winit::{event::WindowEvent, event_loop::{ActiveEventLoop, EventLoop}, window::WindowId};

struct App { /* ... */ }

impl winit::application::ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) { /* create window + ImGui + WinitPlatform */ }

    fn window_event(&mut self, el: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let window = /* get your window */;
        // 1) forward events to ImGui
        let full = winit::event::Event::WindowEvent { window_id: id, event: event.clone() };
        self.imgui.platform.handle_event(&mut self.imgui.context, &window, &full);

        match event {
            WindowEvent::RedrawRequested => {
                // 2) per-frame prep
                self.imgui.platform.prepare_frame(&window, &mut self.imgui.context);
                let ui = self.imgui.context.frame();

                // 3) build UI
                ui.window("Hello").size([400.0, 300.0], Condition::FirstUseEver).build(|| {
                    ui.text("ImGui + winit");
                });

                // 4) update OS cursor from UI
                self.imgui.platform.prepare_render_with_ui(&ui, &window);

                // 5) render via your renderer backend
                let draw_data = self.imgui.context.render();
                /* renderer.render(&draw_data) */
            }
            _ => {}
        }
    }
}
```

APIs of interest:
- `WinitPlatform::new(&mut Context)`
- `WinitPlatform::attach_window(&Window, HiDpiMode, &mut Context)`
- `WinitPlatform::handle_event(&mut Context, &Window, &Event<T>)`
- `WinitPlatform::prepare_frame(&Window, &mut Context)`
- `WinitPlatform::prepare_render_with_ui(&Ui, &Window)` — updates OS cursor from ImGui

## DPI / HiDPI

`HiDpiMode` controls how the backend derives the framebuffer scale:
- `Default`: use winit’s `window.scale_factor()` directly.
- `Rounded`: round the winit factor to the nearest integer to avoid blurry scaling.
- `Locked(f64)`: force a custom factor (e.g. 1.0).

When DPI changes (`ScaleFactorChanged`), the backend adjusts:
- `io.display_size`, `io.display_framebuffer_scale`
- mouse position (keeping pointer location consistent across scales)

Helpers are provided if you pass winit logical values around and need the same
coordinates ImGui uses:
- `scale_size_from_winit(&Window, LogicalSize<f64>) -> LogicalSize<f64>`
- `scale_pos_from_winit(&Window, LogicalPosition<f64>) -> LogicalPosition<f64>`
- `scale_pos_for_winit(&Window, LogicalPosition<f64>) -> LogicalPosition<f64>`

## Input & IME

- Keyboard: press/release is mapped to `dear-imgui::Key`. When `event.text`
  is present on key press, characters are injected via `io.add_input_character`.
  Coverage includes letters/digits, punctuation (',.-/;=[]\\`), function and lock keys,
  and numpad (0-9, decimal/divide/multiply/subtract/add/equal/enter).
- Mouse: buttons, position, wheel. `PixelDelta` wheel is mapped to ±1.0 steps
  (consistent with most ImGui backends); `LineDelta` uses the provided values.
- Modifiers: tracked via `ModifiersChanged` and mirrored into left/right variants.
- IME: preedit is ignored (no transient injection); committed text is appended.

### Touch

Basic touch-to-mouse translation is provided:
- First active finger controls the pointer and Left mouse button.
- Started -> set position + press LMB; Moved -> update position; End/Cancelled -> release LMB.

## Cursor Handling

`prepare_render_with_ui(&Ui, &Window)` updates the OS cursor from `ui.mouse_cursor()`.
Changes are cached to avoid redundant OS calls. If `ConfigFlags::NO_MOUSE_CURSOR_CHANGE`
is set, OS cursor updates are skipped. The software-drawn cursor flag is currently not
exposed via our `Io` wrapper (defaults to OS cursor).

If Dear ImGui requests repositioning (`io.want_set_mouse_pos()`), `prepare_frame`
will set the OS cursor position accordingly.

### Software Cursor

You can force Dear ImGui to draw the cursor by enabling the software cursor:

```rust
// Option 1: via Io directly
imgui_ctx.io_mut().set_mouse_draw_cursor(true);

// Option 2: helper on the platform
platform.set_software_cursor_enabled(&mut imgui_ctx, true);
```

When software cursor is enabled:
- The platform hides the OS cursor.
- Dear ImGui emits cursor geometry in draw data; ensure your renderer renders the draw lists every frame.

## Backend Flags

This backend sets (when appropriate):
- `BackendFlags::HAS_MOUSE_CURSORS`
- `BackendFlags::HAS_SET_MOUSE_POS`

For diagnostics, the backend also sets `BackendPlatformName` to `"dear-imgui-winit {version}"`.

## Multi-Viewport (WIP)

Multi-viewport support is being implemented behind a `multi-viewport` feature,
but is not enabled by default yet. Expect changes and incomplete coverage while
the feature stabilizes. Follow the examples and docs once it’s marked stable.

## Notes & Differences vs imgui-rs

This crate targets the `dear-imgui` bindings in this repository and its API
surface. It’s intentionally separate from `imgui-rs/imgui-winit-support`, though
many behaviors are aligned for familiarity.

Known limitations:
- Key mapping covers digits, letters, navigation, modifiers, and function keys.
  Some punctuation/numpad-specific variants are not mapped yet.
