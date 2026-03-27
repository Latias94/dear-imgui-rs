# dear-imgui-ios-sdl3-smoke

Minimal iOS smoke reference for the `dear-imgui-sdl3 + dear-imgui-wgpu` path:

- `dear-imgui-rs`
- `dear-imgui-sdl3`
- `dear-imgui-wgpu`
- `sdl3` with `raw-window-handle`

<p align="center">
  <img
    src="https://github.com/user-attachments/assets/62c3c24c-e5a1-47d0-8bc5-bb274d1e5c98"
    alt="Dear ImGui iOS SDL3 smoke on the iOS Simulator"
    width="360"
  />
</p>

This sample is intentionally:

- outside the main workspace build
- not published as a crate
- focused on the iOS integration boundary for SDL3 users, not on shipping a
  full Xcode app template
- paired with a checked-in Xcode host that keeps SDL3 packaging explicit

## What It Demonstrates

- SDL3 owns the application bootstrap, window creation, and event queue
- `dear-imgui-sdl3` owns Dear ImGui platform integration for SDL events, IME,
  touch, and gamepads
- `dear-imgui-wgpu` owns the renderer and texture lifecycle
- a downstream iOS app can call a tiny Rust-exported SDL main callback from an
  Objective-C / Objective-C++ host
- the repository can smoke-test that host on a real iOS simulator build

## Build Targets

Install the Rust std targets you need:

```bash
rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios
```

Check the static library for iOS device:

```bash
cargo check --manifest-path examples-ios/dear-imgui-ios-sdl3-smoke/Cargo.toml --lib --target aarch64-apple-ios
```

Check the static library for Apple Silicon simulator:

```bash
cargo check --manifest-path examples-ios/dear-imgui-ios-sdl3-smoke/Cargo.toml --lib --target aarch64-apple-ios-sim
```

Build the static library for a device:

```bash
cargo build --manifest-path examples-ios/dear-imgui-ios-sdl3-smoke/Cargo.toml --target aarch64-apple-ios
```

The output is a static library named similar to:

```text
target/aarch64-apple-ios/debug/libdear_imgui_ios_sdl3_smoke.a
```

## Run The Checked-In Xcode Host

The repository now includes a minimal iOS host project under `xcode-host/`.

Recommended simulator build:

```bash
xcodebuild \
  -project examples-ios/dear-imgui-ios-sdl3-smoke/xcode-host/DearImguiIosSdl3SmokeHost.xcodeproj \
  -target DearImguiIosSdl3SmokeHost \
  -configuration Debug \
  -sdk iphonesimulator \
  CODE_SIGNING_ALLOWED=NO \
  build
```

The host build can obtain SDL3 in three ways:

- `SDL3_FRAMEWORK_PATH=/path/to/SDL3.framework`
- `SDL3_XCFRAMEWORK_PATH=/path/to/SDL3.xcframework`
- auto-resolve the `sdl3-src` crate from Cargo's registry and build
  `SDL3.framework` from the upstream SDL Xcode project

For local smoke testing, the auto-resolve path is the default. For a real app,
shipping an app-owned `SDL3.xcframework` is still the cleaner distribution
story. The auto-resolve path intentionally builds the per-platform `SDL3`
target instead of the upstream `SDL3.xcframework` packaging target, so local
iOS smoke builds do not require unrelated tvOS/visionOS SDK components.

## iOS Integration Shape

This route is intentionally more app-owned than the `winit + wgpu` route.

The consuming app owns:

- the SDL3 framework acquisition strategy
- the Xcode project, signing, and bundle setup
- the host `main` entry point

The Rust static library owns:

- SDL3 initialization
- Dear ImGui setup
- WGPU device / surface / render loop

The exported callback lives in:

- `include/dear_imgui_ios_sdl3_smoke.h`

Header:

```c
int dear_imgui_ios_sdl3_smoke_main(int argc, char *argv[]);
```

Minimal Objective-C bridge shape:

```objc
#import <SDL3/SDL.h>
#import <SDL3/SDL_main.h>
#import "dear_imgui_ios_sdl3_smoke.h"

int main(int argc, char *argv[]) {
    return SDL_RunApp(argc, argv, dear_imgui_ios_sdl3_smoke_main, NULL);
}
```

## SDL3 Packaging Notes

For Apple targets, SDL3 acquisition stays with the consuming application.

Common choices:

- add a direct `sdl3` dependency in the final Cargo graph and let feature
  unification enable `sdl3/build-from-source` or `sdl3/link-framework`
- link an app-owned `SDL3.xcframework` from Xcode and provide headers to the
  Rust build via the normal SDL3 discovery path (`SDL3_INCLUDE_DIR`, pkg-config,
  or framework-managed headers)
- use the checked-in `xcode-host/` project in this repository as a development
  stub while keeping the SDL3 packaging boundary explicit

## Notes

- This smoke sample requests redraw continuously and is meant only as a
  proof-of-shape.
- The sample uses a regular SDL event loop inside a Rust-exported callback, so
  the host `main` stays tiny and explicit.
- The sample includes a small text input field so you can validate IME / soft
  keyboard behavior, not just GPU rendering.
- The checked-in Xcode host intentionally keeps SDL3 outside the Rust crate. It
  can consume an app-owned `SDL3.framework` / `SDL3.xcframework`, or build
  `SDL3.framework` from the upstream SDL source shipped by `sdl3-src`.
