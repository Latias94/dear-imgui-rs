use super::{TextureData, TextureError, TextureFormat};
use super::{require_exact_payload, validate_new_texture_layout};
use crate::{ContextThreadLease, ImGuiError, ImGuiResult, sys};
use std::mem::ManuallyDrop;
use std::ptr::NonNull;

/// Owned texture data managed by Dear ImGui.
///
/// This owns an `ImTextureData` instance allocated by Dear ImGui (C++) and will
/// destroy it on drop. It dereferences to [`TextureData`] while application code owns it.
/// Registering it with a Context transfers ownership; later access uses
/// `Context::with_texture` for inspection and normally `Context::try_with_texture_mut` for fallible
/// pixel changes. `Context::with_texture_mut` remains the lower-level result-composition API, so
/// callers must handle any inner mutation result.
///
/// Empty construction and the former `create`/`set_data` sequence are intentionally unavailable;
/// use [`Self::from_pixels`] so invalid payloads cannot create a partially initialized texture.
///
/// ```compile_fail
/// use dear_imgui_rs::texture::OwnedTextureData;
/// let _ = OwnedTextureData::new();
/// ```
///
/// ```compile_fail
/// use dear_imgui_rs::texture::{OwnedTextureData, TextureFormat};
/// let mut texture =
///     OwnedTextureData::from_pixels(TextureFormat::RGBA32, 1, 1, &[0; 4]).unwrap();
/// texture.create(TextureFormat::RGBA32, 2, 2);
/// ```
///
/// ```compile_fail
/// use dear_imgui_rs::texture::{OwnedTextureData, TextureFormat};
/// let mut texture =
///     OwnedTextureData::from_pixels(TextureFormat::RGBA32, 1, 1, &[0; 4]).unwrap();
/// texture.set_data(&[255; 4]);
/// ```
pub struct OwnedTextureData {
    raw: NonNull<sys::ImTextureData>,
    thread_lease: ContextThreadLease,
}

impl OwnedTextureData {
    #[cfg(test)]
    pub(crate) fn empty() -> Self {
        Self::try_empty().expect("failed to create empty Dear ImGui texture")
    }

    fn try_empty() -> Result<Self, TextureError> {
        let thread_lease = ContextThreadLease::acquire().map_err(|error| match error {
            crate::ContextError::ThreadConflict => TextureError::ContextThreadConflict,
            _ => TextureError::AllocationFailed,
        })?;
        let raw = unsafe { sys::ImTextureData_ImTextureData() };
        let raw = NonNull::new(raw).ok_or(TextureError::AllocationFailed)?;
        Ok(Self { raw, thread_lease })
    }

    /// Create owned texture data from an exact pixel payload.
    ///
    /// Validation completes before the native texture object allocates pixel storage. The payload
    /// must contain exactly `width * height * bytes_per_pixel(format)` bytes.
    ///
    /// # Errors
    ///
    /// Returns [`TextureError`] for an invalid layout or payload, a thread ownership
    /// conflict, or a failed native allocation.
    pub fn from_pixels(
        format: TextureFormat,
        width: u32,
        height: u32,
        pixels: &[u8],
    ) -> Result<Self, TextureError> {
        let layout = validate_new_texture_layout(format, width, height)?;
        require_exact_payload(layout.byte_len, pixels.len())?;

        let mut texture = Self::try_empty()?;
        unsafe {
            sys::ImTextureData_Create(
                texture.raw.as_ptr(),
                format.into(),
                i32::try_from(width).expect("validated texture width must fit i32"),
                i32::try_from(height).expect("validated texture height must fit i32"),
            );
        }
        texture.replace_pixels(pixels)?;
        Ok(texture)
    }

    /// Transfers the native allocation to the caller and releases this owner's thread lease.
    ///
    /// Keep a separate [`ContextThreadLease`] on this thread through native use and destruction,
    /// or use [`Self::into_raw_parts`] to take the existing lease with the pointer.
    pub fn into_raw(self) -> *mut sys::ImTextureData {
        self.into_raw_parts().0
    }

    /// Transfers the native allocation and its thread lease together.
    ///
    /// Keep the lease until `ImTextureData_destroy` returns or ownership has been transferred
    /// to another owner on this thread.
    pub fn into_raw_parts(self) -> (*mut sys::ImTextureData, ContextThreadLease) {
        let texture = ManuallyDrop::new(self);
        // Move the lease out exactly once. ManuallyDrop suppresses both the native destructor
        // and the source lease's destructor.
        (texture.raw.as_ptr(), unsafe {
            std::ptr::read(&texture.thread_lease)
        })
    }

    /// Tries to take ownership of a raw `ImTextureData*`.
    ///
    /// A null pointer or a thread lease failure returns an error. On error the caller retains
    /// ownership of the allocation.
    ///
    /// # Safety
    ///
    /// A non-null `raw` must point to a live, exclusively owned `ImTextureData` allocated by
    /// Dear ImGui on this thread. Keep the allocation's thread lease through the transfer.
    pub unsafe fn try_from_raw_owned(raw: *mut sys::ImTextureData) -> ImGuiResult<Self> {
        let raw = NonNull::new(raw).ok_or_else(|| {
            ImGuiError::invalid_operation("cannot take ownership of a null ImTextureData pointer")
        })?;
        let thread_lease = ContextThreadLease::acquire()?;
        Ok(Self { raw, thread_lease })
    }

    /// Takes ownership of a raw `ImTextureData*`.
    ///
    /// # Panics
    ///
    /// Panics if [`Self::try_from_raw_owned`] returns an error.
    ///
    /// # Safety
    /// The caller must uphold the ownership requirements of [`Self::try_from_raw_owned`].
    pub unsafe fn from_raw_owned(raw: *mut sys::ImTextureData) -> Self {
        unsafe { Self::try_from_raw_owned(raw) }
            .expect("failed to take ownership of raw Dear ImGui texture")
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
