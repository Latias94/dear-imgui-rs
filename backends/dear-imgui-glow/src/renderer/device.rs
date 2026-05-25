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
    /// If multi-viewport support was enabled, this also makes renderer callbacks no-op for this
    /// renderer. Call the matching multi-viewport shutdown helper when you also need to uninstall
    /// callbacks from the ImGui context and destroy platform windows.
    pub fn destroy(&mut self, gl: &Context) {
        #[cfg(feature = "multi-viewport")]
        self.clear_multi_viewport_renderer_state();

        if self.is_destroyed {
            return;
        }

        if let Some(h) = self.vbo_handle {
            unsafe { gl.delete_buffer(h) };
            self.vbo_handle = None;
        }
        if let Some(h) = self.ebo_handle {
            unsafe { gl.delete_buffer(h) };
            self.ebo_handle = None;
        }
        if let Some(p) = self.shaders.program {
            unsafe { gl.delete_program(p) };
            self.shaders.program = None;
        }
        if let Some(h) = self.font_atlas_texture {
            unsafe { gl.delete_texture(h) };
            self.font_atlas_texture = None;
        }

        #[cfg(feature = "bind_vertex_array_support")]
        if let Some(vao) = self.vertex_array_object {
            unsafe { gl.delete_vertex_array(vao) };
            self.vertex_array_object = None;
        }

        self.is_destroyed = true;
    }

    #[cfg(feature = "multi-viewport")]
    fn clear_multi_viewport_renderer_state(&mut self) {
        // Make any installed multi-viewport callbacks become a no-op if the renderer is
        // explicitly destroyed or dropped without an explicit disable/shutdown call.
        super::multi_viewport::clear_for_drop(self as *mut GlowRenderer);
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
    /// This only affects the per-viewport renderer callback installed via
    /// `multi_viewport::enable`. Clearing of the main framebuffer remains
    /// responsibility of the application.
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

    /// Destroy OpenGL device objects
    pub fn destroy_device_objects(&mut self, gl: &Context) {
        if let Some(vbo) = self.vbo_handle.take() {
            unsafe { gl.delete_buffer(vbo) };
        }
        if let Some(ebo) = self.ebo_handle.take() {
            unsafe { gl.delete_buffer(ebo) };
        }
        if let Some(program) = self.shaders.program.take() {
            unsafe { gl.delete_program(program) };
        }
        if let Some(texture) = self.font_atlas_texture.take() {
            unsafe { gl.delete_texture(texture) };
        }
        self.is_destroyed = true;
    }
}

impl Drop for GlowRenderer {
    fn drop(&mut self) {
        #[cfg(feature = "multi-viewport")]
        {
            self.clear_multi_viewport_renderer_state();
        }
        if let Some(gl) = self.gl_context.take() {
            self.destroy_device_objects(&gl);
        }
    }
}
