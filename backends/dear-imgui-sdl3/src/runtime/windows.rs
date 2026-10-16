use std::marker::PhantomData;

use dear_imgui_rs::{Id, NativeWindowInstanceId, NativeWindowTarget};
use sdl3_sys::video::{
    SDL_GetWindowFromID, SDL_GetWindowPosition, SDL_GetWindowSizeInPixels, SDL_Window,
};

use super::*;

/// Owned metadata for one live SDL3 window registration at observation time.
///
/// Keeping this value does not retain the window or grant native access. SDL may reuse
/// `window_id` after destruction; use `target.instance_id` to identify historical registrations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Sdl3WindowSnapshot {
    /// The exact native registration and its current Dear ImGui routing association.
    pub target: NativeWindowTarget,
    /// SDL's native window identifier at observation time.
    pub window_id: u32,
    /// Client area size in framebuffer pixels, when SDL can report it.
    pub physical_client_size: Option<[u32; 2]>,
    /// SDL desktop coordinates, when supported by the active video driver.
    ///
    /// These are SDL window coordinates, which need not be physical pixels on high-DPI systems.
    pub desktop_position: Option<[i32; 2]>,
}

/// Scoped, non-owning access to an exact SDL3 window registration.
///
/// This type cannot be cloned into an owning [`sdl3::video::Window`]. It exposes a raw pointer
/// solely for native FFI integration; using that pointer remains subject to SDL's unsafe lifetime
/// and thread requirements. In particular, this scope is not a lease for a retained GPU surface.
///
/// The scoped wrapper cannot escape the callback:
///
/// ```compile_fail
/// use dear_imgui_rs::NativeWindowInstanceId;
/// use dear_imgui_sdl3::{Sdl3PlatformBackend, Sdl3WindowRef};
///
/// fn retain(backend: &Sdl3PlatformBackend, instance: NativeWindowInstanceId) -> Sdl3WindowRef<'static> {
///     backend.with_window(instance, |window| window).unwrap().unwrap()
/// }
/// ```
pub struct Sdl3WindowRef<'window> {
    snapshot: Sdl3WindowSnapshot,
    pointer: *mut SDL_Window,
    _owner: PhantomData<&'window RuntimeControl>,
}

impl fmt::Debug for Sdl3WindowRef<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Sdl3WindowRef")
            .field("snapshot", &self.snapshot)
            .finish_non_exhaustive()
    }
}

impl Sdl3WindowRef<'_> {
    /// Returns the owned metadata captured immediately before entering the callback.
    pub fn snapshot(&self) -> Sdl3WindowSnapshot {
        self.snapshot
    }

    /// Returns the borrowed SDL pointer for native FFI calls.
    ///
    /// Do not destroy the window, transfer it into an owning wrapper, or retain the pointer beyond
    /// the callback without independently preserving SDL's native lifetime contract. Reentrant
    /// backend teardown can invalidate it before this callback returns. Dereferencing or passing
    /// the pointer to native APIs remains unsafe.
    pub fn as_ptr(&self) -> *mut SDL_Window {
        self.pointer
    }
}

impl RuntimeControl {
    fn with_window_query<R>(&self, callback: impl FnOnce() -> R) -> Result<R, Sdl3BackendError> {
        // Inspection must not consume callback faults or request closes as a side effect of entry.
        // Exact owner validation may add a newly observed drift fault to the existing queue.
        if self.state.get() != RuntimeState::Attached
            || !self.platform_initialized.get()
            || self.platform_session.borrow().is_none()
        {
            return Err(Sdl3BackendError::RuntimeDetached);
        }
        self.binding.try_with_bound_context(|| {
            if !self.validate_platform_ownership_bound() {
                return Err(Sdl3BackendError::RuntimeDetached);
            }
            Ok(callback())
        })?
    }

    fn window_registration_bound(
        &self,
        address: usize,
    ) -> Option<(Sdl3WindowSnapshot, *mut SDL_Window)> {
        let viewport = address as *mut sys::ImGuiViewport;
        // The complete Context registry includes hidden and transitional viewports. Never
        // dereference a historical address until the native registry has proved it is live.
        let key = self.viewport_key(viewport)?;
        let actual = unsafe { ViewportPlatformState::capture(viewport) };
        let mut registry = self.owned_viewports.borrow_mut();
        let lease = registry.get_mut(&address)?;
        if !lease.key.same_identity(key) || lease.state != actual {
            return None;
        }
        lease.key.id = key.id;
        let window_id = actual.sdl_window_id()?;
        let pointer = unsafe { SDL_GetWindowFromID(sdl3_sys::video::SDL_WindowID(window_id)) };
        if pointer.is_null() || pointer as usize != lease.window_address {
            return None;
        }
        let target = NativeWindowTarget {
            instance_id: lease.instance_id,
            viewport_id: Id::from(key.id),
            is_main: viewport == unsafe { sys::igGetMainViewport() },
        };
        let mut width = 0;
        let mut height = 0;
        let physical_client_size =
            unsafe { SDL_GetWindowSizeInPixels(pointer, &mut width, &mut height) }
                .then(|| Some([u32::try_from(width).ok()?, u32::try_from(height).ok()?]))
                .flatten();
        let mut x = 0;
        let mut y = 0;
        let desktop_position =
            unsafe { SDL_GetWindowPosition(pointer, &mut x, &mut y) }.then_some([x, y]);
        Some((
            Sdl3WindowSnapshot {
                target,
                window_id,
                physical_client_size,
                desktop_position,
            },
            pointer,
        ))
    }

