//! OpenGL state backup and restoration

#[cfg(feature = "bind_vertex_array_support")]
use crate::GlVertexArray;
use crate::{GlBuffer, GlProgram, GlTexture, GlVersion};
use glow::{Context, HasContext};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FramebufferSrgbTransition {
    enable_on_enter: bool,
    disable_on_exit: bool,
}

impl FramebufferSrgbTransition {
    fn new(requested: bool, was_enabled: bool) -> Self {
        let changed = requested && !was_enabled;
        Self {
            enable_on_enter: changed,
            disable_on_exit: changed,
        }
    }
}

/// Temporarily enables framebuffer sRGB without changing pre-existing state.
pub(crate) struct FramebufferSrgbScope<'a> {
    gl: &'a Context,
    disable_on_drop: bool,
}

impl<'a> FramebufferSrgbScope<'a> {
    pub(crate) fn enter(gl: &'a Context, requested: bool) -> Self {
        let was_enabled = requested && unsafe { gl.is_enabled(glow::FRAMEBUFFER_SRGB) };
        let transition = FramebufferSrgbTransition::new(requested, was_enabled);
        if transition.enable_on_enter {
            unsafe { gl.enable(glow::FRAMEBUFFER_SRGB) };
        }
        Self {
            gl,
            disable_on_drop: transition.disable_on_exit,
        }
    }
}

impl Drop for FramebufferSrgbScope<'_> {
    fn drop(&mut self) {
        if self.disable_on_drop {
            unsafe { self.gl.disable(glow::FRAMEBUFFER_SRGB) };
        }
    }
}

/// OpenGL state backup for proper state restoration
#[derive(Clone, Default)]
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
    clear_color: [f32; 4],
    color_write_mask: [bool; 4],

    // Buffers
    array_buffer_binding: Option<GlBuffer>,
    element_array_buffer_binding: Option<GlBuffer>,

    // Vertex array
    #[cfg(feature = "bind_vertex_array_support")]
    vertex_array_binding: Option<GlVertexArray>,

    // Textures
    active_texture: u32,
    texture_2d_binding_unit_0: Option<GlTexture>,

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
        let _ = gl_version;
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
            gl.get_parameter_f32_slice(glow::COLOR_CLEAR_VALUE, &mut self.clear_color);
            self.color_write_mask = gl.get_parameter_bool_array(glow::COLOR_WRITEMASK);

            // Buffers
            let buffer_binding = gl.get_parameter_i32(glow::ARRAY_BUFFER_BINDING);
            self.array_buffer_binding = u32::try_from(buffer_binding)
                .ok()
                .and_then(std::num::NonZeroU32::new)
                .map(glow::NativeBuffer);
            let element_buffer_binding = gl.get_parameter_i32(glow::ELEMENT_ARRAY_BUFFER_BINDING);
            self.element_array_buffer_binding = u32::try_from(element_buffer_binding)
                .ok()
                .and_then(std::num::NonZeroU32::new)
                .map(glow::NativeBuffer);

            // Vertex array
            #[cfg(feature = "bind_vertex_array_support")]
            if gl_version.bind_vertex_array_support() {
                let vao_binding = gl.get_parameter_i32(glow::VERTEX_ARRAY_BINDING);
                self.vertex_array_binding = u32::try_from(vao_binding)
                    .ok()
                    .and_then(std::num::NonZeroU32::new)
                    .map(glow::NativeVertexArray);
            }

            // Textures
            self.active_texture = u32::try_from(gl.get_parameter_i32(glow::ACTIVE_TEXTURE))
                .ok()
                .unwrap_or(glow::TEXTURE0);
            gl.active_texture(glow::TEXTURE0);
            let texture_binding = gl.get_parameter_i32(glow::TEXTURE_BINDING_2D);
            self.texture_2d_binding_unit_0 = u32::try_from(texture_binding)
                .ok()
                .and_then(std::num::NonZeroU32::new)
                .map(glow::NativeTexture);

            // Sampler binding
            #[cfg(feature = "bind_sampler_support")]
            if gl_version.bind_sampler_support() {
                self.sampler_binding = u32::try_from(gl.get_parameter_i32(glow::SAMPLER_BINDING))
                    .ok()
                    .unwrap_or(0);
            }
            gl.active_texture(self.active_texture);

            // Shader program
            let program_binding = gl.get_parameter_i32(glow::CURRENT_PROGRAM);
            self.current_program = u32::try_from(program_binding)
                .ok()
                .and_then(std::num::NonZeroU32::new)
                .map(glow::NativeProgram);

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
        }
    }

    /// Restore OpenGL state after rendering
    pub fn restore(&self, gl: &Context, gl_version: GlVersion) {
        let _ = gl_version;
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
            gl.clear_color(
                self.clear_color[0],
                self.clear_color[1],
                self.clear_color[2],
                self.clear_color[3],
            );
            gl.color_mask(
                self.color_write_mask[0],
                self.color_write_mask[1],
                self.color_write_mask[2],
                self.color_write_mask[3],
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
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, self.texture_2d_binding_unit_0);

            #[cfg(feature = "bind_sampler_support")]
            if gl_version.bind_sampler_support() {
                let sampler =
                    std::num::NonZeroU32::new(self.sampler_binding).map(glow::NativeSampler);
                gl.bind_sampler(0, sampler);
            }
            gl.active_texture(self.active_texture);

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
        }
    }
}

