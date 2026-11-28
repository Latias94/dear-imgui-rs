use dear_imgui_rs::{Id, NativeWindowInstanceId, NativeWindowTarget};
use dear_imgui_winit::multi_viewport::WinitViewportRendererAdapter;

use super::multi_viewport_runtime::WgpuViewportError;

/// Creates a surface retaining the exact validated native window allocation.
pub(super) fn create_surface(
    instance: &wgpu::Instance,
    platform: &WinitViewportRendererAdapter,
    viewport: Id,
    expected: Option<NativeWindowInstanceId>,
) -> Result<(wgpu::Surface<'static>, [u32; 2], NativeWindowTarget), WgpuViewportError> {
    let snapshot = match expected {
        Some(instance) => platform.window_snapshot_for_instance(instance)?,
        None => platform.window_snapshot_for_viewport(viewport)?,
    }
    .ok_or(WgpuViewportError::SurfaceOperationFailed {
        operation: "resolve the registered Winit viewport window",
    })?;
    let lease = platform
        .acquire_native_window_lease(snapshot.target.instance_id)?
        .ok_or(WgpuViewportError::SurfaceOperationFailed {
            operation: "lease the registered Winit viewport window",
        })?;
    let window = lease.window_arc();
    let size = snapshot.physical_client_size;
    let surface =
        instance
            .create_surface(window)
            .map_err(|_| WgpuViewportError::SurfaceOperationFailed {
                operation: "create Winit viewport surface",
            })?;
    Ok((surface, [size.width, size.height], snapshot.target))
}

pub(super) fn framebuffer_size(
    platform: &WinitViewportRendererAdapter,
    instance: NativeWindowInstanceId,
) -> Result<[u32; 2], WgpuViewportError> {
    let snapshot = platform.window_snapshot_for_instance(instance)?.ok_or(
        WgpuViewportError::SurfaceOperationFailed {
            operation: "resolve the registered Winit viewport framebuffer",
        },
    )?;
    let size = snapshot.physical_client_size;
    Ok([size.width, size.height])
}
