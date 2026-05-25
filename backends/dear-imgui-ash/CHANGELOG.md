# Changelog

All notable changes to this crate will be documented in this file.

## Unreleased

- Winit and SDL3 multi-viewport renderer callbacks now verify `RendererUserData` ownership before
  reading or freeing per-viewport Vulkan data, ignoring foreign backend pointers instead of treating
  them as `dear-imgui-ash` state.
