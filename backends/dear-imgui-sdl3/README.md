# dear-imgui-sdl3

SDL3 platform backend with optional official renderer backends for the `dear-imgui-rs`
crate. This wraps the official Dear ImGui C++ backends:

- `imgui_impl_sdl3.cpp` (platform layer)
- `imgui_impl_opengl3.cpp` (OpenGL3 renderer, via the shared sys shim)
- `imgui_impl_sdlrenderer3.cpp` (SDLRenderer (canvas) renderer, via this crate's SDL3 shim)
- `imgui_impl_sdlgpu3.cpp` (SDLGPU3 renderer, via this crate's SDL3 shim)

and exposes a small, Rust-friendly API that plugs into an existing
`dear-imgui-rs::Context`.

Typical use cases:

- Drive Dear ImGui input from an SDL3 window (keyboard/mouse/gamepad/IME).
- Render Dear ImGui via the official OpenGL3 backend.
- Render Dear ImGui via the official SDLRenderer3 or SDLGPU3 backends.
- Use SDL3 only for the platform layer together with a Rust renderer
  (e.g. `dear-imgui-glow` or `dear-imgui-wgpu`).

## Notes

- One SDL3 Dear ImGui platform runtime may be active per process. SDL cursor, capture, IME, and
  hint state are process-wide in the official backend, so a second runtime returns
  `Sdl3BackendError::PlatformSessionOccupied` before native initialization mutates that state.
  After the first runtime shuts down, another Context may acquire the session.
- The upstream SDL3 backend source is compiled from the Dear ImGui tree packaged by
  `dear-imgui-sys`, while this crate keeps the SDL3-specific build logic, Rust API, and SDL3
  wrapper boundary.
- When `opengl3-renderer` is enabled, this crate uses the shared OpenGL3 backend shim exported by
  `dear-imgui-sys` instead of compiling a second local OpenGL3 wrapper layer.
- When `sdlrenderer3-renderer` or `sdlgpu3-renderer` is enabled, this crate compiles the
  matching official renderer source and local SDL3 shim. These renderer shims are not shared
  `dear-imgui-sys` features.

## Features

- `opengl3-renderer`: enables the shared official OpenGL3 renderer shim from `dear-imgui-sys`.
- `sdlrenderer3-renderer`: enables this crate's official SDLRenderer3 renderer shim.
- `sdlgpu3-renderer`: enables this crate's official SDLGPU3 renderer shim.
- `multi-viewport`: enables multi-viewport helpers (requires `dear-imgui-rs/multi-viewport`).

Until `0.16.0-alpha.2` is published, test any feature combination from `main`:

```toml
dear-imgui-sdl3 = { git = "https://github.com/Latias94/dear-imgui-rs", branch = "main", features = ["opengl3-renderer"] }
```

After publication, use the exact prerelease requirement in the combinations below.

Platform-only usage (SDL3 + WGPU/Glow, no official OpenGL3 renderer):

```toml
dear-imgui-sdl3 = { version = "=0.16.0-alpha.2", default-features = false }
```

Enable the official OpenGL3 renderer:

```toml
dear-imgui-sdl3 = { version = "=0.16.0-alpha.2", features = ["opengl3-renderer"] }
```

Enable the official SDLRenderer3 renderer:

```toml
dear-imgui-sdl3 = { version = "=0.16.0-alpha.2", features = ["sdlrenderer3-renderer"] }
```

Enable the official SDLGPU3 renderer:

```toml
dear-imgui-sdl3 = { version = "=0.16.0-alpha.2", features = ["sdlgpu3-renderer"] }
```

## Compatibility

| Item          | Version  |
|---------------|----------|
| Crate         | 0.16.0-alpha.2  |
| dear-imgui-rs | 0.16.0-alpha.2  |
| SDL3 crate    | 0.18.4   |
| sdl3-sys      | 0.6      |

See also: [docs/COMPATIBILITY.md](https://github.com/Latias94/dear-imgui-rs/blob/main/docs/COMPATIBILITY.md)
for the full workspace matrix.

## Quick Start

Minimal SDL3 + OpenGL3 flow (single window):

```rust,no_run
use dear_imgui_rs::{Context, Condition};
use dear_imgui_sdl3::{enable_native_ime_ui, Sdl3OpenGl3Backend};
use sdl3::video::GLProfile;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // SDL3 initialization (simplified)
    let sdl = sdl3::init()?;
    let video = sdl.video()?;
    let mut event_pump = sdl.event_pump()?;

    // Recommended on IME-heavy platforms (Windows/Asia locales)
    enable_native_ime_ui();

    // Configure GL context attributes
    {
        let gl_attr = video.gl_attr();
        gl_attr.set_context_profile(GLProfile::Core);
        gl_attr.set_context_version(3, 3);
    }

    let window = video
        .window("Dear ImGui + SDL3 + OpenGL", 1280, 720)
        .opengl()
        .resizable()
        .build()?;
    let gl_context = window.gl_create_context()?;
    window.gl_make_current(&gl_context)?;

    // ImGui context
    let mut imgui = Context::create();

    // Initialize SDL3 + OpenGL3 backends. The owner and Context share teardown state.
    // SAFETY: Window and GLContext are declared before Context, so they outlive its teardown.
    let mut sdl3_backend = unsafe {
        Sdl3OpenGl3Backend::init(&mut imgui, &window, &gl_context, "#version 150")?
    };

    'main: loop {
        // 1) Poll SDL3 events and feed ImGui
        for event in event_pump.poll_iter() {
            if sdl3_backend.process_event(&mut imgui, &event)? {
                // ImGui consumed the event; continue if you do not need it.
            }

            // Handle your own events or quit logic as needed...
        }

        // 2) Start a new frame for the SDL3 + OpenGL backends
        sdl3_backend.new_frame(&mut imgui)?;
        let frame = imgui.begin_frame();
        let ui = frame.ui();

        // 3) Build UI
        ui.window("Hello")
            .size([400.0, 300.0], Condition::FirstUseEver)
            .build(|| {
                ui.text("ImGui + SDL3 + OpenGL3");
            });

        // 4) Render via OpenGL backend
        let pending = frame.render(sdl3_backend.consumer());
        unsafe {
            use sdl3::video::Window;
            use sdl3::video::GLContext;
            // The context passed at initialization must be current for every OpenGL operation.
            window.gl_make_current(&gl_context)?;
        }
        sdl3_backend.render(pending)?;
        window.gl_swap_window();
    }
}
```

APIs of interest (see `src/lib.rs` for full docs):

- `Sdl3OpenGl3Backend` and `SdlRenderer3Backend`:
  RAII renderer owners whose shared runtime retains the Context's renderer consumer through
  explicit or Context-owned teardown, processes request-bound texture feedback, and consumes
  `PendingFrame` values. Each owner exposes its non-cloneable synchronous consumer through
  `consumer()`.
  OpenGL multi-viewport routes can call `reconcile_frame(...)`, run secondary platform-window
  callbacks, then transfer that capability into `render_reconciled(...)` for the main viewport.
  OpenGL users must keep the initialized context current for renderer
  operations. SDLRenderer has one normal `render(...)` path and rejects a `WindowCanvas` backed by
  another raw renderer before texture or draw work starts.
- `SdlGpu3RendererBackend`:
  RAII renderer owner for SDL3 + SDLGPU3. Unsafe `prepare_render(...)` returns an
  `SdlGpu3PreparedFrame` that keeps the renderer and Context frame alive until its unsafe
  `render(...)` call inside the SDL GPU render pass. The calls are unsafe because `sdl3` does not
  expose enough provenance to verify that the command buffer, render pass, and initialized device
  share one native owner. `reconcile_frame(...)` is a surface-independent preparation step for
  applications that must render secondary viewports before attempting to acquire the main
  swapchain image. After the secondary callbacks, `prepare_render_reconciled(...)` prepares that
  same linear capability for the main pass. The methods consume their frame capabilities, so the
  same epoch cannot be reconciled or rendered twice.
- `Sdl3PlatformBackend`:
  platform-only RAII owner for applications that provide a separate renderer. It intentionally
  does not claim a renderer consumer. Construct it with unsafe `Sdl3PlatformBackend::init_for_other`,
  `init_platform_for_opengl`, `init_for_vulkan`, `init_for_metal`, Windows-only `init_for_d3d`,
  `init_for_sdl_gpu`, or `init_for_sdl_renderer` as appropriate. Every constructor is unsafe
  because the upstream backend retains native window and graphics pointers beyond the call; keep
  those owners alive until explicit shutdown succeeds or the Context finishes attachment teardown.
  Vulkan renderers should request the typed `acquire_vulkan_surface_provider` capability through
  their integration instead of caching `Platform_CreateVkSurface`. The exclusive provider is tied
  to one SDL runtime generation, validates each viewport immediately before native entry, and
  blocks platform shutdown until all renderer-owned Vulkan surfaces have been destroyed.
- `shutdown(&mut self, &mut Context)`:
  an idempotent owner method that closes any open frame before reporting actionable teardown and
  callback-ownership errors. Dropping the owner defers native cleanup to the Context attachment,
  because Drop cannot safely normalize a frame without the mutable Context. Managed texture proxy
  state is held by that attachment as well, so uninstalled native allocations remain destroyable
  after the Rust owner is dropped. Official renderer `shutdown(...)` and
  `destroy_device_objects(...)` validate their Context-bound synchronous consumer before changing
  callbacks or native resources, and release that consumer only after teardown succeeds.
  A platform-only owner also rejects explicit shutdown while an external renderer attachment is
  active. This preflight runs before the current frame or native SDL state changes; shut down the
  renderer first, then retry platform shutdown. Context-owned teardown preserves the same ordered
  renderer-before-platform contract automatically.
- `poll_fault()`:
  returns deferred platform callback failures without unwinding through native code. Ordinary
  owner methods also surface the oldest pending fault before entering SDL.
- `process_event(&mut Context, &sdl3::event::Event)`:
  the safe event path for normal `EventPump` loops. Pointer-bearing SDL payloads such as text input
  remain owned while the official backend consumes them.
- `unsafe process_raw_event(&mut Context, &SDL_Event)`:
  the low-level escape hatch for SDL callbacks and foreign event loops. The caller must prove the
  active union variant, payload pointer lifetime, SDL thread, and backend provenance contracts.
- Runtime entry checks are scope-bound. Callback replacement detected while an operation returns
  an error or unwinds remains queued for `poll_fault()` instead of being skipped by an early exit.
The free renderer initialization, render, texture-update, and device-object functions were
removed. They allowed callers to bypass the Context-owned renderer epoch and write directly into
native texture state. Call the owning backend's `shutdown(...)` method when shutdown errors need to
be reported; otherwise retain the Context so its attachment can complete deferred cleanup.

Official renderer teardown is transactional: it first obtains
`Context::prepare_renderer_texture_reset(&consumer)` while its managed texture map is still
intact, releases the upstream renderer resources, then commits the permit. An outstanding frame or
a consumer mismatch rejects preparation before SDL resources or Context bindings change.

Every owning backend registers its platform role with the Context before native initialization.
Composite owners also register a renderer role, so Context-first teardown always releases renderer
resources before platform windows. Callback and platform-data claims are transactional: foreign
replacements are preserved during shutdown and returned as typed faults instead of being silently
cleared.

## SDL3 & Build Requirements

The crate depends on:

- `sdl3` and `sdl3-sys` for SDL3 bindings.
- A system SDL3 installation **or** a build-from-source configuration (OS-dependent).

Build behavior is aligned with `Cargo.toml`:

- **Linux / Windows**
  - The `sdl3` dependency is configured with `features = ["build-from-source"]`.
  - This means SDL3 is downloaded and built via CMake and does **not** require
    a pre-installed system SDL3 library.
  - The build script can also discover SDL3 headers from `SDL3_INCLUDE_DIR`,
    pkg-config, or vcpkg. Linking remains owned by `sdl3` / `sdl3-sys`.
  - You still need a working C toolchain (compiler, linker, CMake).

- **macOS**
  - The crate expects a discoverable SDL3 install (for example via Homebrew):
    - `brew install sdl3`
  - SDL3 headers are typically found under:
    - `/opt/homebrew/include/SDL3/SDL.h` (Apple Silicon)
    - `/usr/local/include/SDL3/SDL.h` (Intel / custom setups)
  - pkg-config and vcpkg are also supported when they provide SDL3 metadata.
  - Linking parameters are handled by `sdl3-sys` / `sdl3`; this crate only
    needs the headers to build the C++ backend sources.

- **iOS**
  - The crate depends on the safe `sdl3` crate on iOS targets as well, but it
    does **not** force one SDL3 acquisition strategy.
  - Treat this as an app-owned integration route:
    - provide SDL3 headers yourself and set `SDL3_INCLUDE_DIR` when discovery is not enough
    - make the final application dependency graph enable `sdl3/build-from-source`
    - or use an app-owned `SDL3.xcframework` / `sdl3/link-framework` setup
  - The consuming app still owns:
    - SDL3 framework packaging
    - the host `main` entry point (`SDL_RunApp` or an `sdl3-main` callback setup)
    - Xcode signing and bundle layout
  - A repository-owned integration shape lives in
    `examples-ios/dear-imgui-ios-sdl3-smoke/`.
  - That smoke template now includes a checked-in Xcode host stub which keeps
    the packaging boundary explicit: it can either consume an app-owned
    `SDL3.framework` / `SDL3.xcframework`, or build `SDL3.framework` from the
    upstream SDL source distributed through `sdl3-src`.

- **Android**
  - The crate depends on the safe `sdl3` crate on Android targets as well, but it
    does **not** force `sdl3/build-from-source`.
  - Android application / NDK / activity packaging still belongs to the consuming
    application.
  - Treat this as a supported integration direction, not a zero-config turn-key path.
  - There are two supported ways to satisfy the SDL3 headers needed by this crate:
    - provide SDL3 headers yourself and set `SDL3_INCLUDE_DIR` when discovery is not enough
    - make the final application dependency graph enable `sdl3/build-from-source`, so
      `sdl3-sys` exports headers via `DEP_SDL3_OUT_DIR`

### Header Search Order and `SDL3_INCLUDE_DIR`

`build.rs` locates SDL3 headers in the following order:

1. `SDL3_INCLUDE_DIR` environment variable (highest priority).
2. `DEP_SDL3_INCLUDE_PATH` / `DEP_SDL3_INCLUDE_DIR` from an upstream SDL3 build
   script, when present.
3. `DEP_SDL3_OUT_DIR/include` from `sdl3-sys` build-from-source.
4. Cargo's target `build/sdl3-sys-*/out/include` cache, used as a fallback when
   Cargo metadata is not directly available to the current build script.
5. `pkg-config sdl3` with Cargo link metadata disabled.
6. vcpkg `sdl3` with Cargo link metadata disabled.
7. A small set of common default paths (e.g. `/opt/homebrew/include`,
   `/usr/local/include`, `/opt/local/include`).

Only header include paths are consumed from pkg-config/vcpkg here. Link flags
and SDL3 runtime selection remain the responsibility of `sdl3` / `sdl3-sys` or
the final application.

**1. Explicit `SDL3_INCLUDE_DIR`**

Set this when SDL3 is installed in a non-standard location:

- macOS (custom Homebrew prefix):

  ```bash
  export SDL3_INCLUDE_DIR=/opt/homebrew/include
  ```

- Linux (hand-built SDL3):

  ```bash
  export SDL3_INCLUDE_DIR=/opt/sdl3/include
  ```

- Windows (PowerShell, headers under `C:\libs\SDL3\include`):

  ```powershell
  $env:SDL3_INCLUDE_DIR="C:\libs\SDL3\include"
  ```

`build.rs` adds this directory to the C/C++ include path and expects to find
`SDL3/SDL.h` under it.

This is the preferred Android route when your application already owns the SDL3
integration (Gradle/NDK/Prefab/custom packaging) and just needs
`dear-imgui-sdl3` to compile the official Dear ImGui SDL3 backend sources.

**2. `sdl3-sys` build metadata**

When the final dependency graph enables `sdl3/build-from-source`, `sdl3-sys`
builds SDL3 from source and exports header locations through Cargo metadata.
This crate reuses those headers automatically.

This is especially useful when the application wants Cargo to drive the SDL3
build instead of relying on a system install.

**3. `pkg-config sdl3`**

If `SDL3_INCLUDE_DIR` is not set, the build script tries:

```bash
pkg-config --cflags sdl3
```

On success, the reported `include_paths` are added to the compiler flags. This
is the preferred route for most Linux distributions and pkg-config-enabled
macOS setups.

**4. vcpkg `sdl3`**

If pkg-config is unavailable, the build script tries vcpkg's `sdl3` port. On
Windows/MSVC, install the triplet that matches vcpkg-rs' selection, for example:

```powershell
vcpkg install sdl3:x64-windows-static-md
```

If you use a dynamic vcpkg triplet such as `x64-windows`, set
`VCPKGRS_DYNAMIC=1` and make the SDL3 DLLs available to the final executable.

**5. Fallback paths**

If the explicit environment, Cargo metadata, pkg-config, and vcpkg checks fail,
`build.rs` tries a few common include roots (such as Homebrew / MacPorts
locations) and looks for `SDL3/SDL.h` there.

### When Headers Cannot Be Found

If the build script cannot locate SDL3 headers, it will panic with a message
similar to:

> dear-imgui-sdl3: could not find SDL3 headers. \
> Install SDL3 development files through pkg-config/vcpkg, set \
> SDL3_INCLUDE_DIR to the SDL3 include path, or make the final \
> dependency graph enable `sdl3/build-from-source`.

To fix this:

1. Install SDL3 development packages and verify `pkg-config sdl3` works, **or**
2. Set `SDL3_INCLUDE_DIR` to the correct include root, **or**
3. Install vcpkg `sdl3` with a triplet that matches your Rust target, **or**
4. Enable `sdl3/build-from-source` in the final dependency graph so `sdl3-sys`
   exports SDL3 headers via `DEP_SDL3_OUT_DIR`.

## Android Integration Notes

Android integration in this crate should be understood as a low-friction
path, not as a turn-key Android application template.

Recommended model:

1. The consuming application owns SDL3 Android packaging, entry-point, and NDK
   toolchain decisions.
2. `dear-imgui-sdl3` owns the Dear ImGui SDL3 backend wrapper and can reuse
   whatever SDL3 headers the application chose to provide.
3. If the application wants Cargo to build SDL3 from source, it should add a
   direct `sdl3` dependency with `features = ["build-from-source"]`.

Example:

```toml
[dependencies]
dear-imgui-sdl3 = { version = "=0.16.0-alpha.2", features = ["opengl3-renderer"] }
sdl3 = { version = "0.18", features = ["build-from-source"] }
```

On Android, that route usually also requires the standard SDL/NDK build
toolchain environment expected by `sdl3-sys`, for example:

- `ANDROID_NDK` / `ANDROID_NDK_HOME`
- `ANDROID_ABI` / `CMAKE_ANDROID_ARCH_ABI` (for example `arm64-v8a`)
- `CMAKE_TOOLCHAIN_FILE`
- `CMAKE_GENERATOR=Ninja`
- a working `ninja` executable

In practice, this usually means the final application should drive the Android
build through a tool that already owns the ABI/toolchain contract
(`cargo-ndk`, Gradle+CMake, or an equivalent application build system) instead
of expecting `dear-imgui-sdl3` alone to infer the full Android CMake setup.

### Common Android Failure Modes

If you choose the `sdl3/build-from-source` route on Android, the most common
failures are application-toolchain issues rather than `dear-imgui-sdl3`
wrapper issues:

- `dear-imgui-sdl3: could not find SDL3 headers`
  - Your final dependency graph did not actually enable `sdl3/build-from-source`,
    or your application did not provide `SDL3_INCLUDE_DIR`.
- `CMake was unable to find a build program corresponding to "Ninja"`
  - `sdl3-sys` is trying to drive SDL3's Android CMake build, but your
    application environment did not make `ninja` available to CMake.
- Android ABI mismatch during CMake compiler checks
  - Example shape: CMake defaults to `armv7` while Rust is building
    `aarch64-linux-android`.
  - In that case the application usually needs to set
    `ANDROID_ABI=arm64-v8a` and/or `CMAKE_ANDROID_ARCH_ABI=arm64-v8a`, or use
    a tool such as `cargo-ndk` / Gradle+CMake that manages those values.

The important boundary is:

- `dear-imgui-sdl3` can reuse SDL3 once the application has made SDL3 headers
  and toolchain metadata available
- `dear-imgui-sdl3` does not try to become the Android application build system

### PowerShell Sketch For `aarch64-linux-android`

If your application owns the Android build directly from Cargo, the setup often
looks roughly like this before you invoke your actual app build command:

```powershell
$ndk = $env:ANDROID_NDK_HOME
$llvm = Join-Path $ndk 'toolchains/llvm/prebuilt/windows-x86_64/bin'

$env:ANDROID_NDK = $ndk
$env:ANDROID_ABI = 'arm64-v8a'
$env:CMAKE_ANDROID_ARCH_ABI = 'arm64-v8a'
$env:CMAKE_TOOLCHAIN_FILE = Join-Path $ndk 'build/cmake/android.toolchain.cmake'
$env:CMAKE_GENERATOR = 'Ninja'

$env:CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER = Join-Path $llvm 'aarch64-linux-android24-clang.cmd'
$env:CC_aarch64_linux_android = Join-Path $llvm 'aarch64-linux-android24-clang.cmd'
$env:CXX_aarch64_linux_android = Join-Path $llvm 'aarch64-linux-android24-clang++.cmd'
```

This is not something `dear-imgui-sdl3` can infer safely on behalf of the
application. The final app build must own it.

If you do not want SDL3 at all, you can still build an Android backend manually
on top of `dear-imgui-rs` plus `dear-imgui-sys::backend_shim::{android, opengl3}`.

## IME and Gamepad Configuration

The underlying SDL3 ImGui backend supports IME and gamepad input. This crate
exposes a couple of small helpers to configure them.

### IME UI

On platforms with heavy IME usage (e.g. Chinese/Japanese/Korean locales), it is
recommended to enable the native IME UI before creating any SDL3 windows:

```rust
// Call this before creating SDL3 windows.
dear_imgui_sdl3::enable_native_ime_ui();
```

This is a convenience wrapper over `SDL_HINT_IME_SHOW_UI`, and failures are
treated as non-fatal.

### Gamepad Mode

By default, the SDL3 backend opens the first available gamepad and feeds its
state into Dear ImGui (the upstream default behavior).

You can switch to a mode where **all** detected gamepads are opened and merged:

```rust
use dear_imgui_sdl3::GamepadMode;

// After initializing an owning backend:
sdl3_backend.set_gamepad_mode(&mut imgui, GamepadMode::AutoAll)?;
```

This is useful for local multiplayer setups or testing environments.

For advanced use cases, you can also opt into **manual** gamepad selection by
providing raw `SDL_Gamepad*` handles opened by your application:

```rust
// Safety: gamepads must be valid, opened SDL_Gamepad pointers.
unsafe {
    sdl3_backend.set_gamepad_mode_manual(&mut imgui, &[gamepad1, gamepad2])?;
}
```

### Mouse Capture Mode

Mouse capture keeps drag coordinates updating after the pointer leaves an SDL window. The official
backend enables it immediately on capable desktop drivers, except on X11 where it waits until a drag
starts so a debugger break is less likely to leave the desktop pointer captured. Applications can
override that policy through any owning backend:

```rust
use dear_imgui_sdl3::MouseCaptureMode;

sdl3_backend.set_mouse_capture_mode(&mut imgui, MouseCaptureMode::EnabledAfterDrag)?;
```

`MouseCaptureMode::Disabled` also releases an active capture. Changing this policy cannot add
global mouse or native viewport capabilities to a video driver that does not provide them.

## Examples

The workspace includes several examples that use this backend:

Multi-viewport status on SDL3:

Native OS viewports depend on the active SDL video driver, not only the Cargo feature. The embedded
official backend currently sets `BackendFlags::PLATFORM_HAS_VIEWPORTS` for the Windows, Cocoa, X11,
DIVE, and VMAN drivers. It intentionally does not set that capability on Wayland, whose compositor
security model does not provide the global pointer position and capture behavior required by Dear
ImGui's current platform-viewport contract. On Wayland, docking and dragging continue to work
inside the main SDL window, but detached panels remain in that host window instead of becoming
independent OS windows. Applications can inspect `imgui.io().backend_flags()` after backend
initialization when they need to report this runtime degradation.

For OpenGL viewports the Rust-owned callback wrapper verifies that each secondary window has a
distinct current GL context and restores the previous window, context, and
`SDL_GL_SHARE_WITH_CURRENT_CONTEXT` attribute before returning. Secondary contexts default to
`Sdl3OpenGlViewportSwapInterval::Immediate`, matching the upstream behavior that avoids serial
VSync waits across several platform windows. Use
`init_with_viewport_swap_interval(...)` or
`init_platform_for_opengl_with_viewport_swap_interval(...)` to choose `VSync`, `Adaptive`, or
`MatchMain`. Swap-interval selection is best effort: if a driver rejects the requested timing after
the secondary context is valid, the viewport keeps the driver's default timing. Native GL context,
state-restoration, and SDL_GPU failures are deferred through `poll_fault()`; a partially initialized
viewport is closed instead of being published as usable.

- **SDL3 + OpenGL3**: multi-viewport is provided by the upstream C++ backends and
  considered stable for desktop use.
- **SDL3 + Glow**: multi-viewport is experimental but functional on native targets.
- **SDL3 + WGPU**: multi-viewport is experimental on native targets; WebGPU/wasm is
  single-window to match upstream `imgui_impl_wgpu`.

- SDL3 + OpenGL3, multi-viewport:

  ```bash
  cargo run -p dear-imgui-examples --bin sdl3_opengl_multi_viewport --features "multi-viewport sdl3-opengl3"
  ```

- SDL3 + OpenGL3, multi-viewport (Glow renderer wrapper):

  ```bash
  cargo run -p dear-imgui-examples --bin sdl3_glow_multi_viewport --features sdl3-glow-multi-viewport
  ```

- SDL3 + WGPU, single-window:

  ```bash
  cargo run -p dear-imgui-examples --bin sdl3_wgpu --features sdl3-platform
  ```

- SDL3 + WGPU, multi-viewport (experimental, native only):

  ```bash
  cargo run -p dear-imgui-examples --bin sdl3_wgpu_multi_viewport --features sdl3-wgpu-multi-viewport
  ```

Note: WGPU multi-viewport support is experimental and only available on native targets
via `dear-imgui-wgpu/multi-viewport-sdl3`. WebGPU/wasm remains single-window to match
upstream `imgui_impl_wgpu`.
