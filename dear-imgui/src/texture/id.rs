use crate::sys;

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

/// Stable identifier for an ImGui-managed texture.
///
/// This wraps Dear ImGui's `ImTextureData::UniqueID`. It is intended for correlating detached
/// texture requests with renderer feedback, not as a renderer texture handle. Use [`TextureId`] for
/// backend-owned GPU texture identifiers.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct ManagedTextureId(i32);

impl ManagedTextureId {
    #[inline]
    pub(crate) const fn from_raw(raw: i32) -> Self {
        Self(raw)
    }

    /// Returns Dear ImGui's raw `ImTextureData::UniqueID` value.
    ///
    /// Renderer backends can use this to derive deterministic backend texture handles for managed
    /// texture requests without relying on probabilistic hashing.
    #[inline]
    pub const fn raw(self) -> i32 {
        self.0
    }
}
