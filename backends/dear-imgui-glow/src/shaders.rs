//! Shader management for Dear ImGui rendering

use crate::{GlProgram, GlUniformLocation, GlVersion, GlslVersion, InitError, InitResult};
use glow::{Context, HasContext};

/// Shader program and uniform locations
pub struct Shaders {
    pub program: Option<GlProgram>,
    pub attrib_location_tex: Option<GlUniformLocation>,
    pub attrib_location_proj_mtx: Option<GlUniformLocation>,
    pub attrib_location_vtx_pos: u32,
    pub attrib_location_vtx_uv: u32,
    pub attrib_location_vtx_color: u32,
}

impl Shaders {
    /// Create and compile shaders
    ///
    /// Following the official OpenGL3 backend approach: uses simple shaders that rely on
    /// OpenGL's GL_FRAMEBUFFER_SRGB for automatic sRGB conversion.
    pub fn new(gl: &Context, gl_version: GlVersion) -> InitResult<Self> {
        let glsl_version = GlslVersion::for_gl_version(gl_version);

        let vertex_shader_source = Self::vertex_shader_source(&glsl_version);
        let fragment_shader_source = Self::fragment_shader_source(&glsl_version);

        unsafe {
            // Create vertex shader
            let vertex_shader = gl
                .create_shader(glow::VERTEX_SHADER)
                .map_err(InitError::CreateShader)?;
            gl.shader_source(vertex_shader, &vertex_shader_source);
            gl.compile_shader(vertex_shader);

            if !gl.get_shader_compile_status(vertex_shader) {
                let error = gl.get_shader_info_log(vertex_shader);
                gl.delete_shader(vertex_shader);
                return Err(InitError::CompileShader(format!(
                    "Vertex shader: {}",
                    error
                )));
            }

            // Create fragment shader
            let fragment_shader = gl
                .create_shader(glow::FRAGMENT_SHADER)
                .map_err(InitError::CreateShader)?;
            gl.shader_source(fragment_shader, &fragment_shader_source);
            gl.compile_shader(fragment_shader);

            if !gl.get_shader_compile_status(fragment_shader) {
                let error = gl.get_shader_info_log(fragment_shader);
                gl.delete_shader(vertex_shader);
                gl.delete_shader(fragment_shader);
                return Err(InitError::CompileShader(format!(
                    "Fragment shader: {}",
                    error
                )));
            }

            // Create program
            let program = gl.create_program().map_err(InitError::CreateShader)?;
            gl.attach_shader(program, vertex_shader);
            gl.attach_shader(program, fragment_shader);
            gl.link_program(program);

            if !gl.get_program_link_status(program) {
                let error = gl.get_program_info_log(program);
                gl.delete_shader(vertex_shader);
                gl.delete_shader(fragment_shader);
                gl.delete_program(program);
                return Err(InitError::LinkProgram(error));
            }

            // Clean up individual shaders
            gl.detach_shader(program, vertex_shader);
            gl.detach_shader(program, fragment_shader);
            gl.delete_shader(vertex_shader);
            gl.delete_shader(fragment_shader);

            // Get uniform locations
            let attrib_location_tex = gl.get_uniform_location(program, "Texture");
            let attrib_location_proj_mtx = gl.get_uniform_location(program, "ProjMtx");

            // Get attribute locations
            let attrib_location_vtx_pos =
                gl.get_attrib_location(program, "Position").ok_or_else(|| {
                    InitError::Generic("Could not find Position attribute".to_string())
                })?;
            let attrib_location_vtx_uv = gl
                .get_attrib_location(program, "UV")
                .ok_or_else(|| InitError::Generic("Could not find UV attribute".to_string()))?;
            let attrib_location_vtx_color = gl
                .get_attrib_location(program, "Color")
                .ok_or_else(|| InitError::Generic("Could not find Color attribute".to_string()))?;

            Ok(Self {
                program: Some(program),
                attrib_location_tex,
                attrib_location_proj_mtx,
                attrib_location_vtx_pos,
                attrib_location_vtx_uv,
                attrib_location_vtx_color,
            })
        }
    }

    /// Generate vertex shader source
    fn vertex_shader_source(glsl_version: &GlslVersion) -> String {
        let version_str = glsl_version.as_str();
        let is_legacy =
            version_str.contains("#version 120") || version_str.contains("#version 100");

        if is_legacy {
            // GLSL 120 and ES 100 use attribute/varying
            format!(
                r#"{version}
{precision}
uniform mat4 ProjMtx;
attribute vec2 Position;
attribute vec2 UV;
attribute vec4 Color;
varying vec2 Frag_UV;
varying vec4 Frag_Color;

void main()
{{
    Frag_UV = UV;
    Frag_Color = Color;
    gl_Position = ProjMtx * vec4(Position.xy, 0, 1);
}}
"#,
                version = version_str,
                precision = if version_str.contains("es") {
                    "precision mediump float;"
                } else {
                    ""
                }
            )
        } else {
            // GLSL 130+ use in/out
            format!(
                r#"{version}
{precision}
uniform mat4 ProjMtx;
in vec2 Position;
in vec2 UV;
in vec4 Color;
out vec2 Frag_UV;
out vec4 Frag_Color;

void main()
{{
    Frag_UV = UV;
    Frag_Color = Color;
    gl_Position = ProjMtx * vec4(Position.xy, 0, 1);
}}
"#,
                version = version_str,
                precision = if version_str.contains("es") {
                    "precision mediump float;"
                } else {
                    ""
                }
            )
        }
    }

    /// Generate fragment shader source
    ///
    /// Following the official OpenGL3 backend approach: simple shader that relies on
    /// OpenGL's GL_FRAMEBUFFER_SRGB for automatic sRGB conversion, rather than
    /// manual shader-based conversion like the WGPU backend.
    fn fragment_shader_source(glsl_version: &GlslVersion) -> String {
        let version_str = glsl_version.as_str();
        let is_legacy =
            version_str.contains("#version 120") || version_str.contains("#version 100");

        if is_legacy {
            // GLSL 120 and ES 100 use gl_FragColor and texture2D
            format!(
                r#"{version}
{precision}
uniform sampler2D Texture;
varying vec2 Frag_UV;
varying vec4 Frag_Color;

void main()
{{
    gl_FragColor = Frag_Color * texture2D(Texture, Frag_UV.st);
}}
"#,
                version = version_str,
                precision = if version_str.contains("es") || version_str.contains("#version 120") {
                    "#ifdef GL_ES\n    precision mediump float;\n#endif"
                } else {
                    ""
                }
            )
        } else {
            // GLSL 130+ use out variables and texture()
            format!(
                r#"{version}
{precision}
uniform sampler2D Texture;
in vec2 Frag_UV;
in vec4 Frag_Color;
out vec4 Out_Color;

void main()
{{
    Out_Color = Frag_Color * texture(Texture, Frag_UV.st);
}}
"#,
                version = version_str,
                precision = if version_str.contains("es") {
                    "precision mediump float;"
                } else {
                    ""
                }
            )
        }
    }
}
