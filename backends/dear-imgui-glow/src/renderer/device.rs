use dear_imgui_rs::Context as ImGuiContext;
use glow::{Context, HasContext};

use super::GlowRenderer;
use crate::{
    error::{RenderError, RenderResult},
    shaders::Shaders,
    texture::TextureMap,
};

impl GlowRenderer {
    /// Destroy the renderer and free OpenGL resources.
    ///
    /// A renderer consumed by `GlowViewportRuntime` must be shut down through that owning runtime.
    pub fn destroy(&mut self, gl: &Context, imgui_context: &mut ImGuiContext) -> RenderResult<()> {
        self.ensure_context_matches(imgui_context)?;
        self.destroy_gpu_resources_only(gl);

        let consumer = self
            .renderer_consumer
            .as_ref()
            .ok_or(RenderError::RendererNotAttached)?;
        imgui_context.reset_renderer_texture_bindings(consumer)?;
        Self::unconfigure_imgui_context_static(imgui_context);
        self.renderer_consumer.take();
        Ok(())
    }

    /// Get a reference to the OpenGL context (if owned by the renderer)
    pub fn gl_context(&self) -> Option<&std::rc::Rc<glow::Context>> {
        self.gl_context.as_ref()
    }

    /// Get a reference to the texture map
    pub fn texture_map(&self) -> &dyn TextureMap {
        self.texture_map
            .as_deref()
            .expect("GlowRenderer texture_map missing (internal borrow bug)")
    }

    /// Get a mutable reference to the texture map
    pub fn texture_map_mut(&mut self) -> &mut dyn TextureMap {
        self.texture_map
            .as_deref_mut()
            .expect("GlowRenderer texture_map missing (internal borrow bug)")
    }

    /// Called every frame to prepare for rendering
    pub fn new_frame(&mut self) -> RenderResult<()> {
        if self.renderer_consumer.is_none() {
            return Err(RenderError::RendererDestroyed);
        }

        // Check if we need to recreate device objects
        let needs_recreation = self.is_destroyed || self.shaders.program.is_none();

        if needs_recreation {
            if let Some(gl) = self.gl_context.clone() {
                self.create_device_objects(&gl)?;
            } else {
                return Err(RenderError::MissingGlContext);
            }
        }
        Ok(())
    }

    /// Enable/disable GL_FRAMEBUFFER_SRGB around ImGui rendering
    /// Default is disabled; prefer application-level control of sRGB.
    pub fn set_framebuffer_srgb_enabled(&mut self, enabled: bool) {
        self.framebuffer_srgb = enabled;
    }

    /// Override the color gamma applied to ImGui vertex colors.
    /// Pass `Some(gamma)` to force a value (e.g., 2.2 or 1.0), or `None` to use auto:
    /// auto = 2.2 when sRGB is enabled, otherwise 1.0.
    pub fn set_color_gamma_override(&mut self, gamma: Option<f32>) {
        self.color_gamma_override = gamma;
    }

    /// Set clear color for secondary viewports when multi-viewport is enabled.
    ///
    /// This affects the callback owned by `GlowViewportRuntime`. Clearing the main framebuffer
    /// remains the application's responsibility.
    pub fn set_viewport_clear_color(&mut self, color: [f32; 4]) {
        self.viewport_clear_color = color;
    }

    /// Create OpenGL device objects (buffers, shaders, etc.)
    pub fn create_device_objects(&mut self, gl: &Context) -> RenderResult<()> {
        if self.shaders.program.is_none() {
            self.shaders =
                Shaders::new(gl, self.gl_version).map_err(RenderError::DeviceObjectInit)?;
        }

        if self.vbo_handle.is_none() {
            self.vbo_handle =
                Some(
                    unsafe { gl.create_buffer() }.map_err(|e| RenderError::CreateResource {
                        resource: "VBO",
                        error: e,
                    })?,
                );
        }

        if self.ebo_handle.is_none() {
            self.ebo_handle =
                Some(
                    unsafe { gl.create_buffer() }.map_err(|e| RenderError::CreateResource {
                        resource: "EBO",
                        error: e,
                    })?,
                );
        }

        self.is_destroyed = false;
        Ok(())
    }

    /// Destroy OpenGL device objects and detach their managed texture bindings.
    pub fn destroy_device_objects(
        &mut self,
        gl: &Context,
        imgui_context: &mut ImGuiContext,
    ) -> RenderResult<()> {
        self.ensure_context_matches(imgui_context)?;
        self.destroy_device_objects_only(gl);
        let consumer = self
            .renderer_consumer
            .as_ref()
            .ok_or(RenderError::RendererNotAttached)?;
        imgui_context.reset_renderer_texture_bindings(consumer)?;
        Ok(())
    }

    fn destroy_device_objects_only(&mut self, gl: &Context) {
        if let Some(vbo) = self.vbo_handle.take() {
            unsafe { gl.delete_buffer(vbo) };
        }
        if let Some(ebo) = self.ebo_handle.take() {
            unsafe { gl.delete_buffer(ebo) };
        }
        if let Some(program) = self.shaders.program.take() {
            unsafe { gl.delete_program(program) };
        }
        for texture in self.owned_textures.drain(..) {
            unsafe { gl.delete_texture(texture) };
        }
        self.managed_textures.clear();
        self.texture_map_mut().clear();
        self.is_destroyed = true;
    }

    pub(super) fn destroy_gpu_resources_only(&mut self, gl: &Context) {
        self.destroy_device_objects_only(gl);
        #[cfg(feature = "bind_vertex_array_support")]
        if let Some(vao) = self.vertex_array_object.take() {
            unsafe { gl.delete_vertex_array(vao) };
        }
    }

    pub(super) fn ensure_context_matches(&self, imgui_context: &ImGuiContext) -> RenderResult<()> {
        let consumer = self
            .renderer_consumer
            .as_ref()
            .ok_or(RenderError::RendererNotAttached)?;
        if consumer.context_id() != imgui_context.id() {
            return Err(RenderError::ContextMismatch {
                expected: consumer.context_id(),
                actual: imgui_context.id(),
            });
        }
        Ok(())
    }
}

impl Drop for GlowRenderer {
    fn drop(&mut self) {
        if let Some(gl) = self.gl_context.take() {
            self.destroy_gpu_resources_only(&gl);
        }
    }
}
