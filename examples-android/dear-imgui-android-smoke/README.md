# dear-imgui-android-smoke

Minimal Android compile-target template for the low-level
`dear-imgui-rs` + `dear-imgui-sys` route, including a minimal
EGL / OpenGL ES 3 render loop.

<p align="center">
  <img
    src="https://github.com/user-attachments/assets/914e3df9-8957-4890-aa79-cf2473224206"
    alt="Dear ImGui Android Smoke on device"
    width="360"
  />
</p>

This sample is intentionally:

- outside the main workspace build
- not published as a crate
- focused on proving the Android integration shape, not shipping a full app template

## What It Demonstrates

- `dear-imgui-rs` owns the safe core `Context`, `Io`, and frame lifecycle.
- `dear-imgui-sys::backend_shim::android` can own the official Android platform
  backend lifecycle (`Init`, `NewFrame`, `Shutdown`).
- a downstream Android app can translate lifecycle and input itself without
  waiting for a dedicated first-party Android convenience crate.
- the sample owns a minimal EGL display / context / window-surface setup and
  wires it into `dear-imgui-sys::backend_shim::opengl3`.
- a downstream Android app can keep this ownership split and swap in its own
  EGL policy, renderer bootstrap, or packaging layer later.

## Why Input Is Mapped Manually Here

This template uses `android-activity` because it is a practical modern entry
point for Rust Android apps. Its input API does not expose raw `AInputEvent*`
values directly, so this sample feeds `dear-imgui-rs::Io` manually from the
safe event wrappers.

If your app uses a lower-level glue path that gives direct `AInputEvent*`
access, you can route those events to
`dear_imgui_sys::backend_shim::android::dear_imgui_backend_android_handle_input_event`
instead.

## Prerequisites

Install and configure these tools before using the smoke template:

- Rust with the Android target you want to build, at minimum
  `aarch64-linux-android` for the checked-in smoke path
- Android SDK with:
  - `platform-tools` for `adb`
  - `build-tools` for `aapt2`, `zipalign`, and `apksigner`
  - `platforms;android-36` (or another installed platform that matches your
    chosen `target_sdk_version`)
- Android NDK, exposed through `ANDROID_NDK_ROOT` or `ANDROID_NDK_HOME`
- Android SDK path, exposed through `ANDROID_HOME`
- A JDK, exposed through `JAVA_HOME`, with `keytool` available
- Python 3 if you want to use the repository-local `scripts/build-apk.py`
  helper

Install the default Rust Android target once:

```powershell
rustup target add aarch64-linux-android
```

Optional but useful during smoke testing:

- a real device or emulator visible in `adb devices`
- extra Rust targets such as `x86_64-linux-android` if you want multi-ABI
  packaging or emulator builds

This smoke path does not require Gradle.

## Build

PowerShell example for `aarch64-linux-android`:

```powershell
$ndk = $env:ANDROID_NDK_HOME
$llvm = Join-Path $ndk 'toolchains/llvm/prebuilt/windows-x86_64/bin'

$env:CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER = Join-Path $llvm 'aarch64-linux-android26-clang.cmd'
$env:CC_aarch64_linux_android = Join-Path $llvm 'aarch64-linux-android26-clang.cmd'
$env:CXX_aarch64_linux_android = Join-Path $llvm 'aarch64-linux-android26-clang++.cmd'
$env:AR_aarch64_linux_android = Join-Path $llvm 'llvm-ar.exe'

cargo check --manifest-path examples-android/dear-imgui-android-smoke/Cargo.toml --target aarch64-linux-android
```

## Build A Debug APK

This sample now includes the minimal `cargo-apk2` manifest metadata needed for
`NativeActivity` packaging.

Install the packager once:

```powershell
cargo install cargo-apk2 --locked
```

If you open a fresh shell before packaging, re-apply the Android linker setup
from the `Build` section above first.

Then build a debug APK:

```powershell
cargo apk2 build --manifest-path examples-android/dear-imgui-android-smoke/Cargo.toml
```

The resulting APK is written to:

```text
examples-android/dear-imgui-android-smoke/target/debug/apk/dear-imgui-android-smoke.apk
```

Notes:

- the sample currently declares `aarch64-linux-android` only, to keep the smoke
  path aligned with the ABI we verified locally
- the sample currently sets `min_sdk_version = 26` because the
  `ndk/nativewindow` wrapper path links `libnativewindow`, which is not present
  in older Android NDK API levels
- the sample opens a Dear ImGui smoke window plus the upstream demo window so
  you can immediately validate rendering and touch input on-device
- `include_cplusplus_shared = true` is enabled because `dear-imgui-sys`
  compiles C++ backend shim code on Android
- you do not need Gradle for this smoke path
- `cargo apk2 run` still requires a real device or emulator visible through
  `adb devices`

## Release Signing

For release packaging, prefer passing signing data through environment
variables or helper-script arguments instead of committing signing paths or
passwords into `Cargo.toml`.

Cross-platform helper example:

```text
python examples-android/dear-imgui-android-smoke/scripts/build-apk.py --profile release --targets aarch64-linux-android --keystore-path /path/to/release.keystore --keystore-password replace-me
```

The release APK is written to:

```text
examples-android/dear-imgui-android-smoke/target/release/apk/dear-imgui-android-smoke.apk
```

## Multi-ABI Packaging Strategy

There are two reasonable packaging strategies:

1. Per-ABI APKs, which keep outputs small and are easy to distribute through
   store-side ABI filtering.
2. A single universal APK, by editing `build_targets` in `Cargo.toml` to
   include multiple Rust Android targets.

This template keeps the checked-in manifest on a single ABI (`aarch64`) so the
smoke path stays fast and deterministic. For practical multi-ABI packaging, the
repository includes a helper script:

```text
python examples-android/dear-imgui-android-smoke/scripts/build-apk.py --targets aarch64-linux-android,x86_64-linux-android
```

Why the script exists:

- `cargo apk2 build --target ...` is a good per-ABI path
- but the default APK output path is reused across targets
- so per-target `--target-dir` isolation is important if you are building more
  than one ABI in a row

Common ABI sets:

- `aarch64-linux-android`: modern physical devices
- `x86_64-linux-android`: Android emulator
- `armv7-linux-androideabi`: legacy 32-bit devices if you still need them

Install extra Rust targets before building extra ABIs:

```powershell
rustup target add x86_64-linux-android armv7-linux-androideabi
```

The helper script also supports release packaging:

```text
python examples-android/dear-imgui-android-smoke/scripts/build-apk.py --profile release --targets aarch64-linux-android,x86_64-linux-android --keystore-path /path/to/release.keystore --keystore-password replace-me
```

## What You Still Need In A Real App

- Android packaging (`cargo-apk2`, Gradle, or your own build system)
- app-specific asset loading, persistence, IME behavior, and lifecycle policy

This ownership split is deliberate: the core crates provide reusable building
blocks, while the Android application still owns Android application setup.
