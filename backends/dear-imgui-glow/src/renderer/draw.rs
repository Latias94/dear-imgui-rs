use dear_imgui_rs::{
    internal::RawWrapper,
    render::{DrawCmd, DrawCmdParams, DrawData, DrawVert},
};
use glow::{Context, HasContext};
use std::mem::size_of;

use super::GlowRenderer;
use crate::{
    draw_indices_as_bytes, draw_verts_as_bytes,
    error::{RenderError, RenderResult},
    gl_debug_message,
    texture::TextureMap,
};

impl GlowRenderer {
    /// Render Dear ImGui draw data
    pub fn render(&mut self, draw_data: &mut DrawData) -> RenderResult<()> {
        let gl = self
            .gl_context
            .clone()
            .ok_or(RenderError::MissingGlContext)?;

        // Handle texture updates first, following the original Dear ImGui OpenGL3 implementation
        let mut textures = draw_data.textures_mut();
        while let Some(mut texture_data) = textures.next() {
            if texture_data.status() != dear_imgui_rs::TextureStatus::OK {
                self.update_texture_from_data(Some(&gl), &mut *texture_data)?;
            }
        }
        drop(textures);

        self.render_internal(&gl, draw_data)
    }

    /// Advanced render method with external OpenGL context
    pub fn render_with_context(
        &mut self,
        gl: &Context,
        draw_data: &mut DrawData,
    ) -> RenderResult<()> {
        // Handle texture updates first
        let mut textures = draw_data.textures_mut();
        while let Some(mut texture_data) = textures.next() {
            if texture_data.status() != dear_imgui_rs::TextureStatus::OK {
                self.update_texture_from_data(Some(gl), &mut *texture_data)?;
            }
        }
        drop(textures);

        self.render_internal(gl, draw_data)
    }

    /// Internal render implementation
    fn render_internal(&mut self, gl: &Context, draw_data: &DrawData) -> RenderResult<()> {
        if self.is_destroyed {
            return Err(RenderError::RendererDestroyed);
        }

        let fb_width = draw_data.display_size[0] * draw_data.framebuffer_scale[0];
        let fb_height = draw_data.display_size[1] * draw_data.framebuffer_scale[1];
        if !(fb_width > 0.0 && fb_height > 0.0) {
            return Ok(());
        }

        gl_debug_message(gl, "dear-imgui-glow: start render");

        self.state_backup.backup(gl, self.gl_version);

        #[cfg(feature = "bind_vertex_array_support")]
        if self.gl_version.bind_vertex_array_support() {
            unsafe {
                self.vertex_array_object =
                    Some(
                        gl.create_vertex_array()
                            .map_err(|err| RenderError::CreateResource {
                                resource: "vertex array object",
                                error: err,
                            })?,
                    );
                gl.bind_vertex_array(self.vertex_array_object);
            }
        }

        self.set_up_render_state(gl, draw_data, fb_width, fb_height)?;

        // Render draw lists. We temporarily move `texture_map` out to avoid creating
        // aliasing references (e.g. `&mut self` + `&self.texture_map`) and relying on raw pointers.
        let texture_map = self
            .texture_map
            .take()
            .expect("GlowRenderer texture_map missing (internal borrow bug)");
        let render_res = self.render_draw_lists(gl, &*texture_map, draw_data);
        self.texture_map = Some(texture_map);
        render_res?;

        // Cleanup
        #[cfg(feature = "bind_vertex_array_support")]
        if self.gl_version.bind_vertex_array_support()
            && let Some(vao) = self.vertex_array_object
        {
            unsafe { gl.delete_vertex_array(vao) };
            self.vertex_array_object = None;
        }

        // Optionally disable FRAMEBUFFER_SRGB before restoring state (we didn't back it up)
        if self.framebuffer_srgb {
            unsafe { gl.disable(glow::FRAMEBUFFER_SRGB) };
        }
        self.state_backup.restore(gl, self.gl_version);
        gl_debug_message(gl, "dear-imgui-glow: end render");

        Ok(())
    }

