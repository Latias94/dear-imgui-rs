//! Backend-only support for Dear ImGui's transient `Renderer_RenderState` slot.

use std::{ffi::c_void, marker::PhantomData, ptr::NonNull};

use crate::sys;

/// Failure while publishing or validating a renderer callback state slot.
///
/// This is backend support rather than an application-level callback API. Backends map these
/// cases into their public error types, which retain API-specific context.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RendererRenderStateGuardError {
    /// Dear ImGui did not provide a Platform IO record for the active Context.
    MissingPlatformIo,
    /// A renderer has already published transient callback state for this draw scope.
    AlreadyOccupied,
    /// A raw callback replaced the state published by the renderer.
    Drift,
}

/// RAII publication for one backend-owned, callback-scoped renderer state value.
///
/// The guard borrows its state for the whole installation interval, clears only the pointer it
/// installed on early return or unwind, and preserves a foreign replacement for the backend to
/// report as drift. It intentionally does not expose a safe application callback abstraction.
#[doc(hidden)]
pub struct RendererRenderStateGuard<'state, State> {
    platform_io: NonNull<sys::ImGuiPlatformIO>,
    expected: NonNull<State>,
    _state: PhantomData<&'state mut State>,
}

impl<'state, State> RendererRenderStateGuard<'state, State> {
    /// Checks that the callback-state slot is usable before renderer draw side effects begin.
    ///
    /// # Safety
    ///
    /// `platform_io` must point to a live `ImGuiPlatformIO` for the current Context. It must
    /// remain valid for the later guard installation and validation.
    pub unsafe fn preflight(
        platform_io: *mut sys::ImGuiPlatformIO,
    ) -> Result<(), RendererRenderStateGuardError> {
        let platform_io =
            NonNull::new(platform_io).ok_or(RendererRenderStateGuardError::MissingPlatformIo)?;
        if unsafe { platform_io.as_ref().Renderer_RenderState }.is_null() {
            Ok(())
        } else {
            Err(RendererRenderStateGuardError::AlreadyOccupied)
        }
    }

    /// Publishes `state` for raw callbacks until this guard is finished or dropped.
    ///
    /// # Safety
    ///
    /// `platform_io` must point to the live Platform IO for the renderer-owned current Context.
    /// No code may replace or free that record until the returned guard is finished or dropped.
    /// Raw callbacks may inspect the slot, but must not replace it.
    pub unsafe fn install(
        platform_io: *mut sys::ImGuiPlatformIO,
        state: &'state mut State,
    ) -> Result<Self, RendererRenderStateGuardError> {
        let platform_io =
            NonNull::new(platform_io).ok_or(RendererRenderStateGuardError::MissingPlatformIo)?;
        unsafe { Self::preflight(platform_io.as_ptr()) }?;
        let expected = NonNull::from(state);
        unsafe {
            (*platform_io.as_ptr()).Renderer_RenderState = expected.cast::<c_void>().as_ptr();
        }
        Ok(Self {
            platform_io,
            expected,
            _state: PhantomData,
        })
    }

    /// Confirms that no callback replaced the state owned by this renderer.
    pub fn validate(&self) -> Result<(), RendererRenderStateGuardError> {
        if unsafe { self.platform_io.as_ref().Renderer_RenderState }
            == self.expected.cast::<c_void>().as_ptr()
        {
            Ok(())
        } else {
            Err(RendererRenderStateGuardError::Drift)
        }
    }

    /// Mutably borrows the renderer state published by this guard.
    ///
    /// Renderers use this between raw callback invocations to keep transient state observations in
    /// sync with the commands they record. The guard's exclusive lifetime prevents another Rust
    /// owner from mutating the value concurrently.
    pub fn state_mut(&mut self) -> &mut State {
        unsafe { self.expected.as_mut() }
    }

