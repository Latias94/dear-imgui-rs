use crate::{
    GammaMode, RendererError, RendererResult, ShaderManager, WgpuBackendData, WgpuTextureManager,
};
use dear_imgui_rs::{BackendFlags, Context, ContextAliveToken, sys};
use wgpu::TextureView;

#[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
use std::sync::atomic::AtomicBool;
#[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
use wgpu::Color;

#[derive(Clone, Debug)]
pub(super) struct ContextBinding {
    raw: *mut sys::ImGuiContext,
    alive: ContextAliveToken,
    renderer_flags_added: BackendFlags,
}

impl ContextBinding {
    pub(super) fn capture(context: &Context, renderer_flags_added: BackendFlags) -> Self {
        Self {
            raw: context.as_raw(),
            alive: context.alive_token(),
            renderer_flags_added,
        }
    }

    fn ensure_alive(&self) -> RendererResult<()> {
        if self.alive.is_alive() {
            Ok(())
        } else {
            Err(RendererError::ContextDropped)
        }
    }

    pub(super) fn ensure_matches(&self, context: &Context) -> RendererResult<()> {
        self.ensure_alive()?;
        if self.raw == context.as_raw() {
            Ok(())
        } else {
            Err(RendererError::ContextMismatch)
        }
    }

    pub(super) fn ensure_current(&self) -> RendererResult<()> {
        self.ensure_alive()?;
        let current = unsafe { sys::igGetCurrentContext() };
        if self.raw == current {
            Ok(())
        } else {
            Err(RendererError::ContextNotCurrent)
        }
    }

    pub(super) fn renderer_flags_added(&self) -> BackendFlags {
        self.renderer_flags_added
    }

    fn platform_io(&self) -> RendererResult<*mut sys::ImGuiPlatformIO> {
        self.ensure_current()?;
        let platform_io = unsafe { sys::igGetPlatformIO_ContextPtr(self.raw) };
        if platform_io.is_null() {
            Err(RendererError::InvalidRenderState(
                "bound Dear ImGui context has no PlatformIO".to_owned(),
            ))
        } else {
            Ok(platform_io)
        }
    }
}

/// Main WGPU renderer for Dear ImGui
///
/// This corresponds to the main renderer functionality in imgui_impl_wgpu.cpp
///
/// An initialized renderer owns the renderer state of exactly one [`Context`]. Create a separate
/// renderer for every Dear ImGui context and call [`Self::shutdown`](WgpuRenderer::shutdown) with
/// the matching context before dropping either value. The retained context liveness token makes
/// this renderer UI-thread-bound.
pub struct WgpuRenderer {
    /// Dear ImGui context whose renderer state this instance owns.
    pub(super) context_binding: Option<ContextBinding>,
    /// Backend data
    pub(super) backend_data: Option<WgpuBackendData>,
    /// Shader manager
    pub(super) shader_manager: ShaderManager,
    /// Texture manager
    pub(super) texture_manager: WgpuTextureManager,
    /// Default texture for fallback
    pub(super) default_texture: Option<TextureView>,
    /// Gamma mode: automatic (by format), force linear (1.0), or force 2.2
    pub(super) gamma_mode: GammaMode,
    /// Clear color used for secondary viewports (multi-viewport mode)
    #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
    pub(super) viewport_clear_color: Color,
    /// Prevents safe lifecycle APIs from replacing GPU state behind registered raw callbacks.
    #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
    pub(super) multi_viewport_active: AtomicBool,
}

impl WgpuRenderer {
    pub(super) fn bind_context(
        &mut self,
        context: &Context,
        renderer_flags_added: BackendFlags,
    ) -> RendererResult<()> {
        if self.context_binding.is_some() {
            return Err(RendererError::InvalidRenderState(
                "renderer is already bound to a Dear ImGui context".to_owned(),
            ));
        }
        self.context_binding = Some(ContextBinding::capture(context, renderer_flags_added));
        Ok(())
    }

    pub(super) fn ensure_context_alive(&self) -> RendererResult<()> {
        self.context_binding
            .as_ref()
            .ok_or(RendererError::ContextNotBound)?
            .ensure_alive()
    }

    pub(super) fn ensure_context_matches(&self, context: &Context) -> RendererResult<()> {
        self.context_binding
            .as_ref()
            .ok_or(RendererError::ContextNotBound)?
            .ensure_matches(context)
    }

    pub(super) fn renderer_flags_added(&self) -> RendererResult<BackendFlags> {
        Ok(self
            .context_binding
            .as_ref()
            .ok_or(RendererError::ContextNotBound)?
            .renderer_flags_added())
    }

    pub(super) fn render_platform_io(&self) -> RendererResult<*mut sys::ImGuiPlatformIO> {
        self.context_binding
            .as_ref()
            .ok_or(RendererError::ContextNotBound)?
            .platform_io()
    }

    pub(super) fn clear_context_binding(&mut self) {
        self.context_binding = None;
    }
}

#[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
impl WgpuRenderer {
    pub(super) fn ensure_multi_viewport_inactive(&self) -> crate::RendererResult<()> {
        if self
            .multi_viewport_active
            .load(std::sync::atomic::Ordering::Acquire)
        {
            Err(crate::RendererError::MultiViewportActive)
        } else {
            Ok(())
        }
    }
}

#[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
impl Drop for WgpuRenderer {
    fn drop(&mut self) {
        self.clear_multi_viewport_renderer_state();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dropped_owner_is_not_confused_with_a_reused_context_address() {
        let owner = Context::create();
        let binding = ContextBinding::capture(&owner, BackendFlags::empty());
        drop(owner);

        let replacement = Context::create();
        assert!(matches!(
            binding.ensure_matches(&replacement),
            Err(RendererError::ContextDropped)
        ));
    }
}
