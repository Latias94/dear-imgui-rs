use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use crate::sys;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct FontAtlasState {
    pub(super) stamp: u64,
    pub(super) generation: u64,
    pub(super) custom_rect_generation: u64,
    texture_borrows: usize,
}

#[derive(Default)]
pub(super) struct FontAtlasStates {
    next_stamp: u64,
    next_custom_rect_nonce: u64,
    by_atlas: HashMap<usize, FontAtlasState>,
    contexts_by_atlas: HashMap<usize, HashSet<usize>>,
    custom_rect_nonces: HashMap<usize, HashMap<sys::ImFontAtlasRectId, u64>>,
    owned_glyph_ranges: HashMap<usize, Vec<Box<[sys::ImWchar]>>>,
}

thread_local! {
    static FONT_ATLAS_STATES: RefCell<FontAtlasStates> = RefCell::new(FontAtlasStates {
        next_stamp: 1,
        next_custom_rect_nonce: 1,
        by_atlas: HashMap::new(),
        contexts_by_atlas: HashMap::new(),
        custom_rect_nonces: HashMap::new(),
        owned_glyph_ranges: HashMap::new(),
    });
}

impl FontAtlasStates {
    fn get_or_insert(&mut self, raw: *mut sys::ImFontAtlas) -> &mut FontAtlasState {
        let key = raw as usize;
        if !self.by_atlas.contains_key(&key) {
            let stamp = self.next_stamp;
            self.next_stamp = self
                .next_stamp
                .checked_add(1)
                .expect("font atlas stamp counter overflowed");
            self.by_atlas.insert(
                key,
                FontAtlasState {
                    stamp,
                    generation: 0,
                    custom_rect_generation: 0,
                    texture_borrows: 0,
                },
            );
        }
        self.by_atlas
            .get_mut(&key)
            .expect("font atlas state was just inserted")
    }
}

pub(super) fn font_atlas_state(raw: *mut sys::ImFontAtlas) -> FontAtlasState {
    assert!(!raw.is_null(), "font atlas pointer must not be null");
    FONT_ATLAS_STATES.with(|states| {
        let mut states = states.borrow_mut();
        *states.get_or_insert(raw)
    })
}

pub(super) fn bump_font_atlas_generation(raw: *mut sys::ImFontAtlas) -> FontAtlasState {
    assert!(!raw.is_null(), "font atlas pointer must not be null");
    FONT_ATLAS_STATES.with(|states| {
        let mut states = states.borrow_mut();
        let state = states.get_or_insert(raw);
        state.generation = state
            .generation
            .checked_add(1)
            .expect("font atlas generation counter overflowed");
        *state
    })
}

pub(super) fn bump_custom_rect_generation(raw: *mut sys::ImFontAtlas) -> FontAtlasState {
    assert!(!raw.is_null(), "font atlas pointer must not be null");
    FONT_ATLAS_STATES.with(|states| {
        let mut states = states.borrow_mut();
        let state = states.get_or_insert(raw);
        state.custom_rect_generation = state
            .custom_rect_generation
            .checked_add(1)
            .expect("custom rectangle generation counter overflowed");
        let state = *state;
        states.custom_rect_nonces.remove(&(raw as usize));
        state
    })
}

pub(super) fn register_custom_rect_nonce(
    atlas: *mut sys::ImFontAtlas,
    raw_id: sys::ImFontAtlasRectId,
) -> (FontAtlasState, u64) {
    assert!(!atlas.is_null(), "font atlas pointer must not be null");
    assert!(raw_id >= 0, "custom rectangle ID must be valid");
    FONT_ATLAS_STATES.with(|states| {
        let mut states = states.borrow_mut();
        let state = *states.get_or_insert(atlas);
        let nonce = states.next_custom_rect_nonce;
        states.next_custom_rect_nonce = nonce
            .checked_add(1)
            .expect("custom rectangle nonce counter overflowed");
        let previous = states
            .custom_rect_nonces
            .entry(atlas as usize)
            .or_default()
            .insert(raw_id, nonce);
        assert!(
            previous.is_none(),
            "native font atlas reused an active custom rectangle ID"
        );
        (state, nonce)
    })
}

