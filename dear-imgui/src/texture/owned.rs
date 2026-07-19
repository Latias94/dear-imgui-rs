use super::TextureData;
use crate::sys;
use std::ptr::NonNull;

/// Owned texture data managed by Dear ImGui.
///
/// This owns an `ImTextureData` instance allocated by Dear ImGui (C++) and will
/// destroy it on drop. It dereferences to [`TextureData`] while application code owns it.
/// Registering it with a Context transfers ownership; later access uses
/// `Context::with_texture` or `Context::with_texture_mut`.
pub struct OwnedTextureData {
    raw: NonNull<sys::ImTextureData>,
}

impl OwnedTextureData {
    /// Create a new empty texture data object (C++ constructed).
    pub fn new() -> Self {
        let raw = unsafe { sys::ImTextureData_ImTextureData() };
        let raw = NonNull::new(raw).expect("ImTextureData_ImTextureData() returned null");
        Self { raw }
    }

    /// Leak the underlying `ImTextureData*` without destroying it.
    pub fn into_raw(self) -> *mut sys::ImTextureData {
        let raw = self.raw.as_ptr();
        std::mem::forget(self);
        raw
    }

    /// Take ownership of a raw `ImTextureData*`.
    ///
    /// # Safety
    /// - `raw` must be a valid pointer returned by `ImTextureData_ImTextureData()`.
    /// - The caller must ensure no other owner will call `ImTextureData_destroy(raw)`.
    pub unsafe fn from_raw_owned(raw: *mut sys::ImTextureData) -> Self {
        let raw = NonNull::new(raw).expect("raw ImTextureData pointer was null");
        Self { raw }
    }
}

impl Drop for OwnedTextureData {
    fn drop(&mut self) {
        unsafe { sys::ImTextureData_destroy(self.raw.as_ptr()) }
    }
}

impl std::ops::Deref for OwnedTextureData {
    type Target = TextureData;

    fn deref(&self) -> &Self::Target {
        unsafe { &*(self.raw.as_ptr() as *const TextureData) }
    }
}

impl std::ops::DerefMut for OwnedTextureData {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *(self.raw.as_ptr() as *mut TextureData) }
    }
}

impl AsRef<TextureData> for OwnedTextureData {
    fn as_ref(&self) -> &TextureData {
        self
    }
}

impl AsMut<TextureData> for OwnedTextureData {
    fn as_mut(&mut self) -> &mut TextureData {
        self
    }
}
