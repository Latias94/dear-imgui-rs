use std::panic::{AssertUnwindSafe, catch_unwind};

use dear_imgui_rs::Context as ImGuiContext;
use glow::{Context, HasContext};

use super::{GlowRenderer, sampler::SamplerObjects};
use crate::{
    GlBuffer,
    error::{InitError, InitResult, RenderError, RenderResult},
    shaders::Shaders,
    texture::TextureMap,
};

pub(super) struct PendingDeviceObjects<'a> {
    gl: &'a Context,
    shaders: Option<Shaders>,
    vbo: Option<GlBuffer>,
    ebo: Option<GlBuffer>,
    samplers: Option<SamplerObjects>,
}

impl<'a> PendingDeviceObjects<'a> {
    pub(super) fn create_all(
        gl: &'a Context,
        gl_version: crate::GlVersion,
        supports_sampler_objects: bool,
    ) -> InitResult<Self> {
        let mut pending = Self {
            gl,
            shaders: None,
            vbo: None,
            ebo: None,
            samplers: None,
        };
        pending.shaders = Some(Shaders::new(gl, gl_version)?);
        pending.vbo = Some(unsafe { gl.create_buffer() }.map_err(InitError::CreateBufferObject)?);
        pending.ebo = Some(unsafe { gl.create_buffer() }.map_err(InitError::CreateBufferObject)?);
        if supports_sampler_objects {
            pending.samplers = Some(SamplerObjects::create(gl)?);
        }
        Ok(pending)
    }

    pub(super) fn into_parts(mut self) -> (Shaders, GlBuffer, GlBuffer, Option<SamplerObjects>) {
        (
            self.shaders
                .take()
                .expect("pending device objects must own shaders"),
            self.vbo
                .take()
                .expect("pending device objects must own a VBO"),
            self.ebo
                .take()
                .expect("pending device objects must own an EBO"),
            self.samplers.take(),
        )
    }

    fn commit(mut self, renderer: &mut GlowRenderer) {
        let shaders = self
            .shaders
            .take()
            .expect("pending device objects must own shaders");
        let vbo = self
            .vbo
            .take()
            .expect("pending device objects must own a VBO");
        let ebo = self
            .ebo
            .take()
            .expect("pending device objects must own an EBO");
        let samplers = self.samplers.take();

        let previous_shaders = std::mem::replace(&mut renderer.shaders, shaders);
        if let Some(program) = previous_shaders.program {
            unsafe { self.gl.delete_program(program) };
        }
        if let Some(previous) = renderer.vbo_handle.replace(vbo) {
            unsafe { self.gl.delete_buffer(previous) };
        }
        if let Some(previous) = renderer.ebo_handle.replace(ebo) {
            unsafe { self.gl.delete_buffer(previous) };
        }
        if let Some(previous) = std::mem::replace(&mut renderer.samplers, samplers) {
            previous.destroy(self.gl);
        }
    }
}

impl Drop for PendingDeviceObjects<'_> {
    fn drop(&mut self) {
        if let Some(samplers) = self.samplers.take() {
            samplers.destroy(self.gl);
        }
        if let Some(ebo) = self.ebo.take() {
            unsafe { self.gl.delete_buffer(ebo) };
        }
        if let Some(vbo) = self.vbo.take() {
            unsafe { self.gl.delete_buffer(vbo) };
        }
        if let Some(program) = self
            .shaders
            .as_mut()
            .and_then(|shaders| shaders.program.take())
        {
            unsafe { self.gl.delete_program(program) };
        }
    }
}

impl GlowRenderer {
    /// Shut down an owned-context renderer and free its OpenGL resources.
    ///
    /// The native context for the retained Glow function table, or a compatible share-group
    /// context, must be current on this thread.
    ///
    /// A renderer consumed by `GlowViewportRuntime` must be shut down through that owning runtime.
    pub fn shutdown(&mut self, imgui_context: &mut ImGuiContext) -> RenderResult<()> {
        let gl = self
            .gl_context
            .clone()
            .ok_or(RenderError::MissingGlContext)?;
        self.shutdown_with_context_inner(&gl, imgui_context)
    }

