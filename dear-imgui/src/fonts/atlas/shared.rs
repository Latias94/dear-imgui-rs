use std::rc::Rc;

use crate::sys;

use super::state::{font_atlas_state, forget_font_atlas_generation};

/// A shared font atlas that can be used across multiple contexts
///
/// This allows multiple ImGui contexts to share the same font atlas,
/// which is useful for applications with multiple windows or contexts.
#[derive(Debug, Clone)]
pub struct SharedFontAtlas(pub(crate) Rc<*mut sys::ImFontAtlas>);

impl SharedFontAtlas {
    /// Creates a new shared font atlas
    pub fn create() -> SharedFontAtlas {
        unsafe {
            let raw_atlas = sys::ImFontAtlas_ImFontAtlas();
            if raw_atlas.is_null() {
                panic!("ImFontAtlas_ImFontAtlas() returned null");
            }
            font_atlas_state(raw_atlas);
            SharedFontAtlas(Rc::new(raw_atlas))
        }
    }

    /// Returns a mutable raw pointer to the underlying ImFontAtlas
    pub(crate) fn as_ptr_mut(&mut self) -> *mut sys::ImFontAtlas {
        *self.0
    }
}

impl Drop for SharedFontAtlas {
    fn drop(&mut self) {
        // Only drop if this is the last reference
        if Rc::strong_count(&self.0) == 1 {
            unsafe {
                let atlas_ptr = *self.0;
                if !atlas_ptr.is_null() {
                    forget_font_atlas_generation(atlas_ptr);
                    sys::ImFontAtlas_destroy(atlas_ptr);
                }
            }
        }
    }
}
