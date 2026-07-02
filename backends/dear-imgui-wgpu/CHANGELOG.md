# Changelog

All notable changes to `dear-imgui-wgpu` will be documented in this file.

The format follows Keep a Changelog and Semantic Versioning.

## [Unreleased]

### Changed

- Default `dear-imgui-wgpu` to `wgpu` 30, add the `wgpu-30` feature, and keep `wgpu-29`, `wgpu-28`, and `wgpu-27` as explicit compatibility features.

### Fixed

- Winit and SDL3 multi-viewport renderer callbacks now verify `RendererUserData` ownership before
  reading or freeing per-viewport WGPU data, ignoring foreign backend pointers instead of treating
  them as `dear-imgui-wgpu` state.
