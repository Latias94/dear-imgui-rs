use super::validation::{require_exact_payload, validate_new_texture_layout};
use super::{TextureData, TextureDataError, TextureFormat};
use crate::sys;
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
}

impl OwnedTextureData {
    pub(crate) fn empty() -> Self {
        let raw = unsafe { sys::ImTextureData_ImTextureData() };
        let raw = NonNull::new(raw).expect("ImTextureData_ImTextureData() returned null");
        Self { raw }
    }

    /// Create owned texture data from an exact pixel payload.
    ///
    /// Validation completes before the native texture object allocates pixel storage. The payload
    /// must contain exactly `width * height * bytes_per_pixel(format)` bytes.
    ///
    /// # Errors
    ///
    /// Returns [`TextureDataError`] when the dimensions or allocation size are not representable,
    /// or when `pixels` does not exactly match the requested layout.
    pub fn from_pixels(
        format: TextureFormat,
        width: u32,
        height: u32,
        pixels: &[u8],
    ) -> Result<Self, TextureDataError> {
        let layout = validate_new_texture_layout(format, width, height)?;
        require_exact_payload(layout.byte_len, pixels.len())?;

        let mut texture = Self::empty();
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
