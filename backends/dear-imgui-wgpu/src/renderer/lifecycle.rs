use dear_imgui_rs::Context;
#[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
use dear_imgui_rs::{ContextError, ContextTeardown};

use super::{
    WgpuRenderer,
    callbacks::{
        draw_callback_matches, draw_callback_reset_render_state, draw_callback_set_sampler_linear,
        draw_callback_set_sampler_nearest,
    },
};
use crate::{FrameResourceArena, RenderResources, RendererError, RendererResult, ShaderManager};

impl WgpuRenderer {
    pub(super) fn prepare_frame_epoch(
        &mut self,
        epoch: u64,
        native_frame_count: i32,
    ) -> RendererResult<()> {
        let needs_recreation = match &self.gpu.backend_data {
            Some(backend_data) => {
                self.ensure_context_alive()?;
                self.ensure_renderer_contract()?;
                !backend_data.is_initialized()
            }
            None => {
                return Err(RendererError::InvalidRenderState(
                    "renderer is not initialized".to_owned(),
                ));
            }
        };

        if needs_recreation {
            let mut backend_data = self
                .gpu
                .backend_data
                .take()
                .expect("prepare_frame_epoch() already verified backend data");
            let result = self.create_device_objects(&mut backend_data);
            self.gpu.backend_data = Some(backend_data);
            result?;
        }
        self.gpu
            .backend_data
            .as_mut()
            .expect("prepare_frame_epoch() already verified backend data")
            .begin_frame(epoch, native_frame_count)
    }

    /// Invalidate renderer-owned device objects and reset managed texture bindings.
    ///
    /// This corresponds to `ImGui_ImplWGPU_InvalidateDeviceObjects`. Passing the context makes
    /// destroying renderer-owned GPU textures and requeueing Context-owned uploads one operation.
    /// Application-owned external texture handles remain registered on this same live device.
    /// This does not replace a lost WGPU device. After actual device loss, shut down this renderer,
    /// construct a new one with the replacement device and queue, and register new external views.
    pub fn invalidate_device_objects(&mut self, imgui_context: &mut Context) -> RendererResult<()> {
        self.ensure_context_matches(imgui_context)?;
        self.ensure_renderer_contract()?;
        let consumer = self
            .renderer_consumer
            .take()
            .ok_or(RendererError::ContextNotBound)?;
        let reset = match imgui_context.prepare_renderer_texture_reset(&consumer) {
            Ok(reset) => reset,
            Err(error) => {
                self.renderer_consumer = Some(consumer);
                return Err(error.into());
            }
        };

        self.invalidate_device_objects_only();
        reset.commit();
        self.gpu.texture_manager.clear_destroyed_managed_textures();
        self.renderer_consumer = Some(consumer);

        #[cfg(feature = "tracing")]
        tracing::debug!(context = ?imgui_context.id(), "invalidated WGPU device objects");

        Ok(())
    }

    pub(super) fn invalidate_device_objects_only(&mut self) {
        if let Some(ref mut backend_data) = self.gpu.backend_data {
            backend_data.pipeline_state = None;
            backend_data.render_resources = RenderResources::new();

            // Clear frame resources
            backend_data.frame_resources = FrameResourceArena::new();
            backend_data.frame_cursor = Default::default();
        }

        self.gpu.texture_manager.clear_managed_textures();
        self.gpu.default_texture = None;
        self.gpu.shader_manager = ShaderManager::new();
    }

    fn release_all_device_objects_only(&mut self) {
        self.invalidate_device_objects_only();
        self.gpu.texture_manager.clear_external_views();
    }

