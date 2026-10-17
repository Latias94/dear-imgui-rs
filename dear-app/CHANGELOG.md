# Changelog

## [Unreleased]

## [0.21.0] - 2026-10-17

### Breaking Changes

- Full `AppConfig` literals need `multi_viewport` and `device_events`, or `..Default::default()`. `EventContext::window()` refers to the event target; use `main_window()` for the main window. [PR #93](https://github.com/Latias94/dear-imgui-rs/pull/93)

### Added

- Opt into native multi-viewport with the `multi-viewport` feature and `AppConfig::multi_viewport`; secondary windows recover with the GPU and retain bounded retries after transient surface failures, including after a successful main-window presentation. [PR #93](https://github.com/Latias94/dear-imgui-rs/pull/93)
- Wake Wait-mode applications from background work or deadlines with `AppWakeHandle`, and handle raw device input through `Application::device_event`. [PR #93](https://github.com/Latias94/dear-imgui-rs/pull/93)
- Record engine commands before UI draws with `Application::record_render` and inspect per-window results through `viewport_frame`. [PR #93](https://github.com/Latias94/dear-imgui-rs/pull/93)

### Fixed

- Redraw Wait-mode applications after input and release configured Surfaces before their Device during recovery and shutdown. [PR #93](https://github.com/Latias94/dear-imgui-rs/pull/93)

See the [workspace release notes](../CHANGELOG.md#0210---2026-10-17) and the [Dear ImGui 1.93](../docs/IMGUI_193_MIGRATION.md) and [editor backend](../docs/EDITOR_BACKEND_MIGRATION.md) migration guides.
