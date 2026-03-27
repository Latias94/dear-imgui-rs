# dear-imgui-sdl3

SDL3 platform backend (with optional OpenGL3 renderer) for the `dear-imgui-rs`
crate. This wraps the official Dear ImGui C++ backends:

- `imgui_impl_sdl3.cpp` (platform layer)
- `imgui_impl_opengl3.cpp` (OpenGL3 renderer, via the shared sys shim)
- `imgui_impl_sdlrenderer3.cpp` (SDLRenderer (canvas) renderer, via the shared sys shim)

and exposes a small, Rust-friendly API that plugs into an existing
`dear-imgui-rs::Context`.

Typical use cases:

- Drive Dear ImGui input from an SDL3 window (keyboard/mouse/gamepad/IME).
- Render Dear ImGui via the official OpenGL3 backend.
- Use SDL3 only for the platform layer together with a Rust renderer
  (e.g. `dear-imgui-glow` or `dear-imgui-wgpu`).

## Notes

- This crate assumes a single Dear ImGui context and a single SDL3 event pump. The upstream
  SDL3 backend notes that multi-context usage is not well tested and may be dysfunctional.
- The upstream SDL3 backend source is compiled from the Dear ImGui tree packaged by
  `dear-imgui-sys`, while this crate keeps the SDL3-specific build logic, Rust API, and SDL3
  wrapper boundary.
- When `opengl3-renderer` is enabled, this crate uses the shared OpenGL3 backend shim exported by
  `dear-imgui-sys` instead of compiling a second local OpenGL3 wrapper layer.

## Features

- `opengl3-renderer`: enables the shared official OpenGL3 renderer shim from `dear-imgui-sys`.
- `multi-viewport`: enables multi-viewport helpers (requires `dear-imgui-rs/multi-viewport`).

Platform-only usage (SDL3 + WGPU/Glow, no official OpenGL3 renderer):

```toml
dear-imgui-sdl3 = { version = "0.10", default-features = false }
```

Enable the official OpenGL3 renderer:

```toml
dear-imgui-sdl3 = { version = "0.10", features = ["opengl3-renderer"] }
```

Enable the official SDLRenderer3 renderer:

```toml
dear-imgui-sdl3 = { version = "0.10", features = ["sdlrenderer3-renderer"] }
```

## Compatibility

| Item          | Version  |
|---------------|----------|
| Crate         | 0.10.x   |
| dear-imgui-rs | 0.10.x   |
| SDL3 crate    | 0.17     |
| sdl3-sys      | 0.6      |

