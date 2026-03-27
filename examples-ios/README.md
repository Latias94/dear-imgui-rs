# iOS Smoke Examples

This directory contains the repository-owned iOS smoke templates.

They are intentionally:

- outside the main workspace build
- focused on proving integration shape
- not published as crates
- narrow enough to validate on a real simulator build

<p align="center">
  <img
    src="https://github.com/user-attachments/assets/62c3c24c-e5a1-47d0-8bc5-bb274d1e5c98"
    alt="Dear ImGui iOS smoke on the iOS Simulator"
    width="360"
  />
</p>

The screenshot above shows one of the repository-owned iOS smoke templates
running on the iOS Simulator.

## Which Example To Start From

| Example | Start here when | Backend pair |
|---|---|---|
| `dear-imgui-ios-smoke` | you want a small `winit + wgpu` reference path | `winit + wgpu` |
| `dear-imgui-ios-sdl3-smoke` | you already own an SDL3-based app or engine stack | `sdl3 + wgpu` |

## How To Read These Examples

- Start with the example that is closest to your real app stack.
- Treat both examples as smoke/reference integrations, not as full mobile app templates.
- Use them to understand host-project setup, packaging boundaries, and backend ownership.
- The repository does not currently provide iOS smoke examples for `glow` or `ash`.

## Included Templates

### `dear-imgui-ios-smoke`

Reference iOS path built around:

- `dear-imgui-rs`
- `dear-imgui-winit`
- `dear-imgui-wgpu`

Includes:

- a checked-in Xcode host project
- a Python helper that builds the Rust static library for device/simulator
- an XCFramework packaging helper for the Rust output

See:

- `examples-ios/dear-imgui-ios-smoke/README.md`

### `dear-imgui-ios-sdl3-smoke`

Reference iOS path for SDL-based applications:

- `dear-imgui-rs`
- `dear-imgui-sdl3`
- `dear-imgui-wgpu`

Includes:

- a checked-in Xcode host project
- a Python helper that builds the Rust static library for device/simulator
- a Python helper that resolves or builds `SDL3.framework`
- support for an app-owned `SDL3.framework` / `SDL3.xcframework`
- a simulator-validated smoke path with a checked-in screenshot in the example
  README

See:

- `examples-ios/dear-imgui-ios-sdl3-smoke/README.md`

## Boundary Reminder

These templates validate the integration shape, not the full application
packaging story.

The consuming app still owns:

- signing
- bundle metadata
- assets and app services
- release packaging
- platform-specific lifecycle policy outside the narrow smoke scope

## Why This Lives Outside `examples/`

These templates are intentionally not part of the regular `dear-imgui-examples`
crate.

Reasons:

- iOS integration needs an Xcode host or XCFramework packaging step
- the SDL3 iOS route also needs explicit SDL3 framework ownership
- the repository should not imply that mobile integration is just another
  `cargo run --bin ...` path