    /// Shut down a renderer that borrows an externally managed OpenGL context.
    ///
    /// `gl` must be the live function table used to create this renderer, and its corresponding
    /// OpenGL context must be current on this thread. Resources shared with another context must
    /// remain valid until this call succeeds.
    pub fn shutdown_with_context(
        &mut self,
        gl: &Context,
        imgui_context: &mut ImGuiContext,
    ) -> RenderResult<()> {
        self.shutdown_with_context_inner(gl, imgui_context)
    }

    fn shutdown_with_context_inner(
        &mut self,
        gl: &Context,
        imgui_context: &mut ImGuiContext,
    ) -> RenderResult<()> {
        self.ensure_context_matches(imgui_context)?;
        self.destroy_resources_and_reset(gl, imgui_context)?;
        self.unconfigure_imgui_context(imgui_context);
        Ok(())
    }

    /// Releases every GPU resource and commits the matching Context texture reset.
    ///
    /// This intentionally leaves raw renderer state published so an owning multi-viewport runtime
    /// can release its callback table before unpublishing the core renderer contract.
    pub(super) fn destroy_resources_and_reset(
        &mut self,
        gl: &Context,
        imgui_context: &mut ImGuiContext,
    ) -> RenderResult<()> {
        self.ensure_context_matches(imgui_context)?;
        let consumer = self
            .renderer_consumer
            .take()
            .ok_or(RenderError::RendererNotAttached)?;
        let reset = match imgui_context.prepare_renderer_texture_reset(&consumer) {
            Ok(reset) => reset,
            Err(error) => {
                self.renderer_consumer = Some(consumer);
                return Err(error.into());
            }
        };
        if let Err(error) = self.destroy_gpu_resources_only(gl) {
            // The permit deliberately has not committed yet. Returning the consumer restores the
            // exact Context/renderer pairing so callers can retry after fixing the failure.
            drop(reset);
            self.renderer_consumer = Some(consumer);
            return Err(error);
        }
        reset.commit();
        self.destroyed_managed_textures.clear();
        Ok(())
    }

    /// Releases GPU resources during a Context-owned renderer-reset transaction.
    ///
    /// The attachment retains the renderer consumer while this method runs and commits the native
    /// reset only after this release succeeds.
    #[cfg(feature = "multi-viewport")]
    pub(super) fn destroy_for_context_teardown(&mut self, gl: &Context) -> RenderResult<()> {
        self.destroy_gpu_resources_only(gl)?;
        self.destroyed_managed_textures.clear();
        // The Context is bound for this attachment phase. Clear exact owned raw state before the
        // attachment commits the matching native texture reset.
        unsafe { self.clear_owned_renderer_state_bound() };
        self.context_binding.take();
        self.gl_context.take();
        Ok(())
    }

    /// Best-effort release after the native Dear ImGui Context has already gone away.
    ///
    /// This fallback cannot touch native state or commit a texture-reset permit. Normal Context
    /// teardown must use [`Self::destroy_for_context_teardown`] during the renderer-resource
    /// phase instead.
    #[cfg(feature = "multi-viewport")]
    pub(super) fn destroy_after_context_destroyed(&mut self, gl: &Context) -> RenderResult<()> {
        self.destroy_gpu_resources_only(gl)?;
        self.destroyed_managed_textures.clear();
        self.renderer_consumer.take();
        self.context_binding.take();
        self.gl_context.take();
        Ok(())
    }

    /// Returns the retained Glow function table, if this renderer was constructed with one.
    ///
    /// This does not indicate that its native OpenGL context is current.
    pub fn gl_context(&self) -> Option<&std::rc::Rc<glow::Context>> {
        self.gl_context.as_ref()
    }

    /// Get a reference to the texture map
    pub fn texture_map(&self) -> &dyn TextureMap {
        self.texture_map
            .as_deref()
            .expect("GlowRenderer texture_map missing (internal borrow bug)")
    }

    pub(super) fn texture_map_mut(&mut self) -> &mut dyn TextureMap {
        self.texture_map
            .as_deref_mut()
            .expect("GlowRenderer texture_map missing (internal borrow bug)")
    }

