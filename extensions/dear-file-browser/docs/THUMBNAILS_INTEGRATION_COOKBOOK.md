# Thumbnails Integration Cookbook (ImGui Backend)

`dear-file-browser` thumbnails are **renderer-agnostic**. The dialog UI can request thumbnails and
cache them, but **the host application must provide**:

- a `ThumbnailProvider` (decode a file into RGBA8 pixels), and
- a `ThumbnailRenderer` (upload RGBA8 → GPU texture `TextureId`, queue retirement on eviction).

If thumbnails are enabled in the UI but no backend is provided, the dialog will show
`No thumbnail backend` and no thumbnails will appear.

This document explains the minimal integration patterns and the typical lifecycle.

## Quick Start (Recommended)

1) Enable a decoder provider (native builds):

- Cargo feature: `dear-file-browser/thumbnails-image`
- Use: `ImageThumbnailProvider` (based on the `image` crate, PNG/JPEG by default)

2) Implement a renderer for your graphics backend:

- Implement `ThumbnailRenderer::upload_rgba8` and `ThumbnailRenderer::destroy`
- Register each uploaded texture with the ImGui renderer and return its renderer-owned `TextureId`. These are application textures, separate from managed font-atlas requests.
- Implement `destroy` by queuing the ID for retirement: `maintain` can call it after UI commands already reference that ID. Preserve both the renderer ID mapping and the texture until the renderer has consumed the current frame and recorded all draws that use it. Only then unregister the ID. Release the underlying GPU resource after the relevant submitted draws complete, or use an API that retains its storage until those draws finish (for example, deleting an OpenGL texture after issuing its draws). Keeping only the GPU allocation alive while removing the ID mapping early is insufficient.
- The renderer owns remaining GPU allocations at shutdown; dropping the cache alone does not call renderer destruction.

3) Pass `ThumbnailBackend` into the dialog draw call:

```rust
use dear_file_browser::{ImageThumbnailProvider, ThumbnailBackend, ThumbnailDecoder};

// Keep these alive across frames:
let decoder = ThumbnailDecoder::spawn(ImageThumbnailProvider::default)?;
let mut renderer = /* your GPU upload/destroy implementation */;

let mut backend = ThumbnailBackend {
    decoder: &decoder,
    renderer: &mut renderer,
};

// Enable thumbnails in the UI state:
state.ui.config.thumbnails_enabled = true;

// Pass backend when drawing:
ui.file_browser().draw_contents_with(
    &mut state,
    None,
    Some(&mut backend),
);
```

For a working Glow/OpenGL reference implementation, see:
`examples/04-integration/file_browser_imgui.rs`.

## Lifecycle Model

Internally, the ImGui UI drives a `ThumbnailCache` stored in `FileDialogUiState`:

- Every drawn frame it calls `thumbnails.advance_frame()`.
- For visible entries, it calls `thumbnails.request_visible(path, max_size)`.
- If a backend is provided, it calls `thumbnails.maintain(&mut backend)`:
  - submits requests to a bounded queue without waiting
  - polls completed `ThumbnailProvider::decode` calls from two native workers
  - uploads completed images via `ThumbnailRenderer::upload_rgba8` on the drawing thread
  - fulfills the cache and requests retirement of evicted textures via `ThumbnailRenderer::destroy`
- The host renderer consumes/records the current frame before draining its retirement queue and unregistering those IDs. GPU resource release must also respect completion of the recorded draws.

If you need tighter control (threading, prioritization, custom decode pipeline), you can skip
`maintain()` and manage requests manually with:

- `ThumbnailCache::take_requests`
- `ThumbnailCache::fulfill_request`
- `ThumbnailCache::take_pending_destroys`

## Tuning & Performance

Read the cache configuration with `config()` and update it with `set_config()`:

- `max_entries`: completed LRU entries, including failures.
- `max_new_requests_per_frame`: admission budget, subject to the 16-request pending limit.
- `state.ui.config.thumbnail_size`: requested maximum thumbnail size (UI-controlled).

Notes:

- A failed decode or upload is cached. Use `invalidate(path)` when a file changes
  and needs another attempt.
- `ImageThumbnailProvider` reads files via `std::fs` and is intended for native builds.
- For large directories, prefer a smaller `max_new_requests_per_frame` to avoid frame spikes.

## Platform Notes

- `wasm32`: the native decoder pool and host-filesystem access are unavailable.
  Use the manual queue with a browser-suitable decode strategy.
- Custom filesystem backends: thumbnails requests include absolute `PathBuf`. If your host uses a
  virtual filesystem, implement a provider that can read and decode through that system.
