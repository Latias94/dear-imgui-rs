use crate::{
    GlBuffer, GlTexture, GlVertexArray, shaders::Shaders, state::GlStateBackup,
    texture::TextureMap, versions::GlVersion,
};
use dear_imgui_rs::render::{RendererConsumer, SnapshotTextureId};
use std::collections::HashMap;

use super::texture::ManagedTextureBinding;

/// Main renderer for Dear ImGui using Glow (OpenGL)
///
/// This renderer provides a unified API similar to the WGPU backend while maintaining
/// flexibility for advanced use cases. It can either own the OpenGL context and texture
/// management (simple usage) or work with externally managed resources (advanced usage).
pub struct GlowRenderer {
    // Core rendering state
    pub(super) shaders: Shaders,
    pub(super) state_backup: GlStateBackup,
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
    pub(super) texture_map: Option<Box<dyn TextureMap>>,
    pub(super) managed_textures: HashMap<SnapshotTextureId, ManagedTextureBinding>,
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
}