    /// Enable/disable GL_FRAMEBUFFER_SRGB around ImGui rendering
    /// Default is disabled; prefer application-level control of sRGB.
    pub fn set_framebuffer_srgb_enabled(&mut self, enabled: bool) -> RenderResult<()> {
        if enabled && !self.supports_framebuffer_srgb_control() {
            return Err(RenderError::FramebufferSrgbUnsupported);
        }
        self.framebuffer_srgb = enabled;
        Ok(())
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

    pub(super) fn ensure_device_objects(&mut self, gl: &Context) -> RenderResult<()> {
        self.ensure_operational()?;
        self.ensure_device_objects_preflighted(gl)
    }

    pub(super) fn ensure_device_objects_preflighted(&mut self, gl: &Context) -> RenderResult<()> {
        if self.device_objects_ready() {
            return Ok(());
        }

        let pending =
            PendingDeviceObjects::create_all(gl, self.gl_version, self.has_sampler_object_support)
                .map_err(RenderError::DeviceObjectInit)?;
        pending.commit(self);
        Ok(())
    }

    /// Destroy OpenGL device objects owned by this renderer.
    ///
    /// The renderer remains attached. The next render recreates the device objects
    /// transactionally before it processes managed textures or reads draw commands.
    /// The native context for the retained function table, or a compatible share-group context,
    /// must be current on this thread.
    pub fn destroy_device_objects(&mut self, imgui_context: &mut ImGuiContext) -> RenderResult<()> {
        let gl = self
            .gl_context
            .clone()
            .ok_or(RenderError::MissingGlContext)?;
        self.destroy_device_objects_with_context_inner(&gl, imgui_context)
    }

    /// Destroy OpenGL device objects using an externally managed context.
    ///
    /// `gl` must be the live function table used to create this renderer, and its corresponding
    /// OpenGL context must be current on this thread. The renderer remains attached and retries
    /// device-object creation at the next render.
    pub fn destroy_device_objects_with_context(
        &mut self,
        gl: &Context,
        imgui_context: &mut ImGuiContext,
    ) -> RenderResult<()> {
        self.destroy_device_objects_with_context_inner(gl, imgui_context)
    }

    fn destroy_device_objects_with_context_inner(
        &mut self,
        gl: &Context,
        imgui_context: &mut ImGuiContext,
    ) -> RenderResult<()> {
        self.ensure_context_matches(imgui_context)?;
        let consumer = self
            .renderer_consumer
            .take()
            .ok_or(RenderError::RendererNotAttached)?;
        let reset = match imgui_context.prepare_renderer_texture_reset(&consumer) {
            Ok(reset) => reset,
            Err(error) => {
                self.renderer_consumer = Some(consumer);
                return Err(error.into());
            }
        };
        if let Err(error) = self.destroy_device_objects_only(gl) {
            drop(reset);
            self.renderer_consumer = Some(consumer);
            return Err(error);
        }
        reset.commit();
        self.destroyed_managed_textures.clear();
        self.renderer_consumer = Some(consumer);
        Ok(())
    }

    fn destroy_device_objects_only(&mut self, gl: &Context) -> RenderResult<()> {
        // A TextureMap is application-provided code. Run it before any irreversible GL deletion
        // and catch panics so a failed release cannot leave Context texture bindings pointing at
        // already-deleted GPU resources.
        self.clear_texture_map_for_release()?;
        self.destroy_device_objects_after_texture_map_clear(gl);
        Ok(())
    }

    fn clear_texture_map_for_release(&mut self) -> RenderResult<()> {
        let texture_map = self
            .texture_map
            .as_deref_mut()
            .ok_or(RenderError::RendererDestroyed)?;
        catch_unwind(AssertUnwindSafe(|| texture_map.clear()))
            .map_err(|_| RenderError::TextureMapCleanupPanicked)
    }

    fn destroy_device_objects_after_texture_map_clear(&mut self, gl: &Context) {
        if let Some(vbo) = self.vbo_handle.take() {
            unsafe { gl.delete_buffer(vbo) };
        }
        if let Some(ebo) = self.ebo_handle.take() {
            unsafe { gl.delete_buffer(ebo) };
        }
        if let Some(program) = self.shaders.program.take() {
            unsafe { gl.delete_program(program) };
        }
        if let Some(samplers) = self.samplers.take() {
            samplers.destroy(gl);
        }
        for texture in self.owned_textures.drain(..) {
            unsafe { gl.delete_texture(texture) };
        }
        self.managed_textures.clear();
    }

    pub(super) fn destroy_gpu_resources_only(&mut self, gl: &Context) -> RenderResult<()> {
        self.destroy_device_objects_only(gl)?;
        Ok(())
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
        // `Drop` has no mutable Context, so it cannot prove the consumer is idle or commit the
        // Context texture-reset transaction. Fail closed by withdrawing only our exact raw
        // publication; explicit shutdown remains the resource-release path.
        if let Some(binding) = self.context_binding.clone() {
            let _ = binding.try_with_bound_context(|| unsafe {
                self.clear_owned_renderer_state_bound();
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::ffi::c_void;
    use std::num::NonZeroU32;
    use std::rc::Rc;

    use dear_imgui_rs::{
        BackendFlags, Context as ImGuiContext, FramePrepareOptions, TextureFormat, TextureId, sys,
    };

    use super::{GlowRenderer, PendingDeviceObjects};
    use crate::error::InitResult;
    use crate::shaders::test_support::{
        FakeFailure, FakeSnapshot, TEST_LOCK, fake_gl, reset, snapshot,
    };
    use crate::texture::TextureMap;
    use crate::{GlTexture, GlVersion, InitError, RenderError, SimpleTextureMap, shaders::Shaders};

    unsafe extern "C" fn foreign_draw_callback(
        _draw_list: *const sys::ImDrawList,
        _draw_cmd: *const sys::ImDrawCmd,
    ) {
    }

    struct PanicOnceTextureMap {
        inner: SimpleTextureMap,
        panic_on_clear: Cell<bool>,
    }

    impl PanicOnceTextureMap {
        fn new() -> Self {
            Self {
                inner: SimpleTextureMap::default(),
                panic_on_clear: Cell::new(true),
            }
        }
    }

    impl TextureMap for PanicOnceTextureMap {
        fn get(&self, texture_id: TextureId) -> Option<GlTexture> {
            self.inner.get(texture_id)
        }

        fn set(&mut self, texture_id: TextureId, gl_texture: GlTexture) {
            self.inner.set(texture_id, gl_texture);
        }

        fn remove(&mut self, texture_id: TextureId) -> Option<GlTexture> {
            self.inner.remove(texture_id)
        }

        fn clear(&mut self) {
            if self.panic_on_clear.replace(false) {
                panic!("injected TextureMap::clear panic");
            }
            self.inner.clear();
        }

        fn register_texture(
            &mut self,
            gl_texture: GlTexture,
            width: u32,
            height: u32,
            format: TextureFormat,
        ) -> InitResult<TextureId> {
            self.inner
                .register_texture(gl_texture, width, height, format)
        }

        fn update_texture(
            &mut self,
            texture_id: TextureId,
            gl_texture: GlTexture,
            width: u32,
            height: u32,
        ) {
            self.inner
                .update_texture(texture_id, gl_texture, width, height);
        }

        fn texture_format(&self, texture_id: TextureId) -> Option<TextureFormat> {
            self.inner.texture_format(texture_id)
        }
    }

    fn renderer_with_existing_buffers() -> GlowRenderer {
        GlowRenderer {
            shaders: Shaders {
                program: None,
                attrib_location_tex: None,
                attrib_location_proj_mtx: None,
                attrib_location_color_gamma: None,
                attrib_location_vtx_pos: 0,
                attrib_location_vtx_uv: 0,
                attrib_location_vtx_color: 0,
            },
            vbo_handle: Some(glow::NativeBuffer(NonZeroU32::new(71).unwrap())),
            ebo_handle: Some(glow::NativeBuffer(NonZeroU32::new(72).unwrap())),
            owned_textures: Vec::new(),
            samplers: None,
            gl_version: GlVersion {
                major: 3,
                minor: 3,
                is_es: false,
            },
            has_clip_origin_support: false,
            has_separate_polygon_modes: false,
            has_sampler_object_support: true,
            gl_context: None,
            context_binding: None,
            backend_user_data: Box::default(),
            renderer_name_ptr: std::ptr::null(),
            renderer_texture_max: [0, 0],
            renderer_state_fault: None,
            synthetic_test_renderer: true,
            texture_map: Some(Box::new(SimpleTextureMap::default())),
            managed_textures: std::collections::HashMap::new(),
            destroyed_managed_textures: std::collections::HashMap::new(),
            renderer_consumer: None,
            framebuffer_srgb: false,
            color_gamma_override: None,
            viewport_clear_color: [0.0, 0.0, 0.0, 1.0],
        }
    }

    fn pending_empty_frame<'context>(
        context: &'context mut ImGuiContext,
        renderer: &GlowRenderer,
    ) -> dear_imgui_rs::render::PendingFrame<'context> {
        context.prepare_frame(
            FramePrepareOptions::new([0.0, 0.0], 1.0 / 60.0).renderer_has_textures(),
        );
        context
            .begin_frame()
            .render(renderer.renderer_consumer().unwrap())
    }

    #[test]
    fn device_object_creation_failure_preserves_renderer_and_cleans_temporaries() {
        let _guard = TEST_LOCK.lock().unwrap();
        for (failure, expected) in [
            (
                FakeFailure::BufferCreate(1),
                FakeSnapshot {
                    deleted_shaders: 2,
                    deleted_programs: 1,
                    ..FakeSnapshot::default()
                },
            ),
            (
                FakeFailure::BufferCreate(2),
                FakeSnapshot {
                    deleted_shaders: 2,
                    deleted_programs: 1,
                    deleted_buffers: 1,
                    generated_buffers: 1,
                    ..FakeSnapshot::default()
                },
            ),
            (
                FakeFailure::SamplerCreate(1),
                FakeSnapshot {
                    deleted_shaders: 2,
                    deleted_programs: 1,
                    deleted_buffers: 2,
                    generated_buffers: 2,
                    ..FakeSnapshot::default()
                },
            ),
            (
                FakeFailure::SamplerCreate(2),
                FakeSnapshot {
                    deleted_shaders: 2,
                    deleted_programs: 1,
                    deleted_buffers: 2,
                    generated_buffers: 2,
                    deleted_samplers: 1,
                    generated_samplers: 1,
                    ..FakeSnapshot::default()
                },
            ),
        ] {
            reset(failure);
            let gl = fake_gl();
            let mut renderer = renderer_with_existing_buffers();
            let original_vbo = renderer.vbo_handle;
            let original_ebo = renderer.ebo_handle;

            let result = renderer.ensure_device_objects(&gl);
            match failure {
                FakeFailure::BufferCreate(_) => assert!(matches!(
                    result,
                    Err(RenderError::DeviceObjectInit(
                        InitError::CreateBufferObject(_)
                    ))
                )),
                FakeFailure::SamplerCreate(_) => assert!(matches!(
                    result,
                    Err(RenderError::DeviceObjectInit(InitError::CreateSampler(_)))
                )),
                _ => unreachable!("this table only covers device-object allocation failures"),
            }
            assert!(renderer.shaders.program.is_none());
            assert_eq!(renderer.vbo_handle, original_vbo);
            assert_eq!(renderer.ebo_handle, original_ebo);
            assert!(!renderer.device_objects_ready());
            assert_eq!(snapshot(), expected);
        }
    }

    #[test]
    fn successful_device_recreation_retires_each_sampler_generation_once() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset(FakeFailure::None);
        let gl = fake_gl();
        let mut renderer = renderer_with_existing_buffers();

        PendingDeviceObjects::create_all(
            &gl,
            renderer.gl_version,
            renderer.has_sampler_object_support,
        )
        .unwrap()
        .commit(&mut renderer);
        assert_eq!(snapshot().generated_samplers, 2);
        assert_eq!(snapshot().deleted_samplers, 0);

        PendingDeviceObjects::create_all(
            &gl,
            renderer.gl_version,
            renderer.has_sampler_object_support,
        )
        .unwrap()
        .commit(&mut renderer);
        assert_eq!(snapshot().generated_samplers, 4);
        assert_eq!(snapshot().deleted_samplers, 2);

        renderer.destroy_device_objects_after_texture_map_clear(&gl);
        assert_eq!(snapshot().generated_samplers, 4);
        assert_eq!(snapshot().deleted_samplers, 4);
        assert!(renderer.samplers.is_none());
    }

    #[test]
    fn owned_render_rebuilds_destroyed_device_objects() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset(FakeFailure::None);
        let gl = Rc::new(fake_gl());
        let mut context = ImGuiContext::create();
        let mut renderer = GlowRenderer::with_shared_context(
            Rc::clone(&gl),
            &mut context,
            Box::new(SimpleTextureMap::default()),
        )
        .unwrap();

        renderer.destroy_device_objects(&mut context).unwrap();
        assert!(!renderer.device_objects_ready());

        let frame = pending_empty_frame(&mut context, &renderer);
        renderer.render(frame).unwrap();
        assert!(renderer.device_objects_ready());

        renderer.shutdown(&mut context).unwrap();
    }

    #[test]
    fn external_render_rebuilds_destroyed_device_objects_and_retries_failure() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset(FakeFailure::None);
        let gl = fake_gl();
        let mut context = ImGuiContext::create();
        let mut renderer = GlowRenderer::with_external_context(
            &gl,
            &mut context,
            Box::new(SimpleTextureMap::default()),
        )
        .unwrap();

        renderer
            .destroy_device_objects_with_context(&gl, &mut context)
            .unwrap();
        assert!(!renderer.device_objects_ready());

        reset(FakeFailure::BufferCreate(1));
        let frame = pending_empty_frame(&mut context, &renderer);
        assert!(matches!(
            renderer.render_with_context(&gl, frame),
            Err(RenderError::DeviceObjectInit(
                InitError::CreateBufferObject(_)
            ))
        ));
        assert!(!renderer.device_objects_ready());
        assert_eq!(snapshot().generated_textures, 0);

        reset(FakeFailure::None);
        let frame = pending_empty_frame(&mut context, &renderer);
        renderer.render_with_context(&gl, frame).unwrap();
        assert!(renderer.device_objects_ready());
        assert!(snapshot().generated_textures > 0);

        renderer.shutdown_with_context(&gl, &mut context).unwrap();
    }

