use crate::{GlBuffer, GlTexture, shaders::Shaders, texture::TextureMap, versions::GlVersion};
use dear_imgui_rs::ContextBinding;
use dear_imgui_rs::render::{SnapshotTextureId, SynchronousRendererConsumer};
use std::collections::HashMap;
use std::ffi::c_void;

use super::sampler::SamplerObjects;
use super::texture::{ManagedTextureBinding, ManagedTextureTombstone};

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
/// flexibility for advanced use cases. It can retain a Glow function table and own texture
/// resources (simple usage) or borrow the function table on each operation (advanced usage).
/// It never owns or switches the native OpenGL context.
pub struct GlowRenderer {
    // Core rendering state
    pub(super) shaders: Shaders,
    pub(super) vbo_handle: Option<GlBuffer>,
    pub(super) ebo_handle: Option<GlBuffer>,
    pub(super) owned_textures: Vec<GlTexture>,
    pub(super) samplers: Option<SamplerObjects>,
    pub(super) gl_version: GlVersion,
    pub(super) has_clip_origin_support: bool,
    pub(super) has_separate_polygon_modes: bool,
    pub(super) has_sampler_object_support: bool,

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
    /// Identities sealed by Destroy until a matching `Destroyed` outcome is acknowledged.
    pub(super) destroyed_managed_textures: HashMap<SnapshotTextureId, ManagedTextureTombstone>,
    pub(super) renderer_consumer: Option<SynchronousRendererConsumer>,
    // Optional: enable GL_FRAMEBUFFER_SRGB during ImGui rendering
    pub(super) framebuffer_srgb: bool,
    // Optional: override color gamma applied to vertex colors (None = auto)
    pub(super) color_gamma_override: Option<f32>,
    // Clear color used for secondary viewports (multi-viewport). Main framebuffer
    // clear remains responsibility of the application.
    pub(super) viewport_clear_color: [f32; 4],
}

impl GlowRenderer {
    /// Returns the synchronous consumer capability owned by this renderer.
    ///
    /// Pass it to [`dear_imgui_rs::Context::render`] to create the pending frame that this
    /// renderer consumes.
    pub fn renderer_consumer(&self) -> crate::RenderResult<&SynchronousRendererConsumer> {
        self.renderer_consumer
            .as_ref()
            .ok_or(crate::RenderError::RendererNotAttached)
    }

    /// Returns the OpenGL version detected from the live context at initialization.
    pub fn gl_version(&self) -> GlVersion {
        self.gl_version
    }

    /// Returns whether the live context supports querying `GL_CLIP_ORIGIN`.
    pub fn supports_clip_origin(&self) -> bool {
        self.has_clip_origin_support
    }

    /// Returns whether the live context exposes desktop `GL_FRAMEBUFFER_SRGB` control.
    pub fn supports_framebuffer_srgb_control(&self) -> bool {
        !self.gl_version.is_es
    }

    /// Returns whether the live context supports independent sampler objects.
    pub fn supports_sampler_objects(&self) -> bool {
        self.has_sampler_object_support
    }

    pub(super) fn device_objects_ready(&self) -> bool {
        self.shaders.program.is_some()
            && self.vbo_handle.is_some()
            && self.ebo_handle.is_some()
            && (!self.has_sampler_object_support || self.samplers.is_some())
    }

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
