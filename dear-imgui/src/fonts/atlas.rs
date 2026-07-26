//! Font atlas management for Dear ImGui v1.92+
//!
//! This module provides a modern, type-safe interface to Dear ImGui's dynamic font system.
//! Key features:
//! - Dynamic glyph loading (no need to pre-specify glyph ranges)
//! - Runtime font size adjustment
//! - Custom font loaders
//! - Incremental texture updates

mod config;
mod core;
mod custom_rect;
mod id;
mod loader;
mod shared;
mod source;
mod state;
#[cfg(test)]
mod tests;
mod texture;
mod validation;

pub use config::FontConfig;
pub use core::FontAtlas;
pub use custom_rect::{CustomRectData, CustomRectId, CustomRectSnapshot};
pub use id::FontId;
pub use loader::{FontLoader, FontLoaderFlags};
pub use shared::SharedFontAtlas;
pub use source::FontSource;
pub use texture::FontAtlasTexture;

pub(crate) use id::{validate_font_id, validate_font_id_for_current_context};
pub(crate) use state::{
    assert_font_atlas_renderer_mode, assert_no_font_atlas_texture_borrows,
    claim_validated_font_atlas_managed_renderer, font_atlas_snapshot_identities,
    font_atlas_texture_identity_is_known, font_atlas_texture_revision_is_current,
    forget_font_atlas_generation, mark_font_atlas_renderer_reset,
    prune_font_atlas_texture_tombstones, record_font_atlas_texture_reference,
    register_font_atlas_context, track_font_atlas_texture_operation, unregister_font_atlas_context,
    validate_font_atlas_context_registration, validate_font_atlas_managed_renderer,
};
