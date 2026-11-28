use std::cell::Cell;
use std::marker::PhantomData;
use std::ptr::NonNull;
use std::rc::Rc;

use crate::FontError;
use crate::context::binding::ContextThreadLease;
use crate::sys;

use super::state::{
    context_font_atlas, font_atlas_contains_font, font_atlas_state, forget_font_atlas_generation,
    try_font_atlas_state,
};

#[derive(Debug)]
struct SharedFontAtlasInner {
    raw: NonNull<sys::ImFontAtlas>,
    next_frame: Cell<i32>,
    // ImGui's allocator accesses GImGui even when this atlas has no registered contexts.
    _thread_lease: ContextThreadLease,
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
/// The atlas and all its contexts belong to one OS thread. An atlas kept alive between contexts
/// retains that thread's ownership of Dear ImGui through native atlas destruction. Drop all
/// clones before transferring Dear ImGui ownership to another thread.
///
/// Multiple contexts may share the atlas only while they use legacy renderer-managed texture
/// handling. Attaching a managed renderer with
/// [`Context::create_synchronous_renderer_consumer`](crate::Context::create_synchronous_renderer_consumer)
/// or [`Context::create_detached_renderer_consumer`](crate::Context::create_detached_renderer_consumer)
/// requires this atlas to be registered with exactly one context. After that claim succeeds,
/// attempts to register another context return
/// [`ContextError::SharedFontAtlasManaged`](crate::ContextError::SharedFontAtlasManaged).
/// To reuse the atlas across Contexts, release the renderer's complete GPU texture map and commit
/// [`Context::prepare_renderer_texture_reset`](crate::Context::prepare_renderer_texture_reset).
/// A renderer can instead mark its [`RendererResetSignal`](crate::render::RendererResetSignal),
/// drop its consumer, and drain outstanding epochs before Context teardown. A shared atlas with
/// unconfirmed renderer bindings rejects later Context registration with
/// [`ContextError::SharedFontAtlasRendererReleasePending`](crate::ContextError::SharedFontAtlasRendererReleasePending).
#[derive(Debug, Clone)]
pub struct SharedFontAtlas(Rc<SharedFontAtlasInner>);

impl SharedFontAtlas {
    /// Creates a shared font atlas on this OS thread.
    ///
    /// Returns [`FontError::Context`] if the thread lease cannot be acquired.
    pub fn try_create() -> Result<Self, FontError> {
        let thread_lease = ContextThreadLease::acquire()?;
        let raw = NonNull::new(unsafe { sys::ImFontAtlas_ImFontAtlas() }).ok_or(
            FontError::AllocationFailed {
                resource: "the shared font atlas",
            },
        )?;
        let atlas = Self(Rc::new(SharedFontAtlasInner {
            raw,
            next_frame: Cell::new(0),
            _thread_lease: thread_lease,
        }));
        font_atlas_state(raw.as_ptr());
        Ok(atlas)
    }

    /// Creates a shared font atlas, panicking if [`Self::try_create`] fails.
    pub fn create() -> Self {
        Self::try_create().expect("Failed to create shared Dear ImGui font atlas")
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

/// A persistent, atlas-validated font handle.
///
/// `FontId` can be stored in application style state and later passed to
/// [`Ui::push_font`](crate::Ui::push_font), but it is not just a raw `ImFont*`.
/// The handle records the originating atlas and atlas generation. Safe push
/// APIs validate that the handle still belongs to the current context's atlas
/// and has not been invalidated by [`crate::FontAtlas::clear`],
/// [`crate::FontAtlas::clear_fonts`], or [`crate::FontAtlas::remove_font`] before calling
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
    validate_font_id_for_context(id, unsafe { sys::igGetCurrentContext() }, caller)
}

pub(crate) fn validate_font_id_for_context(
    id: FontId,
    context: *mut sys::ImGuiContext,
    caller: &str,
) -> *mut sys::ImFont {
    let atlas = context_font_atlas(context, caller);
    validate_font_id_for_atlas(id, atlas, caller)
}

pub(crate) fn validate_font_id(id: FontId, caller: &str) -> *mut sys::ImFont {
    validate_font_id_for_atlas(id, id.atlas, caller)
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
    let state = try_font_atlas_state(atlas)
        .filter(|state| state.stamp == id.atlas_stamp)
        .unwrap_or_else(|| {
            panic!("{caller} received a FontId from a destroyed or reused font atlas")
        });
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
