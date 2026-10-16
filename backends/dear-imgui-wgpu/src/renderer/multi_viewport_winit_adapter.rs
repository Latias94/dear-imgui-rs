use dear_imgui_rs::{NativeWindowInstanceId, NativeWindowTarget, platform_io::Viewport};

use super::multi_viewport_runtime::WgpuViewportError;

pub(super) use dear_imgui_winit::multi_viewport::WinitViewportRendererAdapter as PlatformOwner;

/// Creates a surface retaining the exact validated native window allocation.
pub(super) fn create_surface(
    instance: &wgpu::Instance,
    platform: &PlatformOwner,
    viewport: &Viewport,
    expected: Option<NativeWindowInstanceId>,
) -> Result<(wgpu::Surface<'static>, [u32; 2], NativeWindowTarget), WgpuViewportError> {
    let snapshot = match expected {
        Some(instance) => platform.window_snapshot_for_instance(instance)?,
        None => platform.window_snapshot_for_viewport(viewport.id())?,
    }
    .ok_or(WgpuViewportError::SurfaceOperationFailed {
        operation: "resolve the registered Winit viewport window",
    })?;
    let window = platform.window_lease(snapshot.target.instance_id)?.ok_or(
        WgpuViewportError::SurfaceOperationFailed {
            operation: "lease the registered Winit viewport window",
        },
    )?;
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
    platform: &PlatformOwner,
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

/// Captures metadata through the exact platform owner, without returning a native lease.
pub(super) fn target_for_viewport(
    platform: &PlatformOwner,
    viewport: dear_imgui_rs::Id,
) -> Result<Option<NativeWindowTarget>, WgpuViewportError> {
    Ok(platform
        .window_snapshot_for_viewport(viewport)?
        .map(|snapshot| snapshot.target))
}

pub(super) fn target_for_instance(
    platform: &PlatformOwner,
    instance: NativeWindowInstanceId,
) -> Result<Option<NativeWindowTarget>, WgpuViewportError> {
    Ok(platform
        .window_snapshot_for_instance(instance)?
        .map(|snapshot| snapshot.target))
}
