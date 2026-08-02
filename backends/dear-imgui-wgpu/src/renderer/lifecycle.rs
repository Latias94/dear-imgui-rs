use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use super::{
    WgpuRenderer,
    callbacks::{
        draw_callback_matches, draw_callback_reset_render_state, draw_callback_set_sampler_linear,
        draw_callback_set_sampler_nearest,
    },
    core::{RendererContextState, RendererPublication},
};
use crate::{FrameResources, RenderResources, RendererError, RendererResult, ShaderManager};
use dear_imgui_rs::{
    Context, ContextAttachment, ContextAttachmentError, ContextAttachmentLease,
    ContextAttachmentRole, ContextAttachmentTeardownError, ContextBindingError, ContextDestroyed,
    ContextTeardown, render::RendererConsumer,
};

struct WgpuRendererDropAttachmentMarker;

/// GPU and snapshot state transferred out of a renderer wrapper that was dropped without an
/// explicit `shutdown`. The Context attachment releases GPU resources in its terminal renderer
/// phase; a Context that is already dropping retains the raw-publication token until native
/// destruction has made it inert.
struct DeferredRendererResources {
    context_state: Option<RendererContextState>,
    backend_data: Option<crate::WgpuBackendData>,
    shader_manager: Option<ShaderManager>,
    texture_manager: Option<crate::WgpuTextureManager>,
    default_texture: Option<wgpu::TextureView>,
    renderer_consumer: Option<RendererConsumer>,
}

impl DeferredRendererResources {
    fn release_renderer_resources(
        &mut self,
        context: &ContextTeardown<'_>,
    ) -> Result<(), ContextAttachmentTeardownError> {
        let consumer = self.renderer_consumer.take().ok_or_else(|| {
            ContextAttachmentTeardownError::new(
                "deferred WGPU renderer resources lost their renderer consumer",
            )
        })?;

        // The Context's renderer-resource phase is the only Drop fallback that can perform the
        // same prepare -> release -> commit transaction as explicit shutdown. If preflight
        // rejects an outstanding epoch, the closure does not run and the consumer is restored so
        // no renderer state is silently released out of order.
        let result = context
            .with_renderer_texture_reset(&consumer, || {
                self.backend_data.take();
                self.shader_manager.take();
                self.texture_manager.take();
                self.default_texture.take();
                Ok(())
            })
            .map(|_| ());
        if result.is_err() {
            self.renderer_consumer = Some(consumer);
        }
        result
    }

    fn release_after_context_destroyed(mut self) {
        // Reading and dropping the retained token explicitly documents why it survives the
        // renderer-resource phase when wrapper Drop races Context teardown.
        drop(self.context_state.take());
    }
}

struct RendererDropControl {
    deferred: RefCell<Option<DeferredRendererResources>>,
    publication: RefCell<Option<RendererPublication>>,
    context_destroyed: Cell<bool>,
    #[cfg(test)]
    renderer_release_count: Cell<u32>,
}

struct RendererDropAttachment {
    control: Rc<RendererDropControl>,
}

impl ContextAttachment for RendererDropAttachment {
    fn quiesce(
        &self,
        context: &ContextTeardown<'_>,
    ) -> Result<(), dear_imgui_rs::ContextAttachmentTeardownError> {
        if let Some(publication) = self.control.publication.borrow().as_ref() {
            context.with_bound_context(|| unsafe {
                publication.clear_owned_raw_state_bound();
            });
        }
        Ok(())
    }

    fn release_renderer_resources(
        &self,
        context: &ContextTeardown<'_>,
    ) -> Result<(), ContextAttachmentTeardownError> {
        if let Some(resources) = self.control.deferred.borrow_mut().as_mut() {
            resources.release_renderer_resources(context)?;
            #[cfg(test)]
            self.control
                .renderer_release_count
                .set(self.control.renderer_release_count.get() + 1);
        }
        Ok(())
    }

    fn context_destroyed(&self, _context: ContextDestroyed) {
        // A raw Context pointer can remain published only when this renderer was dropped during
        // Context teardown. At this point the core has tombstoned that pointer, so releasing the
        // final token and any late resources cannot re-enter native Dear ImGui.
        self.control.context_destroyed.set(true);
        self.control.publication.borrow_mut().take();
        if let Some(resources) = self.control.deferred.borrow_mut().take() {
            resources.release_after_context_destroyed();
        }
    }
}