    /// Set up OpenGL render state for ImGui rendering
    fn set_up_render_state(
        &mut self,
        gl: &Context,
        draw_data: &DrawData,
        fb_width: f32,
        fb_height: f32,
    ) -> RenderResult<()> {
        unsafe {
            // Ensure sampler uses texture unit 0 (shader binds sampler to 0)
            gl.active_texture(glow::TEXTURE0);
            // Setup render state: alpha-blending enabled, no face culling, no depth testing, scissor enabled, polygon fill
            gl.enable(glow::BLEND);
            gl.blend_equation(glow::FUNC_ADD);
            gl.blend_func_separate(
                glow::SRC_ALPHA,
                glow::ONE_MINUS_SRC_ALPHA,
                glow::ONE,
                glow::ONE_MINUS_SRC_ALPHA,
            );
            gl.disable(glow::CULL_FACE);
            gl.disable(glow::DEPTH_TEST);
            gl.disable(glow::STENCIL_TEST);
            gl.enable(glow::SCISSOR_TEST);

            // Optionally enable sRGB frame-buffer writes for sRGB-capable surfaces.
            // Note: This is typically controlled by the application. We expose a toggle
            // for convenience; it will be disabled after rendering to avoid leaking state.
            if self.framebuffer_srgb {
                gl.enable(glow::FRAMEBUFFER_SRGB);
            }

            // Note: We don't enable GL_FRAMEBUFFER_SRGB here because:
            // 1. Modern applications typically create sRGB surfaces directly (e.g., glutin's .with_srgb(true))
            // 2. The official OpenGL3 backend also doesn't explicitly enable GL_FRAMEBUFFER_SRGB
            // 3. Enabling it when the surface is already sRGB would cause incorrect double conversion
            // The sRGB conversion is handled by the surface/framebuffer configuration

            #[cfg(feature = "polygon_mode_support")]
            if self.gl_version.polygon_mode_support() {
                gl.polygon_mode(glow::FRONT_AND_BACK, glow::FILL);
            }

            #[cfg(feature = "primitive_restart_support")]
            if self.gl_version.primitive_restart_support() {
                gl.disable(glow::PRIMITIVE_RESTART);
            }

            // Setup viewport, orthographic projection matrix
            gl.viewport(0, 0, fb_width as i32, fb_height as i32);

            // Calculate projection matrix like the original implementation
            let l = draw_data.display_pos[0];
            let r = draw_data.display_pos[0] + draw_data.display_size[0];
            let t = draw_data.display_pos[1];
            let b = draw_data.display_pos[1] + draw_data.display_size[1];

            // Support for GL 4.5 rarely used glClipControl(GL_UPPER_LEFT)
            #[cfg(feature = "clip_origin_support")]
            let (t, b) = if self.has_clip_origin_support {
                // Check current clip origin
                let clip_origin = gl.get_parameter_i32(glow::CLIP_ORIGIN);
                if clip_origin == glow::UPPER_LEFT as i32 {
                    (b, t) // Swap top and bottom if origin is upper left
                } else {
                    (t, b)
                }
            } else {
                (t, b)
            };

            let ortho_projection = [
                [2.0 / (r - l), 0.0, 0.0, 0.0],
                [0.0, 2.0 / (t - b), 0.0, 0.0],
                [0.0, 0.0, -1.0, 0.0],
                [(r + l) / (l - r), (t + b) / (b - t), 0.0, 1.0],
            ];

            gl.use_program(self.shaders.program);
            if let Some(location) = self.shaders.attrib_location_tex {
                gl.uniform_1_i32(Some(&location), 0);
            }
            if let Some(location) = self.shaders.attrib_location_proj_mtx {
                gl.uniform_matrix_4_f32_slice(Some(&location), false, &ortho_projection.concat());
            }
            if let Some(location) = self.shaders.attrib_location_color_gamma {
                // Decode vertex color from sRGB when writing to sRGB framebuffer,
                // otherwise pass-through (1.0). Allow override if set.
                let gamma = self
                    .color_gamma_override
                    .unwrap_or(if self.framebuffer_srgb {
                        2.2_f32
                    } else {
                        1.0_f32
                    });
                gl.uniform_1_f32(Some(&location), gamma);
            }

            #[cfg(feature = "bind_sampler_support")]
            if self.gl_version.bind_sampler_support() {
                gl.bind_sampler(0, None);
            }

            // Bind vertex/index buffers and setup attributes for ImDrawVert
            gl.bind_buffer(glow::ARRAY_BUFFER, self.vbo_handle);
            gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, self.ebo_handle);
            gl.enable_vertex_attrib_array(self.shaders.attrib_location_vtx_pos);
            gl.enable_vertex_attrib_array(self.shaders.attrib_location_vtx_uv);
            gl.enable_vertex_attrib_array(self.shaders.attrib_location_vtx_color);

            // Use memoffset to calculate correct field offsets, following the original implementation
            let pos_offset = memoffset::offset_of!(DrawVert, pos) as i32;
            let uv_offset = memoffset::offset_of!(DrawVert, uv) as i32;
            let color_offset = memoffset::offset_of!(DrawVert, col) as i32;

            gl.vertex_attrib_pointer_f32(
                self.shaders.attrib_location_vtx_pos,
                2,
                glow::FLOAT,
                false,
                size_of::<DrawVert>() as i32,
                pos_offset,
            );
            gl.vertex_attrib_pointer_f32(
                self.shaders.attrib_location_vtx_uv,
                2,
                glow::FLOAT,
                false,
                size_of::<DrawVert>() as i32,
                uv_offset,
            );
            // Color attribute - our DrawVert uses u32 packed color, so we need to handle it as 4 bytes
            // The u32 is stored as RGBA in little-endian format, so we can treat it as 4 unsigned bytes
            gl.vertex_attrib_pointer_f32(
                self.shaders.attrib_location_vtx_color,
                4,
                glow::UNSIGNED_BYTE,
                true, // normalized = true, converts [0,255] to [0.0,1.0]
                size_of::<DrawVert>() as i32,
                color_offset,
            );
        }

