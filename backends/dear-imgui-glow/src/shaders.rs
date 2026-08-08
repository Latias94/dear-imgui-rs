//! Shader management for Dear ImGui rendering

use crate::{
    GlProgram, GlShader, GlUniformLocation, GlVersion, GlslVersion, InitError, InitResult,
};
use glow::{Context, HasContext};

struct PendingShader<'a> {
    gl: &'a Context,
    shader: Option<GlShader>,
}

impl<'a> PendingShader<'a> {
    fn create(gl: &'a Context, shader_type: u32) -> InitResult<Self> {
        let shader = unsafe { gl.create_shader(shader_type) }.map_err(InitError::CreateShader)?;
        Ok(Self {
            gl,
            shader: Some(shader),
        })
    }

    fn handle(&self) -> GlShader {
        self.shader.expect("pending shader must own a handle")
    }
}

impl Drop for PendingShader<'_> {
    fn drop(&mut self) {
        if let Some(shader) = self.shader.take() {
            unsafe { self.gl.delete_shader(shader) };
        }
    }
}

struct PendingProgram<'a> {
    gl: &'a Context,
    program: Option<GlProgram>,
}

impl<'a> PendingProgram<'a> {
    fn create(gl: &'a Context) -> InitResult<Self> {
        let program = unsafe { gl.create_program() }.map_err(InitError::CreateShader)?;
        Ok(Self {
            gl,
            program: Some(program),
        })
    }

    fn handle(&self) -> GlProgram {
        self.program.expect("pending program must own a handle")
    }

    fn commit(mut self) -> GlProgram {
        self.program
            .take()
            .expect("pending program must own a handle")
    }
}

impl Drop for PendingProgram<'_> {
    fn drop(&mut self) {
        if let Some(program) = self.program.take() {
            unsafe { self.gl.delete_program(program) };
        }
    }
}

/// Shader program and uniform locations
pub struct Shaders {
    pub program: Option<GlProgram>,
    pub attrib_location_tex: Option<GlUniformLocation>,
    pub attrib_location_proj_mtx: Option<GlUniformLocation>,
    pub attrib_location_color_gamma: Option<GlUniformLocation>,
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
            let vertex_shader = PendingShader::create(gl, glow::VERTEX_SHADER)?;
            gl.shader_source(vertex_shader.handle(), &vertex_shader_source);
            gl.compile_shader(vertex_shader.handle());

            if !gl.get_shader_compile_status(vertex_shader.handle()) {
                let error = gl.get_shader_info_log(vertex_shader.handle());
                return Err(InitError::CompileShader(format!(
                    "Vertex shader: {}",
                    error
                )));
            }

            let fragment_shader = PendingShader::create(gl, glow::FRAGMENT_SHADER)?;
            gl.shader_source(fragment_shader.handle(), &fragment_shader_source);
            gl.compile_shader(fragment_shader.handle());

            if !gl.get_shader_compile_status(fragment_shader.handle()) {
                let error = gl.get_shader_info_log(fragment_shader.handle());
                return Err(InitError::CompileShader(format!(
                    "Fragment shader: {}",
                    error
                )));
            }

            let program = PendingProgram::create(gl)?;
            gl.attach_shader(program.handle(), vertex_shader.handle());
            gl.attach_shader(program.handle(), fragment_shader.handle());
            gl.link_program(program.handle());

            if !gl.get_program_link_status(program.handle()) {
                let error = gl.get_program_info_log(program.handle());
                return Err(InitError::LinkProgram(error));
            }

            gl.detach_shader(program.handle(), vertex_shader.handle());
            gl.detach_shader(program.handle(), fragment_shader.handle());

            // Get uniform locations
            let attrib_location_tex = gl.get_uniform_location(program.handle(), "Texture");
            let attrib_location_proj_mtx = gl.get_uniform_location(program.handle(), "ProjMtx");
            let attrib_location_color_gamma =
                gl.get_uniform_location(program.handle(), "ColorGamma");

