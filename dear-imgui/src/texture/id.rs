use std::num::NonZeroU64;

use crate::{ContextId, sys};

/// Simple texture ID for backward compatibility
///
/// This is a simple wrapper around u64 that can be used to identify textures.
/// For modern texture management, use TextureData instead.
///
/// Note: Changed from usize to u64 in Dear ImGui 1.91.4+ to support 64-bit handles
/// like Vulkan and DX12 on 32-bit targets.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct TextureId(u64);

impl TextureId {
    /// Creates a new texture id with the given identifier
    #[inline]
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    /// Returns the id of the TextureId
    #[inline]
    pub const fn id(self) -> u64 {
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

    /// Try to view this texture id as a `usize`.
    ///
    /// Returns `None` if the id does not fit on this target.
    pub fn try_as_usize(self) -> Option<usize> {
        usize::try_from(self.0).ok()
    }

    /// Try to view this texture id as a raw pointer.
    ///
    /// Returns `None` if the id does not fit on this target.
    pub fn try_as_ptr<T>(self) -> Option<*const T> {
        self.try_as_usize().map(|value| value as *const T)
    }

    /// Try to view this texture id as a mutable raw pointer.
    ///
    /// Returns `None` if the id does not fit on this target.
    pub fn try_as_mut_ptr<T>(self) -> Option<*mut T> {
        self.try_as_usize().map(|value| value as *mut T)
    }
}

impl From<u64> for TextureId {
    #[inline]
    fn from(id: u64) -> Self {
        TextureId(id)
    }
}

impl<T> From<*const T> for TextureId {
    #[inline]
    fn from(ptr: *const T) -> Self {
        TextureId(ptr as usize as u64)
    }
}

impl<T> From<*mut T> for TextureId {
    #[inline]
    fn from(ptr: *mut T) -> Self {
        TextureId(ptr as usize as u64)
    }
}

// Backward compatibility: allow conversion from usize for legacy code
impl From<usize> for TextureId {
    #[inline]
    fn from(id: usize) -> Self {
        TextureId(id as u64)
    }
}

impl Default for TextureId {
    #[inline]
    fn default() -> Self {
        Self::null()
    }
}

/// Raw texture ID type for compatibility with Dear ImGui
pub type RawTextureId = sys::ImTextureID;

impl From<TextureId> for RawTextureId {
    #[inline]
    fn from(id: TextureId) -> Self {
        id.id() as sys::ImTextureID
    }
}

/// Opaque identity for a Context-owned managed texture.
///
/// The identity remains stable while the texture is active or retiring. It includes the owning
/// Context and a private slot generation, so a stale handle can never address a texture registered
/// later in a reused slot. Use [`TextureId`] for application-owned GPU texture identifiers.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct ManagedTextureId {
    context: ContextId,
    slot: u32,
    generation: NonZeroU64,
}

impl ManagedTextureId {
    #[inline]
    pub(crate) const fn new(context: ContextId, slot: u32, generation: NonZeroU64) -> Self {
        Self {
            context,
            slot,
            generation,
        }
    }

    /// Returns the Context that owns this managed texture.
    #[inline]
    pub const fn context_id(self) -> ContextId {
        self.context
    }

    #[inline]
    pub(crate) const fn slot(self) -> u32 {
        self.slot
    }

    #[inline]
    pub(crate) const fn generation(self) -> NonZeroU64 {
        self.generation
    }
}