    #[test]
    fn framebuffer_srgb_is_an_explicitly_fallible_desktop_capability() {
        let mut renderer = renderer_with_existing_buffers();
        renderer.gl_version.is_es = true;

        assert!(matches!(
            renderer.set_framebuffer_srgb_enabled(true),
            Err(RenderError::FramebufferSrgbUnsupported)
        ));
        assert!(!renderer.framebuffer_srgb);
        renderer.set_framebuffer_srgb_enabled(false).unwrap();

        renderer.gl_version.is_es = false;
        renderer.set_framebuffer_srgb_enabled(true).unwrap();
        assert!(renderer.framebuffer_srgb);
    }

    #[test]
    fn terminal_shutdown_rejects_every_safe_resource_entry_before_gl_work() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset(FakeFailure::None);
        let gl = fake_gl();
        let mut context = ImGuiContext::create();
        let mut renderer = GlowRenderer::with_external_context(
            &gl,
            &mut context,
            Box::new(SimpleTextureMap::default()),
        )
        .unwrap();

        renderer.shutdown_with_context(&gl, &mut context).unwrap();
        let before = snapshot();
        assert!(matches!(
            renderer.ensure_device_objects(&gl),
            Err(RenderError::RendererDestroyed)
        ));
        assert!(matches!(
            renderer.register_texture_with_context(
                &gl,
                1,
                1,
                TextureFormat::RGBA32,
                &[255, 255, 255, 255],
            ),
            Err(RenderError::RendererDestroyed)
        ));
        assert!(matches!(
            renderer.update_texture_with_context(
                &gl,
                TextureId::new(1),
                1,
                1,
                &[255, 255, 255, 255],
            ),
            Err(RenderError::RendererDestroyed)
        ));
        assert!(matches!(
            renderer.register_texture(1, 1, TextureFormat::RGBA32, &[255, 255, 255, 255],),
            Err(RenderError::RendererDestroyed)
        ));
        assert!(matches!(
            renderer.update_texture(TextureId::new(1), 1, 1, &[255, 255, 255, 255],),
            Err(RenderError::RendererDestroyed)
        ));
        assert_eq!(snapshot(), before);
    }

    #[test]
    fn texture_map_clear_panic_preserves_the_reset_transaction_for_retry() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset(FakeFailure::None);
        let gl = fake_gl();
        let mut context = ImGuiContext::create();
        let mut renderer = GlowRenderer::with_external_context(
            &gl,
            &mut context,
            Box::new(PanicOnceTextureMap::new()),
        )
        .unwrap();
        let user_data = context.io().backend_renderer_user_data();
        let name = context.io().backend_renderer_name().unwrap().as_ptr();
        let flags = context.io().backend_flags();
        let before = snapshot();

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            renderer.shutdown_with_context(&gl, &mut context)
        }));
        assert!(matches!(
            result,
            Ok(Err(RenderError::TextureMapCleanupPanicked))
        ));
        assert!(renderer.renderer_consumer.is_some());
        assert!(renderer.device_objects_ready());
        assert_eq!(context.io().backend_renderer_user_data(), user_data);
        assert_eq!(context.io().backend_renderer_name().unwrap().as_ptr(), name);
        assert_eq!(context.io().backend_flags(), flags);
        assert_eq!(snapshot(), before);

        renderer.shutdown_with_context(&gl, &mut context).unwrap();
        assert!(renderer.renderer_consumer.is_none());
        assert!(context.io().backend_renderer_user_data().is_null());
        assert!(context.io().backend_renderer_name().is_none());
    }

    #[test]
    fn dropping_a_live_renderer_fails_closed_without_destroying_gpu_resources() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset(FakeFailure::None);
        let gl = Rc::new(fake_gl());
        let mut context = ImGuiContext::create();
        let renderer = GlowRenderer::with_shared_context(
            Rc::clone(&gl),
            &mut context,
            Box::new(SimpleTextureMap::default()),
        )
        .unwrap();
        let before = snapshot();

        drop(renderer);

        assert!(context.io().backend_renderer_user_data().is_null());
        assert!(context.io().backend_renderer_name().is_none());
        assert!(!context.io().backend_flags().intersects(
            BackendFlags::RENDERER_HAS_VTX_OFFSET | BackendFlags::RENDERER_HAS_TEXTURES
        ));
        assert_eq!(snapshot(), before);

        let mut replacement = GlowRenderer::with_external_context(
            &gl,
            &mut context,
            Box::new(SimpleTextureMap::default()),
        )
        .unwrap();
        replacement
            .shutdown_with_context(&gl, &mut context)
            .unwrap();
    }

    #[test]
    fn first_core_drift_is_sticky_and_revokes_renderer_capabilities() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset(FakeFailure::None);
        let gl = fake_gl();
        let mut context = ImGuiContext::create();
        let mut renderer = GlowRenderer::with_external_context(
            &gl,
            &mut context,
            Box::new(SimpleTextureMap::default()),
        )
        .unwrap();
        context
            .set_renderer_name(Some("foreign renderer".to_owned()))
            .unwrap();

        for _ in 0..2 {
            assert!(matches!(
                renderer.register_texture_with_context(
                    &gl,
                    1,
                    1,
                    TextureFormat::RGBA32,
                    &[255, 255, 255, 255],
                ),
                Err(RenderError::RendererStateDrift {
                    field: "BackendRendererName"
                })
            ));
        }
        assert!(!context.io().backend_flags().intersects(
            BackendFlags::RENDERER_HAS_VTX_OFFSET | BackendFlags::RENDERER_HAS_TEXTURES
        ));
        assert_eq!(
            context
                .io()
                .backend_renderer_name()
                .map(std::ffi::CStr::to_bytes),
            Some(b"foreign renderer".as_slice())
        );

        renderer.shutdown_with_context(&gl, &mut context).unwrap();
    }

    #[test]
    fn shutdown_preserves_a_complete_foreign_renderer_takeover() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset(FakeFailure::None);
        let gl = fake_gl();
        let mut context = ImGuiContext::create();
        let mut renderer = GlowRenderer::with_external_context(
            &gl,
            &mut context,
            Box::new(SimpleTextureMap::default()),
        )
        .unwrap();
        let mut foreign_user_data = 0_u8;
        let foreign_user_data_ptr = std::ptr::from_mut(&mut foreign_user_data).cast::<c_void>();
        context
            .set_renderer_name(Some("foreign renderer".to_owned()))
            .unwrap();
        let foreign_name_ptr = context.io().backend_renderer_name().unwrap().as_ptr();
        #[cfg(feature = "multi-viewport")]
        let foreign_flags = BackendFlags::RENDERER_HAS_VTX_OFFSET
            | BackendFlags::RENDERER_HAS_TEXTURES
            | BackendFlags::RENDERER_HAS_VIEWPORTS;
        #[cfg(not(feature = "multi-viewport"))]
        let foreign_flags =
            BackendFlags::RENDERER_HAS_VTX_OFFSET | BackendFlags::RENDERER_HAS_TEXTURES;
        unsafe {
            context
                .io_mut()
                .set_backend_renderer_user_data(foreign_user_data_ptr);
            context.io_mut().set_backend_flags(foreign_flags);
            let platform_io = context.platform_io_mut();
            platform_io.set_draw_callback_reset_render_state_raw(Some(foreign_draw_callback));
            platform_io.set_draw_callback_set_sampler_linear_raw(Some(foreign_draw_callback));
            platform_io.set_draw_callback_set_sampler_nearest_raw(Some(foreign_draw_callback));
            let raw = &mut *platform_io.as_raw_mut();
            raw.Renderer_TextureMaxWidth = 4096;
            raw.Renderer_TextureMaxHeight = 2048;
        }

        renderer.shutdown_with_context(&gl, &mut context).unwrap();

        assert_eq!(
            context.io().backend_renderer_user_data(),
            foreign_user_data_ptr
        );
        assert_eq!(
            context.io().backend_renderer_name().unwrap().as_ptr(),
            foreign_name_ptr
        );
        assert_eq!(context.io().backend_flags() & foreign_flags, foreign_flags);
        let platform_io = context.platform_io();
        let raw = unsafe { &*platform_io.as_raw() };
        assert!(raw.DrawCallback_ResetRenderState.is_some_and(|callback| {
            std::ptr::fn_addr_eq(
                callback,
                foreign_draw_callback
                    as unsafe extern "C" fn(*const sys::ImDrawList, *const sys::ImDrawCmd),
            )
        }));
        assert!(raw.DrawCallback_SetSamplerLinear.is_some_and(|callback| {
            std::ptr::fn_addr_eq(
                callback,
                foreign_draw_callback
                    as unsafe extern "C" fn(*const sys::ImDrawList, *const sys::ImDrawCmd),
            )
        }));
        assert!(raw.DrawCallback_SetSamplerNearest.is_some_and(|callback| {
            std::ptr::fn_addr_eq(
                callback,
                foreign_draw_callback
                    as unsafe extern "C" fn(*const sys::ImDrawList, *const sys::ImDrawCmd),
            )
        }));
        assert_eq!(raw.Renderer_TextureMaxWidth, 4096);
        assert_eq!(raw.Renderer_TextureMaxHeight, 2048);

        // Model the foreign backend's own shutdown before Dear ImGui destroys the Context.
        unsafe {
            context
                .io_mut()
                .set_backend_renderer_user_data(std::ptr::null_mut());
            context.io_mut().set_backend_flags(BackendFlags::empty());
            let platform_io = context.platform_io_mut();
            platform_io.set_draw_callback_reset_render_state_raw(None);
            platform_io.set_draw_callback_set_sampler_linear_raw(None);
            platform_io.set_draw_callback_set_sampler_nearest_raw(None);
            let raw = &mut *platform_io.as_raw_mut();
            raw.Renderer_TextureMaxWidth = 0;
            raw.Renderer_TextureMaxHeight = 0;
        }
        context.set_renderer_name(None::<String>).unwrap();
    }
}
