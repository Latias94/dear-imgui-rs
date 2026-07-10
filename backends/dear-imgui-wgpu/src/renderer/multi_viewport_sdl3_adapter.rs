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
) -> Option<(wgpu::Surface<'static>, [u32; 2])> {
    let target = unsafe { target_from_viewport(viewport) }?;
    let size = window_size(&target)?;
    #[cfg(any(feature = "wgpu-29", feature = "wgpu-30"))]
    let surface_target_result =
        unsafe { wgpu::SurfaceTargetUnsafe::from_display_and_window(&target, &target) };
    #[cfg(any(feature = "wgpu-27", feature = "wgpu-28"))]
    let surface_target_result = unsafe { wgpu::SurfaceTargetUnsafe::from_window(&target) };
    let surface_target = match surface_target_result {
        Ok(target) => target,
        Err(error) => {
            eprintln!("[wgpu-mv] could not read SDL3 surface handles: {error:?}");
            return None;
        }
    };
    let surface = match unsafe { instance.create_surface_unsafe(surface_target) } {
        Ok(surface) => surface,
        Err(error) => {
            eprintln!("[wgpu-mv] could not create SDL3 surface: {error:?}");
            return None;
        }
    };
    Some((surface, size))
}

pub(super) unsafe fn framebuffer_size(viewport: &Viewport) -> Option<[u32; 2]> {
    let target = unsafe { target_from_viewport(viewport) }?;
    window_size(&target)
}

pub(super) fn logical_size_to_framebuffer(size: [f32; 2], scale: [f32; 2]) -> [u32; 2] {
    let scale_x = valid_scale(scale[0]);
    let scale_y = valid_scale(scale[1]);
    [
        physical_dimension(size[0], scale_x),
        physical_dimension(size[1], scale_y),
    ]
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
    ok.then(|| [clamp_pixels(width), clamp_pixels(height)])
}

fn valid_scale(scale: f32) -> f32 {
    if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    }
}

fn physical_dimension(logical: f32, scale: f32) -> u32 {
    if !logical.is_finite() {
        return 1;
    }
    (logical * scale).max(1.0).round().min(u32::MAX as f32) as u32
}

fn clamp_pixels(pixels: i32) -> u32 {
    pixels.max(1) as u32
}

#[cfg(test)]
mod tests {
    use super::{clamp_pixels, logical_size_to_framebuffer, window_id_from_platform_handle};

    #[test]
    fn rejects_null_window_id() {
        assert!(window_id_from_platform_handle(std::ptr::null_mut()).is_none());
    }

    #[test]
    fn preserves_nonzero_window_id() {
        let handle = 42_usize as *mut std::ffi::c_void;
        let window_id = window_id_from_platform_handle(handle).unwrap();
        assert!(window_id == sdl3_sys::video::SDL_WindowID(42));
    }

    #[test]
    fn clamps_sdl_pixel_dimensions() {
        assert_eq!(clamp_pixels(-5), 1);
        assert_eq!(clamp_pixels(0), 1);
        assert_eq!(clamp_pixels(17), 17);
    }

    #[test]
    fn converts_logical_size_with_framebuffer_scale() {
        assert_eq!(
            logical_size_to_framebuffer([320.0, 200.0], [1.5, 2.0]),
            [480, 400]
        );
        assert_eq!(
            logical_size_to_framebuffer([0.0, f32::NAN], [0.0, f32::INFINITY]),
            [1, 1]
        );
    }
}
