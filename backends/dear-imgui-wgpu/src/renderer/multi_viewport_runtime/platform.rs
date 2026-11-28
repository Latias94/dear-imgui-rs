//! Native window access through the exact platform generation captured at attachment.

use std::ops::Deref;

use dear_imgui_rs::{Id, NativeWindowInstanceId, NativeWindowTarget};
#[cfg(feature = "multi-viewport-sdl3")]
use dear_imgui_sdl3::{Sdl3NativeWindowLease, Sdl3ViewportRendererAdapter};
#[cfg(feature = "multi-viewport-winit")]
use dear_imgui_winit::{WinitWindowRenderLease, multi_viewport::WinitViewportRendererAdapter};

#[cfg(feature = "multi-viewport-sdl3")]
use crate::renderer::multi_viewport_sdl3_adapter as sdl3;
#[cfg(feature = "multi-viewport-winit")]
use crate::renderer::multi_viewport_winit_adapter as winit;

use super::runtime::WgpuViewportError;

/// The owning route selects the adapter when it attaches to its Context.
#[derive(Clone)]
pub(crate) enum ViewportPlatform {
    #[cfg(feature = "multi-viewport-winit")]
    Winit(WinitViewportRendererAdapter),
    #[cfg(feature = "multi-viewport-sdl3")]
    Sdl3(Sdl3ViewportRendererAdapter),
}

impl ViewportPlatform {
    pub(super) fn create_surface(
        &self,
        instance: &wgpu::Instance,
        viewport: Id,
        expected: Option<NativeWindowInstanceId>,
    ) -> Result<(ViewportSurface, [u32; 2], NativeWindowTarget), WgpuViewportError> {
        match self {
            #[cfg(feature = "multi-viewport-winit")]
            Self::Winit(owner) => {
                let (surface, size, target) =
                    winit::create_surface(instance, owner, viewport, expected)?;
                Ok((ViewportSurface::Winit(surface), size, target))
            }
            #[cfg(feature = "multi-viewport-sdl3")]
            Self::Sdl3(owner) => {
                let (surface, size, target) =
                    sdl3::create_surface(instance, owner, viewport, expected)?;
                Ok((ViewportSurface::Sdl3(surface), size, target))
            }
        }
    }

    pub(super) fn framebuffer_size(
        &self,
        instance: NativeWindowInstanceId,
    ) -> Result<[u32; 2], WgpuViewportError> {
        match self {
            #[cfg(feature = "multi-viewport-winit")]
            Self::Winit(owner) => winit::framebuffer_size(owner, instance),
            #[cfg(feature = "multi-viewport-sdl3")]
            Self::Sdl3(owner) => sdl3::framebuffer_size(owner, instance),
        }
    }

    /// Captures metadata through the exact platform owner without returning a native lease.
    pub(super) fn target_for_viewport(
        &self,
        viewport: Id,
    ) -> Result<Option<NativeWindowTarget>, WgpuViewportError> {
        match self {
            #[cfg(feature = "multi-viewport-winit")]
            Self::Winit(owner) => Ok(owner
                .window_snapshot_for_viewport(viewport)?
                .map(|snapshot| snapshot.target)),
            #[cfg(feature = "multi-viewport-sdl3")]
            Self::Sdl3(owner) => Ok(owner
                .window_snapshot_for_viewport(viewport)?
                .map(|snapshot| snapshot.target)),
        }
    }

    pub(super) fn target_for_instance(
        &self,
        instance: NativeWindowInstanceId,
    ) -> Result<Option<NativeWindowTarget>, WgpuViewportError> {
        match self {
            #[cfg(feature = "multi-viewport-winit")]
            Self::Winit(owner) => Ok(owner
                .window_snapshot_for_instance(instance)?
                .map(|snapshot| snapshot.target)),
            #[cfg(feature = "multi-viewport-sdl3")]
            Self::Sdl3(owner) => Ok(owner
                .window_snapshot_for_instance(instance)?
                .map(|snapshot| snapshot.target)),
        }
    }

    pub(super) fn native_window_lease(
        &self,
        instance: NativeWindowInstanceId,
    ) -> Result<Option<NativeWindowLease>, WgpuViewportError> {
        match self {
            #[cfg(feature = "multi-viewport-winit")]
            Self::Winit(owner) => Ok(owner
                .acquire_native_window_lease(instance)?
                .map(NativeWindowLease::Winit)),
            #[cfg(feature = "multi-viewport-sdl3")]
            Self::Sdl3(owner) => Ok(owner
                .acquire_native_window_lease(instance)?
                .map(NativeWindowLease::Sdl3)),
        }
    }
}

pub(super) enum ViewportSurface {
    #[cfg(feature = "multi-viewport-winit")]
    Winit(wgpu::Surface<'static>),
    #[cfg(feature = "multi-viewport-sdl3")]
    Sdl3(sdl3::ViewportSurface),
}

impl Deref for ViewportSurface {
    type Target = wgpu::Surface<'static>;

    fn deref(&self) -> &Self::Target {
        match self {
            #[cfg(feature = "multi-viewport-winit")]
            Self::Winit(surface) => surface,
            #[cfg(feature = "multi-viewport-sdl3")]
            Self::Sdl3(surface) => surface,
        }
    }
}

pub(super) enum NativeWindowLease {
    #[cfg(feature = "multi-viewport-winit")]
    Winit(WinitWindowRenderLease),
    #[cfg(feature = "multi-viewport-sdl3")]
    Sdl3(Sdl3NativeWindowLease),
}

impl NativeWindowLease {
    pub(super) fn is_retired(&self) -> bool {
        match self {
            #[cfg(feature = "multi-viewport-winit")]
            Self::Winit(lease) => lease.is_retired(),
            #[cfg(feature = "multi-viewport-sdl3")]
            Self::Sdl3(lease) => lease.is_retired(),
        }
    }
}
