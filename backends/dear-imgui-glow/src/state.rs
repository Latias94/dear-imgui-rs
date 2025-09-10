//! OpenGL state backup and restoration

use crate::{GlBuffer, GlProgram, GlTexture, GlVersion, GlVertexArray};
use glow::{Context, HasContext};

/// OpenGL state backup for proper state restoration
#[derive(Default)]
pub struct GlStateBackup {
    // Blend state
    blend_enabled: bool,
    blend_src_rgb: u32,
    blend_dst_rgb: u32,
    blend_src_alpha: u32,
    blend_dst_alpha: u32,
    blend_equation_rgb: u32,
    blend_equation_alpha: u32,

    // Viewport and scissor
    viewport: [i32; 4],
    scissor_test_enabled: bool,
    scissor_box: [i32; 4],

    // Buffers
    array_buffer_binding: Option<GlBuffer>,
    element_array_buffer_binding: Option<GlBuffer>,

    // Vertex array
    #[cfg(feature = "bind_vertex_array_support")]
    vertex_array_binding: Option<GlVertexArray>,

    // Textures
    active_texture: u32,
    texture_2d_binding: Option<GlTexture>,

    // Shader program
    current_program: Option<GlProgram>,

    // Other state
    cull_face_enabled: bool,
    depth_test_enabled: bool,
    stencil_test_enabled: bool,

    // Polygon mode (desktop OpenGL only)
    #[cfg(feature = "polygon_mode_support")]
    polygon_mode: [i32; 2],

    // Primitive restart (OpenGL 3.1+)
    #[cfg(feature = "primitive_restart_support")]
    primitive_restart_enabled: bool,

    // Sampler binding (OpenGL 3.3+/ES 3.0+)
    #[cfg(feature = "bind_sampler_support")]
    sampler_binding: u32,
}

impl GlStateBackup {
    /// Backup OpenGL state before rendering
    pub fn backup(&mut self, gl: &Context, gl_version: GlVersion) {
        unsafe {
            // Blend state
            self.blend_enabled = gl.is_enabled(glow::BLEND);
            self.blend_src_rgb = gl.get_parameter_i32(glow::BLEND_SRC_RGB) as u32;
            self.blend_dst_rgb = gl.get_parameter_i32(glow::BLEND_DST_RGB) as u32;
            self.blend_src_alpha = gl.get_parameter_i32(glow::BLEND_SRC_ALPHA) as u32;
            self.blend_dst_alpha = gl.get_parameter_i32(glow::BLEND_DST_ALPHA) as u32;
            self.blend_equation_rgb = gl.get_parameter_i32(glow::BLEND_EQUATION_RGB) as u32;
            self.blend_equation_alpha = gl.get_parameter_i32(glow::BLEND_EQUATION_ALPHA) as u32;

            // Viewport and scissor
            let mut viewport = [0i32; 4];
            gl.get_parameter_i32_slice(glow::VIEWPORT, &mut viewport);
            self.viewport.copy_from_slice(&viewport);
            self.scissor_test_enabled = gl.is_enabled(glow::SCISSOR_TEST);
            let mut scissor = [0i32; 4];
            gl.get_parameter_i32_slice(glow::SCISSOR_BOX, &mut scissor);
            self.scissor_box.copy_from_slice(&scissor);

            // Buffers
            let buffer_binding = gl.get_parameter_i32(glow::ARRAY_BUFFER_BINDING);
            self.array_buffer_binding = if buffer_binding == 0 {
                None
            } else {
                Some(glow::NativeBuffer(
                    std::num::NonZeroU32::new(buffer_binding as u32).unwrap(),
                ))
            };
            let element_buffer_binding = gl.get_parameter_i32(glow::ELEMENT_ARRAY_BUFFER_BINDING);
            self.element_array_buffer_binding = if element_buffer_binding == 0 {
                None
            } else {
                Some(glow::NativeBuffer(
                    std::num::NonZeroU32::new(element_buffer_binding as u32).unwrap(),
                ))
            };

            // Vertex array
            #[cfg(feature = "bind_vertex_array_support")]
            if gl_version.bind_vertex_array_support() {
                let vao_binding = gl.get_parameter_i32(glow::VERTEX_ARRAY_BINDING);
                self.vertex_array_binding = if vao_binding == 0 {
                    None
                } else {
                    Some(glow::NativeVertexArray(
                        std::num::NonZeroU32::new(vao_binding as u32).unwrap(),
                    ))
                };
            }

            // Textures
            self.active_texture = gl.get_parameter_i32(glow::ACTIVE_TEXTURE) as u32;
            let texture_binding = gl.get_parameter_i32(glow::TEXTURE_BINDING_2D);
            self.texture_2d_binding = if texture_binding == 0 {
                None
            } else {
                Some(glow::NativeTexture(
                    std::num::NonZeroU32::new(texture_binding as u32).unwrap(),
                ))
            };

            // Shader program
            let program_binding = gl.get_parameter_i32(glow::CURRENT_PROGRAM);
            self.current_program = if program_binding == 0 {
                None
            } else {
                Some(glow::NativeProgram(
                    std::num::NonZeroU32::new(program_binding as u32).unwrap(),
                ))
            };

            // Other state
            self.cull_face_enabled = gl.is_enabled(glow::CULL_FACE);
            self.depth_test_enabled = gl.is_enabled(glow::DEPTH_TEST);
            self.stencil_test_enabled = gl.is_enabled(glow::STENCIL_TEST);

            // Polygon mode (desktop OpenGL only)
            #[cfg(feature = "polygon_mode_support")]
            if gl_version.polygon_mode_support() {
                let mut polygon_mode = [0i32; 2];
                gl.get_parameter_i32_slice(glow::POLYGON_MODE, &mut polygon_mode);
                self.polygon_mode.copy_from_slice(&polygon_mode);
            }

            // Primitive restart
            #[cfg(feature = "primitive_restart_support")]
            if gl_version.primitive_restart_support() {
                self.primitive_restart_enabled = gl.is_enabled(glow::PRIMITIVE_RESTART);
            }

            // Sampler binding
            #[cfg(feature = "bind_sampler_support")]
            if gl_version.bind_sampler_support() {
                self.sampler_binding = gl.get_parameter_i32(glow::SAMPLER_BINDING) as u32;
            }
        }
    }

