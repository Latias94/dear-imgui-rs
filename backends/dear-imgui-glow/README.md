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

// Per frame, after building the UI. Rendering consumes the Context-borrowed frame.
renderer.new_frame()?;
let frame = imgui.render();
renderer.render(frame)?;
```

## What You Get

- ImGui v1.92 texture system integration (font atlas upload + dynamic texture updates)
- OpenGL 2.1+/ES 2.0+ compatible shaders and state setup
- Full GL state backup/restore around ImGui rendering

## Renderer Lifecycle

`GlowRenderer` owns the Context's sole renderer consumer. `render` and `render_with_context`
therefore take `RenderedFrame` by value, apply every managed texture request, reconcile the
result with the owning Context, and only then issue draw commands. A frame from another Context or
consumer generation is rejected before OpenGL is mutated.

For single-viewport use, explicitly destroy the renderer while its OpenGL context is current. This
deletes renderer-owned GPU textures before their Context bindings are reset and releases the
consumer so another renderer can attach:

```rust
let gl = renderer.gl_context().expect("owned GL context").clone();
renderer.destroy(&gl, &mut imgui)?;
```

For a single-viewport renderer created with `with_external_context`, pass the same live GL context
to `destroy`. `destroy_device_objects` performs the same resource-first reset but keeps the consumer
attached for later device-object recreation.

## Multi-Viewport Runtime

The `multi-viewport` feature provides an owning `GlowViewportRuntime`. It consumes the renderer
into stable storage, owns the renderer callback claim, and registers the renderer lifecycle role
with the Context:

```rust
use std::rc::Rc;
use dear_imgui_glow::{GlowRenderer, SimpleTextureMap, multi_viewport::GlowViewportRuntime};

// Attach an OpenGL-aware platform runtime first.
let gl = Rc::new(unsafe {
    glow::Context::from_loader_function(|name| loader.get_proc_address(name) as *const _)
});
let renderer = GlowRenderer::with_shared_context(
    Rc::clone(&gl),
    &mut imgui,
    Box::new(SimpleTextureMap::default()),
)?;
let mut runtime = GlowViewportRuntime::attach(&mut imgui, renderer)
    .map_err(|failure| failure.into_parts().0)?;

runtime.new_frame()?;
let frame = imgui.render();
runtime.render(frame)?;

runtime.shutdown(&mut imgui)?;
```

Attachment preflights the complete renderer callback table and fails without publishing partial
state. Callback panic, reentry, renderer failure, and foreign callback replacement are contained
and returned by the next Rust entry or `poll_fault`. Explicit shutdown and Context-first teardown
both delete renderer resources before platform windows. Dropping the wrapper performs immediate
best-effort GPU cleanup and releases its consumer; the next renderer initialization resets managed
texture bindings before reuse.

The platform contract is stronger than having non-null `Platform_RenderWindow` and
`Platform_SwapBuffers` callbacks. The platform backend must create GL contexts in the same share
group and make the correct viewport context current from `Platform_RenderWindow`. The current Winit
runtime is window-only and is rejected with `PlatformGlContextUnsupported`. Other platform routes
must document and uphold this external GPU contract; callback preflight alone is not proof of it.
A compatible share-group context must also be current when `shutdown` runs or the Dear ImGui
Context is dropped, because renderer resources are deleted before the platform-window phase. Drop
is a best-effort fallback, not a replacement for an explicitly ordered shutdown at a known-current
GL boundary.

`GlowRenderer::with_external_context` remains a single-viewport API because an unrelated GL
capability cannot be paired safely after renderer creation. Use `with_shared_context` so the
renderer and runtime retain the exact same `Rc<glow::Context>` from initialization onward.

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
- Multi-viewport support is feature-gated and uses `GlowViewportRuntime` (off by default).

## Compatibility

| Item          | Version |
|---------------|---------|
| Crate         | 0.16.0  |
| dear-imgui-rs | 0.16.0  |
| glow          | 0.17    |

See also: [docs/COMPATIBILITY.md](https://github.com/Latias94/dear-imgui-rs/blob/main/docs/COMPATIBILITY.md) for the full workspace matrix.

## Features

- Default (core): `bind_vertex_array_support`, `vertex_offset_support`
- Extras (opt-in as a group): enable `extras` to include
  `gl_extensions_support`, `bind_sampler_support`, `clip_origin_support`,
  `polygon_mode_support`, `primitive_restart_support`
- Debug helper: `debug_message_insert_support` (no-op if disabled)
- Multi-viewport: `multi-viewport` (owning renderer runtime; off by default)

Rule of thumb: use the defaults; turn on `extras` only if you need those GL knobs.
