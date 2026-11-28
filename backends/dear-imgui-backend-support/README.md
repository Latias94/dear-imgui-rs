# Dear ImGui backend support

Shared primitives for renderer crates built on `dear-imgui-rs`.

- `ManagedTextureTable` indexes backend resources by the complete `SnapshotTextureId`.
  Seal a texture when processing a destroy request, take its resource when it can be retired,
  and acknowledge the destroy when the backend can report successful feedback. WGPU and Bevy
  can acknowledge directly on routes that release the binding immediately. Ash can keep a sealed
  resource accessible until it moves into the retirement queue. Prune acknowledged tombstones
  with the owning Context's contiguous completion watermark. A detached executor that rejects
  old epochs before processing texture requests can also prune through that execution boundary.
  During a renderer reset, extract and release resources before clearing the reset Contexts'
  tombstones. Backends own frame ordering, GPU retirement, binding aliases, and texture feedback.
- `DirtyRectNormalizer::validate` checks existing upload rectangles while preserving the geometry
  of their payloads. Use `clamp` and `normalize` when taking pixels from a complete source image.
  Normalization merges overlap and shared edges when the bounding rectangle fits `TextureRect`.
  Convert native rectangles with `TextureRect::from`.
- `DrawCallbackDispatcher` reads the standard callback actions that Core already recognized and
  preserves the texture-owned sampler action for backends such as Glow. `ScopedRenderState`
  re-exports Core's `RendererRenderStateGuard` under a backend-facing name. During a raw
  callback, publish the state through the guard for its full exclusive lifetime. Let the guard
  clear its slot on early return and use `finish` to report drift on a completed draw.
- `compute_scissor_rect` produces a framebuffer scissor with a top-left origin. Use
  `try_compute_scissor_rect` when an invalid projection is a frame error. OpenGL can convert
  the Y origin, and Bevy can intersect with a camera viewport before setting the scissor.
- `srgb_to_linear_channel` and `srgb_vertex_color_to_linear` define the reference for sRGB
  compositing. Interpolate the encoded vertex RGB, decode in the fragment stage, then modulate
  by the linear texture sample. An sRGB texture is decoded once through a GPU sRGB view or an
  explicit decode. Vertex alpha passes through the color decode unchanged.
- `BackendError` covers resource initialization, texture uploads, frame contracts, callback
  conflicts, device loss, and renderer lifecycle. Backend errors can wrap it transparently
  alongside API-specific failure details.
