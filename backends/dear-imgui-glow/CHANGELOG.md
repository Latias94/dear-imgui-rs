# Changelog

All notable changes to `dear-imgui-glow` will be documented in this file.

The format follows Keep a Changelog and Semantic Versioning.

## [Unreleased]

### Added

- Add a runnable `glow_external_context_regression` example that exercises `GlowRenderer::with_external_context()` together with Dear ImGui managed texture create/update/destroy requests.

### Fixed

- Make `render_with_context()` honor the caller-provided GL context for managed texture create/update/destroy requests instead of assuming the renderer owns a context internally. Fixes #22, thanks @CoffeeCatRailway.
- Add `register_texture_with_context()` / `update_texture_with_context()` helpers for applications that keep OpenGL context ownership outside `GlowRenderer`.
