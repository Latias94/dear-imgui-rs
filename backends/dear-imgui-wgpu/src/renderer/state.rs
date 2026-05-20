use crate::{RendererError, RendererResult};
use dear_imgui_rs::sys;

pub(super) struct RendererRenderStateGuard {
    platform_io: *mut sys::ImGuiPlatformIO,
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub(super) enum ActiveSampler {
    Linear,
    Nearest,
    Custom(u64),
}

impl RendererRenderStateGuard {
    pub(super) unsafe fn set(
        platform_io: *mut sys::ImGuiPlatformIO,
        render_state: *mut std::ffi::c_void,
    ) -> RendererResult<Self> {
        if platform_io.is_null() {
            return Err(RendererError::InvalidRenderState(
                "PlatformIO not available for renderer render state".to_string(),
            ));
        }

        unsafe {
            (*platform_io).Renderer_RenderState = render_state;
        }
        Ok(Self { platform_io })
    }
}

impl Drop for RendererRenderStateGuard {
    fn drop(&mut self) {
        unsafe {
            if !self.platform_io.is_null() {
                (*self.platform_io).Renderer_RenderState = std::ptr::null_mut();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renderer_render_state_guard_clears_on_drop() {
        unsafe {
            let platform_io = sys::ImGuiPlatformIO_ImGuiPlatformIO();
            assert!(!platform_io.is_null());

            let mut render_state = 7u8;
            {
                let _guard = RendererRenderStateGuard::set(
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
}
