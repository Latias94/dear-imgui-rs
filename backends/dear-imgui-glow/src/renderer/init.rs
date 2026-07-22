use std::ffi::c_void;

use dear_imgui_rs::{BackendFlags, Context as ImGuiContext, sys};
use glow::{Context, HasContext};

use super::{
    GlowRenderer,
    callbacks::{
        draw_callback_reset_render_state, draw_callback_set_sampler_linear,
        draw_callback_set_sampler_nearest,
    },
    core::{GlowBackendUserData, RendererStateFault},
    device::PendingDeviceObjects,
};
use crate::{
    error::{InitError, InitResult, RenderError, RenderResult},
    texture::{SimpleTextureMap, TextureMap},
    versions::GlVersion,
};

const RENDERER_NAME: &str = concat!("dear-imgui-glow ", env!("CARGO_PKG_VERSION"));
const CORE_RENDERER_FLAGS: i32 = sys::ImGuiBackendFlags_RendererHasVtxOffset as i32
    | sys::ImGuiBackendFlags_RendererHasTextures as i32;
const RENDERER_RESERVED_FLAGS: i32 =
    CORE_RENDERER_FLAGS | sys::ImGuiBackendFlags_RendererHasViewports as i32;

impl GlowRenderer {
    /// Create a new Glow renderer with owned OpenGL context (recommended)
    ///
    /// This is the preferred way to create a Glow renderer as it handles all resource
    /// management automatically and provides a simple API similar to the WGPU backend.
    ///
    /// # Arguments
    /// * `gl` - OpenGL context (will be owned by the renderer)
    /// * `imgui_context` - Dear ImGui context to configure
    ///
    /// # Example
    /// ```rust,no_run
    /// use dear_imgui_glow::GlowRenderer;
    /// # use dear_imgui_glow::glow;
    /// # use dear_imgui_rs::Context as ImGuiContext;
    ///
    /// # let gl_context = unsafe { glow::Context::from_loader_function(|_| std::ptr::null()) };
    /// # let mut imgui_context = ImGuiContext::create();
    /// let mut renderer = GlowRenderer::new(gl_context, &mut imgui_context).unwrap();
    /// ```
    pub fn new(gl: glow::Context, imgui_context: &mut ImGuiContext) -> InitResult<Self> {
        let texture_map = Box::new(SimpleTextureMap::default());
        Self::with_texture_map(Some(gl), imgui_context, texture_map)
    }

    /// Create a new Glow renderer with custom texture management (advanced)
    ///
    /// This method allows you to provide your own texture management implementation
    /// and optionally manage the OpenGL context externally.
    ///
    /// # Arguments
    /// * `gl` - OpenGL context (Some = owned, None = externally managed)
    /// * `imgui_context` - Dear ImGui context to configure
    /// * `texture_map` - Custom texture map implementation
    ///
    /// # Example
    /// ```rust,no_run
    /// use dear_imgui_glow::{GlowRenderer, SimpleTextureMap};
    /// # use dear_imgui_glow::glow;
    /// # use dear_imgui_rs::Context as ImGuiContext;
    ///
    /// let texture_map = Box::new(SimpleTextureMap::default());
    /// # let gl_context = unsafe { glow::Context::from_loader_function(|_| std::ptr::null()) };
    /// # let mut imgui_context = ImGuiContext::create();
    /// let mut renderer = GlowRenderer::with_texture_map(
    ///     Some(gl_context),
    ///     &mut imgui_context,
    ///     texture_map
    /// ).unwrap();
    /// ```
    pub fn with_texture_map(
        gl: Option<glow::Context>,
        imgui_context: &mut ImGuiContext,
        texture_map: Box<dyn TextureMap>,
    ) -> InitResult<Self> {
        match gl {
            Some(context) => {
                let gl_rc = std::rc::Rc::new(context);
                Self::init_internal(Some(gl_rc.clone()), &gl_rc, imgui_context, texture_map)
            }
            None => Err(InitError::MissingGlContext),
        }
    }

