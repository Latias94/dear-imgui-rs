use super::multi_viewport_runtime::WgpuViewportError;
use super::sdl3_raw_window_handle::Sdl3SurfaceTarget;
use dear_imgui_rs::platform_io::Viewport;

#[cfg(target_arch = "wasm32")]
compile_error!("`multi-viewport-sdl3` is not supported on wasm32 targets.");

/// Creates a surface whose native window lifetime is owned by the SDL3 platform backend.
///
/// # Safety
///
/// The viewport platform handle must contain the ID of a live SDL window. Dear ImGui must invoke
/// the renderer destroy callback before the SDL platform backend destroys that window.
pub(super) unsafe fn create_surface(
    instance: &wgpu::Instance,
    viewport: &Viewport,
) -> Result<(wgpu::Surface<'static>, [u32; 2]), WgpuViewportError> {
    let target = unsafe { target_from_viewport(viewport) }.ok_or(
        WgpuViewportError::SurfaceOperationFailed {
            operation: "read SDL3 viewport window handle",
        },
    )?;
    let size = window_size(&target).ok_or(WgpuViewportError::SurfaceOperationFailed {
        operation: "query SDL3 viewport framebuffer size",
    })?;
    #[cfg(any(feature = "wgpu-29", feature = "wgpu-30"))]
    let surface_target_result =
        unsafe { wgpu::SurfaceTargetUnsafe::from_display_and_window(&target, &target) };
    #[cfg(any(feature = "wgpu-27", feature = "wgpu-28"))]
    let surface_target_result = unsafe { wgpu::SurfaceTargetUnsafe::from_window(&target) };
    let surface_target =
        surface_target_result.map_err(|_| WgpuViewportError::SurfaceOperationFailed {
            operation: "read SDL3 surface handles",
        })?;
    let surface = unsafe { instance.create_surface_unsafe(surface_target) }.map_err(|_| {
        WgpuViewportError::SurfaceOperationFailed {
            operation: "create SDL3 viewport surface",
        }
    })?;
    Ok((surface, size))
}

pub(super) unsafe fn framebuffer_size(viewport: &Viewport) -> Result<[u32; 2], WgpuViewportError> {
    let target = unsafe { target_from_viewport(viewport) }.ok_or(
        WgpuViewportError::SurfaceOperationFailed {
            operation: "read SDL3 viewport window handle",
        },
    )?;
    window_size(&target).ok_or(WgpuViewportError::SurfaceOperationFailed {
        operation: "query SDL3 viewport framebuffer size",
    })
}

fn window_id_from_platform_handle(
    handle: *mut std::ffi::c_void,
) -> Option<sdl3_sys::video::SDL_WindowID> {
    if handle.is_null() {
        None
    } else {
        Some(sdl3_sys::video::SDL_WindowID(handle as usize as u32))
    }
}

unsafe fn target_from_viewport(viewport: &Viewport) -> Option<Sdl3SurfaceTarget> {
    let window_id = window_id_from_platform_handle(viewport.platform_handle())?;
    unsafe { Sdl3SurfaceTarget::from_window_id(window_id) }
}

fn window_size(target: &Sdl3SurfaceTarget) -> Option<[u32; 2]> {
    let mut width = 0;
    let mut height = 0;
    let ok = unsafe {
        sdl3_sys::video::SDL_GetWindowSizeInPixels(target.raw_window(), &mut width, &mut height)
    };
    ok.then(|| [physical_pixels(width), physical_pixels(height)])
}

fn physical_pixels(pixels: i32) -> u32 {
    pixels.max(0) as u32
}

#[cfg(test)]
mod tests {
    use super::{framebuffer_size, physical_pixels, window_id_from_platform_handle};
    use crate::renderer::multi_viewport_runtime::WgpuViewportError;

    #[test]
    fn rejects_null_window_id() {
        assert!(window_id_from_platform_handle(std::ptr::null_mut()).is_none());
    }

    #[test]
    fn preserves_nonzero_window_id() {
        let handle = 42_usize as *mut std::ffi::c_void;
        assert!(
            window_id_from_platform_handle(handle)
                .is_some_and(|window_id| window_id == sdl3_sys::video::SDL_WindowID(42))
        );
    }

    #[test]
    fn invalid_viewport_handle_is_a_typed_error() {
        let mut raw_viewport = dear_imgui_rs::sys::ImGuiViewport::default();
        let viewport =
            unsafe { dear_imgui_rs::platform_io::Viewport::from_raw_mut(&mut raw_viewport) };
        assert!(matches!(
            unsafe { framebuffer_size(viewport) },
            Err(WgpuViewportError::SurfaceOperationFailed {
                operation: "read SDL3 viewport window handle"
            })
        ));
    }

    #[test]
    fn preserves_zero_sized_sdl_framebuffers() {
        assert_eq!(physical_pixels(-5), 0);
        assert_eq!(physical_pixels(0), 0);
        assert_eq!(physical_pixels(17), 17);
    }
}
