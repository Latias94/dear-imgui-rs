//! OpenGL state backup and restoration

use std::{cell::Cell, marker::PhantomData, ptr::NonNull, rc::Rc};

use crate::GlVertexArray;
use crate::{GlBuffer, GlProgram, GlSampler, GlTexture, GlVersion, RenderError};
use dear_imgui_rs::{render::RendererRenderStateGuardError, sys};
use glow::{Context, HasContext};
use thiserror::Error;

/// Sampling mechanism selected from the live OpenGL context.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum GlowSamplerStrategy {
    /// OpenGL sampler objects override texture-owned filtering without mutating textures.
    SamplerObjects,
    /// Explicit sampler commands temporarily override and then restore texture parameters.
    TextureParameters,
}

/// Error returned while borrowing transient Glow draw-callback state.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[non_exhaustive]
pub enum GlowRenderStateAccessError {
    /// No Glow render state is active on the current Dear ImGui Context.
    #[error("no Glow render state is active on the current Dear ImGui context")]
    Inactive,
    /// The active callback state is already borrowed by an outer scoped access.
    #[error("the active Glow render state is already borrowed")]
    AlreadyBorrowed,
}

pub(crate) struct GlowRenderStateStorage<'gl> {
    gl: &'gl Context,
    sampler_strategy: GlowSamplerStrategy,
    borrowed: Cell<bool>,
}

impl<'gl> GlowRenderStateStorage<'gl> {
    pub(crate) fn new(gl: &'gl Context, sampler_strategy: GlowSamplerStrategy) -> Self {
        Self {
            gl,
            sampler_strategy,
            borrowed: Cell::new(false),
        }
    }
}

struct GlowRenderStateBorrow<'storage>(&'storage Cell<bool>);

impl Drop for GlowRenderStateBorrow<'_> {
    fn drop(&mut self) {
        self.0.set(false);
    }
}

/// Scoped access to the OpenGL context selected for a raw draw callback.
///
/// This corresponds to `ImGui_ImplOpenGL3_RenderState` in the official backend. The value can
/// only be obtained through [`Self::with_current`] while `dear-imgui-glow` is invoking a raw
/// callback, and cannot outlive that callback scope.
///
/// The returned [`Context`] is the live OpenGL function table, not a safe rendering facade.
/// Calling its methods remains subject to each `glow` method's `unsafe` OpenGL preconditions.
///
/// ```compile_fail
/// use dear_imgui_glow::GlowRenderState;
///
/// let _escaped = unsafe { GlowRenderState::with_current(|state| state.gl()) };
/// ```
#[derive(Debug)]
pub struct GlowRenderState<'callback> {
    storage: NonNull<GlowRenderStateStorage<'callback>>,
    _callback: PhantomData<&'callback mut GlowRenderStateStorage<'callback>>,
    _ui_thread: PhantomData<Rc<()>>,
}

impl GlowRenderState<'_> {
    /// Borrows the state published for the current raw draw callback.
    ///
    /// # Safety
    ///
    /// This function may only be called from a raw draw callback currently invoked by
    /// `dear-imgui-glow`. The current Dear ImGui Context must be the renderer owner and its
    /// `Renderer_RenderState` slot must still contain the Glow state installed for this callback.
    /// The callback must not replace that slot while `callback` is running.
    ///
    /// The higher-ranked closure prevents the OpenGL context reference from escaping. Recursive
    /// access is rejected at runtime.
    pub unsafe fn with_current<R>(
        callback: impl for<'callback> FnOnce(GlowRenderState<'callback>) -> R,
    ) -> Result<R, GlowRenderStateAccessError> {
        let platform_io = unsafe { sys::igGetPlatformIO_Nil() };
        let raw_state = if platform_io.is_null() {
            None
        } else {
            NonNull::new(unsafe { (*platform_io).Renderer_RenderState })
        }
        .ok_or(GlowRenderStateAccessError::Inactive)?;
        let storage = raw_state.cast::<GlowRenderStateStorage<'_>>();
        let borrowed = unsafe { &storage.as_ref().borrowed };
        if borrowed.replace(true) {
            return Err(GlowRenderStateAccessError::AlreadyBorrowed);
        }
        let _borrow = GlowRenderStateBorrow(borrowed);
        Ok(callback(GlowRenderState {
            storage,
            _callback: PhantomData,
            _ui_thread: PhantomData,
        }))
    }

    /// Returns the current OpenGL function table for the callback duration.
    pub fn gl(&self) -> &Context {
        unsafe { self.storage.as_ref().gl }
    }

    /// Returns the renderer's sampling mechanism for this draw scope.
    pub fn sampler_strategy(&self) -> GlowSamplerStrategy {
        unsafe { self.storage.as_ref().sampler_strategy }
    }
}

