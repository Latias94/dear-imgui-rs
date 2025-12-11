# Changelog

All notable changes to `dear-imgui-winit` will be documented in this file.

The format follows Keep a Changelog and Semantic Versioning.

## [Unreleased]

### Added

- IME integration:
  - Wire Dear ImGui's `ImGuiPlatformImeData` to `winit::window::Window::set_ime_cursor_area` so IME candidate/composition windows follow the text caret.
  - Add automatic IME management based on `io.want_text_input()` in `WinitPlatform::prepare_render_with_ui`, with explicit control via:
    - `WinitPlatform::set_ime_allowed(&Window, bool)`
    - `WinitPlatform::set_ime_auto_management(bool)`
    - `WinitPlatform::ime_enabled() -> bool`
- New convenience API:
  - `WinitPlatform::handle_window_event(&mut Context, &Window, &WindowEvent)` for `ApplicationHandler::window_event`-style loops, avoiding the need to wrap events in `Event::WindowEvent`.
- Examples:
  - New `ime_debug` example (`dear-imgui-examples`) demonstrating winit 0.30 IME integration, IME auto-management toggling, and runtime inspection of `io.want_text_input` / backend IME state.

### Changed

- `WinitPlatform::handle_event` remains available for closure-style `EventLoop::run`, but internally delegates to a shared window-event handler instead of duplicating logic.
- All winit 0.30 `ApplicationHandler` examples now use `handle_window_event` instead of constructing synthetic `Event::WindowEvent` values, simplifying the recommended integration pattern.


