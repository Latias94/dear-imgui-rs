# dear-imgui-sdl3

SDL3 platform backend bindings for the `dear-imgui-rs` Rust crate.

This crate wraps the official Dear ImGui SDL3 platform backend and exposes a
safe-ish Rust API to drive Dear ImGui using SDL3 windows and events. It can be
used either with the upstream OpenGL3 renderer backend or with a custom Rust
renderer such as `dear-imgui-wgpu` or `dear-imgui-glow`.

Internally it vendors the following upstream sources:

- `imgui_impl_sdl3.cpp` / `imgui_impl_sdl3.h` (platform backend)
- `imgui_impl_opengl3.cpp` / `imgui_impl_opengl3.h` / `imgui_impl_opengl3_loader.h`
  (OpenGL3 renderer backend)

## Compatibility

| Item          | Version          |
|---------------|------------------|
| Crate         | 0.6.x            |
| dear-imgui-rs | 0.6.x            |
| SDL3          | 0.16.x (via `sdl3`) |

See also: [docs/COMPATIBILITY.md](https://github.com/Latias94/dear-imgui-rs/blob/main/docs/COMPATIBILITY.md)
for the full workspace matrix.

## Quick Start (SDL3 + OpenGL3 backend)

Minimal integration using the upstream OpenGL3 renderer:

```rust,no_run
use dear_imgui_rs::{Context, ConfigFlags};
use dear_imgui_sdl3 as imgui_sdl3_backend;
use sdl3::video::{GLProfile, SwapInterval, WindowPos};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let sdl = sdl3::init()?;
    let video = sdl.video()?;

    let gl_attr = video.gl_attr();
    gl_attr.set_context_version(3, 2);
    gl_attr.set_context_profile(GLProfile::Core);

    let mut window = video
        .window("SDL3 + OpenGL3 + Dear ImGui", 1280, 720)
        .opengl()
        .resizable()
        .high_pixel_density()
        .build()?;

    let gl_context = window.gl_create_context()?;
    window.gl_make_current(&gl_context)?;
    let _ = video.gl_set_swap_interval(SwapInterval::VSync);

    let mut imgui = Context::create();
    {
        let io = imgui.io_mut();
        let mut flags = io.config_flags();
        flags.insert(ConfigFlags::DOCKING_ENABLE);
        flags.insert(ConfigFlags::VIEWPORTS_ENABLE);
        io.set_config_flags(flags);
    }

    imgui_sdl3_backend::init_for_opengl(&mut imgui, &window, &gl_context, "#version 150")?;

    // Main loop sketch:
    // - Poll SDL3 events and feed them to the backend via process_sys_event().
    // - Call imgui_sdl3_backend::new_frame(&mut imgui);
    // - Build your UI.
    // - let draw_data = imgui.render();
    // - Clear your framebuffer.
    // - imgui_sdl3_backend::render(draw_data);
    // - window.gl_swap_window();

    Ok(())
}
```

For a complete multi-viewport sample using SDL3 + OpenGL3, see the
`dear-imgui-examples` crate:

```bash
cargo run -p dear-imgui-examples --bin sdl3_opengl_multi_viewport --features multi-viewport
```

## Platform Backend Only (for custom renderers)

When using a custom or Rust-native renderer (e.g. WGPU or Glow), you can use
only the SDL3 platform backend and handle rendering yourself:

```rust,no_run
use dear_imgui_rs::Context;
use dear_imgui_sdl3 as imgui_sdl3_backend;
use sdl3::video::Window;

fn init(imgui: &mut Context, window: &Window) -> Result<(), Box<dyn std::error::Error>> {
    // Initialize SDL3 platform backend only (no OpenGL3 renderer)
    imgui_sdl3_backend::init_for_other(imgui, window)?;
    Ok(())
}

fn frame(imgui: &mut Context) {
    // Start a new platform frame
    imgui_sdl3_backend::sdl3_new_frame(imgui);
    let ui = imgui.frame();

    // Build your UI...
    ui.text("SDL3 platform backend (custom renderer)");

    // Let your renderer handle draw_data
    let draw_data = imgui.render();
    // renderer.render(&draw_data);
}
```

`dear-imgui-examples` includes an SDL3 + WGPU example using this pattern:

```bash
cargo run -p dear-imgui-examples --bin sdl3_wgpu --features sdl3-backends
```

## Build Requirements

You need SDL3 headers and libraries available on your system. This crate uses
`sdl3` / `sdl3-sys` to handle linking; it only needs the headers to compile the
vendored C++ backend sources.

Header search order in `build.rs`:

1. Explicit `SDL3_INCLUDE_DIR` environment variable (expects `SDL3/SDL.h` under it).
2. `pkg-config sdl3` (if available), using reported include paths.
3. Common default roots such as `/opt/homebrew/include`, `/usr/local/include`,
   `/opt/local/include` (looking for `SDL3/SDL.h`).
4. Fallback: `DEP_SDL3_OUT_DIR/include` from `sdl3-sys` when SDL3 is built from source.

If none of the above succeed, the build fails with a descriptive message
explaining how to install SDL3 and/or set `SDL3_INCLUDE_DIR`.

## Multi-Viewport

The upstream SDL3 backend fully supports Dear ImGui multi-viewport when used
with the OpenGL3 renderer backend. The `sdl3_opengl_multi_viewport` example
demonstrates how to enable `ConfigFlags::VIEWPORTS_ENABLE` and let ImGui manage
additional OS windows via SDL3.

When used as platform-only with a custom renderer, multi-viewport support is
available as far as SDL3 platform callbacks are concerned, but your renderer is
responsible for creating and managing per-viewport render targets and swap
chains.

## Licensing

This crate is part of the `dear-imgui-rs` workspace and is licensed under
MIT OR Apache-2.0. Upstream Dear ImGui SDL3/OpenGL3 backend sources are
included under their original Dear ImGui licensing terms.
