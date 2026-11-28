//! Font atlas management for Dear ImGui v1.92+
//!
//! This module provides a modern, type-safe interface to Dear ImGui's dynamic font system.
//! Key features:
//! - Dynamic glyph loading (no need to pre-specify glyph ranges)
//! - Runtime font size adjustment
//! - Custom font loaders
//! - Incremental texture updates
//!
//! Managed renderers own atlas building and texture upload. Legacy renderers must first acquire
//! [`LegacyFontAtlas`]; the common [`FontAtlas`] capability intentionally has no build or raw
//! texture-data methods:
//!
//! ```compile_fail
//! fn build_directly(atlas: &dear_imgui_rs::FontAtlas) {
//!     atlas.build();
//! }
//! ```
//!
//! ```compile_fail
//! fn query_build_state_directly(atlas: &dear_imgui_rs::FontAtlas) {
//!     let _ = atlas.is_built();
//! }
//! ```
//!
//! ```compile_fail
//! fn inspect_legacy_texture_id(atlas: &dear_imgui_rs::FontAtlas) {
//!     let _ = atlas.texture_id();
//! }
//! ```
//!
//! ```compile_fail
//! fn inspect_legacy_texture_data(atlas: &dear_imgui_rs::FontAtlas) {
//!     let _ = atlas.tex_data();
//! }
//! ```
//!
//! ```compile_fail
//! fn bind_legacy_texture_directly(
//!     atlas: &dear_imgui_rs::FontAtlas,
//!     texture: dear_imgui_rs::TextureId,
//! ) {
//!     unsafe { atlas.set_texture_id(texture) };
//! }
//! ```
//!
//! ```compile_fail
//! fn clear_legacy_pixels_directly(atlas: &dear_imgui_rs::FontAtlas) {
//!     atlas.clear_tex_data();
//! }
//! ```
//!
//! Font addition is intentionally routed through [`FontSource`] so source ownership, format,
//! loader, and merge configuration are validated together. The former direct helpers and raw
//! file constructor are unavailable:
//!
//! ```compile_fail
//! fn add_with_config_directly(atlas: &dear_imgui_rs::FontAtlas) {
//!     let _ = atlas.add_font_with_config(&dear_imgui_rs::FontConfig::new());
//! }
//! ```
//!
//! ```compile_fail
//! fn add_default_directly(atlas: &dear_imgui_rs::FontAtlas) {
//!     let _ = atlas.add_font_default(None);
//! }
//! ```
//!
//! ```compile_fail
//! fn add_default_vector_directly(atlas: &dear_imgui_rs::FontAtlas) {
//!     let _ = atlas.add_font_default_vector(None);
//! }
//! ```
//!
//! ```compile_fail
//! fn add_default_bitmap_directly(atlas: &dear_imgui_rs::FontAtlas) {
//!     let _ = atlas.add_font_default_bitmap(None);
//! }
//! ```
//!
//! ```compile_fail
//! fn add_file_directly(atlas: &dear_imgui_rs::FontAtlas) {
//!     let _ = unsafe { atlas.add_font_from_file_ttf("font.ttf", 16.0, None, None) };
//! }
//! ```
//!
//! ```compile_fail
//! fn add_memory_directly(atlas: &dear_imgui_rs::FontAtlas, bytes: &[u8]) {
//!     let _ = unsafe { atlas.add_font_from_memory_ttf(bytes, 16.0, None, None) };
//! }
//! ```
//!
//! ```compile_fail
//! fn add_compressed_memory_directly(atlas: &dear_imgui_rs::FontAtlas, bytes: &[u8]) {
//!     let _ = unsafe { atlas.add_font_from_memory_compressed_ttf(bytes, 16.0, None, None) };
//! }
//! ```
//!
//! ```compile_fail
//! fn add_base85_directly(atlas: &dear_imgui_rs::FontAtlas, bytes: &str) {
//!     let _ = unsafe {
//!         atlas.add_font_from_memory_compressed_base85_ttf(bytes, 16.0, None, None)
//!     };
//! }
//! ```
//!
//! ```compile_fail
//! let _ = dear_imgui_rs::FontSource::ttf_file("font.ttf");
//! ```
//!
//! ```compile_fail
//! let _ = dear_imgui_rs::FontSource::ttf_file_with_size("font.ttf", 16.0);
//! ```

use std::cell::UnsafeCell;

use bitflags::bitflags;

use crate::FontError;
use crate::sys;

mod config;
mod custom_rect;
mod ownership;
mod source;
mod state;
#[cfg(test)]
mod tests;
mod texture;
mod validated;