pub(crate) fn map_renderer_render_state_error(error: RendererRenderStateGuardError) -> RenderError {
    match error {
        RendererRenderStateGuardError::MissingPlatformIo => RenderError::MissingPlatformIo,
        RendererRenderStateGuardError::AlreadyOccupied | RendererRenderStateGuardError::Drift => {
            RenderError::RendererStateDrift {
                field: "Renderer_RenderState",
            }
        }
    }
}

/// Applies the configured desktop framebuffer sRGB state and restores the prior state on drop.
pub(crate) struct FramebufferSrgbScope<'a> {
    gl: &'a Context,
    requested: bool,
    restore_enabled: bool,
}

impl<'a> FramebufferSrgbScope<'a> {
    pub(crate) fn enter(gl: &'a Context, requested: bool) -> Self {
        let restore_enabled = unsafe { gl.is_enabled(glow::FRAMEBUFFER_SRGB) };
        if restore_enabled != requested {
            set_framebuffer_srgb(gl, requested);
        }
        Self {
            gl,
            requested,
            restore_enabled,
        }
    }

    pub(crate) fn reapply(&self) {
        if unsafe { self.gl.is_enabled(glow::FRAMEBUFFER_SRGB) } != self.requested {
            set_framebuffer_srgb(self.gl, self.requested);
        }
    }
}

impl Drop for FramebufferSrgbScope<'_> {
    fn drop(&mut self) {
        if unsafe { self.gl.is_enabled(glow::FRAMEBUFFER_SRGB) } != self.restore_enabled {
            set_framebuffer_srgb(self.gl, self.restore_enabled);
        }
    }
}

fn set_framebuffer_srgb(gl: &Context, enabled: bool) {
    unsafe {
        if enabled {
            gl.enable(glow::FRAMEBUFFER_SRGB);
        } else {
            gl.disable(glow::FRAMEBUFFER_SRGB);
        }
    }
}

/// OpenGL state backup for proper state restoration
#[derive(Clone, Default)]
struct GlStateBackup {
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

    // Vertex array
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
    polygon_mode: [i32; 2],

    // Primitive restart (OpenGL 3.1+)
    primitive_restart_enabled: bool,

    // Sampler binding (OpenGL 3.3+/ES 3.0+)
    sampler_binding: Option<GlSampler>,
}

impl GlStateBackup {
    /// Backup OpenGL state before rendering
    fn backup(
        &mut self,
        gl: &Context,
        gl_version: GlVersion,
        separate_polygon_modes: bool,
        supports_sampler_objects: bool,
    ) {
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
            self.array_buffer_binding = gl.get_parameter_buffer(glow::ARRAY_BUFFER_BINDING);

            // Vertex array
            self.vertex_array_binding = gl.get_parameter_vertex_array(glow::VERTEX_ARRAY_BINDING);

            // Textures
            self.active_texture = u32::try_from(gl.get_parameter_i32(glow::ACTIVE_TEXTURE))
                .ok()
                .unwrap_or(glow::TEXTURE0);
            gl.active_texture(glow::TEXTURE0);
            self.texture_2d_binding_unit_0 = gl.get_parameter_texture(glow::TEXTURE_BINDING_2D);

            // Sampler binding
            if supports_sampler_objects {
                self.sampler_binding = gl.get_parameter_sampler(glow::SAMPLER_BINDING);
            }
            gl.active_texture(self.active_texture);

            // Shader program
            self.current_program = gl.get_parameter_program(glow::CURRENT_PROGRAM);

            // Other state
            self.cull_face_enabled = gl.is_enabled(glow::CULL_FACE);
            self.depth_test_enabled = gl.is_enabled(glow::DEPTH_TEST);
            self.stencil_test_enabled = gl.is_enabled(glow::STENCIL_TEST);

            // Polygon mode (desktop OpenGL only)
            if gl_version.supports_polygon_mode() {
                let mut polygon_mode = [0i32; 2];
                gl.get_parameter_i32_slice(glow::POLYGON_MODE, &mut polygon_mode);
                self.polygon_mode.copy_from_slice(&polygon_mode);
                if !separate_polygon_modes {
                    self.polygon_mode[1] = self.polygon_mode[0];
                }
            }

            // Primitive restart
            if gl_version.supports_primitive_restart() {
                self.primitive_restart_enabled = gl.is_enabled(glow::PRIMITIVE_RESTART);
            }
        }
    }

    /// Restore OpenGL state after rendering
    fn restore(
        &self,
        gl: &Context,
        gl_version: GlVersion,
        separate_polygon_modes: bool,
        supports_sampler_objects: bool,
    ) {
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

            // The element-array binding is restored with its owning VAO.
            gl.bind_vertex_array(self.vertex_array_binding);
            gl.bind_buffer(glow::ARRAY_BUFFER, self.array_buffer_binding);

            // Restore textures
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, self.texture_2d_binding_unit_0);

            if supports_sampler_objects {
                gl.bind_sampler(0, self.sampler_binding);
            }
            gl.active_texture(self.active_texture);

            // A program pending deletion may have disappeared while callbacks were running.
            if self.current_program.is_none()
                || self
                    .current_program
                    .is_some_and(|program| gl.is_program(program))
            {
                gl.use_program(self.current_program);
            }

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
            if gl_version.supports_polygon_mode() {
                if separate_polygon_modes {
                    gl.polygon_mode(glow::FRONT, self.polygon_mode[0] as u32);
                    gl.polygon_mode(glow::BACK, self.polygon_mode[1] as u32);
                } else {
                    gl.polygon_mode(glow::FRONT_AND_BACK, self.polygon_mode[0] as u32);
                }
            }

            // Restore primitive restart
            if gl_version.supports_primitive_restart() {
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
    separate_polygon_modes: bool,
    supports_sampler_objects: bool,
    backup: GlStateBackup,
}

