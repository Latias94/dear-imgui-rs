# Thumbnails Integration Cookbook (ImGui Backend)

`dear-file-browser` thumbnails are **renderer-agnostic**. The dialog UI can request thumbnails and
cache them, but **the host application must provide**:

- a `ThumbnailProvider` (decode a file into RGBA8 pixels), and
- a `ThumbnailRenderer` (upload RGBA8 → GPU texture `TextureId`, destroy textures on eviction).

If thumbnails are enabled in the UI but no backend is provided, the dialog will show
`No thumbnail backend` and no thumbnails will appear.

This document explains the minimal integration patterns and the typical lifecycle.

## Quick Start (Recommended)

1) Enable a decoder provider (native builds):

- Cargo feature: `dear-file-browser/thumbnails-image`
- Use: `ImageThumbnailProvider` (based on the `image` crate, PNG/JPEG by default)

2) Implement a renderer for your graphics backend:

- Implement `ThumbnailRenderer::upload_rgba8` and `ThumbnailRenderer::destroy`
- `TextureId` is a Dear ImGui texture handle; how it maps to your engine depends on your renderer.

3) Pass `ThumbnailBackend` into the dialog draw call:

```rust
use dear_file_browser::{ThumbnailBackend, ThumbnailRenderer, ThumbnailProvider};

// Keep these alive across frames:
let mut provider = /* e.g. ImageThumbnailProvider::default() */;
let mut renderer = /* your GPU upload/destroy implementation */;

let mut backend = ThumbnailBackend {
    provider: &mut provider,
    renderer: &mut renderer,
};

// Enable thumbnails in the UI state:
state.ui.thumbnails_enabled = true;

// Pass backend when drawing:
ui.file_browser().draw_contents_with(
    &mut state,
    &dear_file_browser::StdFileSystem,
    None,
    Some(&mut backend),
);
```

For a working Glow/OpenGL reference implementation, see:
`examples/04-integration/file_browser_imgui.rs`.

## Lifecycle Model

Internally, the ImGui UI drives a `ThumbnailCache` stored in `FileDialogUiState`:

- Every frame (when thumbnails are enabled) it calls `thumbnails.advance_frame()`.
- For visible entries, it calls `thumbnails.request_visible(path, max_size)`.
- If a backend is provided, it calls `thumbnails.maintain(&mut backend)`:
  - drains requests (`take_requests`)
  - decodes them via `ThumbnailProvider::decode`
  - uploads them via `ThumbnailRenderer::upload_rgba8`
  - fulfills the cache and destroys evicted textures via `ThumbnailRenderer::destroy`

If you need tighter control (threading, prioritization, custom decode pipeline), you can skip
`maintain()` and manage requests manually with:

- `ThumbnailCache::take_requests`
- `ThumbnailCache::fulfill_request`
- `ThumbnailCache::take_pending_destroys`

## Tuning & Performance

The cache is configurable per dialog state:

- `state.ui.thumbnails.config.max_entries`: GPU texture cache size (LRU).
- `state.ui.thumbnails.config.max_new_requests_per_frame`: decode/upload request rate limiting.
- `state.ui.thumbnail_size`: requested maximum thumbnail size (UI-controlled).

Notes:

- `ImageThumbnailProvider` reads files via `std::fs` and is intended for native builds.
- For large directories, prefer a smaller `max_new_requests_per_frame` to avoid frame spikes.

## Platform Notes

- `wasm32`: `ImageThumbnailProvider` does not work by default (no `std::fs` access). Provide a
  custom `ThumbnailProvider` and a filesystem strategy suitable for your platform.
- Custom filesystem backends: thumbnails requests include absolute `PathBuf`. If your host uses a
  virtual filesystem, implement a provider that can read and decode through that system.

