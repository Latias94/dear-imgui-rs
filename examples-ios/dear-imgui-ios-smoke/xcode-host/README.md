# Xcode Host Stub

This folder contains a checked-in minimal iOS host project for wiring the smoke
sample into Xcode.

Project:

- `DearImguiIosSmokeHost.xcodeproj`

Support files:

- `main.m`
- `Info.plist`
- `scripts/build-rust.py`

## What The Project Does

- builds the Rust smoke crate during the Xcode build with a Run Script phase
- uses a Python helper to build the Rust static library for the requested Apple
  target triples and merge simulator slices when Xcode asks for both `arm64`
  and `x86_64`
- links the resulting static library into a plain UIKit app target
- keeps the integration local to this repository, without requiring a
  pre-generated XCFramework for every edit/build loop

## Open In Xcode

```bash
open DearImguiIosSmokeHost.xcodeproj
```

## Build From The Command Line

Recommended simulator build:

```bash
xcodebuild \
  -project DearImguiIosSmokeHost.xcodeproj \
  -target DearImguiIosSmokeHost \
  -configuration Debug \
  -sdk iphonesimulator \
  CODE_SIGNING_ALLOWED=NO \
  build
```

## Notes

- The checked-in Python helper exists because the host build needs more than a
  trivial shell wrapper: it resolves Rust targets, installs missing std
  components if needed, and creates a universal simulator `.a` when Xcode links
  multiple architectures in one build.
- The project is intentionally minimal and does not include storyboards or asset
  catalogs.
- For real devices, set your signing team in Xcode before running on hardware.
- The Rust side owns the `winit` application lifecycle after
  `start_winit_app()` is called.
- For repeatable local or CI-style validation, prefer the target-based
  `-sdk iphonesimulator` build shown above. Make sure the matching iOS SDK and
  simulator runtime are installed in Xcode > Settings > Components.
- If you want a portable artifact for another Xcode project, use the
  repository-level XCFramework helper in `../scripts/build-xcframework.py`.
