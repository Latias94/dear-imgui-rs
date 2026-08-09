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

mod config;
mod core;
mod custom_rect;
mod error;
mod id;
mod legacy;
mod loader;
mod shared;
mod source;
mod state;
#[cfg(test)]
mod tests;
mod texture;
mod validated;
mod validation;

pub use config::FontConfig;
pub use core::FontAtlas;
pub use custom_rect::{CustomRectData, CustomRectId, CustomRectSnapshot};
pub use error::{FontAtlasLoaderError, FontAtlasModeError};
pub use id::FontId;
pub use legacy::LegacyFontAtlas;
pub use loader::{FontLoader, FontLoaderFlags};
pub use shared::SharedFontAtlas;
pub use source::FontSource;
pub use texture::FontAtlasTexture;
pub use validated::{StbTrueTypeFontData, StbTrueTypeFontError, StbTrueTypeFontLoadError};

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