    /// Shutdown the renderer and detach its Dear ImGui state.
    ///
    /// This corresponds to ImGui_ImplWGPU_Shutdown in the C++ implementation.
    ///
    /// The matching context is required so managed texture IDs, backend flags, the renderer name,
    /// and standard draw callbacks cannot outlive the GPU resources they describe. An initialized
    /// renderer consumed by a multi-viewport runtime is intentionally unavailable through this
    /// method until that owning runtime completes teardown.
    pub fn shutdown(&mut self, imgui_context: &mut Context) -> RendererResult<()> {
        self.ensure_context_matches(imgui_context)?;
        // Reset is the transactional preflight for teardown. Outstanding epochs leave
        // the renderer, GPU resources, token, and raw Context contract intact for a later retry.
        let consumer = self
            .renderer_consumer
            .take()
            .ok_or(RendererError::ContextNotBound)?;
        let reset = match imgui_context.prepare_renderer_texture_reset(&consumer) {
            Ok(reset) => reset,
            Err(error) => {
                self.renderer_consumer = Some(consumer);
                return Err(error.into());
            }
        };

        self.release_all_device_objects_only();
        reset.commit();
        self.gpu.texture_manager.clear_destroyed_managed_textures();
        self.gpu.backend_data = None;
        drop(consumer);
        self.clear_bound_imgui_context(imgui_context);
        Ok(())
    }

    /// Validates that shutdown can acquire its renderer reset permit without mutating renderer
    /// state. Multi-viewport teardown calls this before it destroys surfaces or releases callback
    /// slots, so an outstanding renderer epoch leaves the whole runtime retryable.
    #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
    pub(super) fn preflight_shutdown(&self, imgui_context: &mut Context) -> RendererResult<()> {
        self.ensure_context_matches(imgui_context)?;
        let consumer = self
            .renderer_consumer
            .as_ref()
            .ok_or(RendererError::ContextNotBound)?;
        // Dropping an uncommitted permit is intentionally inert. This validates the exact
        // consumer generation now; `shutdown` prepares it again immediately before commit.
        let _reset = imgui_context.prepare_renderer_texture_reset(consumer)?;
        Ok(())
    }

    pub(super) fn owned_draw_callbacks_match(
        platform_io: &dear_imgui_rs::platform_io::PlatformIo,
    ) -> bool {
        draw_callback_matches(
            platform_io.draw_callback_reset_render_state_raw(),
            draw_callback_reset_render_state,
        ) && draw_callback_matches(
            platform_io.draw_callback_set_sampler_linear_raw(),
            draw_callback_set_sampler_linear,
        ) && draw_callback_matches(
            platform_io.draw_callback_set_sampler_nearest_raw(),
            draw_callback_set_sampler_nearest,
        ) && platform_io
            .draw_callback_set_sampler_from_texture_raw()
            .is_none()
    }

    pub(super) fn owns_any_standard_draw_callback(
        platform_io: &dear_imgui_rs::platform_io::PlatformIo,
    ) -> bool {
        draw_callback_matches(
            platform_io.draw_callback_reset_render_state_raw(),
            draw_callback_reset_render_state,
        ) || draw_callback_matches(
            platform_io.draw_callback_set_sampler_linear_raw(),
            draw_callback_set_sampler_linear,
        ) || draw_callback_matches(
            platform_io.draw_callback_set_sampler_nearest_raw(),
            draw_callback_set_sampler_nearest,
        )
    }

    pub(super) fn clear_owned_draw_callbacks(
        platform_io: &mut dear_imgui_rs::platform_io::PlatformIo,
    ) {
        if draw_callback_matches(
            platform_io.draw_callback_reset_render_state_raw(),
            draw_callback_reset_render_state,
        ) {
            unsafe { platform_io.set_draw_callback_reset_render_state_raw(None) };
        }
        if draw_callback_matches(
            platform_io.draw_callback_set_sampler_linear_raw(),
            draw_callback_set_sampler_linear,
        ) {
            unsafe { platform_io.set_draw_callback_set_sampler_linear_raw(None) };
        }
        if draw_callback_matches(
            platform_io.draw_callback_set_sampler_nearest_raw(),
            draw_callback_set_sampler_nearest,
        ) {
            unsafe { platform_io.set_draw_callback_set_sampler_nearest_raw(None) };
        }
        // Renderer_RenderState is a transient draw-scope field and is never owned by the core
        // WGPU renderer between calls. Do not clear a foreign value during teardown.
    }