    pub(crate) fn window_snapshots(&self) -> Result<Vec<Sdl3WindowSnapshot>, Sdl3BackendError> {
        self.with_window_query(|| {
            let addresses: Vec<_> = self.owned_viewports.borrow().keys().copied().collect();
            let mut snapshots: Vec<_> = addresses
                .into_iter()
                .filter_map(|address| {
                    self.window_registration_bound(address)
                        .map(|(snapshot, _)| snapshot)
                })
                .collect();
            snapshots.sort_unstable_by_key(|snapshot| snapshot.target.instance_id.get());
            snapshots
        })
    }

    pub(crate) fn window_snapshot(
        &self,
        window_id: u32,
    ) -> Result<Option<Sdl3WindowSnapshot>, Sdl3BackendError> {
        Ok(self
            .window_snapshots()?
            .into_iter()
            .find(|snapshot| snapshot.window_id == window_id))
    }

    pub(crate) fn window_snapshot_for_instance(
        &self,
        instance_id: NativeWindowInstanceId,
    ) -> Result<Option<Sdl3WindowSnapshot>, Sdl3BackendError> {
        Ok(self
            .window_snapshots()?
            .into_iter()
            .find(|snapshot| snapshot.target.instance_id == instance_id))
    }

    pub(crate) fn window_snapshot_for_viewport(
        &self,
        viewport_id: Id,
    ) -> Result<Option<Sdl3WindowSnapshot>, Sdl3BackendError> {
        Ok(self
            .window_snapshots()?
            .into_iter()
            .find(|snapshot| snapshot.target.viewport_id == viewport_id))
    }

    pub(crate) fn with_window<R>(
        &self,
        instance_id: NativeWindowInstanceId,
        callback: impl for<'window> FnOnce(Sdl3WindowRef<'window>) -> R,
    ) -> Result<Option<R>, Sdl3BackendError> {
        self.with_window_query(|| {
            let address = self
                .owned_viewports
                .borrow()
                .iter()
                .find_map(|(address, lease)| {
                    (lease.instance_id == instance_id).then_some(*address)
                })?;
            let (snapshot, pointer) = self.window_registration_bound(address)?;
            Some(callback(Sdl3WindowRef {
                snapshot,
                pointer,
                _owner: PhantomData,
            }))
        })
    }
}

impl Sdl3ViewportRendererAdapter {
    /// Returns detached metadata for every currently owned live native window.
    pub fn window_snapshots(&self) -> Result<Vec<Sdl3WindowSnapshot>, Sdl3BackendError> {
        self.control.window_snapshots()
    }

    /// Resolves an SDL window identifier against this exact platform owner.
    pub fn window_snapshot(
        &self,
        window_id: u32,
    ) -> Result<Option<Sdl3WindowSnapshot>, Sdl3BackendError> {
        self.control.window_snapshot(window_id)
    }

    /// Resolves an exact native registration without following a reused SDL window identifier.
    pub fn window_snapshot_for_instance(
        &self,
        instance_id: NativeWindowInstanceId,
    ) -> Result<Option<Sdl3WindowSnapshot>, Sdl3BackendError> {
        self.control.window_snapshot_for_instance(instance_id)
    }

    /// Resolves the current native registration associated with a Dear ImGui viewport.
    pub fn window_snapshot_for_viewport(
        &self,
        viewport_id: Id,
    ) -> Result<Option<Sdl3WindowSnapshot>, Sdl3BackendError> {
        self.control.window_snapshot_for_viewport(viewport_id)
    }

    /// Visits one exact live window without creating an additional native owner.
    pub fn with_window<R>(
        &self,
        instance_id: NativeWindowInstanceId,
        callback: impl for<'window> FnOnce(Sdl3WindowRef<'window>) -> R,
    ) -> Result<Option<R>, Sdl3BackendError> {
        self.control.with_window(instance_id, callback)
    }
}
