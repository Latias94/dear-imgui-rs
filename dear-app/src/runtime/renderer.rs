//! One renderer owner per GPU generation, composed from the backend's existing owning routes.

use crate::{ExternalTextureError, RunError};

pub(crate) enum AppRenderer {
    Single(dear_imgui_wgpu::WgpuRenderer),
    #[cfg(feature = "multi-viewport")]
    Multi(dear_imgui_wgpu::multi_viewport::WinitViewportRoute),
}

impl AppRenderer {
    pub(crate) fn single_mut(&mut self) -> Result<&mut dear_imgui_wgpu::WgpuRenderer, RunError> {
        match self {
            Self::Single(renderer) => Ok(renderer),
            #[cfg(feature = "multi-viewport")]
            Self::Multi(_) => Err(RunError::Recovery {
                message: "single-window frame used a native viewport route".to_owned(),
            }),
        }
    }
    pub(crate) fn shutdown(
        &mut self,
        context: &mut dear_imgui_rs::Context,
    ) -> Result<(), RunError> {
        match self {
            Self::Single(renderer) => renderer
                .shutdown(context)
                .map_err(RunError::RendererRelease),
            #[cfg(feature = "multi-viewport")]
            Self::Multi(route) => route.shutdown(context).map_err(RunError::Viewport),
        }
    }

    pub(crate) fn prepare_recovery(
        &mut self,
        context: &mut dear_imgui_rs::Context,
    ) -> Result<(), RunError> {
        match self {
            Self::Single(renderer) => renderer
                .invalidate_device_objects(context)
                .map_err(RunError::GpuInvalidation),
            #[cfg(feature = "multi-viewport")]
            // The owning route performs the managed reset in its shutdown transaction, together
            // with secondary Surface teardown. An earlier rejected surface may already have
            // quiesced its recording API; do not try to reopen that API to perform a second reset.
            Self::Multi(_) => Ok(()),
        }
    }

    pub(crate) fn register_external_texture(
        &mut self,
        view: &wgpu::TextureView,
    ) -> Result<dear_imgui_wgpu::ExternalTextureId, ExternalTextureError> {
        match self {
            Self::Single(renderer) => Ok(renderer.register_external_texture(view)?),
            #[cfg(feature = "multi-viewport")]
            Self::Multi(route) => Ok(route.register_external_texture(view)?),
        }
    }

    pub(crate) fn update_external_texture(
        &mut self,
        texture: dear_imgui_wgpu::ExternalTextureId,
        view: &wgpu::TextureView,
    ) -> Result<(), ExternalTextureError> {
        match self {
            Self::Single(renderer) => Ok(renderer.update_external_texture(texture, view)?),
            #[cfg(feature = "multi-viewport")]
            Self::Multi(route) => Ok(route.update_external_texture(texture, view)?),
        }
    }

    pub(crate) fn unregister_external_texture(
        &mut self,
        texture: dear_imgui_wgpu::ExternalTextureId,
    ) -> Result<(), ExternalTextureError> {
        match self {
            Self::Single(renderer) => Ok(renderer.unregister_external_texture(texture)?),
            #[cfg(feature = "multi-viewport")]
            Self::Multi(route) => Ok(route.unregister_external_texture(texture)?),
        }
    }
}