    /// Create a new Glow renderer with external OpenGL context (advanced)
    ///
    /// This method is for advanced users who want to manage the OpenGL context
    /// externally while still using custom texture management.
    ///
    /// # Arguments
    /// * `gl` - Reference to externally managed OpenGL context
    /// * `imgui_context` - Dear ImGui context to configure
    /// * `texture_map` - Custom texture map implementation
    ///
    /// # Example
    /// ```rust,no_run
    /// use dear_imgui_glow::{GlowRenderer, SimpleTextureMap};
    /// # use dear_imgui_glow::glow;
    /// # use dear_imgui_rs::Context as ImGuiContext;
    ///
    /// let texture_map = Box::new(SimpleTextureMap::default());
    /// # let gl_context = unsafe { glow::Context::from_loader_function(|_| std::ptr::null()) };
    /// # let mut imgui_context = ImGuiContext::create();
    /// let mut renderer = GlowRenderer::with_external_context(
    ///     &gl_context,
    ///     &mut imgui_context,
    ///     texture_map
    /// ).unwrap();
    /// ```
    pub fn with_external_context(
        gl: &Context,
        imgui_context: &mut ImGuiContext,
        texture_map: Box<dyn TextureMap>,
    ) -> InitResult<Self> {
        Self::init_internal(None, gl, imgui_context, texture_map)
    }

    /// Create a renderer that shares ownership of its Glow function table.
    ///
    /// Unlike [`Self::with_external_context`], the renderer retains the exact `Rc` used to create
    /// its GL objects. This is the supported construction path when the renderer will later be
    /// consumed by `GlowViewportRuntime`.
    ///
    /// ```rust,no_run
    /// use std::rc::Rc;
    /// use dear_imgui_glow::{GlowRenderer, SimpleTextureMap, glow};
    /// use dear_imgui_rs::Context as ImGuiContext;
    ///
    /// let gl = Rc::new(unsafe {
    ///     glow::Context::from_loader_function(|_| std::ptr::null())
    /// });
    /// let mut imgui = ImGuiContext::create();
    /// let renderer = GlowRenderer::with_shared_context(
    ///     Rc::clone(&gl),
    ///     &mut imgui,
    ///     Box::new(SimpleTextureMap::default()),
    /// )?;
    /// # Ok::<(), dear_imgui_glow::InitError>(())
    /// ```
    pub fn with_shared_context(
        gl: std::rc::Rc<Context>,
        imgui_context: &mut ImGuiContext,
        texture_map: Box<dyn TextureMap>,
    ) -> InitResult<Self> {
        Self::init_internal(
            Some(std::rc::Rc::clone(&gl)),
            &gl,
            imgui_context,
            texture_map,
        )
    }

    /// Internal initialization method
    fn init_internal(
        owned_gl: Option<std::rc::Rc<glow::Context>>,
        gl: &Context,
        imgui_context: &mut ImGuiContext,
        texture_map: Box<dyn TextureMap>,
    ) -> InitResult<Self> {
        preflight_renderer_state(imgui_context)?;
        let gl_version = GlVersion::read(gl);
        let renderer_texture_max = unsafe { gl.get_parameter_i32(glow::MAX_TEXTURE_SIZE) }.max(0);

        #[cfg(feature = "clip_origin_support")]
        let has_clip_origin_support = {
            let support = gl_version.clip_origin_support();

            #[cfg(feature = "gl_extensions_support")]
            if support {
                support
            } else {
                let extensions_count = unsafe { gl.get_parameter_i32(glow::NUM_EXTENSIONS) } as u32;
                (0..extensions_count).any(|index| {
                    let extension_name =
                        unsafe { gl.get_parameter_indexed_string(glow::EXTENSIONS, index) };
                    extension_name == "GL_ARB_clip_control"
                })
            }
            #[cfg(not(feature = "gl_extensions_support"))]
            support
        };
        #[cfg(not(feature = "clip_origin_support"))]
        let has_clip_origin_support = false;

        let pending_device_objects = PendingDeviceObjects::create_all(gl, gl_version)?;
        preflight_renderer_state(imgui_context)?;
        let renderer_consumer = imgui_context.create_renderer_consumer()?;
        // Construction has not emitted a renderer epoch or installed a Context-managed texture
        // mapping. Commit the empty transaction before this renderer can publish either one.
        let reset = imgui_context.prepare_renderer_texture_reset(&renderer_consumer)?;
        let _ = reset.commit();
        let backend_user_data = Box::<GlowBackendUserData>::default();
        let backend_user_data_ptr = std::ptr::from_ref(backend_user_data.as_ref())
            .cast_mut()
            .cast::<c_void>();
        let renderer_name_ptr = publish_renderer_state(
            imgui_context,
            backend_user_data_ptr,
            [renderer_texture_max; 2],
        )?;
        let (shaders, vbo_handle, ebo_handle) = pending_device_objects.into_parts();

        let renderer = Self {
            shaders,
            vbo_handle: Some(vbo_handle),
            ebo_handle: Some(ebo_handle),
            owned_textures: Vec::new(),
            #[cfg(feature = "bind_vertex_array_support")]
            vertex_array_object: None,
            gl_version,
            has_clip_origin_support,
            is_destroyed: false,
            gl_context: owned_gl,
            context_binding: Some(imgui_context.binding()),
            backend_user_data,
            renderer_name_ptr,
            renderer_texture_max: [renderer_texture_max; 2],
            renderer_state_fault: None,
            #[cfg(test)]
            synthetic_test_renderer: false,
            texture_map: Some(texture_map),
            managed_textures: std::collections::HashMap::new(),
            destroyed_managed_textures: std::collections::HashMap::new(),
            renderer_consumer: Some(renderer_consumer),
            framebuffer_srgb: false,
            color_gamma_override: None,
            viewport_clear_color: [0.0, 0.0, 0.0, 1.0],
        };

        Ok(renderer)
    }

