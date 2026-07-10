use dear_imgui_rs::platform_io::Viewport;

/// Creates a surface whose native window lifetime is owned by the Winit platform backend.
///
/// # Safety
///
/// The viewport platform handle must point to the live `winit::Window` associated with `viewport`.
/// Dear ImGui must invoke the renderer destroy callback before the platform destroys that window.
#[cfg(not(target_arch = "wasm32"))]
pub(super) unsafe fn create_surface(
    instance: &wgpu::Instance,
    viewport: &Viewport,
) -> Option<(wgpu::Surface<'static>, [u32; 2])> {
    let window_ptr = viewport.platform_handle();
    if window_ptr.is_null() {
        return None;
    }

    let window = unsafe { &*(window_ptr.cast::<winit::window::Window>()) };
    #[cfg(any(feature = "wgpu-29", feature = "wgpu-30"))]
    let target_result =
        unsafe { wgpu::SurfaceTargetUnsafe::from_display_and_window(window, window) };
    #[cfg(any(feature = "wgpu-27", feature = "wgpu-28"))]
    let target_result = unsafe { wgpu::SurfaceTargetUnsafe::from_window(window) };
    let target = match target_result {
        Ok(target) => target,
        Err(error) => {
            eprintln!("[wgpu-mv] could not read Winit surface handles: {error:?}");
            return None;
        }
    };
    let surface = match unsafe { instance.create_surface_unsafe(target) } {
        Ok(surface) => surface,
        Err(error) => {
            eprintln!("[wgpu-mv] could not create Winit surface: {error:?}");
            return None;
        }
    };
    let size = window.inner_size();
    Some((surface, [size.width.max(1), size.height.max(1)]))
}

#[cfg(target_arch = "wasm32")]
pub(super) unsafe fn create_surface(
    _instance: &wgpu::Instance,
    _viewport: &Viewport,
) -> Option<(wgpu::Surface<'static>, [u32; 2])> {
    None
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) unsafe fn framebuffer_size(viewport: &Viewport) -> Option<[u32; 2]> {
    let window_ptr = viewport.platform_handle();
    if window_ptr.is_null() {
        return None;
    }
    let window = unsafe { &*(window_ptr.cast::<winit::window::Window>()) };
    let size = window.inner_size();
    Some([size.width.max(1), size.height.max(1)])
}

#[cfg(target_arch = "wasm32")]
pub(super) unsafe fn framebuffer_size(_viewport: &Viewport) -> Option<[u32; 2]> {
    None
}

pub(super) fn logical_size_to_framebuffer(size: [f32; 2], scale: [f32; 2]) -> [u32; 2] {
    let scale_x = valid_scale(scale[0]);
    let scale_y = valid_scale(scale[1]);
    [
        physical_dimension(size[0], scale_x),
        physical_dimension(size[1], scale_y),
    ]
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

#[cfg(test)]
mod tests {
    use super::{framebuffer_size, logical_size_to_framebuffer};

    #[test]
    fn rejects_null_platform_handle() {
        let mut raw_viewport = dear_imgui_rs::sys::ImGuiViewport::default();
        let viewport =
            unsafe { dear_imgui_rs::platform_io::Viewport::from_raw_mut(&mut raw_viewport) };
        assert!(unsafe { framebuffer_size(viewport) }.is_none());
    }

    #[test]
    fn converts_logical_size_with_framebuffer_scale() {
        assert_eq!(
            logical_size_to_framebuffer([320.0, 200.0], [1.5, 2.0]),
            [480, 400]
        );
    }

    #[test]
    fn clamps_invalid_dimensions_and_scale() {
        assert_eq!(
            logical_size_to_framebuffer([0.0, f32::NAN], [0.0, f32::INFINITY]),
            [1, 1]
        );
    }
}
