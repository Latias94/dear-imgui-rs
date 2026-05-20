use std::marker::PhantomData;

use crate::sys;

mod add_font;
mod build;
mod glyph_ranges;
mod lifecycle;
mod loader_settings;
mod mutation;
mod ref_view;
mod texture;

/// Font atlas that manages multiple fonts and their texture data
///
/// The font atlas is responsible for:
/// - Loading and managing multiple fonts
/// - Packing font glyphs into texture atlases
/// - Providing texture data for rendering
#[derive(Debug)]
pub struct FontAtlas {
    pub(in crate::fonts::atlas::core) raw: *mut sys::ImFontAtlas,
    pub(in crate::fonts::atlas::core) owned: bool,
    pub(in crate::fonts::atlas::core) _phantom: PhantomData<*mut sys::ImFontAtlas>,
}

/// Shared view of a font atlas.
///
/// This type allows read-only atlas inspection without exposing safe font mutation from
/// `Context::font_atlas()`.
#[derive(Debug, Clone, Copy)]
pub struct FontAtlasRef<'atlas> {
    pub(in crate::fonts::atlas::core) raw: *const sys::ImFontAtlas,
    pub(in crate::fonts::atlas::core) _phantom: PhantomData<&'atlas sys::ImFontAtlas>,
}

// NOTE: Do not mark FontAtlas as Send/Sync. It wraps pointers owned by the
// ImGui context and is not thread-safe to move/share across threads.
