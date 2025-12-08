# Changelog

All notable changes to `dear-implot3d` will be documented in this file.

The format follows Keep a Changelog and Semantic Versioning.

## [Unreleased]

### Added

- Upgrade ImPlot3D stack to the latest upstream:
  - Pull `cimplot3d` to `main` with `implot3d v0.3` and Dear ImGui 1.92.5.
  - Regenerate `dear-implot3d-sys` bindings against the updated C API.
- Extend safe bindings to cover new/improved APIs:
  - Box helpers now use double-precision (`SetupBoxScale/Rotation/InitialRotation`).
  - Colormap helpers mirror upstream `ImPlot3D_GetColormapColor` / `NextColormapColor` signatures.
  - Image helpers (`image_by_axes`, `image_by_corners`) use the new `ImTextureRef` + f64 coordinates.

### Fixes

- Align `dear-implot3d` README compatibility table with the 0.6.x release train.

