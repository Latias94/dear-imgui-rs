use std::cell::Cell;
use std::ptr::NonNull;
use std::rc::Rc;

use crate::sys;

use super::state::{font_atlas_state, forget_font_atlas_generation};

#[derive(Debug)]
struct SharedFontAtlasInner {
    raw: NonNull<sys::ImFontAtlas>,
    next_frame: Cell<i32>,
    renderer_has_textures: Cell<Option<bool>>,
}

impl Drop for SharedFontAtlasInner {
    fn drop(&mut self) {
        unsafe {
            let raw = self.raw.as_ptr();
            if (*raw).RefCount != 0 {
                debug_assert_eq!(
                    (*raw).RefCount,
                    0,
                    "SharedFontAtlas dropped while still registered with a context"
                );
                return;
            }
            sys::ImFontAtlas_destroy(raw);
            forget_font_atlas_generation(raw);
        }
    }
}

/// A shared font atlas that can be used across multiple contexts.
///
/// The final Rust owner destroys the native atlas. Contexts that use the atlas unregister from it
/// before native context shutdown, so Dear ImGui never races this owner or destroys it twice.
#[derive(Debug, Clone)]
pub struct SharedFontAtlas(Rc<SharedFontAtlasInner>);

impl SharedFontAtlas {
    /// Creates a new shared font atlas
    pub fn create() -> SharedFontAtlas {
        unsafe {
            let raw_atlas = sys::ImFontAtlas_ImFontAtlas();
            if raw_atlas.is_null() {
                panic!("ImFontAtlas_ImFontAtlas() returned null");
            }
            font_atlas_state(raw_atlas);
            SharedFontAtlas(Rc::new(SharedFontAtlasInner {
                raw: NonNull::new_unchecked(raw_atlas),
                next_frame: Cell::new(0),
                renderer_has_textures: Cell::new(None),
            }))
        }
    }

    pub(crate) fn as_ptr(&self) -> *mut sys::ImFontAtlas {
        self.0.raw.as_ptr()
    }

    pub(crate) fn prepare_frame(&self, renderer_has_textures: bool) {
        let expected_renderer_has_textures = self.0.renderer_has_textures.get();
        match expected_renderer_has_textures {
            Some(expected) => assert_eq!(
                renderer_has_textures, expected,
                "all contexts sharing a font atlas must agree on BackendFlags::RENDERER_HAS_TEXTURES"
            ),
            None => {}
        }

        if expected_renderer_has_textures.is_none() {
            self.0
                .renderer_has_textures
                .set(Some(renderer_has_textures));
        }

        let frame = self.0.next_frame.get();
        let next_frame = frame
            .checked_add(1)
            .expect("shared font-atlas frame counter overflowed");
        unsafe {
            sys::igImFontAtlasUpdateNewFrame(self.as_ptr(), frame, renderer_has_textures);
        }
        self.0.next_frame.set(next_frame);
    }

    pub(crate) fn unregister_from_current_context(&self) {
        unsafe {
            let raw = self.as_ptr();
            sys::igUnregisterFontAtlas(raw);
            if (*raw).RefCount == 0 {
                (*raw).Locked = false;
                self.0.renderer_has_textures.set(None);
            }
        }
    }
}
