# Changelog

All notable changes to `dear-imgui-wgpu` will be documented in this file.

The format follows Keep a Changelog and Semantic Versioning.

## [Unreleased]

### Fixed

- Winit and SDL3 multi-viewport renderer callbacks now verify `RendererUserData` ownership before
  reading or freeing per-viewport WGPU data, ignoring foreign backend pointers instead of treating
  them as `dear-imgui-wgpu` state.