impl<'a> GlStateGuard<'a> {
    pub(crate) fn capture(
        gl: &'a Context,
        gl_version: GlVersion,
        separate_polygon_modes: bool,
        supports_sampler_objects: bool,
    ) -> Self {
        let mut backup = GlStateBackup::default();
        backup.backup(
            gl,
            gl_version,
            separate_polygon_modes,
            supports_sampler_objects,
        );
        Self {
            gl,
            gl_version,
            separate_polygon_modes,
            supports_sampler_objects,
            backup,
        }
    }
}

impl Drop for GlStateGuard<'_> {
    fn drop(&mut self) {
        self.backup.restore(
            self.gl,
            self.gl_version,
            self.separate_polygon_modes,
            self.supports_sampler_objects,
        );
    }
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};
    use std::sync::Mutex;

    use glow::HasContext;

    use dear_imgui_rs::{Context as ImGuiContext, render::RendererRenderStateGuard, sys};
    use static_assertions::assert_not_impl_any;

    use super::{
        FramebufferSrgbScope, GlStateGuard, GlowRenderState, GlowRenderStateAccessError,
        GlowRenderStateStorage, GlowSamplerStrategy,
    };
    use crate::GlVersion;

    assert_not_impl_any!(GlowRenderState<'static>: Send, Sync);
    assert_not_impl_any!(crate::GlowRenderer: Send, Sync);

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
            ["query", "enable", "clear-and-draw", "query", "disable"]
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
        assert_eq!(take_gl_events(), ["query", "clear-and-draw", "query"]);
    }

    #[test]
    fn framebuffer_srgb_scope_reapplies_configuration_after_callback_mutation() {
        reset_gl_state(false);
        let gl = fake_gl();

        {
            let scope = FramebufferSrgbScope::enter(&gl, true);
            unsafe { gl.disable(glow::FRAMEBUFFER_SRGB) };
            scope.reapply();
            assert!(FRAMEBUFFER_SRGB_ENABLED.with(Cell::get));
        }

        assert!(!FRAMEBUFFER_SRGB_ENABLED.with(Cell::get));
        assert_eq!(
            take_gl_events(),
            [
                "query", "enable", "disable", "query", "enable", "query", "disable"
            ]
        );
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
            let _guard = GlStateGuard::capture(&gl, version, false, true);
            unsafe {
                gl.active_texture(glow::TEXTURE0);
                gl.bind_texture(
                    glow::TEXTURE_2D,
                    Some(glow::NativeTexture(std::num::NonZeroU32::new(99).unwrap())),
                );
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

    #[test]
    fn glow_render_state_is_callback_scoped_and_rejects_recursive_borrow() {
        let context = ImGuiContext::create();
        let binding = context.binding();
        let gl = fake_gl();

        binding.with_bound_context(|| {
            let platform_io = unsafe { sys::igGetPlatformIO_Nil() };
            let mut storage = GlowRenderStateStorage::new(&gl, GlowSamplerStrategy::SamplerObjects);
            let guard =
                unsafe { RendererRenderStateGuard::install(platform_io, &mut storage) }.unwrap();

            unsafe {
                GlowRenderState::with_current(|state| {
                    assert!(std::ptr::eq(state.gl(), &gl));
                    assert_eq!(
                        state.sampler_strategy(),
                        GlowSamplerStrategy::SamplerObjects
                    );
                    assert!(matches!(
                        GlowRenderState::with_current(|_| ()),
                        Err(GlowRenderStateAccessError::AlreadyBorrowed)
                    ));
                })
                .unwrap();
            }

            guard.finish().unwrap();
            assert!(matches!(
                unsafe { GlowRenderState::with_current(|_| ()) },
                Err(GlowRenderStateAccessError::Inactive)
            ));
        });
    }
}
