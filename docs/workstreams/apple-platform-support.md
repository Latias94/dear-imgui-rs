# Apple Platform Example Notes

Status: Draft
Last updated: 2026-03-27

This document explains how to read the repository-owned Apple examples.

The goal is not to claim that the workspace provides a turn-key mobile runtime.

The goal is:

- give developers concrete iOS examples they can study and adapt
- validate that the integration seams we document are actually viable
- keep platform ownership, renderer ownership, and app ownership explicit
- avoid implying that every backend combination is a repository-owned Apple path

## What These Examples Are For

The repository-owned Apple smoke examples serve three purposes:

1. Validation
   - prove that a documented integration shape can compile, link, and launch
   - give the repository a small but real regression sentinel

2. Reference
   - show how a host project can call into Rust
   - show where platform packaging and framework ownership belong

3. Teaching
   - show touch / IME / event-loop boundaries
   - show which pieces belong to the app, the platform backend, and the renderer backend

## What These Examples Are Not

They are not:

- a turn-key mobile application framework
- a promise that every backend pair is maintained on iOS
- a replacement for app-owned signing, packaging, assets, or store distribution
- a signal that `dear-app` has become a generic mobile runtime layer

## Current Apple Example Paths

### macOS

For macOS, the regular native examples in `examples/` remain the main reference
path. They are regular desktop/native examples and do not need a separate smoke
folder.

### iOS

The repository currently includes two iOS smoke examples under `examples-ios/`.

`examples-ios/dear-imgui-ios-smoke`

- backend pair: `dear-imgui-winit + dear-imgui-wgpu`
- use it when you want a small reference path close to the `dear-app` style
  stack
- shows a Rust static library / XCFramework-oriented integration shape

`examples-ios/dear-imgui-ios-sdl3-smoke`

- backend pair: `dear-imgui-sdl3 + dear-imgui-wgpu`
- use it when your application or engine already owns SDL3
- shows an explicit app-owned SDL3 framework boundary and a tiny host `main`

The repository does not currently provide iOS smoke examples for:

- `glow` / OpenGL
- `ash` / Vulkan

That does not make those routes impossible. It only means they are not the
mobile smoke examples the repository is currently using for validation and
teaching.

## Boundary Notes

### `dear-imgui-rs`

Owns:

- safe Dear ImGui core APIs
- frame lifecycle
- typed texture and render snapshot APIs

Does not own:

- Apple host-project setup
- UIKit / AppKit packaging
- signing or distribution

### Platform backends

`dear-imgui-winit` and `dear-imgui-sdl3` own:

- input translation
- IME / text-input integration
- touch / pointer mapping
- platform event-loop integration inside their documented boundary

They do not own app packaging or renderer setup.

### Renderer backends

`dear-imgui-wgpu`, `dear-imgui-glow`, and `dear-imgui-ash` own:

- GPU resource management
- draw submission
- renderer-side texture lifecycle

They do not own the host app lifecycle.

### The consuming application

The application still owns:

- Xcode project structure
- signing
- bundle metadata
- SDL3 framework ownership when using the SDL route
- app-specific lifecycle policy, services, persistence, and release packaging

## How To Use The Apple Examples

1. Start from the example that is closest to your real application stack.
2. Treat the example as a reference integration, not as a finished mobile app template.
3. Copy only the host/project boundary you need.
4. Keep app-specific policy in your app, not in the core crates.

## What The Repository Should Keep Providing

To keep these examples useful, the repository should provide:

- checked-in smoke examples under `examples-ios/`
- concise README guidance about what each example demonstrates
- compile-target sentinels in CI for the documented iOS paths
- clear statements about what remains application-owned

## Success Criteria

This document is doing its job if:

- developers can quickly tell which iOS example to read first
- the examples are understood as teaching/validation artifacts
- the repository avoids over-claiming mobile product support
- platform and renderer boundaries remain explicit