    /// Restore OpenGL state after rendering
    pub fn restore(&self, gl: &Context, gl_version: GlVersion) {
        unsafe {
            // Restore blend state
            if self.blend_enabled {
                gl.enable(glow::BLEND);
            } else {
                gl.disable(glow::BLEND);
            }
            gl.blend_func_separate(
                self.blend_src_rgb,
                self.blend_dst_rgb,
                self.blend_src_alpha,
                self.blend_dst_alpha,
            );
            gl.blend_equation_separate(self.blend_equation_rgb, self.blend_equation_alpha);

            // Restore viewport and scissor
            gl.viewport(
                self.viewport[0],
                self.viewport[1],
                self.viewport[2],
                self.viewport[3],
            );
            if self.scissor_test_enabled {
                gl.enable(glow::SCISSOR_TEST);
            } else {
                gl.disable(glow::SCISSOR_TEST);
            }
            gl.scissor(
                self.scissor_box[0],
                self.scissor_box[1],
                self.scissor_box[2],
                self.scissor_box[3],
            );

            // Restore buffers
            gl.bind_buffer(glow::ARRAY_BUFFER, self.array_buffer_binding);
            gl.bind_buffer(
                glow::ELEMENT_ARRAY_BUFFER,
                self.element_array_buffer_binding,
            );

            // Restore vertex array
            #[cfg(feature = "bind_vertex_array_support")]
            if gl_version.bind_vertex_array_support() {
                gl.bind_vertex_array(self.vertex_array_binding);
            }

            // Restore textures
            gl.active_texture(self.active_texture);
            gl.bind_texture(glow::TEXTURE_2D, self.texture_2d_binding);

            // Restore shader program
            gl.use_program(self.current_program);

            // Restore other state
            if self.cull_face_enabled {
                gl.enable(glow::CULL_FACE);
            } else {
                gl.disable(glow::CULL_FACE);
            }
            if self.depth_test_enabled {
                gl.enable(glow::DEPTH_TEST);
            } else {
                gl.disable(glow::DEPTH_TEST);
            }
            if self.stencil_test_enabled {
                gl.enable(glow::STENCIL_TEST);
            } else {
                gl.disable(glow::STENCIL_TEST);
            }

            // Restore polygon mode
            #[cfg(feature = "polygon_mode_support")]
            if gl_version.polygon_mode_support() {
                gl.polygon_mode(glow::FRONT_AND_BACK, self.polygon_mode[0] as u32);
            }

            // Restore primitive restart
            #[cfg(feature = "primitive_restart_support")]
            if gl_version.primitive_restart_support() {
                if self.primitive_restart_enabled {
                    gl.enable(glow::PRIMITIVE_RESTART);
                } else {
                    gl.disable(glow::PRIMITIVE_RESTART);
                }
            }

            // Restore sampler binding
            #[cfg(feature = "bind_sampler_support")]
            if gl_version.bind_sampler_support() {
                gl.bind_sampler(0, None); // Reset to default
            }
        }
    }
}
