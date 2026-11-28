use std::ops::Deref;

use dear_imgui_rs::{Id, NativeWindowInstanceId, NativeWindowTarget};
use dear_imgui_sdl3::{Sdl3SurfaceWindowLease, Sdl3ViewportRendererAdapter};

use super::multi_viewport_runtime::WgpuViewportError;
use super::sdl3_raw_window_handle::Sdl3SurfaceTarget;

pub(super) struct ViewportSurface {
    // Declaration order keeps the SDL window and its owners alive through surface destruction.
    surface: wgpu::Surface<'static>,
    _native_window: Sdl3SurfaceWindowLease,
}

impl Deref for ViewportSurface {
    type Target = wgpu::Surface<'static>;

    fn deref(&self) -> &Self::Target {
        &self.surface
    }
}

/// Creates a surface retaining the exact validated SDL window and its native owner chain.
pub(super) fn create_surface(
    instance: &wgpu::Instance,
    platform: &Sdl3ViewportRendererAdapter,
    viewport: Id,
    expected: Option<NativeWindowInstanceId>,
) -> Result<(ViewportSurface, [u32; 2], NativeWindowTarget), WgpuViewportError> {
    let snapshot = match expected {
        Some(instance) => platform.window_snapshot_for_instance(instance)?,
        None => platform.window_snapshot_for_viewport(viewport)?,
    }
    .ok_or(WgpuViewportError::SurfaceOperationFailed {
        operation: "resolve the registered SDL3 viewport window",
    })?;
    let size = snapshot
        .physical_client_size
        .ok_or(WgpuViewportError::SurfaceOperationFailed {
            operation: "query SDL3 viewport framebuffer size",
        })?;
    let lease = platform
        .acquire_surface_window_lease(snapshot.target.instance_id)?
        .ok_or(WgpuViewportError::SurfaceOperationFailed {
            operation: "lease the registered SDL3 viewport window for its surface",
        })?;
    // SAFETY: the typed lease keeps the SDL window live on its owning thread.
    let target = unsafe { Sdl3SurfaceTarget::new(lease.as_ptr()) };
    // SAFETY: both handles come from the same leased SDL window and its display.
    let target = unsafe { wgpu::SurfaceTargetUnsafe::from_display_and_window(&target, &target) }
        .map_err(|_| WgpuViewportError::SurfaceOperationFailed {
            operation: "read SDL3 surface handles",
        })?;
    // SAFETY: ViewportSurface retains the lease until after the surface is destroyed.
    let surface = unsafe { instance.create_surface_unsafe(target) }.map_err(|_| {
        WgpuViewportError::SurfaceOperationFailed {
            operation: "create SDL3 viewport surface",
        }
    })?;
    Ok((
        ViewportSurface {
            surface,
            _native_window: lease,
        },
        size,
        snapshot.target,
    ))
}

pub(super) fn framebuffer_size(
    platform: &Sdl3ViewportRendererAdapter,
    instance: NativeWindowInstanceId,
) -> Result<[u32; 2], WgpuViewportError> {
    platform
        .window_snapshot_for_instance(instance)?
        .and_then(|snapshot| snapshot.physical_client_size)
        .ok_or(WgpuViewportError::SurfaceOperationFailed {
            operation: "resolve the registered SDL3 viewport framebuffer",
        })
}
