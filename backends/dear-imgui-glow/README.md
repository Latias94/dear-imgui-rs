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

// Per frame, after building the UI. The renderer owns the synchronous consumer capability.
let frame = imgui.render(renderer.renderer_consumer()?);
renderer.render(frame)?;
```

## What You Get

- ImGui v1.92 texture system integration (font atlas upload + dynamic texture updates)
- OpenGL 3.0+, OpenGL ES 3.0+, and WebGL 2 shaders and state setup
- Runtime capability detection from the live context
- Full GL state backup/restore around ImGui rendering
- Renderer-owned linear and nearest sampling without taking ownership of application textures

## Renderer Lifecycle

`GlowRenderer` owns the Context's sole synchronous renderer consumer. Obtain that capability with
`renderer_consumer`, pass it to `Context::render`, and give the resulting `PendingFrame` to
`render` or `render_with_context`. These methods consume the pending frame, apply exactly one
feedback outcome to every managed texture request, reconcile the result with the owning Context,
and only then issue draw commands. A frame from another Context or consumer generation is rejected
before OpenGL is mutated.

Engine integrations that need an explicit boundary between managed-texture work and drawing can
use the same protocol in two steps:

```rust
let pending = imgui.render(renderer.renderer_consumer()?);
let reconciled = renderer.reconcile_frame(pending)?;
renderer.render_reconciled(reconciled)?;
```

`reconcile_frame` performs only managed-texture synchronization. `render_reconciled` consumes the
resulting linear capability and issues the draw commands. The ordinary `render` entry point is the
convenience composition of those two operations.

For single-viewport use, explicitly shut down the renderer while its OpenGL context is current.
Teardown first obtains an idle reset permit from the Context. A pending frame is abandoned by its
RAII guard if it cannot be reconciled, and teardown only proceeds once the Context has observed
that completion. Once validated, teardown deletes renderer-owned GPU textures, commits the Context
binding reset, and releases the consumer so another renderer can attach:

```rust
renderer.shutdown(&mut imgui)?;
```

For a renderer created with `with_external_context`, call `shutdown_with_context` with the same
live function table while its OpenGL context is current. `destroy_device_objects` and
`destroy_device_objects_with_context` use the same prepare-delete-commit transaction but keep the
consumer attached. The next owned or external render recreates device objects transactionally
before managed-texture reconciliation. Dropping an unreconciled `PendingFrame` records abandonment
before the Context becomes mutably available again, so teardown can retry without a separate
completion-poll step.

## Runtime Capabilities and Texture Sampling

The renderer derives its OpenGL behavior from the context used during initialization. Desktop
OpenGL 3.3+, desktop contexts exposing `GL_ARB_sampler_objects`, and OpenGL ES 3.0+/WebGL 2 use
renderer-owned sampler objects. Other desktop OpenGL 3.0-3.2 contexts use a restorative
texture-parameter fallback. The fallback changes `TEXTURE_MIN_FILTER` and `TEXTURE_MAG_FILTER`
only for an explicit linear or nearest sampler command and restores their exact previous values
after the draw, including mipmapped filters.

On sampler-object contexts, the renderer's default and reset state use its linear sampler without
changing the texture object. On fallback contexts, the default path honors the texture's own
filtering. A standard sampler command affects only subsequent ImGui elements in the current
command stream. On the fallback path, `ResetRenderState` clears an explicit sampler selection and
returns to texture-owned filtering; on the sampler-object path it rebinds the renderer's linear
sampler. This is an intentional backend contract: reset produces a deterministic state regardless
of what a raw callback changed.

Raw callbacks can inspect the live renderer state through a callback-scoped borrow:

```rust
use dear_imgui_glow::GlowRenderState;

unsafe extern "C" fn callback(
    _parent_list: *const dear_imgui_rs::sys::ImDrawList,
    _command: *const dear_imgui_rs::sys::ImDrawCmd,
) {
    // SAFETY: this runs synchronously inside dear-imgui-glow's raw callback scope.
    let _ = unsafe {
        GlowRenderState::with_current(|state| {
            let gl = state.gl();
            let sampler_strategy = state.sampler_strategy();
            // Use `gl` only for this callback and do not unwind across the C ABI.
            let _ = (gl, sampler_strategy);
        })
    };
}
```

`GlowRenderState` cannot escape the closure, is neither `Send` nor `Sync`, and rejects recursive
borrows. The returned `glow::Context` is still an unsafe OpenGL function table; callers remain
responsible for valid OpenGL operations and must not unwind across the native callback ABI.

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
// SAFETY: the platform creates every secondary GL context in `gl`'s share group, makes the
// viewport context current before Platform_RenderWindow, and keeps a compatible context current
// for runtime GL work and teardown.
let mut runtime = unsafe { GlowViewportRuntime::attach(&mut imgui, renderer) }
    .map_err(|failure| failure.into_parts().0)?;

runtime.render_context_with_platform_windows(&mut imgui)?;

runtime.shutdown(&mut imgui)?;
```

Integrations that own their frame schedule may split this convenience path as well:

```rust
let pending = runtime.with_renderer(|renderer| {
    renderer
        .renderer_consumer()
        .map(|consumer| imgui.render(consumer))
})??;
let reconciled = runtime.reconcile_frame(pending)?;
let reconciled = runtime.render_with_platform_windows_reconciled(reconciled)?;
drop(reconciled);
```

