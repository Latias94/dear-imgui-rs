# dear-imgui-sdl3

SDL3 platform backend (with optional OpenGL3 renderer) for the `dear-imgui-rs`
crate. This wraps the official Dear ImGui C++ backends:

- `imgui_impl_sdl3.cpp` (platform layer)
- `imgui_impl_opengl3.cpp` (OpenGL3 renderer)

and exposes a small, Rust-friendly API that plugs into an existing
`dear-imgui-rs::Context`.

Typical use cases:

- Drive Dear ImGui input from an SDL3 window (keyboard/mouse/gamepad/IME).
- Render Dear ImGui via the official OpenGL3 backend.
- Use SDL3 only for the platform layer together with a Rust renderer
  (e.g. `dear-imgui-glow` or `dear-imgui-wgpu`).

## Compatibility

| Item          | Version  |
|---------------|----------|
| Crate         | 0.6.x    |
| dear-imgui-rs | 0.6.x    |
| SDL3 crate    | 0.16.2   |
| sdl3-sys      | 0.5      |

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
- `init_platform_for_opengl(&mut Context, &Window, &GLContext)`:
  initialize only the platform backend (for use with Rust OpenGL renderers).
- `init_for_other(&mut Context, &Window)`:
  initialize only the platform backend (for use with WGPU or other APIs).
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

### When Headers Cannot Be Found

If the build script cannot locate SDL3 headers, it will panic with a message
similar to:

> dear-imgui-sdl3: could not find SDL3 headers. \
> Install SDL3 development files (e.g. `brew install sdl3`) \
> or set SDL3_INCLUDE_DIR to the SDL3 include path.

To fix this:

1. Install SDL3 development packages and verify `pkg-config sdl3` works, **or**
2. Set `SDL3_INCLUDE_DIR` to the correct include root.

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

## Examples

The workspace includes several examples that use this backend:

- SDL3 + OpenGL3, multi-viewport:

  ```bash
  cargo run -p dear-imgui-examples --bin sdl3_opengl_multi_viewport --features "multi-viewport sdl3-backends"
  ```

- SDL3 + OpenGL3, multi-viewport (Glow renderer wrapper):

  ```bash
  cargo run -p dear-imgui-examples --bin sdl3_glow_multi_viewport --features "multi-viewport sdl3-backends"
  ```

- SDL3 + WGPU, single-window:

  ```bash
  cargo run -p dear-imgui-examples --bin sdl3_wgpu --features sdl3-backends
  ```

Note: multi-viewport support for WebGPU/WGPU follows the upstream
`imgui_impl_wgpu` design and is currently **not** enabled; for multi-viewport
use SDL3 + OpenGL3 or a winit + OpenGL route instead.
