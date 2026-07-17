use std::marker::PhantomData;

use super::{ManagedTextureId, TextureId};
use crate::sys;

/// A logical image source accepted by safe Dear ImGui drawing APIs.
///
/// A texture reference contains no borrowed user-texture pointer. Legacy [`TextureId`] values are
/// forwarded directly, while managed handles are resolved through the `Ui`'s owning Context only
/// for the duration of the native call. Font-atlas references are created internally together with
/// an owner-backed atlas lease.
///
/// ```compile_fail
/// # use dear_imgui_rs::{TextureData, texture::TextureRef};
/// let mut texture = TextureData::new();
/// let _: TextureRef<'_> = (&mut *texture).into();
/// ```
#[derive(Copy, Clone, Debug)]
pub struct TextureRef<'texture> {
    source: TextureSource,
    _lease: PhantomData<&'texture ()>,
}

#[derive(Copy, Clone, Debug)]
pub(crate) enum TextureSource {
    Legacy(TextureId),
    Managed(ManagedTextureId),
    FontAtlas {
        atlas: *mut sys::ImFontAtlas,
        texture: sys::ImTextureRef,
    },
}

impl<'texture> TextureRef<'texture> {
    pub(crate) fn from_font_atlas_raw(
        atlas: *mut sys::ImFontAtlas,
        texture: sys::ImTextureRef,
    ) -> Self {
        assert!(!atlas.is_null(), "font atlas texture requires an owner");
        Self {
            source: TextureSource::FontAtlas { atlas, texture },
            _lease: PhantomData,
        }
    }

    pub(crate) const fn source(self) -> TextureSource {
        self.source
    }

    /// Returns the embedded legacy texture ID, if this is a legacy reference.
    pub const fn legacy_id(self) -> Option<TextureId> {
        match self.source {
            TextureSource::Legacy(id) => Some(id),
            TextureSource::Managed(_) | TextureSource::FontAtlas { .. } => None,
        }
    }
}

impl<'texture> From<TextureId> for TextureRef<'texture> {
    #[inline]
    fn from(id: TextureId) -> Self {
        Self {
            source: TextureSource::Legacy(id),
            _lease: PhantomData,
        }
    }
}

impl<'texture> From<ManagedTextureId> for TextureRef<'texture> {
    #[inline]
    fn from(id: ManagedTextureId) -> Self {
        Self {
            source: TextureSource::Managed(id),
            _lease: PhantomData,
        }
    }
}

/// Resolve a raw texture reference without asserting that a managed texture has already been
/// uploaded by a renderer.
///
/// # Safety
///
/// A non-null `_TexData` pointer must be valid for reads.
pub(crate) unsafe fn effective_texture_id(raw: &sys::ImTextureRef) -> TextureId {
    if raw._TexData.is_null() {
        TextureId::from(raw._TexID)
    } else {
        unsafe { TextureId::from((*raw._TexData).TexID) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logical_references_do_not_expose_managed_native_pointers() {
        let legacy = TextureId::new(7);
        assert_eq!(TextureRef::from(legacy).legacy_id(), Some(legacy));
        assert!(
            TextureRef::from(ManagedTextureId::new(
                crate::ContextId::allocate().expect("context id"),
                3,
                std::num::NonZeroU64::new(2).expect("generation"),
            ))
            .legacy_id()
            .is_none()
        );
    }
}