        Ok(())
    }

    /// Render all draw lists
    fn render_draw_lists(
        &mut self,
        gl: &Context,
        texture_map: &dyn TextureMap,
        draw_data: &DrawData,
    ) -> RenderResult<()> {
        gl_debug_message(gl, "start loop over draw lists");

        let mut sampler_filter = glow::LINEAR;

        for draw_list in draw_data.draw_lists() {
            // Upload vertex/index buffers
            self.upload_vertex_buffer(gl, draw_list.vtx_buffer())?;
            self.upload_index_buffer(gl, draw_list.idx_buffer())?;

            gl_debug_message(gl, "start loop over commands");
            for command in draw_list.commands() {
                match command {
                    DrawCmd::Elements {
                        count,
                        cmd_params,
                        raw_cmd,
                    } => {
                        let tex_id_u64 = unsafe {
                            let mut cmd_copy = *raw_cmd;
                            dear_imgui_rs::sys::ImDrawCmd_GetTexID(&mut cmd_copy)
                        } as u64;
                        let tex_id = dear_imgui_rs::TextureId::from(tex_id_u64);
                        self.render_elements(
                            gl,
                            texture_map,
                            count,
                            tex_id,
                            &cmd_params,
                            draw_data,
                            sampler_filter,
                        )?;
                    }
                    DrawCmd::ResetRenderState => {
                        self.set_up_render_state(
                            gl,
                            draw_data,
                            draw_data.display_size[0] * draw_data.framebuffer_scale[0],
                            draw_data.display_size[1] * draw_data.framebuffer_scale[1],
                        )?;
                        sampler_filter = glow::LINEAR;
                    }
                    DrawCmd::SetSamplerLinear => {
                        sampler_filter = glow::LINEAR;
                    }
                    DrawCmd::SetSamplerNearest => {
                        sampler_filter = glow::NEAREST;
                    }
                    DrawCmd::RawCallback { callback, raw_cmd } => {
                        let res =
                            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
                                callback(draw_list.raw(), raw_cmd)
                            }));
                        if res.is_err() {
                            eprintln!("dear-imgui-glow: panic in DrawCmd raw callback");
                            std::process::abort();
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Upload vertex buffer data
    ///
    /// Following the original Dear ImGui OpenGL3 implementation, we always use glBufferData()
    /// instead of glBufferSubData() to avoid issues with Intel GPU drivers.
    /// See: https://github.com/ocornut/imgui/issues/4468
    fn upload_vertex_buffer(&mut self, gl: &Context, vertices: &[DrawVert]) -> RenderResult<()> {
        unsafe {
            gl.bind_buffer(glow::ARRAY_BUFFER, self.vbo_handle);

            // Always use glBufferData() following the original implementation
            // This avoids corruption issues reported with Intel GPU drivers when using glBufferSubData()
            gl.buffer_data_u8_slice(
                glow::ARRAY_BUFFER,
                draw_verts_as_bytes(vertices),
                glow::STREAM_DRAW,
            );
        }

        Ok(())
    }

    /// Upload index buffer data
    ///
    /// Following the original Dear ImGui OpenGL3 implementation, we always use glBufferData()
    /// instead of glBufferSubData() to avoid issues with Intel GPU drivers.
    /// See: https://github.com/ocornut/imgui/issues/4468
    fn upload_index_buffer(
        &mut self,
        gl: &Context,
        indices: &[dear_imgui_rs::render::DrawIdx],
    ) -> RenderResult<()> {
        unsafe {
            gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, self.ebo_handle);

            // Always use glBufferData() following the original implementation
            // This avoids corruption issues reported with Intel GPU drivers when using glBufferSubData()
            gl.buffer_data_u8_slice(
                glow::ELEMENT_ARRAY_BUFFER,
                draw_indices_as_bytes(indices),
                glow::STREAM_DRAW,
            );
        }

        Ok(())
    }

    /// Render elements with the given parameters
    fn render_elements(
        &self,
        gl: &Context,
        texture_map: &dyn TextureMap,
        count: usize,
        effective_tex_id: dear_imgui_rs::TextureId,
        cmd_params: &DrawCmdParams,
        draw_data: &DrawData,
        sampler_filter: u32,
    ) -> RenderResult<()> {
        // Get texture
        let texture = if let Some(tex) = texture_map.get(effective_tex_id) {
            tex
        } else {
            // Use font atlas texture as fallback
            self.font_atlas_texture.ok_or_else(|| {
                RenderError::InvalidTexture(format!("Texture ID {:?} not found", effective_tex_id))
            })?
        };

        unsafe {
            // Bind texture
            gl.bind_texture(glow::TEXTURE_2D, Some(texture));
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MIN_FILTER,
                sampler_filter as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MAG_FILTER,
                sampler_filter as i32,
            );

            // Set scissor rectangle
            let clip_rect = cmd_params.clip_rect;
            let clip_min_x =
                (clip_rect[0] - draw_data.display_pos[0]) * draw_data.framebuffer_scale[0];
            let clip_min_y =
                (clip_rect[1] - draw_data.display_pos[1]) * draw_data.framebuffer_scale[1];
            let clip_max_x =
                (clip_rect[2] - draw_data.display_pos[0]) * draw_data.framebuffer_scale[0];
            let clip_max_y =
                (clip_rect[3] - draw_data.display_pos[1]) * draw_data.framebuffer_scale[1];

            if clip_max_x <= clip_min_x || clip_max_y <= clip_min_y {
                return Ok(());
            }

            // Apply scissor/clipping rectangle (Y is inverted in OpenGL)
            let fb_height = draw_data.display_size[1] * draw_data.framebuffer_scale[1];
            gl.scissor(
                clip_min_x as i32,
                (fb_height - clip_max_y) as i32,
                (clip_max_x - clip_min_x) as i32,
                (clip_max_y - clip_min_y) as i32,
            );

            // Draw - dynamically detect index type like the original implementation
            let idx_offset = cmd_params.idx_offset * size_of::<dear_imgui_rs::render::DrawIdx>();
            let index_type = if size_of::<dear_imgui_rs::render::DrawIdx>() == 2 {
                glow::UNSIGNED_SHORT
            } else {
                glow::UNSIGNED_INT
            };

            #[cfg(feature = "vertex_offset_support")]
            if self.gl_version.vertex_offset_support() {
                gl.draw_elements_base_vertex(
                    glow::TRIANGLES,
                    count as i32,
                    index_type,
                    idx_offset as i32,
                    cmd_params.vtx_offset as i32,
                );
            } else {
                gl.draw_elements(glow::TRIANGLES, count as i32, index_type, idx_offset as i32);
            }

            #[cfg(not(feature = "vertex_offset_support"))]
            gl.draw_elements(glow::TRIANGLES, count as i32, index_type, idx_offset as i32);
        }

        Ok(())
    }
}