pub(super) fn custom_rect_nonce_is_active(
    atlas: *mut sys::ImFontAtlas,
    raw_id: sys::ImFontAtlasRectId,
    nonce: u64,
) -> bool {
    FONT_ATLAS_STATES.with(|states| {
        states
            .borrow()
            .custom_rect_nonces
            .get(&(atlas as usize))
            .and_then(|nonces| nonces.get(&raw_id))
            .is_some_and(|active| *active == nonce)
    })
}

pub(super) fn unregister_custom_rect_nonce(
    atlas: *mut sys::ImFontAtlas,
    raw_id: sys::ImFontAtlasRectId,
    nonce: u64,
) {
    FONT_ATLAS_STATES.with(|states| {
        let mut states = states.borrow_mut();
        let atlas_key = atlas as usize;
        let remove_atlas_entry = if let Some(nonces) = states.custom_rect_nonces.get_mut(&atlas_key)
        {
            if nonces.get(&raw_id).is_some_and(|active| *active == nonce) {
                nonces.remove(&raw_id);
            }
            nonces.is_empty()
        } else {
            false
        };
        if remove_atlas_entry {
            states.custom_rect_nonces.remove(&atlas_key);
        }
    });
}

pub(crate) fn register_font_atlas_context(
    atlas: *mut sys::ImFontAtlas,
    context: *mut sys::ImGuiContext,
) {
    assert!(!atlas.is_null(), "font atlas pointer must not be null");
    assert!(!context.is_null(), "ImGui context pointer must not be null");
    FONT_ATLAS_STATES.with(|states| {
        let mut states = states.borrow_mut();
        states.get_or_insert(atlas);
        let inserted = states
            .contexts_by_atlas
            .entry(atlas as usize)
            .or_default()
            .insert(context as usize);
        assert!(
            inserted,
            "ImGui context was registered with its font atlas twice"
        );
    });
}

pub(crate) fn unregister_font_atlas_context(
    atlas: *mut sys::ImFontAtlas,
    context: *mut sys::ImGuiContext,
) {
    if atlas.is_null() || context.is_null() {
        return;
    }
    FONT_ATLAS_STATES.with(|states| {
        let mut states = states.borrow_mut();
        let atlas_key = atlas as usize;
        let remove_atlas_entry =
            if let Some(contexts) = states.contexts_by_atlas.get_mut(&atlas_key) {
                let removed = contexts.remove(&(context as usize));
                debug_assert!(
                    removed,
                    "ImGui context was not registered with this font atlas"
                );
                contexts.is_empty()
            } else {
                debug_assert!(false, "font atlas has no registered ImGui contexts");
                false
            };
        if remove_atlas_entry {
            states.contexts_by_atlas.remove(&atlas_key);
        }
    });
}

pub(crate) fn assert_no_open_font_atlas_frames(raw: *mut sys::ImFontAtlas, caller: &str) {
    assert!(!raw.is_null(), "{caller} requires a valid font atlas");
    FONT_ATLAS_STATES.with(|states| {
        let states = states.borrow();
        let Some(contexts) = states.contexts_by_atlas.get(&(raw as usize)) else {
            return;
        };
        for &context in contexts {
            let context = context as *mut sys::ImGuiContext;
            assert!(
                !unsafe { (*context).WithinFrameScope },
                "{caller} cannot modify a font atlas while a registered ImGui context is mid-frame"
            );
        }
    });
}

pub(super) fn acquire_font_atlas_texture_borrow(raw: *mut sys::ImFontAtlas) -> u64 {
    assert!(!raw.is_null(), "font atlas pointer must not be null");
    FONT_ATLAS_STATES.with(|states| {
        let mut states = states.borrow_mut();
        let state = states.get_or_insert(raw);
        state.texture_borrows = state
            .texture_borrows
            .checked_add(1)
            .expect("font atlas texture borrow counter overflowed");
        state.stamp
    })
}