    pub(super) fn ensure_operational(&mut self) -> RenderResult<()> {
        if self.renderer_consumer.is_none() {
            #[cfg(test)]
            if self.synthetic_test_renderer {
                return self.validate_renderer_state();
            }
            return Err(RenderError::RendererDestroyed);
        }
        self.validate_renderer_state()
    }

    pub(super) fn validate_renderer_state(&mut self) -> RenderResult<()> {
        if let Some(fault) = self.renderer_state_fault {
            return Err(fault.into_error());
        }
        let Some(binding) = self.context_binding.clone() else {
            #[cfg(test)]
            if self.synthetic_test_renderer {
                return Ok(());
            }
            return Err(RenderError::RendererNotAttached);
        };

        let fault = binding.try_with_bound_context(|| {
            let fault = self.detect_renderer_state_fault_bound();
            if fault.is_some() {
                unsafe { self.clear_owned_renderer_state_bound() };
            }
            fault
        })?;
        if let Some(fault) = fault {
            self.renderer_state_fault = Some(fault);
            return Err(fault.into_error());
        }
        Ok(())
    }

    fn detect_renderer_state_fault_bound(&self) -> Option<RendererStateFault> {
        let io = unsafe { sys::igGetIO_Nil() };
        if io.is_null() {
            return Some(RendererStateFault::State("ImGuiIO"));
        }
        let io = unsafe { &*io };
        if io.BackendRendererUserData != self.backend_user_data_ptr() {
            return Some(RendererStateFault::State("BackendRendererUserData"));
        }
        if io.BackendRendererName != self.renderer_name_ptr {
            return Some(RendererStateFault::State("BackendRendererName"));
        }
        if io.BackendFlags & sys::ImGuiBackendFlags_RendererHasVtxOffset as i32 == 0 {
            return Some(RendererStateFault::Capability("RENDERER_HAS_VTX_OFFSET"));
        }
        if io.BackendFlags & sys::ImGuiBackendFlags_RendererHasTextures as i32 == 0 {
            return Some(RendererStateFault::Capability("RENDERER_HAS_TEXTURES"));
        }

        let platform_io = unsafe { sys::igGetPlatformIO_Nil() };
        if platform_io.is_null() {
            return Some(RendererStateFault::State("ImGuiPlatformIO"));
        }
        let platform_io = unsafe { &*platform_io };
        if platform_io.Renderer_RenderState != std::ptr::null_mut() {
            return Some(RendererStateFault::State("Renderer_RenderState"));
        }
        if platform_io.Renderer_TextureMaxWidth != self.renderer_texture_max[0] {
            return Some(RendererStateFault::State("Renderer_TextureMaxWidth"));
        }
        if platform_io.Renderer_TextureMaxHeight != self.renderer_texture_max[1] {
            return Some(RendererStateFault::State("Renderer_TextureMaxHeight"));
        }
        for (matches, callback) in [
            (
                draw_callback_matches(
                    platform_io.DrawCallback_ResetRenderState,
                    draw_callback_reset_render_state,
                ),
                "DrawCallback_ResetRenderState",
            ),
            (
                draw_callback_matches(
                    platform_io.DrawCallback_SetSamplerLinear,
                    draw_callback_set_sampler_linear,
                ),
                "DrawCallback_SetSamplerLinear",
            ),
            (
                draw_callback_matches(
                    platform_io.DrawCallback_SetSamplerNearest,
                    draw_callback_set_sampler_nearest,
                ),
                "DrawCallback_SetSamplerNearest",
            ),
        ] {
            if !matches {
                return Some(RendererStateFault::Callback(callback));
            }
        }
        if let Some(callback) = first_renderer_window_callback_drift(platform_io) {
            return Some(RendererStateFault::Callback(callback));
        }

        let viewport_flag =
            io.BackendFlags & sys::ImGuiBackendFlags_RendererHasViewports as i32 != 0;
        let viewport_callback = renderer_render_callback_is_glow(platform_io.Renderer_RenderWindow);
        if viewport_flag != viewport_callback {
            return Some(RendererStateFault::Capability("RENDERER_HAS_VIEWPORTS"));
        }
        None
    }