    #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
    pub(super) fn shutdown_during_context_teardown(
        &mut self,
        context: &ContextTeardown<'_>,
        release_viewports: impl FnOnce() -> Result<(), ContextError>,
    ) -> Result<(), ContextError> {
        let consumer = self.renderer_consumer.take().ok_or_else(|| {
            ContextError::attachment_teardown(
                "WGPU viewport renderer lost its renderer consumer before Context teardown",
            )
        })?;

        // Context-owned teardown has no retry channel, but it still follows the normal reset
        // contract exactly: validate the idle consumer, release sidecars and every GPU texture,
        // then commit native binding invalidation. A failed preflight or sidecar release restores
        // the consumer and leaves the renderer fields intact for the fail-stop Context owner.
        let result = context.with_renderer_texture_reset(&consumer, || {
            release_viewports()?;
            self.release_all_device_objects_only();
            self.gpu.backend_data = None;
            Ok(())
        });
        match result {
            Ok(()) => {
                self.gpu.texture_manager.clear_destroyed_managed_textures();
                drop(consumer);
                self.clear_context_state();
                Ok(())
            }
            Err(error) => {
                self.renderer_consumer = Some(consumer);
                Err(error)
            }
        }
    }

    #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
    pub(super) fn shutdown_after_context_destroyed(&mut self) {
        self.release_all_device_objects_only();
        self.gpu.backend_data = None;
        self.renderer_consumer = None;
        self.clear_context_state();
    }
}

impl Drop for WgpuRenderer {
    fn drop(&mut self) {
        self.clear_context_state();
        // WGPU command buffers and pending submissions keep their own resource references.
        // Releasing these handles leaves any already recorded GPU work valid.
        self.release_all_device_objects_only();
        self.gpu.backend_data = None;
        if let Some(consumer) = self.renderer_consumer.take() {
            consumer.reset_signal().mark_renderer_dropped();
        }
    }
}

#[cfg(test)]
mod tests {
    use dear_imgui_rs::{
        BackendFlags, Context, sys,
        texture::{OwnedTextureData, TextureFormat, TextureStatus},
    };

    use super::WgpuRenderer;
    use crate::{
        RendererError,
        renderer::callbacks::{
            draw_callback_reset_render_state, draw_callback_set_sampler_linear,
            draw_callback_set_sampler_nearest,
        },
    };

    fn configured_test_renderer(context: &mut Context) -> WgpuRenderer {
        let (flags, _) = WgpuRenderer::configure_imgui_context(context)
            .expect("fresh Context should accept WGPU renderer state");
        let mut renderer = WgpuRenderer::empty();
        renderer
            .bind_context(context, flags)
            .expect("configured Context should bind once");
        renderer.renderer_consumer = Some(
            context
                .create_synchronous_renderer_consumer()
                .expect("test Context should create a renderer consumer"),
        );
        renderer
    }

    unsafe extern "C" fn foreign_draw_callback(
        _parent_list: *const sys::ImDrawList,
        _cmd: *const sys::ImDrawCmd,
    ) {
    }

    fn renderer_flags() -> BackendFlags {
        BackendFlags::RENDERER_HAS_TEXTURES | BackendFlags::RENDERER_HAS_VTX_OFFSET
    }

    fn install_complete_foreign_renderer_takeover(context: &mut Context) {
        context
            .set_renderer_name(Some("foreign-renderer".to_owned()))
            .unwrap();
        unsafe {
            context
                .io_mut()
                .set_backend_renderer_user_data(std::ptr::dangling_mut::<u8>().cast());
            let platform_io = context.platform_io_mut();
            platform_io.set_draw_callback_reset_render_state_raw(Some(foreign_draw_callback));
            platform_io.set_draw_callback_set_sampler_linear_raw(Some(foreign_draw_callback));
            platform_io.set_draw_callback_set_sampler_nearest_raw(Some(foreign_draw_callback));
        }
    }

