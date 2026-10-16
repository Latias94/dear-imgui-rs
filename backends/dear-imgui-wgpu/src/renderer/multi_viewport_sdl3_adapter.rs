use dear_imgui_rs::{NativeWindowInstanceId, NativeWindowTarget, platform_io::Viewport};

use super::multi_viewport_runtime::WgpuViewportError;
use super::sdl3_raw_window_handle::Sdl3SurfaceTarget;

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
    let surface = platform
        .with_window(snapshot.target.instance_id, |window| {
            // SAFETY: the scoped owner lookup validated this exact live SDL allocation. The renderer
            // callback teardown contract keeps it alive until the returned surface is destroyed.
            let target = unsafe { Sdl3SurfaceTarget::new(window.as_ptr()) };
            #[cfg(any(feature = "wgpu-29", feature = "wgpu-30"))]
            let target_result =
                unsafe { wgpu::SurfaceTargetUnsafe::from_display_and_window(&target, &target) };
            #[cfg(any(feature = "wgpu-27", feature = "wgpu-28"))]
            let target_result = unsafe { wgpu::SurfaceTargetUnsafe::from_window(&target) };
            let target = target_result.map_err(|_| WgpuViewportError::SurfaceOperationFailed {
                operation: "read SDL3 surface handles",
            })?;
            unsafe { instance.create_surface_unsafe(target) }.map_err(|_| {
                WgpuViewportError::SurfaceOperationFailed {
                    operation: "create SDL3 viewport surface",
                }
            })
        })?
        .ok_or(WgpuViewportError::SurfaceOperationFailed {
            operation: "borrow the registered SDL3 viewport window",
        })??;
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
