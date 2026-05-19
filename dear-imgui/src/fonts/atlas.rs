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
pub use core::{FontAtlas, FontAtlasRef};
pub use id::FontId;
pub use loader::{FontLoader, FontLoaderFlags};
pub use shared::SharedFontAtlas;
pub use source::FontSource;
pub use texture::FontAtlasTexture;

pub(crate) use id::{validate_font_for_current_context, validate_font_id_for_current_context};
pub(crate) use state::forget_font_atlas_generation;
