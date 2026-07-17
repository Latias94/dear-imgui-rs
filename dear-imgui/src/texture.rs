//! Texture management for Dear ImGui
//!
//! This module provides access to Dear ImGui's modern texture management system
//! introduced in version 1.92+. It includes support for ImTextureData, texture
//! status management, and automatic texture updates.

mod data;
mod error;
mod format;
mod id;
mod owned;
mod rect;
mod reference;
mod status;
#[cfg(test)]
mod tests;
mod validation;

pub use data::TextureData;
pub use error::ManagedTextureError;
pub use format::{TextureFormat, get_format_bytes_per_pixel, get_format_name};
pub use id::{ManagedTextureId, RawTextureId, TextureId};
pub use owned::OwnedTextureData;
pub use rect::TextureRect;
pub use reference::TextureRef;
pub(crate) use reference::{TextureSource, effective_texture_id};
pub use status::{TextureStatus, get_status_name};