            // Get attribute locations
            let attrib_location_vtx_pos = gl
                .get_attrib_location(program.handle(), "Position")
                .ok_or(InitError::MissingShaderAttribute("Position"))?;
            let attrib_location_vtx_uv = gl
                .get_attrib_location(program.handle(), "UV")
                .ok_or(InitError::MissingShaderAttribute("UV"))?;
            let attrib_location_vtx_color = gl
                .get_attrib_location(program.handle(), "Color")
                .ok_or(InitError::MissingShaderAttribute("Color"))?;

            Ok(Self {
                program: Some(program.commit()),
                attrib_location_tex,
                attrib_location_proj_mtx,
                attrib_location_color_gamma,
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
uniform float ColorGamma;

void main()
{{
    vec4 col = Frag_Color;
    col.rgb = pow(col.rgb, vec3(ColorGamma));
    gl_FragColor = col * texture2D(Texture, Frag_UV.st);
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
uniform float ColorGamma;

void main()
{{
    vec4 col = Frag_Color;
    col.rgb = pow(col.rgb, vec3(ColorGamma));
    Out_Color = col * texture(Texture, Frag_UV.st);
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

#[cfg(test)]
pub(crate) mod test_support {
    use std::ffi::{CStr, c_char};
    use std::sync::Mutex;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(crate) enum FakeFailure {
        None,
        FragmentShaderCreate,
        ProgramCreate,
        MissingAttribute,
        BufferCreate(u32),
        SamplerCreate(u32),
    }

    #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
    pub(crate) struct FakeSnapshot {
        pub(crate) deleted_shaders: u32,
        pub(crate) deleted_programs: u32,
        pub(crate) deleted_buffers: u32,
        pub(crate) generated_buffers: u32,
        pub(crate) deleted_samplers: u32,
        pub(crate) generated_samplers: u32,
        pub(crate) deleted_textures: u32,
        pub(crate) generated_textures: u32,
    }

    #[derive(Clone, Copy, Debug)]
    struct FakeState {
        failure: FakeFailure,
        created_shaders: u32,
        buffer_calls: u32,
        sampler_calls: u32,
        snapshot: FakeSnapshot,
    }

    impl FakeState {
        const DEFAULT: Self = Self {
            failure: FakeFailure::None,
            created_shaders: 0,
            buffer_calls: 0,
            sampler_calls: 0,
            snapshot: FakeSnapshot {
                deleted_shaders: 0,
                deleted_programs: 0,
                deleted_buffers: 0,
                generated_buffers: 0,
                deleted_samplers: 0,
                generated_samplers: 0,
                deleted_textures: 0,
                generated_textures: 0,
            },
        };
    }

    static FAKE_STATE: Mutex<FakeState> = Mutex::new(FakeState::DEFAULT);
    pub(crate) static TEST_LOCK: Mutex<()> = Mutex::new(());

    pub(crate) fn reset(failure: FakeFailure) {
        *FAKE_STATE.lock().unwrap() = FakeState {
            failure,
            ..FakeState::DEFAULT
        };
    }

    pub(crate) fn snapshot() -> FakeSnapshot {
        FAKE_STATE.lock().unwrap().snapshot
    }

    unsafe extern "system" fn get_string(name: u32) -> *const u8 {
        match name {
            glow::VERSION => c"4.6".as_ptr().cast(),
            _ => c"".as_ptr().cast(),
        }
    }

    unsafe extern "system" fn get_string_i(_name: u32, _index: u32) -> *const u8 {
        c"".as_ptr().cast()
    }

    unsafe extern "system" fn get_integer(_name: u32, value: *mut i32) {
        if !value.is_null() {
            unsafe { *value = 0 };
        }
    }

    unsafe extern "system" fn create_shader(_shader_type: u32) -> u32 {
        let mut state = FAKE_STATE.lock().unwrap();
        state.created_shaders += 1;
        if state.failure == FakeFailure::FragmentShaderCreate && state.created_shaders == 2 {
            0
        } else {
            state.created_shaders
        }
    }

    unsafe extern "system" fn delete_shader(_shader: u32) {
        FAKE_STATE.lock().unwrap().snapshot.deleted_shaders += 1;
    }

    unsafe extern "system" fn shader_source(
        _shader: u32,
        _count: i32,
        _source: *const *const c_char,
        _length: *const i32,
    ) {
    }

    unsafe extern "system" fn compile_shader(_shader: u32) {}

    unsafe extern "system" fn get_shader_iv(_shader: u32, parameter: u32, value: *mut i32) {
        if !value.is_null() {
            unsafe {
                *value = if parameter == glow::COMPILE_STATUS {
                    1
                } else {
                    0
                };
            }
        }
    }

    unsafe extern "system" fn create_program() -> u32 {
        if FAKE_STATE.lock().unwrap().failure == FakeFailure::ProgramCreate {
            0
        } else {
            10
        }
    }

    unsafe extern "system" fn delete_program(_program: u32) {
        FAKE_STATE.lock().unwrap().snapshot.deleted_programs += 1;
    }

    unsafe extern "system" fn attach_shader(_program: u32, _shader: u32) {}
    unsafe extern "system" fn detach_shader(_program: u32, _shader: u32) {}
    unsafe extern "system" fn link_program(_program: u32) {}

    unsafe extern "system" fn get_program_iv(_program: u32, parameter: u32, value: *mut i32) {
        if !value.is_null() {
            unsafe {
                *value = if parameter == glow::LINK_STATUS { 1 } else { 0 };
            }
        }
    }

    unsafe extern "system" fn get_uniform_location(_program: u32, _name: *const c_char) -> i32 {
        -1
    }

    unsafe extern "system" fn get_attrib_location(_program: u32, name: *const c_char) -> i32 {
        let name = unsafe { CStr::from_ptr(name) }.to_bytes();
        let state = FAKE_STATE.lock().unwrap();
        if state.failure == FakeFailure::MissingAttribute && name == b"UV" {
            -1
        } else {
            match name {
                b"Position" => 0,
                b"UV" => 1,
                b"Color" => 2,
                _ => -1,
            }
        }
    }

    unsafe extern "system" fn gen_buffers(count: i32, buffers: *mut u32) {
        let mut state = FAKE_STATE.lock().unwrap();
        for index in 0..count.max(0) as usize {
            state.buffer_calls += 1;
            let failed = matches!(state.failure, FakeFailure::BufferCreate(call) if call == state.buffer_calls);
            let buffer = if failed { 0 } else { 20 + state.buffer_calls };
            if buffer != 0 {
                state.snapshot.generated_buffers += 1;
            }
            unsafe { *buffers.add(index) = buffer };
        }
    }

    unsafe extern "system" fn delete_buffers(count: i32, _buffers: *const u32) {
        FAKE_STATE.lock().unwrap().snapshot.deleted_buffers += count.max(0) as u32;
    }

    unsafe extern "system" fn gen_samplers(count: i32, samplers: *mut u32) {
        let mut state = FAKE_STATE.lock().unwrap();
        for index in 0..count.max(0) as usize {
            state.sampler_calls += 1;
            let failed = matches!(
                state.failure,
                FakeFailure::SamplerCreate(call) if call == state.sampler_calls
            );
            let sampler = if failed { 0 } else { 30 + state.sampler_calls };
            if sampler != 0 {
                state.snapshot.generated_samplers += 1;
            }
            unsafe { *samplers.add(index) = sampler };
        }
    }

    unsafe extern "system" fn delete_samplers(count: i32, _samplers: *const u32) {
        FAKE_STATE.lock().unwrap().snapshot.deleted_samplers += count.max(0) as u32;
    }

    unsafe extern "system" fn sampler_parameter_i(_sampler: u32, _parameter: u32, _value: i32) {}

    unsafe extern "system" fn gen_textures(count: i32, textures: *mut u32) {
        let mut state = FAKE_STATE.lock().unwrap();
        for index in 0..count.max(0) as usize {
            state.snapshot.generated_textures += 1;
            unsafe { *textures.add(index) = 40 + state.snapshot.generated_textures };
        }
    }

    unsafe extern "system" fn delete_textures(count: i32, _textures: *const u32) {
        FAKE_STATE.lock().unwrap().snapshot.deleted_textures += count.max(0) as u32;
    }

    unsafe extern "system" fn active_texture(_texture: u32) {}
    unsafe extern "system" fn bind_texture(_target: u32, _texture: u32) {}
    unsafe extern "system" fn bind_buffer(_target: u32, _buffer: u32) {}
    unsafe extern "system" fn pixel_store_i(_parameter: u32, _value: i32) {}
    unsafe extern "system" fn tex_parameter_i(_target: u32, _parameter: u32, _value: i32) {}
    unsafe extern "system" fn tex_image_2d(
        _target: u32,
        _level: i32,
        _internal_format: i32,
        _width: i32,
        _height: i32,
        _border: i32,
        _format: u32,
        _pixel_type: u32,
        _pixels: *const std::ffi::c_void,
    ) {
    }

    pub(crate) fn fake_gl() -> glow::Context {
        unsafe {
            glow::Context::from_loader_function(|name| {
                match name {
                    "glGetString" => get_string as *const (),
                    "glGetStringi" => get_string_i as *const (),
                    "glGetIntegerv" => get_integer as *const (),
                    "glCreateShader" => create_shader as *const (),
                    "glDeleteShader" => delete_shader as *const (),
                    "glShaderSource" => shader_source as *const (),
                    "glCompileShader" => compile_shader as *const (),
                    "glGetShaderiv" => get_shader_iv as *const (),
                    "glCreateProgram" => create_program as *const (),
                    "glDeleteProgram" => delete_program as *const (),
                    "glAttachShader" => attach_shader as *const (),
                    "glDetachShader" => detach_shader as *const (),
                    "glLinkProgram" => link_program as *const (),
                    "glGetProgramiv" => get_program_iv as *const (),
                    "glGetUniformLocation" => get_uniform_location as *const (),
                    "glGetAttribLocation" => get_attrib_location as *const (),
                    "glGenBuffers" => gen_buffers as *const (),
                    "glDeleteBuffers" => delete_buffers as *const (),
                    "glGenSamplers" => gen_samplers as *const (),
                    "glDeleteSamplers" => delete_samplers as *const (),
                    "glSamplerParameteri" => sampler_parameter_i as *const (),
                    "glGenTextures" => gen_textures as *const (),
                    "glDeleteTextures" => delete_textures as *const (),
                    "glActiveTexture" => active_texture as *const (),
                    "glBindTexture" => bind_texture as *const (),
                    "glBindBuffer" => bind_buffer as *const (),
                    "glPixelStorei" => pixel_store_i as *const (),
                    "glTexParameteri" => tex_parameter_i as *const (),
                    "glTexImage2D" => tex_image_2d as *const (),
                    _ => std::ptr::null(),
                }
                .cast()
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use glow::HasContext;

    use super::{Shaders, test_support::*};
    use crate::{GlVersion, InitError};

    fn version() -> GlVersion {
        GlVersion {
            major: 3,
            minor: 3,
            is_es: false,
        }
    }

    #[test]
    fn shader_creation_failures_release_every_owned_resource() {
        let _guard = TEST_LOCK.lock().unwrap();
        for (failure, expected_error, expected) in [
            (
                FakeFailure::FragmentShaderCreate,
                "create",
                FakeSnapshot {
                    deleted_shaders: 1,
                    ..FakeSnapshot::default()
                },
            ),
            (
                FakeFailure::ProgramCreate,
                "create",
                FakeSnapshot {
                    deleted_shaders: 2,
                    ..FakeSnapshot::default()
                },
            ),
            (
                FakeFailure::MissingAttribute,
                "attribute",
                FakeSnapshot {
                    deleted_shaders: 2,
                    deleted_programs: 1,
                    ..FakeSnapshot::default()
                },
            ),
        ] {
            reset(failure);
            let gl = fake_gl();
            let error = match Shaders::new(&gl, version()) {
                Ok(_) => panic!("injected shader failure unexpectedly succeeded"),
                Err(error) => error,
            };
            match expected_error {
                "create" => assert!(matches!(error, InitError::CreateShader(_))),
                "attribute" => {
                    assert!(matches!(error, InitError::MissingShaderAttribute("UV")))
                }
                _ => unreachable!(),
            }
            assert_eq!(snapshot(), expected);
        }

        reset(FakeFailure::None);
        let gl = fake_gl();
        let mut shaders = Shaders::new(&gl, version()).unwrap();
        assert_eq!(snapshot().deleted_shaders, 2);
        unsafe { gl.delete_program(shaders.program.take().unwrap()) };
        assert_eq!(snapshot().deleted_programs, 1);
    }
}
