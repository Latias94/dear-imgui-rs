use std::fmt;
use std::ops::Deref;
use std::ptr::NonNull;

use crate::FontError;
use crate::sys;
use crate::texture::{TextureData, TextureRef};

use super::FontAtlas;
use super::state::{
    acquire_font_atlas_texture_borrow, assert_no_open_font_atlas_frames,
    bump_custom_rect_generation, claim_font_atlas_legacy_renderer,
    release_font_atlas_legacy_renderer, release_font_atlas_texture_borrow,
};

/// Legacy renderer capability for a font atlas.
///
/// The common [`FontAtlas`] view is intentionally limited to font configuration and managed
/// texture state. Acquire this capability before using a renderer-owned CPU atlas texture or
/// calling [`LegacyFontAtlas::build`]. Acquiring it claims legacy renderer mode immediately, so a
/// managed renderer conflict is reported at this call instead of being deferred to the next
/// frame.
#[must_use = "dropping this capability releases its active lease but preserves legacy renderer mode until the atlas is fully cleared"]
#[derive(Debug)]
pub struct LegacyFontAtlas<'atlas> {
    atlas: &'atlas FontAtlas,
}

impl<'atlas> LegacyFontAtlas<'atlas> {
    /// Borrow the common font-configuration capability.
    pub fn atlas(&self) -> &'atlas FontAtlas {
        self.atlas
    }

    /// Build the legacy font atlas texture.
    ///
    /// This is the complete build entry point for legacy renderers. Managed renderers build and
    /// upload the atlas through texture requests owned by their renderer consumer instead.
    #[doc(alias = "Build")]
    pub fn build(&self) {
        self.atlas
            .assert_mutation_allowed("LegacyFontAtlas::build()");
        let raw = self.atlas.raw();
        // NOTE: In Dear ImGui, `ImFontAtlasBuildMain()` will call `ImFontAtlasBuildInit()`
        // lazily if needed (Builder == NULL). Calling BuildInit unconditionally would leak
        // the builder and is not idempotent.
        unsafe {
            let rebuilds_builder =
                !(*raw).TexData.is_null() && (*(*raw).TexData).Format != (*raw).TexDesiredFormat;
            sys::igImFontAtlasBuildMain(raw);
            if rebuilds_builder {
                bump_custom_rect_generation(raw);
            }
        }
    }

    /// Check if the texture is built
    pub fn is_built(&self) -> bool {
        unsafe { (*self.atlas.raw()).TexIsBuilt }
    }

    /// Resolve the current atlas texture ID.
    pub fn texture_id(&self) -> crate::texture::TextureId {
        self.atlas.texture_id_internal()
    }

    /// Set a legacy renderer-owned atlas texture ID and mark its native status as ready.
    ///
    /// Managed renderers should process the atlas request from `PendingFrame` and return
    /// request-bound `TextureFeedback` instead. This method also updates `TexRef` so legacy draw
    /// commands continue to follow `ImTextureData` when one is available.
    ///
    /// # Safety
    ///
    /// `tex_id` must identify a live texture owned by the active renderer for every draw command
    /// that can observe it. The atlas must be in legacy texture mode and must not be shared with a
    /// Context using managed renderer textures. The caller must clear or replace the binding only
    /// after all GPU use has completed.
    pub unsafe fn set_texture_id(&self, tex_id: crate::texture::TextureId) {
        unsafe {
            let raw = self.atlas.raw();
            let texture = (*raw).TexData;
            (*raw).TexRef = if texture.is_null() {
                sys::ImTextureRef {
                    _TexData: std::ptr::null_mut(),
                    _TexID: sys::ImTextureID::from(tex_id),
                }
            } else {
                sys::ImTextureData_SetTexID(texture, sys::ImTextureID::from(tex_id));
                sys::ImTextureData_SetStatus(texture, sys::ImTextureStatus_OK);
                sys::ImTextureData_GetTexRef(texture)
            };
        }
    }

    /// Lease the current atlas texture data, if available.
    ///
    /// Pixel slices borrowed from this view remain valid until the lease is dropped. While it is
    /// alive, safe atlas mutation and frame advancement reject operations that could invalidate
    /// the texture.
    ///
    /// Keep the CPU copy after uploading. Dear ImGui uses it to copy resident glyphs during
    /// repacking. To discard the atlas contents, use [`FontAtlas::clear`] and repopulate the
    /// atlas before rebuilding.
    ///
    /// ```compile_fail,E0599
    /// fn discard_pixels(atlas: &dear_imgui_rs::LegacyFontAtlas<'_>) {
    ///     atlas.clear_cpu_texture_data();
    /// }
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if a context using this atlas has an open frame. During drawing, use
    /// [`Ui::font_atlas_texture`](crate::Ui::font_atlas_texture).
    pub fn tex_data(&self) -> Option<crate::fonts::FontAtlasTexture<'atlas>> {
        self.atlas.tex_data_internal()
    }
}

impl Deref for LegacyFontAtlas<'_> {
    type Target = FontAtlas;

    fn deref(&self) -> &Self::Target {
        self.atlas
    }
}

impl Drop for LegacyFontAtlas<'_> {
    fn drop(&mut self) {
        release_font_atlas_legacy_renderer(self.atlas.raw());
    }
}

pub(super) fn claim_legacy_renderer(atlas: &FontAtlas) -> Result<LegacyFontAtlas<'_>, FontError> {
    claim_font_atlas_legacy_renderer(atlas.raw())?;
    Ok(LegacyFontAtlas { atlas })
}

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

    pub(crate) fn texture_id_internal(&self) -> crate::texture::TextureId {
        unsafe { crate::texture::effective_texture_id(&(*self.raw()).TexRef) }
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
    /// between frames, use [`LegacyFontAtlas::tex_data`](LegacyFontAtlas::tex_data).
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
