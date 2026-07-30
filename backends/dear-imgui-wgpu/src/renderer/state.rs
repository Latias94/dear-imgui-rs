use std::{ffi::c_void, ptr::NonNull};

use crate::{RendererError, RendererResult};
use dear_imgui_rs::sys;

pub(super) struct RendererRenderStateGuard {
    platform_io: NonNull<sys::ImGuiPlatformIO>,
    expected: NonNull<c_void>,
    finished: bool,
}

impl RendererRenderStateGuard {
    pub(super) fn preflight(platform_io: *mut sys::ImGuiPlatformIO) -> RendererResult<()> {
        let platform_io = NonNull::new(platform_io).ok_or_else(|| {
            RendererError::InvalidRenderState(
                "PlatformIO not available for renderer render state".to_owned(),
            )
        })?;
        if unsafe { platform_io.as_ref().Renderer_RenderState }.is_null() {
            Ok(())
        } else {
            Err(RendererError::InvalidRenderState(
                "PlatformIO Renderer_RenderState is already occupied".to_owned(),
            ))
        }
    }

    pub(super) unsafe fn install(
        platform_io: *mut sys::ImGuiPlatformIO,
        render_state: *mut c_void,
    ) -> RendererResult<Self> {
        let platform_io = NonNull::new(platform_io).ok_or_else(|| {
            RendererError::InvalidRenderState(
                "PlatformIO not available for renderer render state".to_owned(),
            )
        })?;
        let expected = NonNull::new(render_state).ok_or_else(|| {
            RendererError::InvalidRenderState(
                "WGPU renderer state storage must not be null".to_owned(),
            )
        })?;
        Self::preflight(platform_io.as_ptr())?;

        unsafe {
            (*platform_io.as_ptr()).Renderer_RenderState = expected.as_ptr();
        }
        Ok(Self {
            platform_io,
            expected,
            finished: false,
        })
    }

    pub(super) fn validate(&self) -> RendererResult<()> {
        if unsafe { self.platform_io.as_ref().Renderer_RenderState } == self.expected.as_ptr() {
            Ok(())
        } else {
            Err(RendererError::RendererStateDrift {
                field: "Renderer_RenderState",
            })
        }
    }

    pub(super) fn finish(mut self) -> RendererResult<()> {
        let result = self.validate();
        if result.is_ok() {
            unsafe {
                (*self.platform_io.as_ptr()).Renderer_RenderState = std::ptr::null_mut();
            }
        }
        self.finished = true;
        result
    }
}

impl Drop for RendererRenderStateGuard {
    fn drop(&mut self) {
        if !self.finished
            && unsafe { self.platform_io.as_ref().Renderer_RenderState } == self.expected.as_ptr()
        {
            unsafe {
                (*self.platform_io.as_ptr()).Renderer_RenderState = std::ptr::null_mut();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renderer_render_state_guard_clears_its_own_state_on_drop() {
        unsafe {
            let platform_io = sys::ImGuiPlatformIO_ImGuiPlatformIO();
            assert!(!platform_io.is_null());

            let mut render_state = 7u8;
            {
                let _guard = RendererRenderStateGuard::install(
                    platform_io,
                    (&mut render_state as *mut u8).cast(),
                )
                .expect("render state guard should set a valid PlatformIO");
                assert_eq!(
                    (*platform_io).Renderer_RenderState,
                    (&mut render_state as *mut u8).cast()
                );
            }

            assert!((*platform_io).Renderer_RenderState.is_null());
            sys::ImGuiPlatformIO_destroy(platform_io);
        }
    }

    #[test]
    fn renderer_render_state_guard_rejects_an_occupied_slot() {
        unsafe {
            let platform_io = sys::ImGuiPlatformIO_ImGuiPlatformIO();
            let mut foreign = 1u8;
            let mut ours = 2u8;
            (*platform_io).Renderer_RenderState = (&mut foreign as *mut u8).cast();

            let result =
                RendererRenderStateGuard::install(platform_io, (&mut ours as *mut u8).cast());
            assert!(matches!(result, Err(RendererError::InvalidRenderState(_))));
            assert_eq!(
                (*platform_io).Renderer_RenderState,
                (&mut foreign as *mut u8).cast()
            );
            sys::ImGuiPlatformIO_destroy(platform_io);
        }
    }

    #[test]
    fn renderer_render_state_guard_preserves_foreign_replacement_and_reports_drift() {
        unsafe {
            let platform_io = sys::ImGuiPlatformIO_ImGuiPlatformIO();
            let mut ours = 1u8;
            let mut foreign = 2u8;
            let guard =
                RendererRenderStateGuard::install(platform_io, (&mut ours as *mut u8).cast())
                    .unwrap();
            (*platform_io).Renderer_RenderState = (&mut foreign as *mut u8).cast();

            assert!(matches!(
                guard.finish(),
                Err(RendererError::RendererStateDrift {
                    field: "Renderer_RenderState"
                })
            ));
            assert_eq!(
                (*platform_io).Renderer_RenderState,
                (&mut foreign as *mut u8).cast()
            );
            (*platform_io).Renderer_RenderState = std::ptr::null_mut();
            sys::ImGuiPlatformIO_destroy(platform_io);
        }
    }

    #[test]
    fn renderer_render_state_guard_clears_during_unwind() {
        unsafe {
            let platform_io = sys::ImGuiPlatformIO_ImGuiPlatformIO();
            let mut ours = 1u8;
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _guard =
                    RendererRenderStateGuard::install(platform_io, (&mut ours as *mut u8).cast())
                        .unwrap();
                panic!("injected renderer unwind");
            }));
            assert!(result.is_err());
            assert!((*platform_io).Renderer_RenderState.is_null());
            sys::ImGuiPlatformIO_destroy(platform_io);
        }
    }

    #[test]
    fn renderer_render_state_guard_clears_on_early_error() {
        unsafe {
            let platform_io = sys::ImGuiPlatformIO_ImGuiPlatformIO();
            let mut ours = 1u8;
            let result: RendererResult<()> = (|| {
                let _guard =
                    RendererRenderStateGuard::install(platform_io, (&mut ours as *mut u8).cast())?;
                Err(RendererError::Generic("injected render error".to_owned()))
            })();
            assert!(matches!(result, Err(RendererError::Generic(_))));
            assert!((*platform_io).Renderer_RenderState.is_null());
            sys::ImGuiPlatformIO_destroy(platform_io);
        }
    }
}
