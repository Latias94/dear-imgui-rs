# Changelog

## [Unreleased]

### Added

- Optional native multi-viewport support through the `multi-viewport` Cargo feature and `AppConfig::multi_viewport`, using the existing Winit platform and WGPU owning route.
- `EventContext::outcome()` and `window_id()` expose exact native routing and capture intent. `window()` refers to the event target; `main_window()` provides the stable host window. Secondary close requests keep the application running.
- `Application::viewport_frame` provides owned-data recording, submission, and presentation observations for editor history. Engine commands recorded by `record_render` precede every UI consumer.
- A multi-window editor example and a native undock, merge, close, and device-recovery regression.

### Fixed

- Keep bounded redraw retries for transient secondary-surface timeouts and recovered lost/outdated surfaces, independently of successful main-window presentation. Occluded, zero-sized, and retired targets do not start polling.
- GPU recovery and shutdown release all configured Surfaces before their Device, and create a new main Surface for each replacement GPU generation. Context, main native registration, and application state survive recovery; recreated secondary windows receive new native instance IDs.
- Redraws observe confirmed device loss before its queued event is delivered, while unrelated validation errors remain terminal. GPU submission draining has a bounded wait.
