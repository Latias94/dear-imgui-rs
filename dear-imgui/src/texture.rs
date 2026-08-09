//! Texture management for Dear ImGui
//!
//! This module provides access to Dear ImGui's modern texture management system introduced in
//! version 1.92+. [`OwnedTextureData::from_pixels`] creates a complete CPU texture, while
//! [`TextureData::replace_pixels`] and [`TextureData::update_subresource`] perform transactional
//! mutations.
//!
//! [`TextureRegion`] is the `u32` input rectangle for a requested update. [`TextureSubresource`]
//! pairs that region with an explicit source row pitch and payload. [`TextureRect`] is the narrower
//! `u16` snapshot of rectangles already accepted by Dear ImGui's native update queue; it is not an
//! input builder.
//!
//! ```
//! use dear_imgui_rs::texture::{
//!     OwnedTextureData, TextureFormat, TextureRegion, TextureSubresource,
//! };
//!
//! let mut texture =
//!     OwnedTextureData::from_pixels(TextureFormat::Alpha8, 4, 2, &[0; 8])?;
//! let region = TextureRegion::new(1, 0, 2, 2)?;
//! texture.update_subresource(TextureSubresource::new(
//!     region,
//!     3,
//!     &[1, 2, 99, 3, 4],
//! ))?;
//! # Ok::<(), dear_imgui_rs::TextureDataError>(())
//! ```

mod data;
mod error;
mod format;
mod id;
mod managed;
mod owned;
mod rect;
mod reference;
mod region;
mod status;
#[cfg(test)]
mod tests;
mod validation;

pub use data::TextureData;
pub use error::{ManagedTextureError, ManagedTextureMutationError, TextureDataError};
pub use format::{TextureFormat, get_format_bytes_per_pixel, get_format_name};
pub use id::{ManagedTextureId, RawTextureId, TextureId};
pub use managed::{ManagedTextureMut, ManagedTextureRef};
pub use owned::OwnedTextureData;
pub use rect::TextureRect;
pub use reference::TextureRef;
pub(crate) use reference::{TextureSource, effective_texture_id};
pub use region::{TextureRegion, TextureSubresource};
pub use status::{TextureStatus, get_status_name};
