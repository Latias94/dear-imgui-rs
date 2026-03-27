# Xcode Host Stub

This folder contains a checked-in minimal iOS host project for wiring the SDL3
smoke sample into Xcode.

Project:

- `DearImguiIosSdl3SmokeHost.xcodeproj`

Support files:

- `main.m`
- `Info.plist`
- `scripts/build-rust.py`
- `scripts/build-sdl3-framework.py`

## What The Project Does

- builds the Rust smoke crate during the Xcode build with a Run Script phase
- resolves SDL3 as an app-owned dependency instead of hiding it inside the Rust
  crates
- can consume:
  - a direct `SDL3.framework`
  - a direct `SDL3.xcframework`
  - or the upstream SDL Xcode project shipped by the transitive `sdl3-src`
    crate
- copies the selected/built `SDL3.framework` into a local build folder and
  embeds it in the host app
- intentionally avoids requiring the upstream multi-platform
  `SDL3.xcframework` target during local iOS smoke builds

## Open In Xcode

```bash
open DearImguiIosSdl3SmokeHost.xcodeproj
```

## Build From The Command Line

Recommended simulator build:

```bash
xcodebuild \
  -project DearImguiIosSdl3SmokeHost.xcodeproj \
  -target DearImguiIosSdl3SmokeHost \
  -configuration Debug \
  -sdk iphonesimulator \
  CODE_SIGNING_ALLOWED=NO \
  build
```

## SDL3 Inputs

The build helper checks these inputs in order:

1. `SDL3_FRAMEWORK_PATH`
2. `SDL3_XCFRAMEWORK_PATH`
3. `SDL3_FRAMEWORK_SOURCE_ROOT`
4. auto-resolve the `sdl3-src` crate from Cargo's registry and build the
   official `SDL3` target from `SDL.xcodeproj`

## Notes

- The checked-in helper exists because the SDL3 path needs more than a trivial
  shell wrapper: it may need to resolve the transitive `sdl3-src` crate,
  select a matching slice from an XCFramework, or build the upstream SDL
  framework on demand.
- For real application distribution, an app-owned `SDL3.xcframework` is still
  the cleanest packaging story.
- The Rust side owns the SDL event loop, Dear ImGui setup, and WGPU rendering
  after `SDL_RunApp(..., dear_imgui_ios_sdl3_smoke_main, ...)` hands off.
- For real devices, set your signing team in Xcode before running on hardware.
- The helper intentionally builds the per-platform `SDL3` target from the
  upstream SDL Xcode project instead of invoking the upstream
  `SDL3.xcframework` target. The XCFramework packaging target may require extra
  Apple SDK components such as tvOS/visionOS even when you only want an iOS
  smoke build.