pub use config::{FontConfig, FontLoader, FontLoaderFlags};
pub use custom_rect::{CustomRectData, CustomRectId, CustomRectSnapshot};
pub use ownership::{FontId, SharedFontAtlas};
pub use source::FontSource;
pub use texture::{FontAtlasTexture, LegacyFontAtlas};
pub use validated::StbTrueTypeFontData;

pub(crate) use ownership::{
    validate_font_id, validate_font_id_for_context, validate_font_id_for_current_context,
};
pub(crate) use state::{
    assert_font_atlas_renderer_mode, assert_no_font_atlas_texture_borrows,
    claim_validated_font_atlas_managed_renderer, font_atlas_snapshot_identities,
    font_atlas_texture_identity_is_known, forget_font_atlas_generation,
    mark_font_atlas_renderer_reset, mark_font_atlas_texture_staged,
    prune_font_atlas_texture_tombstones, record_font_atlas_texture_reference,
    register_font_atlas_context, unregister_font_atlas_context,
    validate_font_atlas_context_registration, validate_font_atlas_managed_renderer,
};

use ownership::validate_font_id_for_atlas;
use state::{
    assert_no_open_font_atlas_frames, bump_custom_rect_generation, bump_font_atlas_generation,
    font_atlas_state, reset_font_atlas_mode_after_full_clear,
};
use texture::claim_legacy_renderer;

/// Borrowed view of an ImGui font atlas.
///
/// Obtain this view from [`Context::font_atlas`](crate::Context::font_atlas). Native ownership
/// stays with the context or a [`SharedFontAtlas`]. Mutating methods use
/// Dear ImGui's own lock protocol instead of claiming a globally unique Rust reference, because an
/// atlas may be registered with multiple contexts.
#[repr(transparent)]
#[derive(Debug)]
pub struct FontAtlas(UnsafeCell<sys::ImFontAtlas>);

const _: [(); std::mem::size_of::<sys::ImFontAtlas>()] = [(); std::mem::size_of::<FontAtlas>()];
const _: [(); std::mem::align_of::<sys::ImFontAtlas>()] = [(); std::mem::align_of::<FontAtlas>()];

// NOTE: Do not mark FontAtlas as Send/Sync. It wraps pointers owned by the
// ImGui context and is not thread-safe to move/share across threads.

bitflags! {
    /// Atlas texture options, configured before its first build.
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
    pub struct FontAtlasFlags: i32 {
        const NONE = sys::ImFontAtlasFlags_None;
        /// Do not round the texture height to a power of two.
        const NO_POWER_OF_TWO_HEIGHT = sys::ImFontAtlasFlags_NoPowerOfTwoHeight;
        /// Omit software mouse cursor images.
        const NO_MOUSE_CURSORS = sys::ImFontAtlasFlags_NoMouseCursors;
        /// Omit line textures. This also disables line and line-end anti-aliasing.
        const NO_BAKED_LINES = sys::ImFontAtlasFlags_NoBakedLines;
        /// Omit round-corner textures, using tessellated geometry instead.
        const NO_BAKED_ROUND_CORNERS = sys::ImFontAtlasFlags_NoBakedRoundCorners;
    }
}

