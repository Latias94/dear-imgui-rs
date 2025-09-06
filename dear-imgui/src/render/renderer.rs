//! Renderer abstractions and texture management
//!
//! This module provides texture ID management and renderer trait definitions
//! for integrating with various graphics APIs.

use std::os::raw::c_void;

/// An opaque texture identifier
///
/// This is compatible with Dear ImGui's ImTextureID and can be used
/// to reference textures in draw commands.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct TextureId(usize);

impl TextureId {
    /// Creates a new texture id with the given identifier
    #[inline]
    pub const fn new(id: usize) -> Self {
        Self(id)
    }

    /// Returns the id of the TextureId
    #[inline]
    pub const fn id(self) -> usize {
        self.0
    }

    /// Creates a null texture ID
    #[inline]
    pub const fn null() -> Self {
        Self(0)
    }

    /// Checks if this texture ID is null
    #[inline]
    pub const fn is_null(self) -> bool {
        self.0 == 0
    }
}

impl From<usize> for TextureId {
    #[inline]
    fn from(id: usize) -> Self {
        TextureId(id)
    }
}

impl<T> From<*const T> for TextureId {
    #[inline]
    fn from(ptr: *const T) -> Self {
        TextureId(ptr as usize)
    }
}

impl<T> From<*mut T> for TextureId {
    #[inline]
    fn from(ptr: *mut T) -> Self {
        TextureId(ptr as usize)
    }
}

impl From<TextureId> for *const c_void {
    #[inline]
    fn from(id: TextureId) -> Self {
        id.0 as *const c_void
    }
}

impl From<TextureId> for *mut c_void {
    #[inline]
    fn from(id: TextureId) -> Self {
        id.0 as *mut c_void
    }
}

impl Default for TextureId {
    #[inline]
    fn default() -> Self {
        Self::null()
    }
}

/// Raw texture ID type for compatibility with Dear ImGui
pub type RawTextureId = *const c_void;

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem;

    #[test]
    fn test_texture_id_memory_layout() {
        // Ensure TextureId has the same size as a pointer
        assert_eq!(mem::size_of::<TextureId>(), mem::size_of::<*const c_void>());
        assert_eq!(mem::align_of::<TextureId>(), mem::align_of::<*const c_void>());
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