    /// Clears the state slot after validating its ownership.
    pub fn finish(self) -> Result<(), RendererRenderStateGuardError> {
        let result = self.validate();
        if result.is_ok() {
            unsafe {
                (*self.platform_io.as_ptr()).Renderer_RenderState = std::ptr::null_mut();
            }
        }
        result
    }
}

impl<State> Drop for RendererRenderStateGuard<'_, State> {
    fn drop(&mut self) {
        if unsafe { self.platform_io.as_ref().Renderer_RenderState }
            == self.expected.cast::<c_void>().as_ptr()
        {
            unsafe {
                (*self.platform_io.as_ptr()).Renderer_RenderState = std::ptr::null_mut();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{RendererRenderStateGuard, RendererRenderStateGuardError};
    use crate::sys;

    #[test]
    fn guard_clears_its_own_state_on_drop() {
        unsafe {
            let platform_io = sys::ImGuiPlatformIO_ImGuiPlatformIO();
            let mut state = 7_u8;
            let expected = std::ptr::from_mut(&mut state).cast();
            {
                let _guard = RendererRenderStateGuard::install(platform_io, &mut state).unwrap();
                assert_eq!((*platform_io).Renderer_RenderState, expected);
            }
            assert!((*platform_io).Renderer_RenderState.is_null());
            sys::ImGuiPlatformIO_destroy(platform_io);
        }
    }

    #[test]
    fn guard_rejects_an_occupied_slot_without_mutation() {
        unsafe {
            let platform_io = sys::ImGuiPlatformIO_ImGuiPlatformIO();
            let mut foreign = 1_u8;
            let mut state = 2_u8;
            (*platform_io).Renderer_RenderState = std::ptr::from_mut(&mut foreign).cast();

            assert!(matches!(
                RendererRenderStateGuard::install(platform_io, &mut state),
                Err(RendererRenderStateGuardError::AlreadyOccupied)
            ));
            assert_eq!(
                (*platform_io).Renderer_RenderState,
                std::ptr::from_mut(&mut foreign).cast()
            );
            sys::ImGuiPlatformIO_destroy(platform_io);
        }
    }

    #[test]
    fn guard_preserves_a_foreign_replacement_and_reports_drift() {
        unsafe {
            let platform_io = sys::ImGuiPlatformIO_ImGuiPlatformIO();
            let mut state = 1_u8;
            let mut foreign = 2_u8;
            let guard = RendererRenderStateGuard::install(platform_io, &mut state).unwrap();
            (*platform_io).Renderer_RenderState = std::ptr::from_mut(&mut foreign).cast();

            assert_eq!(guard.finish(), Err(RendererRenderStateGuardError::Drift));
            assert_eq!(
                (*platform_io).Renderer_RenderState,
                std::ptr::from_mut(&mut foreign).cast()
            );
            (*platform_io).Renderer_RenderState = std::ptr::null_mut();
            sys::ImGuiPlatformIO_destroy(platform_io);
        }
    }

    #[test]
    fn guard_can_update_its_published_state_between_callbacks() {
        unsafe {
            let platform_io = sys::ImGuiPlatformIO_ImGuiPlatformIO();
            let mut state = 1_u8;
            let mut guard = RendererRenderStateGuard::install(platform_io, &mut state).unwrap();

            *guard.state_mut() = 7;
            assert_eq!(*((*platform_io).Renderer_RenderState.cast::<u8>()), 7);

            guard.finish().unwrap();
            assert_eq!(state, 7);
            sys::ImGuiPlatformIO_destroy(platform_io);
        }
    }

    #[test]
    fn guard_clears_during_unwind() {
        unsafe {
            let platform_io = sys::ImGuiPlatformIO_ImGuiPlatformIO();
            let mut state = 1_u8;
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _guard = RendererRenderStateGuard::install(platform_io, &mut state).unwrap();
                panic!("injected renderer unwind");
            }));
            assert!(result.is_err());
            assert!((*platform_io).Renderer_RenderState.is_null());
            sys::ImGuiPlatformIO_destroy(platform_io);
        }
    }
}
