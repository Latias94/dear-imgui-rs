use std::cell::Cell;
use std::ptr::NonNull;
use std::rc::Rc;

use crate::sys;

use super::state::{font_atlas_state, forget_font_atlas_generation};

#[derive(Debug)]
struct SharedFontAtlasInner {
    raw: NonNull<sys::ImFontAtlas>,
    next_frame: Cell<i32>,
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
///
/// Multiple contexts may share the atlas only while they use legacy renderer-managed texture
/// handling. Attaching a managed renderer with
/// [`Context::create_synchronous_renderer_consumer`](crate::Context::create_synchronous_renderer_consumer)
/// or [`Context::create_detached_renderer_consumer`](crate::Context::create_detached_renderer_consumer)
/// requires this atlas to be registered with exactly one context. After that claim succeeds,
/// attempts to register another context return
/// [`ImGuiError::SharedFontAtlasManaged`](crate::ImGuiError::SharedFontAtlasManaged).
/// Before that managed Context is dropped, its renderer must release its complete GPU texture map
/// and commit [`Context::prepare_renderer_texture_reset`](crate::Context::prepare_renderer_texture_reset).
/// Otherwise the atlas preserves its old native bindings and rejects later Context registration
/// with [`ImGuiError::SharedFontAtlasRendererReleasePending`](crate::ImGuiError::SharedFontAtlasRendererReleasePending).
/// After releasing the external renderer resources, drop and recreate such an atlas rather than
/// transferring its unproven renderer namespace to another Context.
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
            }))
        }
    }

    pub(crate) fn as_ptr(&self) -> *mut sys::ImFontAtlas {
        self.0.raw.as_ptr()
    }

    pub(crate) fn prepare_frame(&self, renderer_has_textures: bool) {
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
            }
        }
    }
}