The final call renders the main draw data, invokes the secondary viewport renderer callbacks while
their OpenGL contexts are current, and finishes all OpenGL draw work before the platform presents
the main back buffer. This OpenGL ordering intentionally differs from acquire-based WSI renderers:
there is no separate main-surface acquisition phase to defer.

Attachment preflights the complete renderer callback table and renderer capability bit, and fails
without publishing partial state. Callback panic, reentry, renderer failure, and foreign callback
replacement are contained and returned by the next Rust entry or `poll_fault`. Explicit shutdown
validates the synchronous consumer lifecycle before deleting GL resources. Explicit shutdown and
Context-first teardown both delete renderer resources before platform windows. Every Rust and
direct renderer callback entry revalidates the renderer capability, platform capability, required
platform callbacks, and complete renderer callback table; dependency drift clears Glow's advertised
capability and skips GL work. Dropping the wrapper defers the renderer attachment to its Context;
it does not release GPU resources or the consumer while Context-managed texture bindings remain
observable. Context-owned teardown performs the ordered renderer release before platform windows.

The platform contract is stronger than having non-null `Platform_RenderWindow` and
`Platform_SwapBuffers` callbacks. The platform backend must create GL contexts in the same share
group and make the correct viewport context current from `Platform_RenderWindow`. Because callback
preflight cannot prove an OS-level GL share group, `attach` is unsafe and the integration must
document why it upholds this contract. A compatible share-group context must also be current for
runtime methods that perform GL work, explicit `shutdown`, and Dear ImGui Context teardown,
because renderer resources are deleted before the platform-window phase. Dropping the wrapper
defers that cleanup to the Context attachment; it is not a replacement for explicitly ordered
shutdown at a known-current GL boundary.

`GlowRenderer::with_external_context` remains a single-viewport API because an unrelated GL
capability cannot be paired safely after renderer creation. Use `with_shared_context` so the
renderer and runtime retain the exact same `Rc<glow::Context>` from initialization onward.

## sRGB / Gamma

- Pipeline choice
  - Linear FB: keep `FRAMEBUFFER_SRGB` disabled (default). Colors are passed through without gamma.
  - sRGB FB: request an sRGB-capable surface and enable `FRAMEBUFFER_SRGB`.
    ```rust
    renderer.set_framebuffer_srgb_enabled(true)?; // enabled during render, then restored
    ```
  - Pick exactly one path to avoid double correction.
  - `set_framebuffer_srgb_enabled(true)` is fallible and returns
    `RenderError::FramebufferSrgbUnsupported` on OpenGL ES and WebGL, where this desktop state is
    not a portable renderer contract. Disabling it is always accepted.

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
| Crate         | 0.16.0-alpha.2  |
| dear-imgui-rs | 0.16.0-alpha.2  |
| glow          | 0.17    |

See also: [docs/COMPATIBILITY.md](https://github.com/Latias94/dear-imgui-rs/blob/main/docs/COMPATIBILITY.md) for the full workspace matrix.

## Features

- Default: no optional features. Renderer capabilities are detected from the live context.
- WebAssembly provider: `wasm` (required for `wasm32-unknown-unknown`/WebGL 2 builds)
- Multi-viewport: `multi-viewport` (owning renderer runtime; off by default)

The former capability features, including `bind_sampler_support`, have been removed. They could
describe how the crate was compiled, but not what the active OpenGL context supports.

## 0.16 Migration

- OpenGL 2.1, OpenGL ES 2.0, and WebGL 1 contexts are no longer accepted. Use OpenGL 3.0,
  OpenGL ES 3.0, or WebGL 2 or newer.
- Remove `extras`, `bind_vertex_array_support`, `vertex_offset_support`,
  `gl_extensions_support`, `bind_sampler_support`, `clip_origin_support`,
  `polygon_mode_support`, `primitive_restart_support`, and
  `debug_message_insert_support` from dependency features.
- Renderer GPU handles and capability fields are private implementation details. Use
  `gl_version()`, `supports_clip_origin()`, `supports_framebuffer_srgb_control()`,
  and `supports_sampler_objects()` for the supported observations. Device-object destruction is
  recoverable at the next render, while `shutdown` terminates the renderer, so there is no shared
  boolean state for both operations.
- Mutable access to the texture map has been removed. Use `register_texture`/`unregister_texture`
  for renderer-owned GL textures and
  `register_external_texture`/`update_external_texture`/`unregister_external_texture` for
  application-owned mappings.
- `GlVersion` capability queries now use explicit runtime names:
  `bind_vertex_array_support` -> `is_supported`, `vertex_offset_support` ->
  `supports_vertex_offset`, `clip_origin_support` -> `supports_clip_origin`,
  `bind_sampler_support` -> `supports_sampler_objects`, `polygon_mode_support` ->
  `supports_polygon_mode`, and `primitive_restart_support` ->
  `supports_primitive_restart`. The last query now correctly reports `false` for OpenGL ES,
  where fixed-index restart is not the desktop toggle restored by this backend.
- Match `RenderError::UnknownTextureId(TextureId)` for a legacy texture ID that is not in the
  renderer map, or `RenderError::ManagedTextureMissing(SnapshotTextureId)` for a managed update
  received before its matching GPU create request, instead of parsing `RenderError::InvalidTexture(String)`.
