#[cfg(feature = "bind_vertex_array_support")]
use crate::GlVertexArray;
use crate::{GlBuffer, GlTexture, shaders::Shaders, texture::TextureMap, versions::GlVersion};
use dear_imgui_rs::ContextBinding;
use dear_imgui_rs::render::{RendererConsumer, SnapshotTextureId};
use std::collections::HashMap;
use std::ffi::c_void;

use super::texture::ManagedTextureBinding;

/// Stable marker stored in `BackendRendererUserData` while Glow owns renderer state.
#[derive(Debug, Default)]
pub(super) struct GlowBackendUserData {
    _marker: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RendererStateFault {
    State(&'static str),
    Callback(&'static str),
    Capability(&'static str),
}

impl RendererStateFault {
    pub(super) fn into_error(self) -> crate::RenderError {
        match self {
            Self::State(field) => crate::RenderError::RendererStateDrift { field },
            Self::Callback(callback) => crate::RenderError::RendererCallbackReplaced { callback },
            Self::Capability(flag) => crate::RenderError::RendererCapabilityDrift { flag },
        }
    }
}

/// Main renderer for Dear ImGui using Glow (OpenGL)
///
/// This renderer provides a unified API similar to the WGPU backend while maintaining
/// flexibility for advanced use cases. It can either own the OpenGL context and texture
/// management (simple usage) or work with externally managed resources (advanced usage).
pub struct GlowRenderer {
    // Core rendering state
    pub(super) shaders: Shaders,
    pub vbo_handle: Option<GlBuffer>,
    pub ebo_handle: Option<GlBuffer>,
    pub(super) owned_textures: Vec<GlTexture>,
    #[cfg(feature = "bind_vertex_array_support")]
    pub vertex_array_object: Option<GlVertexArray>,
    pub gl_version: GlVersion,
    pub has_clip_origin_support: bool,
    pub is_destroyed: bool,

    // Resource management
    pub(super) gl_context: Option<std::rc::Rc<glow::Context>>, // None = externally managed
    pub(super) context_binding: Option<ContextBinding>,
    pub(super) backend_user_data: Box<GlowBackendUserData>,
    pub(super) renderer_name_ptr: *const std::ffi::c_char,
    pub(super) renderer_texture_max: [i32; 2],
    pub(super) renderer_state_fault: Option<RendererStateFault>,
    #[cfg(test)]
    pub(super) synthetic_test_renderer: bool,
    pub(super) texture_map: Option<Box<dyn TextureMap>>,
    pub(super) managed_textures: HashMap<SnapshotTextureId, ManagedTextureBinding>,
    /// Identities sealed by Destroy, paired with their latest request epoch.
    pub(super) destroyed_managed_textures: HashMap<SnapshotTextureId, u64>,
    pub(super) renderer_consumer: Option<RendererConsumer>,
    // Optional: enable GL_FRAMEBUFFER_SRGB during ImGui rendering
    pub(super) framebuffer_srgb: bool,
    // Optional: override color gamma applied to vertex colors (None = auto)
    pub(super) color_gamma_override: Option<f32>,
    // Clear color used for secondary viewports (multi-viewport). Main framebuffer
    // clear remains responsibility of the application.
    pub(super) viewport_clear_color: [f32; 4],
}

impl GlowRenderer {
    pub(super) fn track_owned_texture(&mut self, texture: GlTexture) {
        if !self.owned_textures.contains(&texture) {
            self.owned_textures.push(texture);
        }
    }

    pub(super) fn forget_owned_texture(&mut self, texture: GlTexture) {
        self.owned_textures.retain(|owned| *owned != texture);
    }

    pub(super) fn backend_user_data_ptr(&self) -> *mut c_void {
        std::ptr::from_ref(self.backend_user_data.as_ref())
            .cast_mut()
            .cast()
    }
}
