# dear-imgui-glow

Glow (OpenGL) renderer for Dear ImGui.

## Quick Start

```rust
use dear_imgui::Context;
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

- No gamma (pow) in the shader by default. sRGB encoding is controlled by your surface/context.
- If your window uses an sRGB surface, enable `GL_FRAMEBUFFER_SRGB` on your side, or let the renderer toggle it:
  ```rust
  renderer.set_framebuffer_srgb_enabled(true); // enabled before render, disabled after
  ```
- Pick exactly one path to avoid double correction: use an sRGB target (no shader pow) or a linear target (no shader pow).

## Notes

- Alpha8 textures currently expand to RGBA8 for broad compatibility. On GL 3.3+/GLES 3.0+, RED + texture swizzle can reduce memory (see code comments).
- Multi-viewport support is feature-gated (off by default).

## Compatibility

| Item       | Version |
|------------|---------|
| Crate      | 0.2.x   |
| dear-imgui | 0.2.x   |
| glow       | 0.16    |

See also: [docs/COMPATIBILITY.md](../../docs/COMPATIBILITY.md) for the full workspace matrix.