    pub(super) fn unconfigure_imgui_context(&mut self, imgui_context: &mut ImGuiContext) {
        let owned_name = unsafe { self.clear_owned_renderer_state_bound() };
        if owned_name {
            imgui_context
                .set_renderer_name::<String>(None)
                .expect("clearing Glow BackendRendererName must not fail");
        }
        self.context_binding.take();
    }

    /// Clear only values still carrying this renderer's exact identity.
    pub(super) unsafe fn clear_owned_renderer_state_bound(&self) -> bool {
        let io = unsafe { sys::igGetIO_Nil() };
        if io.is_null() {
            return false;
        }
        let io = unsafe { &mut *io };
        let user_data_is_ours = io.BackendRendererUserData == self.backend_user_data_ptr();
        let owned_name =
            !self.renderer_name_ptr.is_null() && io.BackendRendererName == self.renderer_name_ptr;
        if owned_name {
            io.BackendRendererName = std::ptr::null();
        }
        if user_data_is_ours {
            io.BackendRendererUserData = std::ptr::null_mut();
        }

        let platform_io = unsafe { sys::igGetPlatformIO_Nil() };
        if platform_io.is_null() {
            if user_data_is_ours || owned_name {
                io.BackendFlags &= !CORE_RENDERER_FLAGS;
            }
            return owned_name;
        }
        let platform_io = unsafe { &mut *platform_io };
        let owns_standard_draw_callback = [
            draw_callback_matches(
                platform_io.DrawCallback_ResetRenderState,
                draw_callback_reset_render_state,
            ),
            draw_callback_matches(
                platform_io.DrawCallback_SetSamplerLinear,
                draw_callback_set_sampler_linear,
            ),
            draw_callback_matches(
                platform_io.DrawCallback_SetSamplerNearest,
                draw_callback_set_sampler_nearest,
            ),
        ]
        .into_iter()
        .any(|matches| matches);
        let owns_viewport_callback =
            renderer_render_callback_is_glow(platform_io.Renderer_RenderWindow);
        let still_owns_core_publication =
            user_data_is_ours || owned_name || owns_standard_draw_callback;
        if still_owns_core_publication {
            io.BackendFlags &= !CORE_RENDERER_FLAGS;
        }
        if owns_viewport_callback {
            io.BackendFlags &= !(sys::ImGuiBackendFlags_RendererHasViewports as i32);
        }
        if still_owns_core_publication {
            if platform_io.Renderer_TextureMaxWidth == self.renderer_texture_max[0] {
                platform_io.Renderer_TextureMaxWidth = 0;
            }
            if platform_io.Renderer_TextureMaxHeight == self.renderer_texture_max[1] {
                platform_io.Renderer_TextureMaxHeight = 0;
            }
        }
        if draw_callback_matches(
            platform_io.DrawCallback_ResetRenderState,
            draw_callback_reset_render_state,
        ) {
            platform_io.DrawCallback_ResetRenderState = None;
        }
        if draw_callback_matches(
            platform_io.DrawCallback_SetSamplerLinear,
            draw_callback_set_sampler_linear,
        ) {
            platform_io.DrawCallback_SetSamplerLinear = None;
        }
        if draw_callback_matches(
            platform_io.DrawCallback_SetSamplerNearest,
            draw_callback_set_sampler_nearest,
        ) {
            platform_io.DrawCallback_SetSamplerNearest = None;
        }
        if owns_viewport_callback {
            platform_io.Renderer_RenderWindow = None;
        }
        owned_name
    }
}

