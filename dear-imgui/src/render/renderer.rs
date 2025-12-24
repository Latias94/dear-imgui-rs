//! Renderer abstractions and texture management
//!
//! This module provides renderer trait definitions for integrating with various graphics APIs.
//! For texture management, use the types from the `texture` module.

// Re-export texture types for backward compatibility
pub use crate::texture::{RawTextureId, TextureData, TextureFormat, TextureRect, TextureStatus};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::texture::TextureId;
    use std::mem;

    #[test]
    fn test_texture_id_memory_layout() {
        // Ensure TextureId matches Dear ImGui's ImTextureID (ImU64 in v1.91.4+).
        assert_eq!(
            mem::size_of::<TextureId>(),
            mem::size_of::<crate::sys::ImTextureID>()
        );
        assert_eq!(
            mem::align_of::<TextureId>(),
            mem::align_of::<crate::sys::ImTextureID>()
        );
    }

    #[test]
    fn test_texture_id_conversions() {
        let id = TextureId::new(12345);
        assert_eq!(id.id(), 12345);

        let ptr = 0x1000 as *const u8;
        let id_from_ptr = TextureId::from(ptr);
        assert_eq!(id_from_ptr.id(), 0x1000);

        let raw: RawTextureId = id.into();
        let id_back: TextureId = raw.into();
        assert_eq!(id, id_back);
    }

    #[test]
    fn test_null_texture_id() {
        let null_id = TextureId::null();
        assert!(null_id.is_null());
        assert_eq!(null_id.id(), 0);

        let non_null = TextureId::new(1);
        assert!(!non_null.is_null());
    }
}
