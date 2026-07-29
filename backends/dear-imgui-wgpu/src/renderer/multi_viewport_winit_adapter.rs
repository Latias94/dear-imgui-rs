use dear_imgui_rs::platform_io::Viewport;

use super::multi_viewport_runtime::WgpuViewportError;

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
) -> Result<(wgpu::Surface<'static>, [u32; 2]), WgpuViewportError> {
    let window_ptr = viewport.platform_handle();
    if window_ptr.is_null() {
        return Err(WgpuViewportError::SurfaceOperationFailed {
            operation: "read Winit viewport window handle",
        });
    }

    let window = unsafe { &*(window_ptr.cast::<winit::window::Window>()) };
    #[cfg(any(feature = "wgpu-29", feature = "wgpu-30"))]
    let target_result =
        unsafe { wgpu::SurfaceTargetUnsafe::from_display_and_window(window, window) };
    #[cfg(any(feature = "wgpu-27", feature = "wgpu-28"))]
    let target_result = unsafe { wgpu::SurfaceTargetUnsafe::from_window(window) };
    let target = target_result.map_err(|_| WgpuViewportError::SurfaceOperationFailed {
        operation: "read Winit surface handles",
    })?;
    let surface = unsafe { instance.create_surface_unsafe(target) }.map_err(|_| {
        WgpuViewportError::SurfaceOperationFailed {
            operation: "create Winit viewport surface",
        }
    })?;
    let size = window.inner_size();
    Ok((surface, [size.width, size.height]))
}

#[cfg(target_arch = "wasm32")]
pub(super) unsafe fn create_surface(
    _instance: &wgpu::Instance,
    _viewport: &Viewport,
) -> Result<(wgpu::Surface<'static>, [u32; 2]), WgpuViewportError> {
    Err(WgpuViewportError::UnsupportedTarget)
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) unsafe fn framebuffer_size(viewport: &Viewport) -> Result<[u32; 2], WgpuViewportError> {
    let window_ptr = viewport.platform_handle();
    if window_ptr.is_null() {
        return Err(WgpuViewportError::SurfaceOperationFailed {
            operation: "read Winit viewport window handle",
        });
    }
    let window = unsafe { &*(window_ptr.cast::<winit::window::Window>()) };
    let size = window.inner_size();
    Ok([size.width, size.height])
}

#[cfg(target_arch = "wasm32")]
pub(super) unsafe fn framebuffer_size(_viewport: &Viewport) -> Result<[u32; 2], WgpuViewportError> {
    Err(WgpuViewportError::UnsupportedTarget)
}

#[cfg(test)]
mod tests {
    use super::framebuffer_size;

    #[test]
    fn rejects_null_platform_handle() {
        let mut raw_viewport = dear_imgui_rs::sys::ImGuiViewport::default();
        let viewport =
            unsafe { dear_imgui_rs::platform_io::Viewport::from_raw_mut(&mut raw_viewport) };
        assert!(unsafe { framebuffer_size(viewport) }.is_err());
    }
}
