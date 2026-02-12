# dear-imgui-glow

Glow (OpenGL) renderer for Dear ImGui.

<p align="center">
  <img src="https://github.com/user-attachments/assets/a9212184-d9c5-4e16-820a-cd98b471a6ea" alt="Docking (OpenGL/Glow)" width="75%"/>
  <br/>
</p>

## Quick Start

```rust
use dear_imgui_rs::Context;
use dear_imgui_glow::GlowRenderer;
use glow::HasContext;

let gl = unsafe { glow::Context::from_loader_function(|s| loader.get_proc_address(s) as *const _) };
let mut imgui = Context::create();
let mut renderer = GlowRenderer::new(gl, &mut imgui)?;

// per-frame
let draw_data = imgui.render();
renderer.new_frame()?;
renderer.render(&draw_data)?;
```

## What You Get

- ImGui v1.92 texture system integration (font atlas upload + dynamic texture updates)
- OpenGL 2.1+/ES 2.0+ compatible shaders and state setup
- Full GL state backup/restore around ImGui rendering

## sRGB / Gamma

- Pipeline choice
  - Linear FB: keep `FRAMEBUFFER_SRGB` disabled (default). Colors are passed through without gamma.
  - sRGB FB: request an sRGB-capable surface and enable `FRAMEBUFFER_SRGB`.
    ```rust
    renderer.set_framebuffer_srgb_enabled(true) // enabled during render, disabled after
    ```
  - Pick exactly one path to avoid double correction.

- Vertex color gamma (auto + override)
  - The renderer applies gamma to ImGui vertex colors in the fragment shader via a `ColorGamma` uniform.
  - Auto (default):
    - `2.2` when `FRAMEBUFFER_SRGB` is enabled (decode vertex colors from sRGB → linear before write)
    - `1.0` when `FRAMEBUFFER_SRGB` is disabled (pass-through)
  - Override if needed:
    ```rust
    // Force a custom gamma (e.g., 2.2 or 1.0). Use None to restore auto.
    renderer.set_color_gamma_override(Some(2.2));
    renderer.set_color_gamma_override(None);
    ```

- Clear color
  - `gl.clear_color(r,g,b,a)` is specified in linear space. With sRGB FB, the driver encodes it on write,
    so the on-screen hex may not equal `r,g,b * 255` exactly (this is expected).

## Notes

- Alpha8 textures currently expand to RGBA8 for broad compatibility. On GL 3.3+/GLES 3.0+, RED + texture swizzle can reduce memory (see code comments).
- Multi-viewport support is feature-gated (off by default).

## Compatibility

| Item          | Version |
|---------------|---------|
| Crate         | 0.9.x   |
| dear-imgui-rs | 0.9.x   |
| glow          | 0.16    |

See also: [docs/COMPATIBILITY.md](https://github.com/Latias94/dear-imgui-rs/blob/main/docs/COMPATIBILITY.md) for the full workspace matrix.

## Features

- Default (core): `bind_vertex_array_support`, `vertex_offset_support`
- Extras (opt-in as a group): enable `extras` to include
  `gl_extensions_support`, `bind_sampler_support`, `clip_origin_support`,
  `polygon_mode_support`, `primitive_restart_support`
- Debug helper: `debug_message_insert_support` (no-op if disabled)
- Multi-viewport: `multi-viewport` (declared but currently not fully supported; off by default)

Rule of thumb: use the defaults; turn on `extras` only if you need those GL knobs.
