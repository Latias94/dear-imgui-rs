use std::fmt;
use std::marker::PhantomData;
use std::ops::Deref;
use std::ptr::NonNull;
use std::rc::Rc;

use crate::sys;
use crate::texture::TextureData;

use super::FontAtlas;
use super::state::{acquire_font_atlas_texture_borrow, release_font_atlas_texture_borrow};

/// Read-only lease for the current font atlas texture.
///
/// The lease keeps safe atlas operations and frame advancement from invalidating the texture or
/// its pixel buffer. Drop it before rebuilding, compacting, clearing, writing custom rectangles,
/// or starting another frame that uses the atlas.
#[must_use = "keep the texture lease alive while using texture data or pixel slices"]
pub struct FontAtlasTexture<'atlas> {
    atlas: NonNull<sys::ImFontAtlas>,
    texture: NonNull<sys::ImTextureData>,
    atlas_stamp: u64,
    _atlas: PhantomData<&'atlas FontAtlas>,
    _not_send_sync: PhantomData<Rc<()>>,
}

impl<'atlas> FontAtlasTexture<'atlas> {
    pub(crate) unsafe fn from_raw(
        atlas: *mut sys::ImFontAtlas,
        texture: *mut sys::ImTextureData,
    ) -> Option<Self> {
        let atlas = NonNull::new(atlas)?;
        let texture = NonNull::new(texture)?;
        let atlas_stamp = acquire_font_atlas_texture_borrow(atlas.as_ptr());
        Some(Self {
            atlas,
            texture,
            atlas_stamp,
            _atlas: PhantomData,
            _not_send_sync: PhantomData,
        })
    }

    /// Returns an owner-backed logical reference for image and plotting APIs.
    pub fn texture_ref(&self) -> crate::texture::TextureRef<'_> {
        let texture = unsafe { sys::ImTextureData_GetTexRef(self.texture.as_ptr()) };
        crate::texture::TextureRef::from_font_atlas_raw(self.atlas.as_ptr(), texture)
    }
}

impl crate::Ui {
    /// Lease the current Context's font-atlas texture for immediate image submission.
    pub fn font_atlas_texture(&self) -> Option<FontAtlasTexture<'_>> {
        self.run_with_bound_context(|| unsafe {
            let io = sys::igGetIO_ContextPtr(self.context_raw());
            if io.is_null() || (*io).Fonts.is_null() {
                return None;
            }
            let atlas = (*io).Fonts;
            FontAtlasTexture::from_raw(atlas, (*atlas).TexData)
        })
    }
}

impl Deref for FontAtlasTexture<'_> {
    type Target = TextureData;

    fn deref(&self) -> &Self::Target {
        unsafe { TextureData::from_raw_ref(self.texture.as_ptr()) }
    }
}

impl fmt::Debug for FontAtlasTexture<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FontAtlasTexture")
            .field("native_unique_id", &self.native_unique_id())
            .field("status", &self.status())
            .field("format", &self.format())
            .field("width", &self.width())
            .field("height", &self.height())
            .finish_non_exhaustive()
    }
}

impl Drop for FontAtlasTexture<'_> {
    fn drop(&mut self) {
        release_font_atlas_texture_borrow(self.atlas.as_ptr(), self.atlas_stamp);
    }
}
