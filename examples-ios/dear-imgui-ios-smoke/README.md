# dear-imgui-ios-smoke

Minimal iOS smoke reference for the `dear-imgui-winit + dear-imgui-wgpu` path:

- `dear-imgui-rs`
- `dear-imgui-winit`
- `dear-imgui-wgpu`

This sample is intentionally:

- outside the main workspace build
- not published as a crate
- focused on proving the iOS integration shape, not shipping a complete app template

## What It Demonstrates

- `winit` owns the iOS application lifecycle and window creation path
- `dear-imgui-winit` owns touch, IME, and per-frame platform integration
- `dear-imgui-wgpu` owns the renderer and texture lifecycle
- a downstream iOS app can integrate the Rust app loop through a tiny exported
  `start_winit_app()` symbol

## Build Targets

Install the Rust std targets you need:

```bash
rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios
```

Build the static library for a device:

```bash
cargo build --manifest-path examples-ios/dear-imgui-ios-smoke/Cargo.toml --target aarch64-apple-ios
```

Build the static library for Apple Silicon simulator:

```bash
cargo build --manifest-path examples-ios/dear-imgui-ios-smoke/Cargo.toml --target aarch64-apple-ios-sim
```

Build the static library for Intel simulator hosts when needed:

```bash
cargo build --manifest-path examples-ios/dear-imgui-ios-smoke/Cargo.toml --target x86_64-apple-ios
```

The output is a static library named similar to:

```text
target/aarch64-apple-ios/debug/libdear_imgui_ios_smoke.a
```

## Build An XCFramework

For Xcode integration, packaging the Rust output as an XCFramework is usually
more convenient than dragging raw `.a` files around.

The sample includes a helper script:

```bash
python3 examples-ios/dear-imgui-ios-smoke/scripts/build-xcframework.py
```

Debug variant:

```bash
python3 examples-ios/dear-imgui-ios-smoke/scripts/build-xcframework.py --profile debug
```

If you prefer a shell entry point, `scripts/build-xcframework.sh` forwards to the
same Python implementation.

The default output path is:

```text
examples-ios/dear-imgui-ios-smoke/dist/DearImguiIosSmoke.xcframework
```

## Xcode Integration Shape

This sample follows the integration model described by `winit` for iOS:

1. Build the Rust crate as an XCFramework or static library.
2. Add the resulting artifact to your Xcode project.
3. Add the header from `include/dear_imgui_ios_smoke.h`.
4. Call `start_winit_app()` from your Objective-C / Objective-C++ entry point.

Header:

```c
void start_winit_app(void);
```

Minimal Objective-C bridge:

```objc
#include "dear_imgui_ios_smoke.h"

int main(int argc, char * argv[]) {
    start_winit_app();
}
```

A minimal host-side reference is included in `xcode-host/`:

- `xcode-host/main.m`
- `xcode-host/Info.plist`
- `xcode-host/README.md`
- `xcode-host/DearImguiIosSmokeHost.xcodeproj`
- `xcode-host/scripts/build-rust.py`

You can also open the checked-in host project directly:

```bash
open examples-ios/dear-imgui-ios-smoke/xcode-host/DearImguiIosSmokeHost.xcodeproj
```

## Notes

- This smoke sample requests redraw continuously and is meant only as a proof-of-shape.
- The sample uses `winit` iOS window attributes to hide the status bar and
  prefer deferred system gestures.
- The sample includes a small text input field so you can validate soft-keyboard
  and IME behavior, not just GPU rendering.
- The checked-in Xcode host uses a Python build helper so it can translate Xcode
  architecture requests into the right Rust targets and merge simulator slices
  into one static library for the linker.
- This sample includes a checked-in `.xcodeproj`, but still intentionally omits
  storyboards and asset catalogs.

## What You Still Need In A Real App

- an Xcode project and signing configuration
- an app bundle / Info.plist setup appropriate for your app
- app-specific lifecycle policy, assets, persistence, and platform services
- any engine-specific render scheduling or host/native interop layers
