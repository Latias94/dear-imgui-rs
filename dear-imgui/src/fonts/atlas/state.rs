use std::cell::RefCell;
use std::collections::HashMap;

use crate::sys;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct FontAtlasState {
    pub(super) stamp: u64,
    pub(super) generation: u64,
}

#[derive(Default)]
pub(super) struct FontAtlasStates {
    next_stamp: u64,
    by_atlas: HashMap<usize, FontAtlasState>,
}

thread_local! {
    static FONT_ATLAS_STATES: RefCell<FontAtlasStates> = RefCell::new(FontAtlasStates {
        next_stamp: 1,
        by_atlas: HashMap::new(),
    });
}

pub(super) fn font_atlas_state(raw: *mut sys::ImFontAtlas) -> FontAtlasState {
    assert!(!raw.is_null(), "font atlas pointer must not be null");
    FONT_ATLAS_STATES.with(|states| {
        let mut states = states.borrow_mut();
        let key = raw as usize;
        if let Some(state) = states.by_atlas.get(&key).copied() {
            return state;
        }
        let stamp = states.next_stamp;
        states.next_stamp = states
            .next_stamp
            .checked_add(1)
            .expect("font atlas stamp counter overflowed");
        let state = FontAtlasState {
            stamp,
            generation: 0,
        };
        states.by_atlas.insert(key, state);
        state
    })
}

pub(super) fn bump_font_atlas_generation(raw: *mut sys::ImFontAtlas) -> FontAtlasState {
    assert!(!raw.is_null(), "font atlas pointer must not be null");
    FONT_ATLAS_STATES.with(|states| {
        let mut states = states.borrow_mut();
        let key = raw as usize;
        let mut state = states.by_atlas.get(&key).copied().unwrap_or_else(|| {
            let stamp = states.next_stamp;
            states.next_stamp = states
                .next_stamp
                .checked_add(1)
                .expect("font atlas stamp counter overflowed");
            FontAtlasState {
                stamp,
                generation: 0,
            }
        });
        state.generation = state
            .generation
            .checked_add(1)
            .expect("font atlas generation counter overflowed");
        states.by_atlas.insert(key, state);
        state
    })
}

pub(crate) fn forget_font_atlas_generation(raw: *mut sys::ImFontAtlas) {
    if raw.is_null() {
        return;
    }
    FONT_ATLAS_STATES.with(|states| {
        states.borrow_mut().by_atlas.remove(&(raw as usize));
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