See also: [docs/COMPATIBILITY.md](https://github.com/Latias94/dear-imgui-rs/blob/main/docs/COMPATIBILITY.md)
for the full workspace matrix.

## Quick Start

Minimal SDL3 + OpenGL3 flow (single window):

```rust,no_run
use dear_imgui_rs::{Context, Condition};
use dear_imgui_sdl3::{
    enable_native_ime_ui, init_for_opengl, new_frame, render, sdl3_poll_event_ll,
    process_sys_event,
};
use sdl3::{video::GLProfile, EventPump};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // SDL3 initialization (simplified)
    let sdl = sdl3::init()?;
    let video = sdl.video()?;

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

    // Initialize SDL3 + OpenGL3 backends
    init_for_opengl(&mut imgui, &window, &gl_context, "#version 150")?;

    let mut event_pump: EventPump = sdl.event_pump()?;
    'main: loop {
        // 1) Poll SDL3 events and feed ImGui
        while let Some(event) = sdl3_poll_event_ll() {
            if process_sys_event(&event) {
                // ImGui consumed the event; continue if you do not need it.
            }

            // Handle your own events or quit logic as needed...
        }

        // 2) Start a new frame for the SDL3 + OpenGL backends
        new_frame(&mut imgui);
        let ui = imgui.frame();

        // 3) Build UI
        ui.window("Hello")
            .size([400.0, 300.0], Condition::FirstUseEver)
            .build(|| {
                ui.text("ImGui + SDL3 + OpenGL3");
            });

        // 4) Render via OpenGL backend
        let draw_data = imgui.render();
        unsafe {
            use sdl3::video::Window;
            use sdl3::video::GLContext;
            // Make context current if needed, clear framebuffer, etc.
            // window.gl_make_current(&gl_context)?;
        }
        render(&draw_data);
        window.gl_swap_window();
    }
}
```

APIs of interest (see `src/lib.rs` for full docs):

- `init_for_opengl(&mut Context, &Window, &GLContext, &str)`:
  initialize SDL3 platform + OpenGL3 renderer.
- `init_for_opengl_default(&mut Context, &Window, &GLContext)`:
  initialize SDL3 platform + OpenGL3 renderer using the upstream default GLSL version.
- `init_platform_for_opengl(&mut Context, &Window, &GLContext)`:
  initialize only the platform backend (for use with Rust OpenGL renderers).
- `init_for_other(&mut Context, &Window)`:
  initialize only the platform backend (for use with WGPU or other APIs).
- `init_for_vulkan(&mut Context, &Window)` / `init_for_metal(&mut Context, &Window)` /
  `init_for_d3d(&mut Context, &Window)` / `init_for_sdl_gpu(&mut Context, &Window)`:
  initialize the SDL3 platform backend for specific renderer families (use `init_for_vulkan` for Vulkan multi-viewport support).
- `unsafe init_for_sdl_renderer(&mut Context, &Window, *mut SDL_Renderer)`:
  initialize SDL3 platform backend for SDL_Renderer-based renderers.
- `shutdown_for_opengl(&mut Context)` / `shutdown(&mut Context)`:
  shut down the backends before destroying the ImGui context or window.
- `new_frame(&mut Context)`:
  begin a frame for SDL3 + OpenGL3.
- `sdl3_new_frame(&mut Context)`:
  begin a frame for SDL3 platform only.
- `sdl3_poll_event_ll() -> Option<SDL_Event>` and
  `process_sys_event(&SDL_Event) -> bool`:
  low-level event polling/processing helpers.
- `render(&DrawData)`:
  render Dear ImGui draw data via the OpenGL3 backend.
- `update_texture(&mut TextureData)`:
  advanced helper that delegates texture updates to `ImGui_ImplOpenGL3_UpdateTexture`.
- `create_device_objects()` / `destroy_device_objects()`:
  advanced OpenGL3 helpers mirroring upstream device object management.

## SDL3 & Build Requirements

The crate depends on:

- `sdl3` and `sdl3-sys` for SDL3 bindings.
- A system SDL3 installation **or** a build-from-source configuration (OS-dependent).

Build behavior is aligned with `Cargo.toml`:

- **Linux / Windows**
  - The `sdl3` dependency is configured with `features = ["build-from-source"]`.
  - This means SDL3 is downloaded and built via CMake and does **not** require
    a pre-installed system SDL3 library.
  - You still need a working C toolchain (compiler, linker, CMake).

- **macOS**
  - The crate expects a system SDL3 install (for example via Homebrew):
    - `brew install sdl3`
  - SDL3 headers are typically found under:
    - `/opt/homebrew/include/SDL3/SDL.h` (Apple Silicon)
    - `/usr/local/include/SDL3/SDL.h` (Intel / custom setups)
  - Linking parameters are handled by `sdl3-sys` / `sdl3`; this crate only
    needs the headers to build the C++ backend sources.

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

For cases where a system SDL3 is used, `build.rs` locates the headers in the
following order:

1. `SDL3_INCLUDE_DIR` environment variable (highest priority).
2. `pkg-config sdl3` (if available).
3. A small set of common default paths (e.g. `/opt/homebrew/include`,
   `/usr/local/include`, `/opt/local/include`).

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

**2. `pkg-config sdl3`**

If `SDL3_INCLUDE_DIR` is not set, the build script tries:

```bash
pkg-config --cflags sdl3
```

On success, the reported `include_paths` are added to the compiler flags. This
is the preferred route for most Linux distributions and pkg-config-enabled
macOS setups.

**3. Fallback paths**

If both the environment variable and pkg-config checks fail, `build.rs` tries
a few common include roots (such as Homebrew / MacPorts locations) and looks
for `SDL3/SDL.h` there.

**4. `sdl3/build-from-source` feature unification**

If the final dependency graph enables `sdl3/build-from-source`, `sdl3-sys`
builds SDL3 from source and exports `DEP_SDL3_OUT_DIR`. This crate will then
reuse `DEP_SDL3_OUT_DIR/include` automatically.

This is especially useful when the application wants Cargo to drive the SDL3
build instead of relying on a system install.

### When Headers Cannot Be Found

If the build script cannot locate SDL3 headers, it will panic with a message
similar to:

> dear-imgui-sdl3: could not find SDL3 headers. \
> Install SDL3 development files (e.g. `brew install sdl3`) \
> or set SDL3_INCLUDE_DIR to the SDL3 include path.

To fix this:

1. Install SDL3 development packages and verify `pkg-config sdl3` works, **or**
2. Set `SDL3_INCLUDE_DIR` to the correct include root, **or**
3. Enable `sdl3/build-from-source` in the final dependency graph so `sdl3-sys`
   exports SDL3 headers via `DEP_SDL3_OUT_DIR`.

## Android Integration Notes

Android support in this crate should be understood as a low-friction integration
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
dear-imgui-sdl3 = { version = "0.10", features = ["opengl3-renderer"] }
sdl3 = { version = "0.17", features = ["build-from-source"] }
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
use dear_imgui_sdl3::{set_gamepad_mode, GamepadMode};

// After init_for_opengl/init_for_other/init_platform_for_opengl:
set_gamepad_mode(GamepadMode::AutoAll);
```

This is useful for local multiplayer setups or testing environments.

For advanced use cases, you can also opt into **manual** gamepad selection by
providing raw `SDL_Gamepad*` handles opened by your application:

```rust
// Safety: gamepads must be valid, opened SDL_Gamepad pointers.
unsafe {
    dear_imgui_sdl3::set_gamepad_mode_manual(&[gamepad1, gamepad2]);
}
```

## Examples

The workspace includes several examples that use this backend:

Multi-viewport status on SDL3:

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
  cargo run -p dear-imgui-examples --bin sdl3_glow_multi_viewport --features "multi-viewport sdl3-platform"
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