    fn assert_complete_foreign_takeover_is_preserved(context: &Context) {
        assert_eq!(
            context.io().backend_renderer_name().unwrap().to_bytes(),
            b"foreign-renderer"
        );
        assert!(!context.io().backend_renderer_user_data().is_null());
        assert!(context.io().backend_flags().contains(renderer_flags()));
        let platform_io = context.platform_io();
        for callback in [
            platform_io.draw_callback_reset_render_state_raw(),
            platform_io.draw_callback_set_sampler_linear_raw(),
            platform_io.draw_callback_set_sampler_nearest_raw(),
        ] {
            assert!(callback.is_some_and(|callback| {
                std::ptr::fn_addr_eq(
                    callback,
                    foreign_draw_callback
                        as unsafe extern "C" fn(*const sys::ImDrawList, *const sys::ImDrawCmd),
                )
            }));
        }
    }

    fn clear_complete_foreign_renderer_takeover(context: &mut Context) {
        context.set_renderer_name::<String>(None).unwrap();
        unsafe {
            context
                .io_mut()
                .set_backend_renderer_user_data(std::ptr::null_mut());
            let mut flags = context.io().backend_flags();
            flags.remove(renderer_flags());
            context.io_mut().set_backend_flags(flags);
            let platform_io = context.platform_io_mut();
            platform_io.set_draw_callback_reset_render_state_raw(None);
            platform_io.set_draw_callback_set_sampler_linear_raw(None);
            platform_io.set_draw_callback_set_sampler_nearest_raw(None);
        }
    }

    #[test]
    fn unbound_device_invalidation_preserves_managed_texture_bindings() {
        let mut context = Context::create();
        let texture = OwnedTextureData::from_pixels(TextureFormat::RGBA32, 1, 1, &[0; 4]).unwrap();
        let texture = context.register_texture(texture);

        let mut renderer = WgpuRenderer::empty();
        let result = renderer.invalidate_device_objects(&mut context);

        assert!(matches!(result, Err(RendererError::ContextNotBound)));
        context
            .with_texture(texture, |texture| {
                assert_eq!(texture.status(), TextureStatus::WantCreate);
                assert!(texture.texture_id().is_null());
            })
            .expect("registered texture should remain active");
    }

    #[test]
    fn renderer_owns_the_context_synchronous_consumer() {
        let mut context = Context::create();
        let mut renderer = configured_test_renderer(&mut context);

        let consumer = renderer
            .renderer_consumer()
            .expect("configured renderer should own a consumer");
        assert_eq!(consumer.context_id(), context.id());
        assert!(matches!(
            context.create_detached_renderer_consumer(),
            Err(dear_imgui_rs::FrameError::ConsumerAlreadyActive)
        ));

        renderer.shutdown(&mut context).unwrap();
    }

    #[test]
    fn dropping_renderer_clears_publication_and_releases_consumer() {
        let mut context = Context::create();
        let renderer = configured_test_renderer(&mut context);
        let generation = renderer.renderer_consumer().unwrap().generation();
        drop(renderer);

        assert!(context.io().backend_renderer_user_data().is_null());
        assert!(context.io().backend_renderer_name().is_none());
        assert!(!context.io().backend_flags().intersects(renderer_flags()));
        let replacement = configured_test_renderer(&mut context);
        assert!(replacement.renderer_consumer().unwrap().generation() > generation);
    }

    #[test]
    fn context_first_renderer_drop_clears_native_publication_in_teardown() {
        let mut context = Context::create();
        let renderer = configured_test_renderer(&mut context);
        let binding = context.binding();

        drop(context);

        assert!(!binding.is_alive());
        assert!(matches!(
            renderer.ensure_context_alive(),
            Err(RendererError::ContextDropped)
        ));
        drop(renderer);
    }

