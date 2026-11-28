use crate::{
    GlBuffer, shaders::Shaders, state::BindProgramPipeline, texture::TextureRegistry,
    versions::GlVersion,
};
use dear_imgui_backend_support::ManagedTextureTable;
use dear_imgui_rs::ContextBinding;
use dear_imgui_rs::render::SynchronousRendererConsumer;
use std::rc::Rc;

use super::init::RendererPublication;
use super::sampler::SamplerObjects;
use super::texture::ManagedTextureBinding;

/// Clip origin used by the application-owned OpenGL context.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum GlowClipOrigin {
    /// Matches the OpenGL default and avoids querying GL state during rendering.
    #[default]
    LowerLeft,
    /// Matches `glClipControl(GL_UPPER_LEFT, ...)`.
    UpperLeft,
    /// Queries `GL_CLIP_ORIGIN` when the context supports clip control.
    Auto,
}

pub(super) type ActivateGlContext = Box<dyn Fn() -> Result<(), String>>;

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
/// On native platforms, [`Self::with_owned_context`] retains the application-provided context
/// owner and uses it to make the correct context current for automatic resource release.
/// [`Self::new`] and [`Self::with_shared_context`] retain a Glow function table, and
/// [`Self::with_external_context`] borrows it per operation. When the application manages the
/// native current context directly, it also arranges explicit shutdown.
pub struct GlowRenderer {
    // Core rendering state
    pub(super) shaders: Shaders,
    pub(super) vbo_handle: Option<GlBuffer>,
    pub(super) ebo_handle: Option<GlBuffer>,
    pub(super) samplers: Option<SamplerObjects>,
    pub(super) gl_version: GlVersion,
    pub(super) has_clip_origin_support: bool,
    pub(super) clip_origin: GlowClipOrigin,
    pub(super) has_separate_polygon_modes: bool,
    pub(super) has_sampler_object_support: bool,
    pub(super) bind_program_pipeline: Option<BindProgramPipeline>,

    // Resource management
    pub(super) gl_context: Option<Rc<glow::Context>>, // None = externally managed
    pub(super) context_owner: Option<ActivateGlContext>,
    pub(super) context_binding: Option<ContextBinding>,
    pub(super) publication: Option<RendererPublication>,
    pub(super) renderer_state_fault: Option<RendererStateFault>,
    #[cfg(test)]
    pub(super) synthetic_test_renderer: bool,
    pub(super) texture_registry: TextureRegistry,
    pub(super) managed_textures: ManagedTextureTable<ManagedTextureBinding>,
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

    /// Returns the configured origin. The default is [`GlowClipOrigin::LowerLeft`].
    pub fn clip_origin(&self) -> GlowClipOrigin {
        self.clip_origin
    }

    /// Matches the projection to the application's `glClipControl()` origin.
    ///
    /// This does not call `glClipControl()`. Use [`GlowClipOrigin::Auto`] to query the
    /// current GL origin during rendering, or select an explicit origin to avoid that query.
    pub fn set_clip_origin(&mut self, origin: GlowClipOrigin) {
        self.clip_origin = origin;
    }

    /// Returns whether the live context exposes desktop `GL_FRAMEBUFFER_SRGB` control.
    pub fn supports_framebuffer_srgb_control(&self) -> bool {
        !self.gl_version.is_es
    }

    /// Returns whether the live context supports independent sampler objects.
    pub fn supports_sampler_objects(&self) -> bool {
        self.has_sampler_object_support
    }

    /// Supplies a native OpenGL binding function for preserving program pipeline state.
    ///
    /// Use `glBindProgramPipeline` for core or ARB support, or `glBindProgramPipelineEXT` for
    /// an OpenGL ES context that exposes only `GL_EXT_separate_shader_objects`. Glow does not
    /// expose this entry point. The renderer uses it only on contexts with pipeline support.
    ///
    /// # Safety
    ///
    /// `bind` must be the corresponding GL entry point from the application's context loader.
    /// It must remain valid for all supported contexts used by this renderer, including viewports.
    #[cfg(any(not(target_family = "wasm"), target_os = "emscripten"))]
    pub unsafe fn set_bind_program_pipeline(
        &mut self,
        bind: Option<unsafe extern "system" fn(u32)>,
    ) {
        self.bind_program_pipeline = bind;
    }

    pub(super) fn device_objects_ready(&self) -> bool {
        self.shaders.program.is_some()
            && self.vbo_handle.is_some()
            && self.ebo_handle.is_some()
            && (!self.has_sampler_object_support || self.samplers.is_some())
    }

    pub(super) fn has_owned_gl_cleanup(&self) -> bool {
        // In browser WebGL, the retained Glow table owns a WebGL rendering context object.
        self.context_owner.is_some()
            || (cfg!(all(target_family = "wasm", not(target_os = "emscripten")))
                && self.gl_context.is_some())
    }

    pub(super) fn activate_gl_context(&self, gl: &glow::Context) -> crate::RenderResult<()> {
        if self.has_owned_gl_cleanup()
            && !self
                .gl_context
                .as_deref()
                .is_some_and(|owned| std::ptr::eq(owned, gl))
        {
            return Err(crate::RenderError::GlContextMismatch);
        }
        if let Some(activate) = &self.context_owner {
            activate().map_err(crate::RenderError::ActivateGlContext)?;
        }
        Ok(())
    }
}
