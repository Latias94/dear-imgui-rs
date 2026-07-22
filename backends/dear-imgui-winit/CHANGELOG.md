# Changelog

All notable changes to `dear-imgui-winit` will be documented in this file.

The format follows Keep a Changelog and Semantic Versioning.

Changelog prose uses soft wrapping: do not hard-wrap paragraphs or bullet text just to fit a fixed column width.

## [Unreleased]

## [0.16.0-alpha.1]

### Breaking Changes

- Replace the free multi-viewport `enable` and `shutdown_multi_viewport_support` APIs with `WinitPlatformRuntime::new(&mut Context, &WinitPlatform)`. `WinitPlatform::new` installs the sole Context platform attachment, and `WinitPlatform::attach_window(Arc<Window>, ...)` retains the main window while the runtime owns secondary windows and its callback claim without borrowing caller-managed callback state. Multi-viewport currently requires `HiDpiMode::Default`; custom primary-window coordinate scaling is rejected.
- Make `WinitPlatform::attach_window` safe by requiring `Arc<Window>`. The platform retains the exact shared allocation until detach or Context teardown, so the IME callback cannot outlive the window.
- Make `WinitPlatform::set_hidpi_mode` fallible. Once `WinitPlatformRuntime` is attached, both scaling-mode changes and main-window reattachment return `WinitPlatformError::RuntimeConfigurationLocked` without changing state.
- Route event-loop-dependent callbacks through the non-escaping `WinitPlatformRuntime::with_event_loop` closure. The active event loop cannot outlive the callback scope, nested scopes restore the outer loop, and callback panics or native faults are reported only after control returns to Rust.
- Use `WinitPlatformRuntime::shutdown(&mut Context)` for reportable ordered teardown. It closes any open frame while callbacks remain attached, requires renderer callbacks and viewport state to be released first, destroys secondary windows only while Winit still owns the destroy callback, releases the callback table, and detaches from the Context. Dropping the runtime without a Context defers native cleanup to the Context attachment.
- Treat `BackendFlags::PLATFORM_HAS_VIEWPORTS` and `BackendFlags::HAS_MOUSE_HOVERED_VIEWPORT` as exclusive runtime capabilities: attachment rejects contexts that preadvertise either flag, and teardown clears Winit-owned bits without restoring stale values after callback drift.

### Added

- Add `WinitPlatformRuntime::{poll_fault,handle_event,route_secondary_event,main_window}` so applications can inspect deferred callback faults, route main and secondary viewport events, and access the runtime-owned main window without raw callback userdata.

### Changed

- Register `WinitPlatformControl` as the Context's exclusive platform attachment. The runtime shares that owner rather than installing a second attachment; renderer attachments must be created after it, and Context teardown releases renderer resources before Winit destroys platform windows.
- Stop advertising the unsupported `Platform_SetWindowAlpha` callback, suppress delayed programmatic move/resize events through the next frame, route secondary-window touch input through the primary touch translator, apply `NO_INPUTS` through Winit cursor hit testing, and honor `NO_TASK_BAR_ICON` on Windows and X11. Mixed-DPI monitor layouts now fail attachment explicitly because the current logical coordinate model cannot represent them without overlap.
- Reject Wayland and non-desktop targets before publishing multi-viewport capability, fail closed when a viewport requests focus or taskbar semantics Winit cannot guarantee, synchronize live decoration/top-most/taskbar policies where the platform supports them, translate primary and secondary touches to screen-logical coordinates with Context-local first-finger tracking, and suppress only move/resize events matching a pending programmatic target.
- Snapshot only the `Platform_*` callback slots Winit claims at attachment and validate them before every Winit C trampoline touches runtime state. Foreign callbacks Winit does not implement, including alpha, work-area-inset, and Vulkan-surface hooks, remain outside that lease. Drift in owned callbacks latches a fault, revokes viewport capability, and blocks every remaining public callback; only the owned destroy callback receives a narrow runtime-controlled teardown scope.

## [0.15.1] - 2026-06-30

### Breaking Changes

- `multi_viewport::shutdown_multi_viewport_support` now takes `&mut Context`, matching the renderer backend shutdown helpers and making the target ImGui context explicit.

### Added

- IME integration:
  - Wire Dear ImGui's `ImGuiPlatformImeData` to `winit::window::Window::set_ime_cursor_area` for the main window and winit-owned multi-viewport windows so IME candidate/composition windows follow the text caret.
  - Add automatic IME management based on `io.want_text_input()` in `WinitPlatform::prepare_render_with_ui`, with explicit control via:
    - `WinitPlatform::set_ime_allowed(&Window, bool)`
    - `WinitPlatform::set_ime_auto_management(bool)`
    - `WinitPlatform::ime_enabled() -> bool`
    - `WinitPlatform::detach_window(&Window, &mut Context)` for clearing winit-owned IME hooks before a window is destroyed while the context remains alive.
- New convenience API:
  - `WinitPlatform::handle_window_event(&mut Context, &Window, &WindowEvent)` for `ApplicationHandler::window_event`-style loops, avoiding the need to wrap events in `Event::WindowEvent`.
- Examples:
  - New `ime_debug` example (`dear-imgui-examples`) demonstrating winit 0.30 IME integration, IME auto-management toggling, and runtime inspection of `io.want_text_input` / backend IME state.

### Changed

- `WinitPlatform::handle_event` remains available for closure-style `EventLoop::run`, but internally delegates to a shared window-event handler instead of duplicating logic.
- All winit 0.30 `ApplicationHandler` examples now use `handle_window_event` instead of constructing synthetic `Event::WindowEvent` values, simplifying the recommended integration pattern.

### Fixed

- Filter non-finite winit-provided coordinates, sizes, scale factors, and wheel deltas before forwarding them to Dear ImGui IO, including multi-viewport callbacks. This prevents `Io::set_mouse_pos()` / mouse-position event panics during window-manager-driven moves on Wayland/KDE. Fixes #35, thanks @AndreasPantle.
- `WinitPlatform::attach_window` no longer overwrites `Platform_ImeUserData` when another backend owns `Platform_SetImeDataFn`; it only updates the IME userdata for winit-owned callbacks.
- `Platform_SetImeDataFn` now resolves `Platform_ImeUserData` from the `ImGuiContext*` passed by Dear ImGui instead of whichever context is currently bound.
- Multi-viewport shutdown now binds the provided `Context` before destroying platform windows and clearing platform callbacks, avoiding cleanup against a different current context.