    #[test]
    fn foreign_context_lifecycle_calls_are_transactional() {
        let mut owner = Context::create();
        let mut renderer = configured_test_renderer(&mut owner);

        let mut foreign = Context::create();
        let foreign_flags = foreign.io().backend_flags();

        assert!(matches!(
            renderer.invalidate_device_objects(&mut foreign),
            Err(RendererError::ContextMismatch)
        ));
        assert!(matches!(
            renderer.shutdown(&mut foreign),
            Err(RendererError::ContextMismatch)
        ));
        assert_eq!(foreign.io().backend_flags(), foreign_flags);
        assert!(renderer.context_state.is_some());

        renderer
            .shutdown(&mut owner)
            .expect("matching context should shut down the test renderer");
        assert!(renderer.context_state.is_none());
        drop(foreign);
    }

    #[test]
    fn moving_renderer_preserves_published_token_and_name_identity() {
        let mut context = Context::create();
        let renderer = configured_test_renderer(&mut context);
        let token = context.io().backend_renderer_user_data();
        let name = context
            .io()
            .backend_renderer_name()
            .expect("renderer name should be published")
            .as_ptr();

        let mut moved = renderer;
        assert_eq!(context.io().backend_renderer_user_data(), token);
        assert_eq!(context.io().backend_renderer_name().unwrap().as_ptr(), name);
        moved
            .ensure_renderer_contract()
            .expect("moving WgpuRenderer must preserve its raw identities");
        moved.shutdown(&mut context).unwrap();
    }

    #[test]
    fn same_bytes_at_a_foreign_name_pointer_are_drift_and_survive_teardown() {
        let mut context = Context::create();
        let mut renderer = configured_test_renderer(&mut context);
        let owned_name = context.io().backend_renderer_name().unwrap().as_ptr();
        context
            .set_renderer_name(Some(format!(
                "dear-imgui-wgpu {}",
                env!("CARGO_PKG_VERSION")
            )))
            .unwrap();
        let foreign_name = context.io().backend_renderer_name().unwrap().as_ptr();
        assert_ne!(foreign_name, owned_name);

        assert!(matches!(
            renderer.ensure_renderer_contract(),
            Err(RendererError::RendererStateDrift {
                field: "BackendRendererName"
            })
        ));
        assert_eq!(
            context.io().backend_renderer_name().unwrap().as_ptr(),
            foreign_name
        );

        renderer.shutdown(&mut context).unwrap();
        assert_eq!(
            context.io().backend_renderer_name().unwrap().as_ptr(),
            foreign_name
        );
        context.set_renderer_name::<String>(None).unwrap();
    }

    #[test]
    fn foreign_user_data_is_drift_and_survives_teardown() {
        let mut context = Context::create();
        let mut renderer = configured_test_renderer(&mut context);
        let foreign = std::ptr::dangling_mut::<u8>().cast();
        unsafe { context.io_mut().set_backend_renderer_user_data(foreign) };

        assert!(matches!(
            renderer.ensure_renderer_contract(),
            Err(RendererError::RendererStateDrift {
                field: "BackendRendererUserData"
            })
        ));
        assert_eq!(context.io().backend_renderer_user_data(), foreign);

        renderer.shutdown(&mut context).unwrap();
        assert_eq!(context.io().backend_renderer_user_data(), foreign);
        unsafe {
            context
                .io_mut()
                .set_backend_renderer_user_data(std::ptr::null_mut())
        };
    }

    #[test]
    fn shutdown_preserves_flags_after_complete_foreign_renderer_takeover() {
        let mut context = Context::create();
        let mut renderer = configured_test_renderer(&mut context);
        install_complete_foreign_renderer_takeover(&mut context);

        assert!(matches!(
            renderer.ensure_renderer_contract(),
            Err(RendererError::RendererStateDrift {
                field: "BackendRendererUserData"
            })
        ));
        assert_complete_foreign_takeover_is_preserved(&context);

        renderer.shutdown(&mut context).unwrap();
        assert_complete_foreign_takeover_is_preserved(&context);
        clear_complete_foreign_renderer_takeover(&mut context);
    }