fn preflight_renderer_state(imgui_context: &ImGuiContext) -> InitResult<()> {
    imgui_context.binding().with_bound_context(|| {
        let io = unsafe { sys::igGetIO_Nil() };
        if io.is_null() {
            return Err(InitError::RendererStateOccupied { field: "ImGuiIO" });
        }
        let io = unsafe { &*io };
        for (occupied, field) in [
            (
                !io.BackendRendererUserData.is_null(),
                "BackendRendererUserData",
            ),
            (!io.BackendRendererName.is_null(), "BackendRendererName"),
        ] {
            if occupied {
                return Err(InitError::RendererStateOccupied { field });
            }
        }
        let occupied_flags = io.BackendFlags & RENDERER_RESERVED_FLAGS;
        if occupied_flags != 0 {
            return Err(InitError::RendererCapabilityOccupied {
                flags: occupied_flags,
            });
        }

        let platform_io = unsafe { sys::igGetPlatformIO_Nil() };
        if platform_io.is_null() {
            return Err(InitError::RendererStateOccupied {
                field: "ImGuiPlatformIO",
            });
        }
        let platform_io = unsafe { &*platform_io };
        for (occupied, field) in [
            (
                !platform_io.Renderer_RenderState.is_null(),
                "Renderer_RenderState",
            ),
            (
                platform_io.Renderer_TextureMaxWidth != 0,
                "Renderer_TextureMaxWidth",
            ),
            (
                platform_io.Renderer_TextureMaxHeight != 0,
                "Renderer_TextureMaxHeight",
            ),
        ] {
            if occupied {
                return Err(InitError::RendererStateOccupied { field });
            }
        }
        for (occupied, callback) in [
            (
                platform_io.DrawCallback_ResetRenderState.is_some(),
                "DrawCallback_ResetRenderState",
            ),
            (
                platform_io.DrawCallback_SetSamplerLinear.is_some(),
                "DrawCallback_SetSamplerLinear",
            ),
            (
                platform_io.DrawCallback_SetSamplerNearest.is_some(),
                "DrawCallback_SetSamplerNearest",
            ),
            (
                platform_io.Renderer_CreateWindow.is_some(),
                "Renderer_CreateWindow",
            ),
            (
                platform_io.Renderer_DestroyWindow.is_some(),
                "Renderer_DestroyWindow",
            ),
            (
                platform_io.Renderer_SetWindowSize.is_some(),
                "Renderer_SetWindowSize",
            ),
            (
                platform_io.Renderer_RenderWindow.is_some(),
                "Renderer_RenderWindow",
            ),
            (
                platform_io.Renderer_SwapBuffers.is_some(),
                "Renderer_SwapBuffers",
            ),
        ] {
            if occupied {
                return Err(InitError::RendererCallbackOccupied { callback });
            }
        }
        Ok(())
    })
}

fn publish_renderer_state(
    imgui_context: &mut ImGuiContext,
    backend_user_data: *mut c_void,
    texture_max: [i32; 2],
) -> InitResult<*const std::ffi::c_char> {
    imgui_context
        .set_renderer_name(Some(RENDERER_NAME.to_owned()))
        .map_err(|error| InitError::Generic(error.to_string()))?;
    let renderer_name_ptr = imgui_context
        .io()
        .backend_renderer_name()
        .expect("Glow just published BackendRendererName")
        .as_ptr();

    let io = imgui_context.io_mut();
    unsafe { io.set_backend_renderer_user_data(backend_user_data) };
    io.set_backend_flags(
        io.backend_flags()
            | BackendFlags::RENDERER_HAS_VTX_OFFSET
            | BackendFlags::RENDERER_HAS_TEXTURES,
    );

    let platform_io = imgui_context.platform_io_mut();
    let raw = unsafe { &mut *platform_io.as_raw_mut() };
    raw.Renderer_TextureMaxWidth = texture_max[0];
    raw.Renderer_TextureMaxHeight = texture_max[1];
    unsafe { platform_io.set_renderer_render_state(std::ptr::null_mut()) };
    unsafe {
        platform_io
            .set_draw_callback_reset_render_state_raw(Some(draw_callback_reset_render_state));
        platform_io
            .set_draw_callback_set_sampler_linear_raw(Some(draw_callback_set_sampler_linear));
        platform_io
            .set_draw_callback_set_sampler_nearest_raw(Some(draw_callback_set_sampler_nearest));
    }
    Ok(renderer_name_ptr)
}

