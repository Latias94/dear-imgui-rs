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

#[cfg(test)]
mod tests {
    use super::framebuffer_size;

    #[test]
    fn rejects_null_platform_handle() {
        let mut raw_viewport = dear_imgui_rs::sys::ImGuiViewport::default();
        let viewport =
            unsafe { dear_imgui_rs::platform_io::Viewport::from_raw_mut(&mut raw_viewport) };
        assert!(unsafe { framebuffer_size(viewport) }.is_none());
    }
}
