use crate::{
    GammaMode, RendererError, RendererResult, ShaderManager, WgpuBackendData, WgpuTextureManager,
};
use dear_imgui_rs::{
    BackendFlags, Context, ContextBinding,
    render::{RenderedFrame, RendererConsumer},
};
use wgpu::TextureView;

#[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
use wgpu::Color;

#[derive(Clone, Debug)]
pub(super) struct RendererContextBinding {
    context: ContextBinding,
    renderer_flags_added: BackendFlags,
}

impl RendererContextBinding {
    pub(super) fn capture(context: &Context, renderer_flags_added: BackendFlags) -> Self {
        Self {
            context: context.binding(),
            renderer_flags_added,
        }
    }

    fn ensure_alive(&self) -> RendererResult<()> {
        if self.context.is_alive() {
            Ok(())
        } else {
            Err(RendererError::ContextDropped)
        }
    }

    pub(super) fn ensure_matches(&self, context: &Context) -> RendererResult<()> {
        self.ensure_alive()?;
        if self.context.id() == context.id() {
            Ok(())
        } else {
            Err(RendererError::ContextMismatch)
        }
    }

    pub(super) fn context(&self) -> ContextBinding {
        self.context.clone()
    }

    pub(super) fn renderer_flags_added(&self) -> BackendFlags {
        self.renderer_flags_added
    }
}

/// Main WGPU renderer for Dear ImGui
///
/// This corresponds to the main renderer functionality in imgui_impl_wgpu.cpp
///
/// An initialized renderer owns the renderer state of exactly one [`Context`]. Create a separate
/// renderer for every Dear ImGui context and call [`Self::shutdown`](WgpuRenderer::shutdown) with
/// the matching context before dropping either value. The retained context binding makes
/// this renderer UI-thread-bound.
pub struct WgpuRenderer {
    /// Dear ImGui context whose renderer state this instance owns.
    pub(super) context_binding: Option<RendererContextBinding>,
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
    /// Sole managed-texture consumer generation owned by this renderer.
    pub(super) renderer_consumer: Option<RendererConsumer>,
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
        self.context_binding = Some(RendererContextBinding::capture(
            context,
            renderer_flags_added,
        ));
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

    pub(super) fn bound_context(&self) -> RendererResult<ContextBinding> {
        Ok(self
            .context_binding
            .as_ref()
            .ok_or(RendererError::ContextNotBound)?
            .context())
    }

    pub(super) fn clear_context_binding(&mut self) {
        self.context_binding = None;
    }

    pub(super) fn renderer_consumer(&self) -> RendererResult<&RendererConsumer> {
        self.renderer_consumer
            .as_ref()
            .ok_or(RendererError::ContextNotBound)
    }

    pub(super) fn ensure_frame_matches(&self, frame: &RenderedFrame<'_>) -> RendererResult<()> {
        let consumer = self.renderer_consumer()?;
        if frame.context_id() != consumer.context_id() {
            return Err(RendererError::ContextMismatch);
        }
        let epoch = frame.epoch().ok_or_else(|| {
            RendererError::InvalidRenderState(
                "WGPU requires a managed-texture renderer epoch".to_owned(),
            )
        })?;
        if epoch.consumer_generation() != consumer.generation() {
            return Err(RendererError::InvalidRenderState(format!(
                "rendered frame uses consumer generation {}, WGPU owns generation {}",
                epoch.consumer_generation(),
                consumer.generation()
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dropped_owner_is_not_confused_with_a_reused_context_address() {
        let owner = Context::create();
        let binding = RendererContextBinding::capture(&owner, BackendFlags::empty());
        drop(owner);

        let replacement = Context::create();
        assert!(matches!(
            binding.ensure_matches(&replacement),
            Err(RendererError::ContextDropped)
        ));
    }
}
