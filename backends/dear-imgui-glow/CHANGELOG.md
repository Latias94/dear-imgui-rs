# Changelog

All notable changes to `dear-imgui-glow` will be documented in this file.

The format follows Keep a Changelog and Semantic Versioning.

## [Unreleased]

### Changed

- OpenGL capabilities are now detected from the live context. The renderer requires OpenGL 3.0+,
  OpenGL ES 3.0+, or WebGL 2; the old compile-time capability features were removed. WebAssembly
  users must enable the new `wasm` provider feature. The `debug_message_insert_support` feature was
  also removed because WebGL cannot execute that debug entry point and native availability is a
  live extension property rather than a Cargo feature.
- Linear and nearest filtering now use renderer-owned sampler objects where available. OpenGL
  3.0-3.2 uses a temporary, fully restorative texture-parameter fallback, so application-owned
  mipmap filters are no longer overwritten by ImGui rendering.
- Raw draw callbacks execute with callback-scoped `GlowRenderState`; the renderer restores its GL
  bindings and clears `Renderer_RenderState` on success, error, and unwind-adjacent paths.
- `set_framebuffer_srgb_enabled` now returns `RenderResult<()>` and rejects `true` on OpenGL ES
  and WebGL rather than issuing a desktop-only GL enum. Its temporary desktop state is restored
  after rendering and re-established after a standard reset callback.
- `GlowRenderer` GPU handles and capability fields are private. Use `gl_version()`,
  `supports_clip_origin()`, `supports_framebuffer_srgb_control()`, `supports_sampler_objects()`,
  and `is_destroyed()` for supported observations.
- `GlVersion::{bind_vertex_array_support,vertex_offset_support,clip_origin_support,
  bind_sampler_support,polygon_mode_support,primitive_restart_support}` were replaced by
  `is_supported`, `supports_vertex_offset`, `supports_clip_origin`,
  `supports_sampler_objects`, `supports_polygon_mode`, and `supports_primitive_restart`.
- `RenderError::InvalidTexture(String)` was replaced by typed
  `RenderError::UnknownTextureId(TextureId)` and `RenderError::ManagedTextureMissing(SnapshotTextureId)`.

### Fixed

- Renderer capability flags now reflect whether the live API can execute vertex-offset commands.
  Invalid draw ranges, non-finite clip rectangles, and oversized OpenGL parameters fail before the
  invalid draw command is issued, while the draw-scope guard restores OpenGL state.
- Uniform locations are borrowed rather than moved, keeping the same render path valid for native
  Glow and WebGL handles.
- Texture uploads validate the complete byte length before issuing GL calls, preserve unpack-buffer
  state, and keep legacy registrations renderer-owned if a custom `TextureMap` panics during setup.

## [0.16.0-alpha.1]

### Changed

- `GlowRenderer` now owns the Context's move-only renderer consumer and consumes
  `RenderedFrame` by value. Managed texture uploads use pointer-free `SnapshotTextureId` keys and
  owned request bytes instead of mutating `ImTextureData` through `DrawData`.
- `TextureMap` now stores only Rust/OpenGL mappings and pixel formats. Its registration method is
  fallible, and native `TextureData` accessors were removed.
- `GlowRenderer::destroy` and `destroy_device_objects` now return `RenderResult`, prepare a
  Context texture-reset permit before OpenGL resource release, and commit it only after their
  texture mappings have been destroyed.
- Rendering now rejects frames from another Context, another consumer generation, or a path that
  omitted the managed-texture renderer epoch before making OpenGL calls.
- The `multi-viewport` feature now exposes owning `GlowViewportRuntime`, replacing free
  `enable`/`disable`/`shutdown_multi_viewport_support` functions and their caller-address contract.
  The runtime consumes `GlowRenderer` into stable storage, transactionally claims the renderer
  callback table, defers callback faults to Rust, and shares one ordered shutdown state machine with
  its Context attachment. `GlowViewportRuntime::attach` is now unsafe because only the platform
  integration can prove that secondary contexts share renderer objects and are current at every GL
  entry and teardown boundary. Attachment also rejects a pre-existing renderer viewport capability
  bit instead of taking ownership of an ambiguous external flag.
- Added `GlowRenderer::with_shared_context` for multi-viewport integrations that share the exact
  `Rc<glow::Context>` used during renderer creation. Existing `with_external_context` renderers are
  typed-rejected by `GlowViewportRuntime` and remain supported for single-viewport rendering.
- The low-level `update_texture` helper now accepts a `GlTextureUpdate` descriptor with a typed
  `TextureFormat`. Upload length is validated before OpenGL reads the slice, and the previous
  active texture, binding, and unpack alignment are restored after the update.

### Fixed

- `update_texture_with_context` now updates the GL texture already registered for an existing
  `TextureId` instead of creating or replacing a separate texture mapping.
- `Alpha8` texture creation and updates now use the same RGBA expansion path, matching the
  renderer's shader expectations.
- Renderer initialization commits an empty reset transaction before the new consumer can publish
  managed texture mappings. This does not make dropping a live renderer recoverable; explicitly
  destroy its GPU resources before releasing the consumer.
- Explicit multi-viewport shutdown now retains its renderer capability when detached snapshots are
  still outstanding, allowing shutdown to be retried after those epochs complete.
- Context-owned teardown now reports callback drift and GPU cleanup failures through the attachment
  fail-stop contract instead of continuing into platform-window destruction with live GL resources.
- Rust and direct C callback entries now fail closed if the renderer capability, platform
  capability, required platform callbacks, or any renderer callback slot drifts while attached;
  Glow clears its capability and skips GL work before returning the typed fault.
- Managed-texture destroy tombstones now retain their latest renderer epoch and are pruned by the
  contiguous completion watermark, bounding long-running renderer memory without allowing delayed
  or abandoned work to resurrect retired GPU textures.

## [0.10.4] - 2026-03-17

### Added

- Add a runnable `glow_external_context_regression` example that exercises `GlowRenderer::with_external_context()` together with Dear ImGui managed texture create/update/destroy requests.

### Fixed

- Make `render_with_context()` honor the caller-provided GL context for managed texture create/update/destroy requests instead of assuming the renderer owns a context internally. Fixes #22, thanks @CoffeeCatRailway.
- Add `register_texture_with_context()` / `update_texture_with_context()` helpers for applications that keep OpenGL context ownership outside `GlowRenderer`.
