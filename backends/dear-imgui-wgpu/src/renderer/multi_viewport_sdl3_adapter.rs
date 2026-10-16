use dear_imgui_rs::{NativeWindowInstanceId, NativeWindowTarget, platform_io::Viewport};

use super::multi_viewport_runtime::WgpuViewportError;
use super::sdl3_raw_window_handle::Sdl3SurfaceTarget;

pub(super) use dear_imgui_sdl3::Sdl3NativeWindowLease as NativeWindowLease;
pub(super) use dear_imgui_sdl3::Sdl3ViewportRendererAdapter as PlatformOwner;

/// Creates a surface only after the captured SDL owner validates the exact native registration.
///
/// # Safety
///
/// The renderer must destroy the returned surface before the SDL owner releases the native window.
/// Scoped lookup validates ownership at creation; it does not extend the SDL allocation lifetime.
pub(super) unsafe fn create_surface(
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
        operation: "resolve the registered SDL3 viewport window",
    })?;
    let size = snapshot
        .physical_client_size
        .ok_or(WgpuViewportError::SurfaceOperationFailed {
            operation: "query SDL3 viewport framebuffer size",
        })?;
    let lease = native_window_lease(platform, snapshot.target.instance_id)?.ok_or(
        WgpuViewportError::SurfaceOperationFailed {
            operation: "lease the registered SDL3 viewport window",
        },
    )?;
    // SAFETY: the exact allocation is pinned while handles are read and surface creation runs.
    // The baseline surface is destroyed in the renderer-before-platform phase. Any escaping
    // recorded work retains its own NativeWindowLease until frame and surface release.
    let target = unsafe { Sdl3SurfaceTarget::new(lease.as_ptr()) };
    #[cfg(any(feature = "wgpu-29", feature = "wgpu-30"))]
    let target_result =
        unsafe { wgpu::SurfaceTargetUnsafe::from_display_and_window(&target, &target) };
    #[cfg(any(feature = "wgpu-27", feature = "wgpu-28"))]
    let target_result = unsafe { wgpu::SurfaceTargetUnsafe::from_window(&target) };
    let target = target_result.map_err(|_| WgpuViewportError::SurfaceOperationFailed {
        operation: "read SDL3 surface handles",
    })?;
    let surface = unsafe { instance.create_surface_unsafe(target) }.map_err(|_| {
        WgpuViewportError::SurfaceOperationFailed {
            operation: "create SDL3 viewport surface",
        }
    })?;
    Ok((surface, size, snapshot.target))
}

pub(super) fn framebuffer_size(
    platform: &PlatformOwner,
    instance: NativeWindowInstanceId,
) -> Result<[u32; 2], WgpuViewportError> {
    platform
        .window_snapshot_for_instance(instance)?
        .and_then(|snapshot| snapshot.physical_client_size)
        .ok_or(WgpuViewportError::SurfaceOperationFailed {
            operation: "resolve the registered SDL3 viewport framebuffer",
        })
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

pub(super) fn native_window_lease(
    platform: &PlatformOwner,
    instance: NativeWindowInstanceId,
) -> Result<Option<NativeWindowLease>, WgpuViewportError> {
    platform
        .acquire_native_window_lease(instance)
        .map_err(Into::into)
}
