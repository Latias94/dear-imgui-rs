use dear_imgui_rs::sys;

use crate::{
    GlBuffer, GlTexture, GlVertexArray, shaders::Shaders, state::GlStateBackup,
    texture::TextureMap, versions::GlVersion,
};

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
    pub font_atlas_texture: Option<GlTexture>,
    pub(super) font_atlas_texture_data: *mut sys::ImTextureData,
    #[cfg(feature = "bind_vertex_array_support")]
    pub vertex_array_object: Option<GlVertexArray>,
    pub gl_version: GlVersion,
    pub has_clip_origin_support: bool,
    pub is_destroyed: bool,

    // Resource management
    pub(super) gl_context: Option<std::rc::Rc<glow::Context>>, // None = externally managed
    pub(super) texture_map: Option<Box<dyn TextureMap>>,
    // Optional: enable GL_FRAMEBUFFER_SRGB during ImGui rendering
    pub(super) framebuffer_srgb: bool,
    // Optional: override color gamma applied to vertex colors (None = auto)
    pub(super) color_gamma_override: Option<f32>,
    // Clear color used for secondary viewports (multi-viewport). Main framebuffer
    // clear remains responsibility of the application.
    pub(super) viewport_clear_color: [f32; 4],
}