    #[test]
    fn drop_preserves_flags_after_complete_foreign_renderer_takeover() {
        let mut context = Context::create();
        let renderer = configured_test_renderer(&mut context);
        install_complete_foreign_renderer_takeover(&mut context);

        assert!(matches!(
            renderer.ensure_renderer_contract(),
            Err(RendererError::RendererStateDrift {
                field: "BackendRendererUserData"
            })
        ));
        drop(renderer);

        assert_complete_foreign_takeover_is_preserved(&context);
        clear_complete_foreign_renderer_takeover(&mut context);
    }

    #[test]
    fn renderer_contract_fault_is_sticky_across_raw_aba_restoration() {
        let mut context = Context::create();
        let mut renderer = configured_test_renderer(&mut context);
        let token = context.io().backend_renderer_user_data();
        let name = context.io().backend_renderer_name().unwrap().as_ptr();
        let mut flags = context.io().backend_flags();
        flags.remove(BackendFlags::RENDERER_HAS_TEXTURES);
        context.io_mut().set_backend_flags(flags);

        assert!(matches!(
            renderer.ensure_renderer_contract(),
            Err(RendererError::RendererStateDrift {
                field: "RENDERER_HAS_TEXTURES"
            })
        ));

        let io = unsafe { sys::igGetIO_ContextPtr(context.as_raw()) };
        unsafe {
            (*io).BackendRendererUserData = token;
            (*io).BackendRendererName = name;
            (*io).BackendFlags |= (BackendFlags::RENDERER_HAS_VTX_OFFSET
                | BackendFlags::RENDERER_HAS_TEXTURES)
                .bits();
            let platform_io = context.platform_io_mut();
            platform_io
                .set_draw_callback_reset_render_state_raw(Some(draw_callback_reset_render_state));
            platform_io
                .set_draw_callback_set_sampler_linear_raw(Some(draw_callback_set_sampler_linear));
            platform_io
                .set_draw_callback_set_sampler_nearest_raw(Some(draw_callback_set_sampler_nearest));
        }

        assert!(matches!(
            renderer.ensure_renderer_contract(),
            Err(RendererError::RendererStateDrift {
                field: "RENDERER_HAS_TEXTURES"
            })
        ));
        assert!(context.io().backend_renderer_user_data().is_null());
        assert!(context.io().backend_renderer_name().is_none());
        assert!(!context.io().backend_flags().intersects(
            BackendFlags::RENDERER_HAS_VTX_OFFSET | BackendFlags::RENDERER_HAS_TEXTURES
        ));

        renderer.shutdown(&mut context).unwrap();
    }

    #[test]
    fn explicit_shutdown_revokes_context_state_and_allows_replacement() {
        let mut context = Context::create();
        let mut renderer = configured_test_renderer(&mut context);
        let mut foreign = Context::create();
        let mut foreign_renderer = configured_test_renderer(&mut foreign);
        renderer.shutdown(&mut context).unwrap();

        assert_eq!(unsafe { sys::igGetCurrentContext() }, foreign.as_raw());
        foreign_renderer.ensure_renderer_contract().unwrap();
        assert!(context.io().backend_renderer_user_data().is_null());
        assert!(context.io().backend_renderer_name().is_none());
        assert!(!context.io().backend_flags().intersects(
            BackendFlags::RENDERER_HAS_VTX_OFFSET | BackendFlags::RENDERER_HAS_TEXTURES
        ));
        assert!(
            context
                .platform_io()
                .draw_callback_reset_render_state_raw()
                .is_none()
        );
        assert!(
            context
                .platform_io()
                .draw_callback_set_sampler_linear_raw()
                .is_none()
        );
        assert!(
            context
                .platform_io()
                .draw_callback_set_sampler_nearest_raw()
                .is_none()
        );

        foreign_renderer.shutdown(&mut foreign).unwrap();
        drop(foreign);
        let mut replacement = configured_test_renderer(&mut context);
        replacement.shutdown(&mut context).unwrap();
    }
}