impl FontAtlas {
    /// Claim the legacy renderer capability for this atlas.
    ///
    /// The claim persists after the returned capability is dropped so later legacy frames cannot
    /// silently change renderer mode. Drop every legacy capability and call [`Self::clear`] before
    /// attaching a managed renderer. Managed renderers and pending renderer teardown are rejected
    /// with a typed error at this call.
    pub fn try_claim_legacy_renderer(&self) -> Result<LegacyFontAtlas<'_>, FontError> {
        claim_legacy_renderer(self)
    }

    /// Creates a shared font-atlas view from a raw pointer.
    ///
    /// # Safety
    /// The owner must keep the atlas alive on this thread for `'a`. Native mutation must honor
    /// the atlas's frame and texture borrows throughout that lifetime.
    pub(crate) unsafe fn from_raw<'a>(raw: *mut sys::ImFontAtlas) -> &'a Self {
        assert!(
            !raw.is_null(),
            "FontAtlas::from_raw() requires non-null pointer"
        );
        font_atlas_state(raw);
        unsafe { &*raw.cast::<Self>() }
    }

    /// Returns the raw `ImFontAtlas` pointer for explicit FFI interop.
    ///
    /// The pointer is valid only while the atlas owner remains alive. Dereferencing it or passing
    /// it to native code is unsafe; mutating the atlas outside this wrapper can violate its handle,
    /// texture-lease, and frame-lifecycle checks.
    pub fn raw(&self) -> *mut sys::ImFontAtlas {
        self.0.get()
    }

    pub(crate) fn font_id_for_raw(&self, font: *mut sys::ImFont) -> FontId {
        FontId::from_raw_parts(font, self.raw())
    }

    pub(crate) fn assert_mutation_allowed(&self, caller: &str) {
        let raw = self.raw();
        assert_no_font_atlas_texture_borrows(raw, caller);
        assert_no_open_font_atlas_frames(raw, caller);
        assert!(
            !unsafe { (*raw).Locked },
            "{caller} cannot modify a locked font atlas"
        );
    }

    /// Remove a font from the atlas.
    ///
    /// Existing [`FontId`] handles from this atlas are invalidated.
    #[doc(alias = "RemoveFont")]
    pub fn remove_font(&self, font: FontId) {
        self.assert_mutation_allowed("FontAtlas::remove_font()");
        let raw = self.raw();
        let font = validate_font_id_for_atlas(font, raw, "FontAtlas::remove_font()");
        unsafe { sys::ImFontAtlas_RemoveFont(raw, font) }
        bump_font_atlas_generation(raw);
    }

    /// Clear all fonts and texture data.
    ///
    /// Existing [`FontId`] handles from this atlas are invalidated.
    /// A legacy renderer claim is released only when no [`LegacyFontAtlas`]
    /// capability remains alive. Drop every legacy capability before calling this method when
    /// preparing the atlas for a managed renderer.
    #[doc(alias = "Clear")]
    pub fn clear(&self) {
        self.assert_mutation_allowed("FontAtlas::clear()");
        let raw = self.raw();
        unsafe { sys::ImFontAtlas_Clear(raw) }
        bump_font_atlas_generation(raw);
        bump_custom_rect_generation(raw);
        reset_font_atlas_mode_after_full_clear(raw);
    }

    /// Clear only the fonts (keep texture data).
    ///
    /// Existing [`FontId`] handles from this atlas are invalidated.
    #[doc(alias = "ClearFonts")]
    pub fn clear_fonts(&self) {
        self.assert_mutation_allowed("FontAtlas::clear_fonts()");
        let raw = self.raw();
        unsafe { sys::ImFontAtlas_ClearFonts(raw) }
        bump_font_atlas_generation(raw);
        bump_custom_rect_generation(raw);
    }

    /// Discard baked font caches.
    ///
    /// This clears cached glyph data (including cached "not found" entries) so that
    /// newly added font sources (e.g. merged CJK/emoji fonts) can take effect.
    ///
    /// Pass `unused_frames = 0` to discard everything (recommended after font merging).
    ///
    /// Notes:
    /// - Only call this when the atlas is not locked (typically before `Context::frame()`).
    /// - No-op if the atlas builder hasn't been created yet.
    #[doc(alias = "ImFontAtlasBuildDiscardBakes")]
    pub fn discard_bakes(&self, unused_frames: usize) {
        self.assert_mutation_allowed("FontAtlas::discard_bakes()");
        let raw = self.raw();
        let unused_frames = i32::try_from(unused_frames).unwrap_or_else(|_| {
            panic!("FontAtlas::discard_bakes() unused_frames exceeded Dear ImGui's i32 range")
        });
        unsafe {
            if (*raw).Builder.is_null() {
                return;
            }
            sys::igImFontAtlasBuildDiscardBakes(raw, unused_frames);
        }
    }

    /// Compact cached glyphs and the current atlas texture.
    ///
    /// This is a no-op before the atlas builder and texture have been created.
    /// It must not be called while the atlas is locked by a frame.
    #[doc(alias = "CompactCache")]
    pub fn compact_cache(&self) {
        self.assert_mutation_allowed("FontAtlas::compact_cache()");
        let raw = self.raw();
        unsafe {
            if (*raw).Builder.is_null() || (*raw).TexData.is_null() {
                return;
            }
            sys::ImFontAtlas_CompactCache(raw);
        }
    }

    /// Returns atlas texture options.
    pub fn flags(&self) -> FontAtlasFlags {
        FontAtlasFlags::from_bits_retain(unsafe { (*self.raw()).Flags })
    }

    /// Configures atlas texture options before its first build.
    ///
    /// # Panics
    ///
    /// Panics during an open frame, while texture data is borrowed, for unknown flags,
    /// or after building has started. Call [`Self::clear`] and repopulate the atlas
    /// before changing options on an existing atlas.
    pub fn set_flags(&self, flags: FontAtlasFlags) {
        self.assert_mutation_allowed("FontAtlas::set_flags()");
        assert!(
            flags.difference(FontAtlasFlags::all()).is_empty(),
            "FontAtlas::set_flags() requires known atlas flags"
        );
        let raw = self.raw();
        assert!(
            unsafe { (*raw).Builder.is_null() && !(*raw).TexIsBuilt },
            "FontAtlas::set_flags() requires an atlas that has not started building; clear and repopulate it first"
        );
        unsafe { (*raw).Flags = flags.bits() };
    }
}
