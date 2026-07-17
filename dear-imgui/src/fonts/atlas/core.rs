use std::cell::UnsafeCell;

use crate::sys;

mod add_font;
mod build;
mod glyph_ranges;
mod lifecycle;
mod loader_settings;
mod mutation;
mod texture;

/// Borrowed view of an ImGui font atlas.
///
/// Obtain this view from [`Context::font_atlas`](crate::Context::font_atlas). Native ownership
/// stays with the context or a [`SharedFontAtlas`](crate::SharedFontAtlas). Mutating methods use
/// Dear ImGui's own lock protocol instead of claiming a globally unique Rust reference, because an
/// atlas may be registered with multiple contexts.
#[repr(transparent)]
#[derive(Debug)]
pub struct FontAtlas(UnsafeCell<sys::ImFontAtlas>);

const _: [(); std::mem::size_of::<sys::ImFontAtlas>()] = [(); std::mem::size_of::<FontAtlas>()];
const _: [(); std::mem::align_of::<sys::ImFontAtlas>()] = [(); std::mem::align_of::<FontAtlas>()];

// NOTE: Do not mark FontAtlas as Send/Sync. It wraps pointers owned by the
// ImGui context and is not thread-safe to move/share across threads.
