use std::fmt;
use std::ops::Deref;
use std::ptr::NonNull;

use crate::sys;
use crate::texture::{TextureData, TextureRef};

use super::FontAtlas;
use super::state::{
    acquire_font_atlas_texture_borrow, assert_no_open_font_atlas_frames,
    release_font_atlas_texture_borrow,
};

/// Read-only lease for the current font atlas texture between frames.
///
/// The lease keeps safe atlas operations and frame advancement from invalidating the texture or
/// its pixel buffer. Drop it before rebuilding, compacting, clearing, writing custom rectangles,
/// or starting another frame that uses the atlas.
#[must_use = "keep the texture lease alive while using texture data or pixel slices"]
pub struct FontAtlasTexture<'atlas> {
    atlas: &'atlas FontAtlas,
    texture: NonNull<sys::ImTextureData>,
}

impl FontAtlas {
    pub(crate) fn tex_data_internal(&self) -> Option<FontAtlasTexture<'_>> {
        let raw = self.raw();
        assert_no_open_font_atlas_frames(raw, "borrowing font atlas texture data");
        let texture = NonNull::new(unsafe { (*raw).TexData })?;
        acquire_font_atlas_texture_borrow(self);
        Some(FontAtlasTexture {
            atlas: self,
            texture,
        })
    }
}

impl FontAtlasTexture<'_> {
    /// Returns an owner-backed logical reference for image and plotting APIs.
    pub fn texture_ref(&self) -> TextureRef<'_> {
        let texture = unsafe { sys::ImTextureData_GetTexRef(self.texture.as_ptr()) };
        TextureRef::from_font_atlas_raw(self.atlas.raw(), texture)
    }
}

impl crate::Ui {
    /// Return the current font atlas texture for image submission in this frame.
    ///
    /// Dear ImGui keeps the texture allocation alive through the frame. Drawing text may write
    /// new glyphs into its pixels or grow the list of pending updates. For CPU pixel access
    /// between frames, use [`LegacyFontAtlas::tex_data`](super::LegacyFontAtlas::tex_data).
    ///
    /// ```compile_fail,E0599
    /// fn borrow_pixels(ui: &dear_imgui_rs::Ui) {
    ///     let image = ui.font_atlas_texture().unwrap();
    ///     let _ = image.pixels();
    /// }
    /// ```
    pub fn font_atlas_texture(&self) -> Option<TextureRef<'_>> {
        self.run_with_bound_context(|| unsafe {
            let io = sys::igGetIO_ContextPtr(self.context_raw());
            if io.is_null() || (*io).Fonts.is_null() {
                return None;
            }
            let atlas = (*io).Fonts;
            let texture = NonNull::new((*atlas).TexData)?;
            Some(TextureRef::from_font_atlas_raw(
                atlas,
                sys::ImTextureData_GetTexRef(texture.as_ptr()),
            ))
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
        release_font_atlas_texture_borrow(self.atlas);
    }
}
