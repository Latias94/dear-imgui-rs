# Changelog

All notable changes to `dear-imgui-winit` will be documented in this file.

The format follows Keep a Changelog and Semantic Versioning.

Changelog prose uses soft wrapping: do not hard-wrap paragraphs or bullet text just to fit a fixed column width.

## [Unreleased]

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