/// Lease and storage that make unreported renderer Drop defer to Context teardown.
pub(super) struct RendererDropDeferral {
    control: Rc<RendererDropControl>,
    attachment: ContextAttachmentLease,
}

impl RendererDropDeferral {
    pub(super) fn register(context: &mut Context) -> Result<Self, ContextAttachmentError> {
        let control = Rc::new(RendererDropControl {
            deferred: RefCell::new(None),
            publication: RefCell::new(None),
            context_destroyed: Cell::new(false),
            #[cfg(test)]
            renderer_release_count: Cell::new(0),
        });
        let attachment = context.register_attachment::<WgpuRendererDropAttachmentMarker>(
            ContextAttachmentRole::Extension,
            Rc::new(RendererDropAttachment {
                control: Rc::clone(&control),
            }),
        )?;
        Ok(Self {
            control,
            attachment,
        })
    }

    pub(super) fn set_publication(&self, publication: RendererPublication) {
        debug_assert!(self.control.publication.borrow().is_none());
        self.control.publication.borrow_mut().replace(publication);
    }

    fn defer_to_context(self, resources: DeferredRendererResources) {
        debug_assert!(self.control.deferred.borrow().is_none());
        self.control.deferred.borrow_mut().replace(resources);
        self.attachment.defer_to_context();
    }

    #[cfg(test)]
    fn control_for_test(&self) -> Rc<RendererDropControl> {
        Rc::clone(&self.control)
    }
}

#[cfg(test)]
impl RendererDropControl {
    fn has_deferred_resources(&self) -> bool {
        self.deferred.borrow().is_some()
    }

    fn renderer_release_count(&self) -> u32 {
        self.renderer_release_count.get()
    }

    fn context_was_destroyed(&self) -> bool {
        self.context_destroyed.get()
    }
}

impl WgpuRenderer {
    /// Called every frame to prepare for rendering
    ///
    /// This corresponds to ImGui_ImplWGPU_NewFrame in the C++ implementation
    pub fn new_frame(&mut self) -> RendererResult<()> {
        let needs_recreation = match &self.backend_data {
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
                .backend_data
                .take()
                .expect("new_frame() already verified backend data");
            let result = self.create_device_objects(&mut backend_data);
            self.backend_data = Some(backend_data);
            result?;
        }
        Ok(())
    }

    /// Invalidate renderer-owned device objects and reset managed texture bindings.
    ///
    /// This corresponds to `ImGui_ImplWGPU_InvalidateDeviceObjects`. Passing the context makes
    /// destroying renderer-owned GPU textures and requeueing Context-owned uploads one operation.
    /// Application-owned external texture handles remain registered; after device loss, replace
    /// their views through [`Self::update_external_texture`] before drawing them again.
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
        let _invalidated = reset.commit();
        self.texture_manager.clear_destroyed_managed_textures();
        self.renderer_consumer = Some(consumer);

