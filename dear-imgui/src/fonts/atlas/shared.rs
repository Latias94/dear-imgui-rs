use std::cell::Cell;
use std::ptr::NonNull;
use std::rc::Rc;

use crate::context::binding::ContextThreadLease;
use crate::error::{ImGuiError, ImGuiResult};
use crate::sys;

use super::state::{font_atlas_state, forget_font_atlas_generation};

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
/// [`ImGuiError::SharedFontAtlasManaged`](crate::ImGuiError::SharedFontAtlasManaged).
/// To reuse the atlas across Contexts, release the renderer's complete GPU texture map and commit
/// [`Context::prepare_renderer_texture_reset`](crate::Context::prepare_renderer_texture_reset).
/// A renderer can instead mark its [`RendererResetSignal`](crate::render::RendererResetSignal),
/// drop its consumer, and drain outstanding epochs before Context teardown. A shared atlas with
/// unconfirmed renderer bindings rejects later Context registration with
/// [`ImGuiError::SharedFontAtlasRendererReleasePending`](crate::ImGuiError::SharedFontAtlasRendererReleasePending).
#[derive(Debug, Clone)]
pub struct SharedFontAtlas(Rc<SharedFontAtlasInner>);

impl SharedFontAtlas {
    /// Creates a shared font atlas on this OS thread.
    ///
    /// Returns [`ImGuiError::ContextThreadConflict`] if another thread owns Dear ImGui.
    pub fn try_create() -> ImGuiResult<Self> {
        let thread_lease = ContextThreadLease::acquire()?;
        let raw = NonNull::new(unsafe { sys::ImFontAtlas_ImFontAtlas() })
            .ok_or_else(|| ImGuiError::resource_allocation("shared font atlas"))?;
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