type RawDrawCallback = unsafe extern "C" fn(*const sys::ImDrawList, *const sys::ImDrawCmd);

fn draw_callback_matches(actual: sys::ImDrawCallback, expected: RawDrawCallback) -> bool {
    actual.is_some_and(|actual| std::ptr::fn_addr_eq(actual, expected))
}

fn first_renderer_window_callback_drift(raw: &sys::ImGuiPlatformIO) -> Option<&'static str> {
    if raw.Renderer_CreateWindow.is_some() {
        return Some("Renderer_CreateWindow");
    }
    if raw.Renderer_DestroyWindow.is_some() {
        return Some("Renderer_DestroyWindow");
    }
    if raw.Renderer_SetWindowSize.is_some() {
        return Some("Renderer_SetWindowSize");
    }
    if raw.Renderer_RenderWindow.is_some()
        && !renderer_render_callback_is_glow(raw.Renderer_RenderWindow)
    {
        return Some("Renderer_RenderWindow");
    }
    raw.Renderer_SwapBuffers
        .is_some()
        .then_some("Renderer_SwapBuffers")
}

#[cfg(feature = "multi-viewport")]
fn renderer_render_callback_is_glow(
    callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void)>,
) -> bool {
    callback.is_some_and(|callback| {
        std::ptr::fn_addr_eq(
            callback,
            super::multi_viewport::renderer_render_window_sys
                as unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void),
        )
    })
}

#[cfg(not(feature = "multi-viewport"))]
fn renderer_render_callback_is_glow(
    _callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void)>,
) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use dear_imgui_rs::{OwnedTextureData, TextureFormat};

    use super::*;
    use crate::shaders::test_support::{FakeFailure, TEST_LOCK, fake_gl, reset};

    #[test]
    fn gpu_creation_failure_does_not_claim_or_reset_context_state() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset(FakeFailure::FragmentShaderCreate);
        let gl = fake_gl();
        let mut context = ImGuiContext::create();
        let mut texture = OwnedTextureData::new();
        texture.create(TextureFormat::RGBA32, 1, 1);
        texture.set_data(&[255, 255, 255, 255]);
        let texture = context.register_texture(texture);
        let before_texture = context
            .with_texture(texture, |texture| (texture.status(), texture.texture_id()))
            .unwrap();

        let result = GlowRenderer::with_external_context(
            &gl,
            &mut context,
            Box::new(SimpleTextureMap::default()),
        );
        assert!(matches!(result, Err(InitError::CreateShader(_))));

        assert!(context.io().backend_renderer_user_data().is_null());
        assert!(context.io().backend_renderer_name().is_none());
        assert_eq!(
            context.io().backend_flags().bits() & RENDERER_RESERVED_FLAGS,
            0
        );
        let platform_io = context.platform_io();
        let raw = unsafe { &*platform_io.as_raw() };
        assert!(raw.DrawCallback_ResetRenderState.is_none());
        assert!(raw.DrawCallback_SetSamplerLinear.is_none());
        assert!(raw.DrawCallback_SetSamplerNearest.is_none());
        assert_eq!(raw.Renderer_TextureMaxWidth, 0);
        assert_eq!(raw.Renderer_TextureMaxHeight, 0);
        assert_eq!(
            context
                .with_texture(texture, |texture| {
                    (texture.status(), texture.texture_id())
                })
                .unwrap(),
            before_texture
        );

        let consumer = context.create_renderer_consumer().unwrap();
        assert_eq!(consumer.generation(), 1);
        reset(FakeFailure::None);
    }
}
