use std::marker::PhantomData;
use std::rc::Rc;

use crate::fonts::Font;
use crate::sys;

use super::state::{current_context_font_atlas, font_atlas_contains_font, font_atlas_state};

/// A persistent, atlas-validated font handle.
///
/// `FontId` can be stored in application style state and later passed to
/// [`Ui::push_font`](crate::Ui::push_font), but it is not just a raw `ImFont*`.
/// The handle records the originating atlas and atlas generation. Safe push
/// APIs validate that the handle still belongs to the current context's atlas
/// and has not been invalidated by [`FontAtlas::clear`],
/// [`FontAtlas::clear_fonts`], or [`FontAtlas::remove_font`] before calling
/// Dear ImGui.
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub struct FontId {
    pub(crate) raw: *mut sys::ImFont,
    atlas: *mut sys::ImFontAtlas,
    atlas_stamp: u64,
    generation: u64,
    _not_send_sync: PhantomData<Rc<()>>,
}

impl FontId {
    pub(crate) fn from_raw_parts(font: *mut sys::ImFont, atlas: *mut sys::ImFontAtlas) -> Self {
        assert!(!font.is_null(), "FontId requires a non-null ImFont pointer");
        assert!(
            !atlas.is_null(),
            "FontId requires a non-null ImFontAtlas pointer"
        );
        let state = font_atlas_state(atlas);
        Self {
            raw: font,
            atlas,
            atlas_stamp: state.stamp,
            generation: state.generation,
            _not_send_sync: PhantomData,
        }
    }

    pub(crate) unsafe fn from_font(font: *mut sys::ImFont, caller: &str) -> Self {
        assert!(!font.is_null(), "{caller} requires a non-null font");
        let atlas = unsafe { (*font).OwnerAtlas };
        assert!(
            !atlas.is_null(),
            "{caller} requires the font to have an owning atlas"
        );
        Self::from_raw_parts(font, atlas)
    }
}

pub(crate) fn validate_font_id_for_current_context(id: FontId, caller: &str) -> *mut sys::ImFont {
    let atlas = current_context_font_atlas(caller);
    validate_font_id_for_atlas(id, atlas, caller)
}

pub(crate) fn validate_font_for_current_context(font: &Font, caller: &str) -> *mut sys::ImFont {
    let atlas = current_context_font_atlas(caller);
    validate_font_for_atlas(font, atlas, caller)
}

pub(crate) fn validate_font_id_for_atlas(
    id: FontId,
    atlas: *mut sys::ImFontAtlas,
    caller: &str,
) -> *mut sys::ImFont {
    assert!(!id.raw.is_null(), "{caller} received a null FontId");
    assert!(
        std::ptr::addr_eq(id.atlas.cast_const(), atlas.cast_const()),
        "{caller} received a FontId from a different font atlas"
    );
    let state = font_atlas_state(atlas);
    assert!(
        state.stamp == id.atlas_stamp,
        "{caller} received a FontId from a destroyed or reused font atlas"
    );
    assert!(
        state.generation == id.generation,
        "{caller} received a stale FontId invalidated by font atlas mutation"
    );
    assert!(
        font_atlas_contains_font(atlas, id.raw),
        "{caller} received a FontId that is not present in the current font atlas"
    );
    id.raw
}

pub(crate) fn validate_font_for_atlas(
    font: &Font,
    atlas: *mut sys::ImFontAtlas,
    caller: &str,
) -> *mut sys::ImFont {
    let raw = font.raw();
    assert!(!raw.is_null(), "{caller} received a null font");
    unsafe {
        let owner = (*raw).OwnerAtlas;
        assert!(
            std::ptr::addr_eq(owner.cast_const(), atlas.cast_const()),
            "{caller} received a font from a different font atlas"
        );
    }
    assert!(
        font_atlas_contains_font(atlas, raw),
        "{caller} received a font that is not present in the current font atlas"
    );
    raw
}
