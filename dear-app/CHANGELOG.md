# Changelog

## [Unreleased]

### Breaking Changes

- Full `AppConfig` literals need `multi_viewport` and `device_events`, or `..Default::default()`. `EventContext::window()` now refers to the event target; use `main_window()` for the main window.

### Added

- Opt into native multi-viewport with the `multi-viewport` feature and `AppConfig::multi_viewport`; secondary windows recover with the GPU and retain bounded retries after transient surface failures. [PR #93](https://github.com/Latias94/dear-imgui-rs/pull/93)
- Wake Wait-mode applications from background work or deadlines with `AppWakeHandle`, and handle raw device input.
- Record engine commands before UI draws with `Application::record_render` and inspect per-window results through `viewport_frame`.

### Fixed

- Redraw Wait-mode applications after input and release configured Surfaces before their Device during recovery and shutdown.