        Ok(())
    }

    pub(super) fn invalidate_device_objects_only(&mut self) {
        if let Some(ref mut backend_data) = self.backend_data {
            backend_data.pipeline_state = None;
            backend_data.render_resources = RenderResources::new();

            // Clear frame resources
            for frame_resources in &mut backend_data.frame_resources {
                *frame_resources = FrameResources::new();
            }
        }

        self.texture_manager.clear_renderer_owned_textures();
        self.default_texture = None;
        self.shader_manager = ShaderManager::new();
    }

    fn release_all_device_objects_only(&mut self) {
        self.invalidate_device_objects_only();
        self.texture_manager.clear_external_views();
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
        // Reset is the transactional preflight for teardown. Outstanding frames/snapshots leave
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
        let _invalidated = reset.commit();
        self.texture_manager.clear_destroyed_managed_textures();
        self.backend_data = None;
        drop(consumer);
        self.clear_bound_imgui_context(imgui_context);
        Ok(())
    }

    /// Validates that shutdown can acquire its renderer reset permit without mutating renderer
    /// state. Multi-viewport teardown calls this before it destroys surfaces or releases callback
    /// slots, so an outstanding frame or detached snapshot leaves the whole runtime retryable.
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
        )
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
        release_viewports: impl FnOnce() -> Result<(), ContextAttachmentTeardownError>,
    ) -> Result<(), ContextAttachmentTeardownError> {
        let consumer = self.renderer_consumer.take().ok_or_else(|| {
            ContextAttachmentTeardownError::new(
                "WGPU viewport renderer lost its renderer consumer before Context teardown",
            )
        })?;

        // Context-owned teardown has no retry channel, but it still follows the normal reset
        // contract exactly: validate the idle consumer, release sidecars and every GPU texture,
        // then commit native binding invalidation. A failed preflight or sidecar release restores
        // the consumer and leaves the renderer fields intact for the fail-stop Context owner.
        let result = context
            .with_renderer_texture_reset(&consumer, || {
                release_viewports()?;
                self.release_all_device_objects_only();
                self.backend_data = None;
                Ok(())
            })
            .map(|_| ());
        match result {
            Ok(()) => {
                self.texture_manager.clear_destroyed_managed_textures();
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
        self.backend_data = None;
        self.renderer_consumer = None;
        self.clear_context_state();
    }

    fn defer_resources_to_context(&mut self, context_state: Option<RendererContextState>) {
        // `bind_context` installs this attachment before it publishes `context_state`. Keep the
        // relationship fail-stop: silently dropping or forgetting the texture map here would
        // leave live Context bindings pointing at renderer resources that no longer exist.
        let deferral = self.drop_deferral.take().unwrap_or_else(|| {
            debug_assert!(
                false,
                "a bound WGPU renderer must retain its Context drop attachment"
            );
            std::process::abort();
        });
        let resources = DeferredRendererResources {
            context_state,
            backend_data: self.backend_data.take(),
            shader_manager: Some(std::mem::take(&mut self.shader_manager)),
            texture_manager: Some(std::mem::take(&mut self.texture_manager)),
            default_texture: self.default_texture.take(),
            renderer_consumer: self.renderer_consumer.take(),
        };
        deferral.defer_to_context(resources);
    }
}

impl Drop for WgpuRenderer {
    fn drop(&mut self) {
        let clear_result = self.context_state.as_ref().map(|state| {
            let binding = state.context();
            binding.try_with_bound_context(|| unsafe { state.clear_owned_raw_state_bound() })
        });
        match clear_result {
            Some(Ok(_)) => {
                // Exact raw publication is gone, so the token can be released immediately. The
                // GPU map and consumer remain Context-owned through the deferred attachment.
                drop(self.context_state.take());
                self.defer_resources_to_context(None);
            }
            Some(Err(error)) => {
                if matches!(error, ContextBindingError::NativeDestroyed) {
                    // Native teardown has already invalidated every Context binding. It is now
                    // safe for normal field Drop to release backend resources without entering
                    // Dear ImGui.
                    self.drop_deferral = None;
                    drop(self.context_state.take());
                } else {
                    // The raw Context still exists, so retain the publication token until
                    // `context_destroyed`. The attachment owns terminal GPU cleanup meanwhile.
                    let context_state = self.context_state.take();
                    self.defer_resources_to_context(context_state);
                }
            }
            None => {
                self.drop_deferral = None;
            }
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
                .create_renderer_consumer()
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
        let mut texture = OwnedTextureData::new();
        texture.create(TextureFormat::RGBA32, 1, 1);
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
    fn outstanding_snapshot_rejects_invalidation_without_detaching_renderer() {
        let mut context = Context::create();
        context.io_mut().set_display_size([128.0, 128.0]);
        context.io_mut().set_delta_time(1.0 / 60.0);
        let mut renderer = configured_test_renderer(&mut context);
        let snapshot = {
            let consumer = renderer
                .renderer_consumer
                .as_ref()
                .expect("configured renderer should own a consumer");
            context.begin_frame().render_snapshot(consumer).unwrap()
        };

        assert!(matches!(
            renderer.invalidate_device_objects(&mut context),
            Err(RendererError::RendererConsumer(
                dear_imgui_rs::render::RendererConsumerError::OutstandingEpochs { count: 1 }
            ))
        ));
        assert!(renderer.renderer_consumer.is_some());
        assert!(!context.io().backend_renderer_user_data().is_null());
        assert!(context.io().backend_renderer_name().is_some());

        drop(snapshot);
        context.poll_snapshot_completions().unwrap();
        renderer.invalidate_device_objects(&mut context).unwrap();
        assert!(renderer.renderer_consumer.is_some());
        assert!(!context.io().backend_renderer_user_data().is_null());
        assert!(context.io().backend_renderer_name().is_some());

        renderer.shutdown(&mut context).unwrap();
    }

    #[test]
    fn dropping_renderer_with_outstanding_snapshot_defers_to_context_teardown() {
        let mut context = Context::create();
        context.io_mut().set_display_size([128.0, 128.0]);
        context.io_mut().set_delta_time(1.0 / 60.0);
        let renderer = configured_test_renderer(&mut context);
        let control = renderer
            .drop_deferral
            .as_ref()
            .expect("bound renderer must install a deferred-drop attachment")
            .control_for_test();
        let snapshot = {
            let consumer = renderer
                .renderer_consumer
                .as_ref()
                .expect("configured renderer should own a consumer");
            context.begin_frame().render_snapshot(consumer).unwrap()
        };

        drop(renderer);

        assert!(control.has_deferred_resources());
        assert_eq!(control.renderer_release_count(), 0);
        assert!(context.io().backend_renderer_user_data().is_null());
        assert!(context.io().backend_renderer_name().is_none());
        assert!(!context.io().backend_flags().intersects(renderer_flags()));
        assert!(matches!(
            WgpuRenderer::empty().bind_context(&mut context, BackendFlags::empty()),
            Err(RendererError::ContextAttachment(
                dear_imgui_rs::ContextAttachmentError::DuplicateAttachment
            ))
        ));

        drop(snapshot);
        context.poll_snapshot_completions().unwrap();
        assert!(control.has_deferred_resources());
        assert_eq!(control.renderer_release_count(), 0);

        drop(context);

        assert_eq!(control.renderer_release_count(), 1);
        assert!(!control.has_deferred_resources());
        assert!(control.context_was_destroyed());
    }

    #[test]
    fn explicit_shutdown_retries_after_outstanding_snapshot_and_releases_deferral() {
        let mut context = Context::create();
        context.io_mut().set_display_size([128.0, 128.0]);
        context.io_mut().set_delta_time(1.0 / 60.0);
        let mut renderer = configured_test_renderer(&mut context);
        let control = renderer
            .drop_deferral
            .as_ref()
            .expect("bound renderer must install a deferred-drop attachment")
            .control_for_test();
        let snapshot = {
            let consumer = renderer
                .renderer_consumer
                .as_ref()
                .expect("configured renderer should own a consumer");
            context.begin_frame().render_snapshot(consumer).unwrap()
        };

        assert!(matches!(
            renderer.shutdown(&mut context),
            Err(RendererError::RendererConsumer(
                dear_imgui_rs::render::RendererConsumerError::OutstandingEpochs { count: 1 }
            ))
        ));
        assert!(renderer.drop_deferral.is_some());
        assert!(!control.has_deferred_resources());
        assert!(!context.io().backend_renderer_user_data().is_null());

        drop(snapshot);
        context.poll_snapshot_completions().unwrap();
        renderer.shutdown(&mut context).unwrap();
        assert!(renderer.drop_deferral.is_none());
        assert!(!control.has_deferred_resources());

        let mut replacement = configured_test_renderer(&mut context);
        replacement.shutdown(&mut context).unwrap();
    }

    #[test]
    fn context_first_renderer_drop_uses_the_native_destroyed_path() {
        let mut context = Context::create();
        let renderer = configured_test_renderer(&mut context);
        let control = renderer
            .drop_deferral
            .as_ref()
            .expect("bound renderer must install a deferred-drop attachment")
            .control_for_test();

        drop(context);

        assert!(control.context_was_destroyed());
        assert!(!control.has_deferred_resources());
        assert_eq!(control.renderer_release_count(), 0);
        drop(renderer);
    }

    #[test]
    fn foreign_context_lifecycle_calls_are_transactional() {
        let mut owner = Context::create();
        let mut renderer = configured_test_renderer(&mut owner);

        let suspended_owner = owner.suspend();
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

        let suspended_foreign = foreign.suspend();
        let mut owner = suspended_owner
            .activate()
            .expect("owner context should reactivate");
        renderer
            .shutdown(&mut owner)
            .expect("matching context should shut down the test renderer");
        assert!(renderer.context_state.is_none());
        drop(suspended_foreign);
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
        renderer.shutdown(&mut context).unwrap();

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

        let mut replacement = configured_test_renderer(&mut context);
        context.io_mut().set_display_size([128.0, 128.0]);
        context.io_mut().set_delta_time(1.0 / 60.0);
        let _ui = context.frame();
        let frame = context.render();
        drop(frame);
        replacement.shutdown(&mut context).unwrap();
    }
}