/// Restores all renderer-visible OpenGL state when the current scope exits.
pub(crate) struct GlStateGuard<'a> {
    gl: &'a Context,
    gl_version: GlVersion,
    backup: GlStateBackup,
}

impl<'a> GlStateGuard<'a> {
    pub(crate) fn capture(gl: &'a Context, gl_version: GlVersion) -> Self {
        let mut backup = GlStateBackup::default();
        backup.backup(gl, gl_version);
        Self {
            gl,
            gl_version,
            backup,
        }
    }
}

impl Drop for GlStateGuard<'_> {
    fn drop(&mut self) {
        self.backup.restore(self.gl, self.gl_version);
    }
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};
    use std::sync::Mutex;

    use glow::HasContext;

    use super::{FramebufferSrgbScope, FramebufferSrgbTransition, GlStateGuard};
    use crate::GlVersion;

    thread_local! {
        static FRAMEBUFFER_SRGB_ENABLED: Cell<bool> = const { Cell::new(false) };
        static GL_EVENTS: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
    }

    unsafe extern "system" fn get_string(_name: u32) -> *const u8 {
        c"4.6".as_ptr().cast()
    }

    unsafe extern "system" fn get_string_i(_name: u32, _index: u32) -> *const u8 {
        c"".as_ptr().cast()
    }

    unsafe extern "system" fn get_integer(_name: u32, value: *mut i32) {
        if !value.is_null() {
            unsafe { *value = 0 };
        }
    }

    unsafe extern "system" fn is_enabled(capability: u32) -> u8 {
        assert_eq!(capability, glow::FRAMEBUFFER_SRGB);
        GL_EVENTS.with(|events| events.borrow_mut().push("query"));
        FRAMEBUFFER_SRGB_ENABLED.with(Cell::get).into()
    }

    unsafe extern "system" fn enable(capability: u32) {
        assert_eq!(capability, glow::FRAMEBUFFER_SRGB);
        GL_EVENTS.with(|events| events.borrow_mut().push("enable"));
        FRAMEBUFFER_SRGB_ENABLED.with(|enabled| enabled.set(true));
    }

    unsafe extern "system" fn disable(capability: u32) {
        assert_eq!(capability, glow::FRAMEBUFFER_SRGB);
        GL_EVENTS.with(|events| events.borrow_mut().push("disable"));
        FRAMEBUFFER_SRGB_ENABLED.with(|enabled| enabled.set(false));
    }

    fn fake_gl() -> glow::Context {
        unsafe {
            glow::Context::from_loader_function(|name| {
                match name {
                    "glGetString" => get_string as *const (),
                    "glGetStringi" => get_string_i as *const (),
                    "glGetIntegerv" => get_integer as *const (),
                    "glIsEnabled" => is_enabled as *const (),
                    "glEnable" => enable as *const (),
                    "glDisable" => disable as *const (),
                    _ => std::ptr::null(),
                }
                .cast()
            })
        }
    }

    fn reset_gl_state(enabled: bool) {
        FRAMEBUFFER_SRGB_ENABLED.with(|state| state.set(enabled));
        GL_EVENTS.with(|events| events.borrow_mut().clear());
    }

    fn take_gl_events() -> Vec<&'static str> {
        GL_EVENTS.with(|events| std::mem::take(&mut *events.borrow_mut()))
    }

    #[test]
    fn framebuffer_srgb_transition_only_reverts_state_changed_by_the_scope() {
        for (requested, was_enabled, expected) in [
            (
                false,
                false,
                FramebufferSrgbTransition {
                    enable_on_enter: false,
                    disable_on_exit: false,
                },
            ),
            (
                false,
                true,
                FramebufferSrgbTransition {
                    enable_on_enter: false,
                    disable_on_exit: false,
                },
            ),
            (
                true,
                false,
                FramebufferSrgbTransition {
                    enable_on_enter: true,
                    disable_on_exit: true,
                },
            ),
            (
                true,
                true,
                FramebufferSrgbTransition {
                    enable_on_enter: false,
                    disable_on_exit: false,
                },
            ),
        ] {
            assert_eq!(
                FramebufferSrgbTransition::new(requested, was_enabled),
                expected
            );
        }
    }

    #[test]
    fn framebuffer_srgb_scope_enables_before_work_and_restores_afterward() {
        reset_gl_state(false);
        let gl = fake_gl();

        {
            let _scope = FramebufferSrgbScope::enter(&gl, true);
            GL_EVENTS.with(|events| events.borrow_mut().push("clear-and-draw"));
            assert!(FRAMEBUFFER_SRGB_ENABLED.with(Cell::get));
        }

        assert!(!FRAMEBUFFER_SRGB_ENABLED.with(Cell::get));
        assert_eq!(
            take_gl_events(),
            ["query", "enable", "clear-and-draw", "disable"]
        );
    }

    #[test]
    fn framebuffer_srgb_scope_preserves_preexisting_enabled_state() {
        reset_gl_state(true);
        let gl = fake_gl();

        {
            let _scope = FramebufferSrgbScope::enter(&gl, true);
            GL_EVENTS.with(|events| events.borrow_mut().push("clear-and-draw"));
        }

        assert!(FRAMEBUFFER_SRGB_ENABLED.with(Cell::get));
        assert_eq!(take_gl_events(), ["query", "clear-and-draw"]);
    }

    #[derive(Clone, Copy, Debug, PartialEq)]
    struct StatefulGl {
        active_texture: u32,
        texture_bindings: [u32; 4],
        sampler_binding_unit_0: u32,
        clear_color: [f32; 4],
        color_write_mask: [bool; 4],
        scissor_enabled: bool,
        scissor_box: [i32; 4],
    }

    impl StatefulGl {
        const ORIGINAL: Self = Self {
            active_texture: glow::TEXTURE0 + 3,
            texture_bindings: [11, 0, 0, 33],
            sampler_binding_unit_0: 17,
            clear_color: [0.1, 0.2, 0.3, 0.4],
            color_write_mask: [false, true, false, true],
            scissor_enabled: true,
            scissor_box: [3, 4, 50, 60],
        };

        fn active_unit_index(self) -> usize {
            usize::try_from(self.active_texture - glow::TEXTURE0).unwrap()
        }
    }

    static STATEFUL_GL: Mutex<StatefulGl> = Mutex::new(StatefulGl::ORIGINAL);

    unsafe extern "system" fn stateful_get_integer(name: u32, value: *mut i32) {
        if value.is_null() {
            return;
        }
        let state = *STATEFUL_GL.lock().unwrap();
        match name {
            glow::VIEWPORT => unsafe {
                std::ptr::copy_nonoverlapping([1, 2, 640, 480].as_ptr(), value, 4)
            },
            glow::SCISSOR_BOX => unsafe {
                std::ptr::copy_nonoverlapping(state.scissor_box.as_ptr(), value, 4)
            },
            glow::COLOR_WRITEMASK => {
                let mask = state.color_write_mask.map(i32::from);
                unsafe { std::ptr::copy_nonoverlapping(mask.as_ptr(), value, 4) };
            }
            glow::POLYGON_MODE => unsafe {
                std::ptr::copy_nonoverlapping(
                    [glow::FILL as i32, glow::FILL as i32].as_ptr(),
                    value,
                    2,
                )
            },
            glow::ACTIVE_TEXTURE => unsafe { *value = state.active_texture as i32 },
            glow::TEXTURE_BINDING_2D => unsafe {
                *value = state.texture_bindings[state.active_unit_index()] as i32
            },
            glow::SAMPLER_BINDING => unsafe { *value = state.sampler_binding_unit_0 as i32 },
            _ => unsafe { *value = 0 },
        }
    }

    unsafe extern "system" fn stateful_get_float(name: u32, value: *mut f32) {
        if name == glow::COLOR_CLEAR_VALUE && !value.is_null() {
            let state = *STATEFUL_GL.lock().unwrap();
            unsafe { std::ptr::copy_nonoverlapping(state.clear_color.as_ptr(), value, 4) };
        }
    }

    unsafe extern "system" fn stateful_get_boolean(name: u32, value: *mut u8) {
        if name == glow::COLOR_WRITEMASK && !value.is_null() {
            let mask = STATEFUL_GL.lock().unwrap().color_write_mask.map(u8::from);
            unsafe { std::ptr::copy_nonoverlapping(mask.as_ptr(), value, 4) };
        }
    }

    unsafe extern "system" fn stateful_is_enabled(capability: u32) -> u8 {
        (capability == glow::SCISSOR_TEST && STATEFUL_GL.lock().unwrap().scissor_enabled).into()
    }

    unsafe extern "system" fn stateful_enable(capability: u32) {
        if capability == glow::SCISSOR_TEST {
            STATEFUL_GL.lock().unwrap().scissor_enabled = true;
        }
    }

    unsafe extern "system" fn stateful_disable(capability: u32) {
        if capability == glow::SCISSOR_TEST {
            STATEFUL_GL.lock().unwrap().scissor_enabled = false;
        }
    }

    unsafe extern "system" fn stateful_active_texture(texture: u32) {
        STATEFUL_GL.lock().unwrap().active_texture = texture;
    }

    unsafe extern "system" fn stateful_bind_texture(_target: u32, texture: u32) {
        let mut state = STATEFUL_GL.lock().unwrap();
        let unit = state.active_unit_index();
        state.texture_bindings[unit] = texture;
    }

    unsafe extern "system" fn stateful_bind_sampler(unit: u32, sampler: u32) {
        assert_eq!(unit, 0);
        STATEFUL_GL.lock().unwrap().sampler_binding_unit_0 = sampler;
    }

    unsafe extern "system" fn stateful_clear_color(r: f32, g: f32, b: f32, a: f32) {
        STATEFUL_GL.lock().unwrap().clear_color = [r, g, b, a];
    }

    unsafe extern "system" fn stateful_color_mask(r: u8, g: u8, b: u8, a: u8) {
        STATEFUL_GL.lock().unwrap().color_write_mask = [r != 0, g != 0, b != 0, a != 0];
    }

    unsafe extern "system" fn stateful_scissor(x: i32, y: i32, width: i32, height: i32) {
        STATEFUL_GL.lock().unwrap().scissor_box = [x, y, width, height];
    }

    unsafe extern "system" fn noop_blend_func_separate(_: u32, _: u32, _: u32, _: u32) {}
    unsafe extern "system" fn noop_blend_equation_separate(_: u32, _: u32) {}
    unsafe extern "system" fn noop_viewport(_: i32, _: i32, _: i32, _: i32) {}
    unsafe extern "system" fn noop_bind_buffer(_: u32, _: u32) {}
    unsafe extern "system" fn noop_bind_vertex_array(_: u32) {}
    unsafe extern "system" fn noop_use_program(_: u32) {}
    unsafe extern "system" fn noop_polygon_mode(_: u32, _: u32) {}

    fn stateful_fake_gl() -> glow::Context {
        *STATEFUL_GL.lock().unwrap() = StatefulGl::ORIGINAL;
        unsafe {
            glow::Context::from_loader_function(|name| {
                match name {
                    "glGetString" => get_string as *const (),
                    "glGetStringi" => get_string_i as *const (),
                    "glGetIntegerv" => stateful_get_integer as *const (),
                    "glGetFloatv" => stateful_get_float as *const (),
                    "glGetBooleanv" => stateful_get_boolean as *const (),
                    "glIsEnabled" => stateful_is_enabled as *const (),
                    "glEnable" => stateful_enable as *const (),
                    "glDisable" => stateful_disable as *const (),
                    "glBlendFuncSeparate" => noop_blend_func_separate as *const (),
                    "glBlendEquationSeparate" => noop_blend_equation_separate as *const (),
                    "glViewport" => noop_viewport as *const (),
                    "glScissor" => stateful_scissor as *const (),
                    "glClearColor" => stateful_clear_color as *const (),
                    "glColorMask" => stateful_color_mask as *const (),
                    "glBindBuffer" => noop_bind_buffer as *const (),
                    "glBindVertexArray" => noop_bind_vertex_array as *const (),
                    "glActiveTexture" => stateful_active_texture as *const (),
                    "glBindTexture" => stateful_bind_texture as *const (),
                    "glBindSampler" => stateful_bind_sampler as *const (),
                    "glUseProgram" => noop_use_program as *const (),
                    "glPolygonMode" => noop_polygon_mode as *const (),
                    _ => std::ptr::null(),
                }
                .cast()
            })
        }
    }

    #[test]
    fn gl_state_guard_restores_unit_zero_sampler_and_clear_state_after_panic() {
        let gl = stateful_fake_gl();
        let version = GlVersion {
            major: 3,
            minor: 3,
            is_es: false,
        };

        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = GlStateGuard::capture(&gl, version);
            unsafe {
                gl.active_texture(glow::TEXTURE0);
                gl.bind_texture(
                    glow::TEXTURE_2D,
                    Some(glow::NativeTexture(std::num::NonZeroU32::new(99).unwrap())),
                );
                #[cfg(feature = "bind_sampler_support")]
                gl.bind_sampler(0, None);
                gl.disable(glow::SCISSOR_TEST);
                gl.scissor(0, 0, 1, 1);
                gl.clear_color(1.0, 1.0, 1.0, 1.0);
                gl.color_mask(true, true, true, true);
            }
            panic!("injected render panic");
        }));

        assert!(panic.is_err());
        assert_eq!(*STATEFUL_GL.lock().unwrap(), StatefulGl::ORIGINAL);
    }
}