pub(super) fn release_font_atlas_texture_borrow(raw: *mut sys::ImFontAtlas, stamp: u64) {
    if raw.is_null() {
        return;
    }
    FONT_ATLAS_STATES.with(|states| {
        let mut states = states.borrow_mut();
        let Some(state) = states.by_atlas.get_mut(&(raw as usize)) else {
            debug_assert!(false, "font atlas texture borrow outlived its atlas state");
            return;
        };
        debug_assert_eq!(
            state.stamp, stamp,
            "font atlas texture borrow belongs to a reused atlas address"
        );
        if state.stamp == stamp {
            debug_assert!(state.texture_borrows > 0);
            state.texture_borrows = state.texture_borrows.saturating_sub(1);
        }
    });
}

pub(crate) fn assert_no_font_atlas_texture_borrows(raw: *mut sys::ImFontAtlas, caller: &str) {
    let state = font_atlas_state(raw);
    assert_eq!(
        state.texture_borrows, 0,
        "{caller} cannot invalidate a borrowed font atlas texture; drop the FontAtlasTexture view first"
    );
}

pub(crate) fn assert_font_atlas_renderer_mode(
    raw: *mut sys::ImFontAtlas,
    renderer_has_textures: bool,
    caller: &str,
) {
    assert!(!raw.is_null(), "{caller} requires a valid font atlas");
    unsafe {
        let builder = (*raw).Builder;
        if renderer_has_textures {
            assert!(
                builder.is_null() || !(*builder).PreloadedAllGlyphsRanges,
                "{caller} cannot switch a legacy-built font atlas to RENDERER_HAS_TEXTURES; clear and repopulate the atlas before changing renderer mode"
            );
        } else {
            assert!(
                (*raw).TexIsBuilt && !builder.is_null() && (*builder).PreloadedAllGlyphsRanges,
                "{caller} requires a font atlas built for legacy glyph preloading; call FontAtlas::build() after configuring the legacy context"
            );
        }
    }
}

pub(crate) fn forget_font_atlas_generation(raw: *mut sys::ImFontAtlas) {
    if raw.is_null() {
        return;
    }
    FONT_ATLAS_STATES.with(|states| {
        let mut states = states.borrow_mut();
        states.by_atlas.remove(&(raw as usize));
        states.contexts_by_atlas.remove(&(raw as usize));
        states.custom_rect_nonces.remove(&(raw as usize));
        states.owned_glyph_ranges.remove(&(raw as usize));
    });
}

pub(super) fn store_font_atlas_glyph_ranges(
    raw: *mut sys::ImFontAtlas,
    ranges: Vec<sys::ImWchar>,
) -> *const sys::ImWchar {
    if ranges.is_empty() {
        return std::ptr::null();
    }
    assert!(!raw.is_null(), "font atlas pointer must not be null");
    FONT_ATLAS_STATES.with(|states| {
        let mut states = states.borrow_mut();
        let ranges = ranges.into_boxed_slice();
        let ptr = ranges.as_ptr();
        states
            .owned_glyph_ranges
            .entry(raw as usize)
            .or_default()
            .push(ranges);
        ptr
    })
}

pub(super) fn clear_font_atlas_glyph_ranges(raw: *mut sys::ImFontAtlas) {
    if raw.is_null() {
        return;
    }
    FONT_ATLAS_STATES.with(|states| {
        states
            .borrow_mut()
            .owned_glyph_ranges
            .remove(&(raw as usize));
    });
}

pub(super) fn font_atlas_contains_font(
    atlas: *mut sys::ImFontAtlas,
    font: *mut sys::ImFont,
) -> bool {
    if atlas.is_null() || font.is_null() {
        return false;
    }
    unsafe {
        let fonts = &(*atlas).Fonts;
        if fonts.Size <= 0 || fonts.Data.is_null() {
            return false;
        }
        for index in 0..fonts.Size {
            if *fonts.Data.add(index as usize) == font {
                return true;
            }
        }
    }
    false
}

pub(super) fn current_context_font_atlas(caller: &str) -> *mut sys::ImFontAtlas {
    unsafe {
        let ctx = sys::igGetCurrentContext();
        assert!(!ctx.is_null(), "{caller} requires an active ImGui context");
        let io = sys::igGetIO_ContextPtr(ctx);
        assert!(!io.is_null(), "{caller} requires a valid ImGui IO");
        let atlas = (*io).Fonts;
        assert!(
            !atlas.is_null(),
            "{caller} requires the current ImGui context to have a font atlas"
        );
        atlas
    }
}
