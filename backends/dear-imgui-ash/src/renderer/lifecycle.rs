use super::*;
use std::rc::Rc;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DeviceIdleOutcome {
    Complete,
    DeviceLost,
}

pub(super) fn classify_device_idle(
    result: Result<(), ash::vk::Result>,
) -> Result<DeviceIdleOutcome, ash::vk::Result> {
    match result {
        Ok(()) => Ok(DeviceIdleOutcome::Complete),
        Err(ash::vk::Result::ERROR_DEVICE_LOST) => Ok(DeviceIdleOutcome::DeviceLost),
        Err(error) => Err(error),
    }
}

#[cfg(all(
    test,
    any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3")
))]
pub(super) fn test_render_mode() -> RenderMode {
    #[cfg(feature = "render-pass")]
    {
        RenderMode::RenderPass
    }
    #[cfg(not(feature = "render-pass"))]
    {
        RenderMode::DynamicRendering
    }
}

#[cfg(test)]
pub(super) fn renderer_for_test(context: &mut Context) -> AshRenderer {
    unsafe extern "system" fn device_idle(_: vk::Device) -> vk::Result {
        vk::Result::SUCCESS
    }
    let device = unsafe {
        Device::load_with(
            |name| {
                if name == c"vkDeviceWaitIdle" {
                    device_idle as *const std::ffi::c_void
                } else {
                    std::ptr::null()
                }
            },
            vk::Device::null(),
        )
    };
    let context_state = Rc::new(RendererContextState::prepare(context).unwrap());
    let publication_attachment = context_state
        .register_publication_attachment(context)
        .unwrap();
    let consumer = context.create_synchronous_renderer_consumer().unwrap();
    // This synthetic renderer has no Vulkan texture map or submitted consumer epoch.
    let reset = context.prepare_renderer_texture_reset(&consumer).unwrap();
    reset.commit();
    context_state.publish(context).unwrap();
    AshRenderer {
        device,
        allocator: RendererAllocator::new(Allocator::new_default(
            vk::PhysicalDeviceMemoryProperties::default(),
        )),
        queue: vk::Queue::null(),
        command_pool: vk::CommandPool::null(),
        resources: VulkanRendererResources::empty(),
        textures: TextureManager::new(),
        consumer: Some(consumer),
        context_state,
        publication_attachment: Some(publication_attachment),
        default_texture_id: 0,
        default_texture_pending: false,
        options: Options::default(),
        frames: Frames::new(0),
        destroyed: false,
        uploads: UploadRing::new(0),
        #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
        render_mode: test_render_mode(),
        #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
        viewport_pipelines: HashMap::new(),
        #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
        viewport_clear_color: [0.0, 0.0, 0.0, 1.0],
    }
}

impl AshRenderer {
    /// Create a new renderer using the internal default allocator.
    ///
    /// The provided `command_pool` is used for short-lived upload command buffers.
    ///
    /// # Safety
    ///
    /// `physical_device`, `device`, `queue`, `command_pool`, and the render target configuration
    /// must share one live Vulkan device lineage. The queue must support graphics and transfer
    /// from command buffers allocated by `command_pool`, and every command buffer recorded by
    /// this renderer must be submitted to that queue unless the application supplies equivalent
    /// cross-queue synchronization. A render pass or dynamic rendering format must remain
    /// compatible with every target used by this renderer. Keep the logical device, queue, command
    /// pool and Vulkan loader live until renderer cleanup (including `Drop`) completes. The renderer
    /// holds a device dispatch handle; the application owns the logical device. Externally
    /// synchronize upload submissions, command-pool access, and device-idle waits with application
    /// use of the same pool and device queues.
    pub unsafe fn with_default_allocator(
        instance: &Instance,
        physical_device: vk::PhysicalDevice,
        config: AshRendererConfig,
        imgui: &mut Context,
    ) -> RendererResult<Self> {
        let memory_properties =
            unsafe { instance.get_physical_device_memory_properties(physical_device) };
        let allocator = Allocator::new_default(memory_properties);

        Self::init_renderer(allocator, config, imgui)
    }

    /// Create a new renderer using a shared `gpu-allocator` allocator.
    ///
    /// # Safety
    ///
    /// The allocator, device, queue, command pool, and render target configuration must satisfy
    /// the same device-lineage and capability contract as [`Self::with_default_allocator`].
    #[cfg(feature = "gpu-allocator")]
    pub unsafe fn with_gpu_allocator(
        allocator: std::sync::Arc<std::sync::Mutex<gpu_allocator::vulkan::Allocator>>,
        config: AshRendererConfig,
        imgui: &mut Context,
    ) -> RendererResult<Self> {
        let allocator = Allocator::new_gpu(allocator);
        Self::init_renderer(allocator, config, imgui)
    }

    /// Create a new renderer using a shared `vk-mem` allocator.
    ///
    /// # Safety
    ///
    /// The allocator, device, queue, command pool, and render target configuration must satisfy
    /// the same device-lineage and capability contract as [`Self::with_default_allocator`].
    #[cfg(feature = "vk-mem")]
    pub unsafe fn with_vk_mem_allocator(
        allocator: std::sync::Arc<std::sync::Mutex<vk_mem::Allocator>>,
        config: AshRendererConfig,
        imgui: &mut Context,
    ) -> RendererResult<Self> {
        let allocator = Allocator::new_vk_mem(allocator);
        Self::init_renderer(allocator, config, imgui)
    }

    fn init_renderer(
        allocator: Allocator,
        config: AshRendererConfig,
        imgui: &mut Context,
    ) -> RendererResult<Self> {
        let AshRendererConfig {
            device,
            queue,
            command_pool,
            render_target,
            #[cfg(feature = "dynamic-rendering")]
            stencil_attachment_format,
            options,
        } = config;
        if options.in_flight_frames == 0 {
            return Err(RendererError::InvalidRenderState(
                "Options::in_flight_frames must be >= 1".to_string(),
            ));
        }
        let context_state = Rc::new(RendererContextState::prepare(imgui)?);
        let publication_attachment = context_state.register_publication_attachment(imgui)?;

        let resources = VulkanRendererResources::create(
            &device,
            render_target,
            options,
            #[cfg(feature = "dynamic-rendering")]
            stencil_attachment_format,
        )?;

        let mut renderer = Self {
            device,
            allocator: RendererAllocator::new(allocator),
            queue,
            command_pool,
            resources,
            textures: TextureManager::new(),
            consumer: None,
            context_state,
            publication_attachment: Some(publication_attachment),
            default_texture_id: 0,
            default_texture_pending: false,
            options,
            frames: Frames::new(options.in_flight_frames),
            destroyed: false,
            uploads: UploadRing::new(options.in_flight_frames),
            #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
            render_mode: render_target.mode(),
            #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
            viewport_pipelines: HashMap::new(),
            #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
            viewport_clear_color: [0.0, 0.0, 0.0, 1.0],
        };

        let initialization = (|| -> RendererResult<()> {
            renderer.default_texture_id = renderer.create_default_texture()?;
            let consumer = imgui.create_synchronous_renderer_consumer()?;
            imgui.prepare_renderer_texture_reset(&consumer)?.commit();
            renderer.consumer = Some(consumer);
            renderer.context_state.publish(imgui)
        })();
        if let Err(error) = initialization {
            renderer.destroy_unsubmitted_internal()?;
            return Err(error);
        }
        Ok(renderer)
    }
}

impl AshRenderer {
    pub(super) fn ensure_operational(&self) -> RendererResult<()> {
        if self.destroyed {
            return Err(RendererError::RendererDestroyed);
        }
        self.allocator.check_cleanup()?;
        self.context_state.validate()
    }

    pub(super) fn ensure_pending_frame_matches(
        &self,
        frame: &PendingFrame<'_>,
    ) -> RendererResult<()> {
        self.ensure_frame_matches(frame.context_id(), Some(frame.epoch()))
    }

    pub(super) fn ensure_reconciled_frame_matches(
        &self,
        frame: &ReconciledFrame<'_>,
    ) -> RendererResult<()> {
        self.ensure_frame_matches(frame.context_id(), frame.epoch())
    }

    fn ensure_frame_matches(
        &self,
        context_id: dear_imgui_rs::ContextId,
        epoch: Option<dear_imgui_rs::render::SnapshotEpoch>,
    ) -> RendererResult<()> {
        self.ensure_operational()?;
        let consumer = self
            .consumer
            .as_ref()
            .ok_or(RendererError::RendererNotAttached)?;
        if context_id != consumer.context_id() {
            return Err(RendererError::ContextMismatch {
                expected: consumer.context_id(),
                actual: context_id,
            });
        }
        let epoch = epoch.ok_or_else(|| {
            RendererError::InvalidRenderState(
                "Ash requires a managed-texture renderer epoch".to_string(),
            )
        })?;
        if epoch.consumer_generation() != consumer.generation() {
            return Err(RendererError::ConsumerGenerationMismatch {
                expected: consumer.generation(),
                actual: epoch.consumer_generation(),
            });
        }
        Ok(())
    }

    pub(super) fn ensure_context_matches(&self, imgui_context: &Context) -> RendererResult<()> {
        let consumer = self
            .consumer
            .as_ref()
            .ok_or(RendererError::RendererNotAttached)?;
        if imgui_context.id() != consumer.context_id() {
            return Err(RendererError::ContextMismatch {
                expected: consumer.context_id(),
                actual: imgui_context.id(),
            });
        }
        Ok(())
    }

    pub fn options(&self) -> Options {
        self.options
    }

    /// Set clear color for secondary viewports (multi-viewport mode).
    #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
    pub fn set_viewport_clear_color(&mut self, color: [f32; 4]) {
        self.viewport_clear_color = color;
    }

    /// Get clear color for secondary viewports (multi-viewport mode).
    #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
    pub fn viewport_clear_color(&self) -> [f32; 4] {
        self.viewport_clear_color
    }

    pub(super) fn gamma(&self) -> f32 {
        self.shader_gamma(self.options.framebuffer_srgb)
    }

    #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
    pub(super) fn gamma_for_format(&self, format: vk::Format) -> f32 {
        self.shader_gamma(is_srgb_format(format))
    }

    fn shader_gamma(&self, framebuffer_srgb: bool) -> f32 {
        self.options
            .color_gamma_override
            .unwrap_or(if framebuffer_srgb { -1.0 } else { 1.0 })
    }

    #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
    pub(super) fn viewport_pipeline(
        &mut self,
        format: vk::Format,
    ) -> RendererResult<ViewportPipeline> {
        self.ensure_operational()?;
        if let Some(pipeline) = self.viewport_pipelines.get(&format).copied() {
            return Ok(pipeline);
        }

        let options = Options {
            // Viewports are rendered by ImGui itself; keep it simple and disable depth/MSAA.
            in_flight_frames: 1,
            enable_depth_test: false,
            enable_depth_write: false,
            subpass: 0,
            sample_count: vk::SampleCountFlags::TYPE_1,
            max_textures: self.options.max_textures,
            framebuffer_srgb: false,
            color_gamma_override: self.options.color_gamma_override,
        };

        let target = match self.render_mode {
            #[cfg(feature = "render-pass")]
            RenderMode::RenderPass => {
                let clear =
                    create_viewport_render_pass(&self.device, format, vk::AttachmentLoadOp::CLEAR)?;
                let discard = match create_viewport_render_pass(
                    &self.device,
                    format,
                    vk::AttachmentLoadOp::DONT_CARE,
                ) {
                    Ok(pass) => pass,
                    Err(error) => {
                        unsafe {
                            self.device.destroy_render_pass(clear, None);
                        }
                        return Err(error);
                    }
                };
                ViewportRenderTarget::RenderPass { clear, discard }
            }
            #[cfg(feature = "dynamic-rendering")]
            RenderMode::DynamicRendering => {
                ViewportRenderTarget::DynamicRendering(DynamicRendering {
                    color_attachment_format: format,
                    depth_attachment_format: None,
                })
            }
        };

        let pipeline = match create_vulkan_pipeline(
            &self.device,
            self.resources.pipeline_layout,
            target.pipeline_target(),
            options,
            #[cfg(feature = "dynamic-rendering")]
            vk::Format::UNDEFINED,
        ) {
            Ok(pipeline) => pipeline,
            Err(err) => {
                target.destroy(&self.device);
                return Err(err);
            }
        };

        let vp = ViewportPipeline { pipeline, target };

        self.viewport_pipelines.insert(format, vp);
        Ok(vp)
    }
}

impl AshRenderer {
    /// Wait for the device, release all renderer-owned Vulkan resources, and detach from ImGui.
    ///
    /// This method reports completion-wait and resource-release errors. A nonterminal device wait
    /// error leaves the renderer available for a retry. Context-owned texture bindings are reset
    /// after complete resource release, including when the device wait reports terminal loss.
    ///
    /// `Drop` also attempts device completion and resource release, then signals a deferred
    /// Context reset after release and native callback cleanup. Incomplete release can leave
    /// Vulkan objects and their backing allocator retained. Use this method to observe that error
    /// while the borrowed Vulkan owners are still live.
    ///
    /// Any recorded but unsubmitted command buffer that references released Vulkan resources
    /// becomes invalid and must never be submitted. See [`Self::cmd_draw`].
    pub fn shutdown(&mut self, imgui_context: &mut Context) -> RendererResult<()> {
        self.shutdown_with_destroy(imgui_context, |renderer| renderer.destroy_internal())
    }

    #[cfg(all(
        test,
        any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3")
    ))]
    pub(super) fn shutdown_without_vulkan_for_test(
        &mut self,
        imgui_context: &mut Context,
    ) -> RendererResult<()> {
        self.shutdown_with_destroy(imgui_context, |renderer| {
            renderer.allocator.release()?;
            renderer.destroyed = true;
            Ok(())
        })
    }

    fn shutdown_with_destroy(
        &mut self,
        imgui_context: &mut Context,
        destroy: impl FnOnce(&mut Self) -> RendererResult<()>,
    ) -> RendererResult<()> {
        if self.destroyed {
            return Err(RendererError::RendererDestroyed);
        }
        self.ensure_context_matches(imgui_context)?;

        // Take the consumer out of `self` so the reset permit can borrow it while the renderer
        // mutably destroys its complete Vulkan texture map. Preparation is deliberately before
        // any GPU destruction; dropping the permit on a retryable failure leaves Context state
        // and the consumer attached exactly as they were.
        let consumer = self.take_shutdown_consumer()?;
        let permit = match imgui_context.prepare_renderer_texture_reset(&consumer) {
            Ok(permit) => permit,
            Err(error) => {
                self.restore_shutdown_consumer(consumer);
                return Err(error.into());
            }
        };

        let destroy_result = destroy(self);
        if !self.destroyed {
            drop(permit);
            self.restore_shutdown_consumer(consumer);
            return destroy_result;
        }

        // Device loss permits destruction. An allocator release failure, in contrast, can leave
        // parts of the map alive after this renderer stops accepting work.
        if self.allocator.is_released() {
            permit.commit();
        } else {
            drop(permit);
        }
        self.finalize_shutdown_after_reset(imgui_context);
        destroy_result
    }

    /// Extract the consumer while a caller holds the renderer's complete texture map.
    ///
    /// Restore it after a retryable failure. A matching reset permit can be committed after
    /// the map has been destroyed and the allocator has been released.
    pub(super) fn take_shutdown_consumer(&mut self) -> RendererResult<SynchronousRendererConsumer> {
        self.consumer
            .take()
            .ok_or(RendererError::RendererNotAttached)
    }

    pub(super) fn restore_shutdown_consumer(&mut self, consumer: SynchronousRendererConsumer) {
        debug_assert!(self.consumer.is_none());
        self.consumer = Some(consumer);
    }

    /// Clears the renderer's Context publication at the end of explicit shutdown.
    pub(super) fn finalize_shutdown_after_reset(&mut self, imgui_context: &mut Context) {
        self.textures.clear_destroyed_managed_textures();
        self.context_state.unpublish(imgui_context);
        drop(self.publication_attachment.take());
    }

    /// Releases Vulkan resources inside a Context-owned renderer-reset transaction.
    ///
    /// The viewport attachment retains the renderer consumer and commits the native reset after
    /// the GPU resources and allocator have been released.
    #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
    pub(super) fn shutdown_during_context_teardown(&mut self) -> RendererResult<()> {
        let destroy_result = self.destroy_internal();
        if self.destroyed {
            self.context_state.unpublish_bound();
        }
        destroy_result
    }

    /// Releases Vulkan resources after Context native teardown has completed.
    ///
    /// The Context's native state no longer exists, so this only clears Rust bookkeeping and
    /// must not inspect or mutate whichever Context happens to be current later.
    #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
    pub(super) fn shutdown_after_context_destroyed(&mut self) -> RendererResult<()> {
        let destroy_result = self.destroy_internal();
        if self.destroyed {
            self.context_state.forget_destroyed_context();
            self.signal_resource_release();
            drop(self.consumer.take());
        }
        destroy_result
    }

    pub(super) fn destroy_internal(&mut self) -> RendererResult<()> {
        if self.destroyed {
            return self.allocator.release();
        }

        let completion_result =
            match classify_device_idle(unsafe { self.device.device_wait_idle() }) {
                Ok(DeviceIdleOutcome::Complete) => Ok(()),
                Ok(DeviceIdleOutcome::DeviceLost) => {
                    Err(RendererError::Vulkan(ash::vk::Result::ERROR_DEVICE_LOST))
                }
                Err(error) => return Err(error.into()),
            };
        self.destroy_resources_after_gpu_completion(completion_result)
    }

    fn destroy_unsubmitted_internal(&mut self) -> RendererResult<()> {
        if self.destroyed {
            return Ok(());
        }
        if self.uploads.has_pending() {
            return Err(RendererError::InvalidRenderState(
                "cannot use unsubmitted Ash initialization rollback after queue submission"
                    .to_owned(),
            ));
        }
        self.destroy_resources_after_gpu_completion(Ok(()))
    }

    fn destroy_resources_after_gpu_completion(
        &mut self,
        completion_result: RendererResult<()>,
    ) -> RendererResult<()> {
        let mut cleanup =
            self.uploads
                .destroy(&self.device, &mut self.allocator, self.command_pool);

        let textures = self.textures.textures.drain().map(|(_, texture)| texture);
        for texture in textures.chain(self.textures.unconfirmed_uploads.drain(..)) {
            cleanup = cleanup.and(texture.destroy(
                &self.device,
                &mut self.allocator,
                self.resources.descriptor_pool,
            ));
        }
        for (_, managed) in self.textures.managed_textures.drain() {
            cleanup = cleanup.and(managed.texture.destroy(
                &self.device,
                &mut self.allocator,
                self.resources.descriptor_pool,
            ));
        }
        for (_, managed) in self.textures.retiring_textures.drain() {
            cleanup = cleanup.and(managed.texture.destroy(
                &self.device,
                &mut self.allocator,
                self.resources.descriptor_pool,
            ));
        }
        self.textures.managed_ids.clear();
        self.textures.external_textures.clear();

        #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
        {
            unsafe {
                let viewport_pipelines = std::mem::take(&mut self.viewport_pipelines);
                for (_, vp) in viewport_pipelines {
                    self.device.destroy_pipeline(vp.pipeline, None);
                    vp.target.destroy(&self.device);
                }
            }
        }

        let resources = std::mem::replace(&mut self.resources, VulkanRendererResources::empty());
        resources.destroy(&self.device);

        let frames = std::mem::replace(&mut self.frames, Frames::new(0));
        cleanup = cleanup.and(frames.destroy(&self.device, &mut self.allocator));
        let allocator_result = self.allocator.release();
        self.destroyed = true;
        // Report incomplete release even if the completion wait reported terminal device loss.
        allocator_result.and(completion_result).and(cleanup)
    }

    fn signal_resource_release(&self) {
        if self.allocator.is_released()
            && let Some(consumer) = &self.consumer
        {
            self.context_state
                .mark_resources_released(consumer.reset_signal());
        }
    }
}

impl Drop for AshRenderer {
    fn drop(&mut self) {
        let unpublished = self.context_state.unpublish_for_drop();
        let _ = self.destroy_internal();
        self.signal_resource_release();
        if !unpublished && let Some(attachment) = self.publication_attachment.take() {
            attachment.defer_to_context();
        }
    }
}

#[cfg(test)]
mod device_idle_tests {
    use super::*;

    #[test]
    fn device_lost_is_terminal_but_other_wait_errors_are_retryable() {
        assert_eq!(
            classify_device_idle(Ok(())),
            Ok(DeviceIdleOutcome::Complete)
        );
        assert_eq!(
            classify_device_idle(Err(ash::vk::Result::ERROR_DEVICE_LOST)),
            Ok(DeviceIdleOutcome::DeviceLost)
        );
        assert_eq!(
            classify_device_idle(Err(ash::vk::Result::ERROR_OUT_OF_HOST_MEMORY)),
            Err(ash::vk::Result::ERROR_OUT_OF_HOST_MEMORY)
        );
    }
}

#[cfg(test)]
mod shutdown_transaction_tests {
    use super::*;
    use dear_imgui_rs::BackendFlags;

    fn seed_external_texture(renderer: &mut AshRenderer) -> TextureId {
        renderer
            .textures
            .register_external_texture(
                vk::DescriptorSet::null(),
                vk::ImageView::null(),
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            )
            .into()
    }

    fn finish_retryable_renderer(renderer: &mut AshRenderer) {
        // The injected destroy path deliberately leaves the renderer live. Avoid invoking the
        // real Vulkan teardown from Drop for this pure transaction test.
        renderer.allocator.release().unwrap();
        renderer.destroyed = true;
    }

    #[test]
    fn retryable_destroy_failure_restores_consumer_and_leaves_map_intact() {
        let mut context = Context::create();
        let mut renderer = renderer_for_test(&mut context);
        let texture = seed_external_texture(&mut renderer);

        let result = renderer.shutdown_with_destroy(&mut context, |renderer| {
            assert!(renderer.consumer.is_none());
            assert!(
                renderer
                    .textures
                    .external_textures
                    .contains_key(&texture.id())
            );
            Err(RendererError::Vulkan(vk::Result::ERROR_OUT_OF_HOST_MEMORY))
        });

        assert!(matches!(
            result,
            Err(RendererError::Vulkan(vk::Result::ERROR_OUT_OF_HOST_MEMORY))
        ));
        assert!(renderer.consumer.is_some());
        assert!(
            renderer
                .textures
                .external_textures
                .contains_key(&texture.id())
        );
        assert!(!renderer.destroyed);
        finish_retryable_renderer(&mut renderer);
    }

    #[test]
    fn terminal_device_loss_commits_reset_and_releases_consumer() {
        let mut context = Context::create();
        let mut renderer = renderer_for_test(&mut context);
        let texture = seed_external_texture(&mut renderer);
        let mut foreign = Context::create();
        let foreign_state = RendererContextState::prepare(&foreign).unwrap();
        foreign_state.publish(&mut foreign).unwrap();

        let result = renderer.shutdown_with_destroy(&mut context, |renderer| {
            renderer.textures.external_textures.clear();
            renderer.allocator.release().unwrap();
            renderer.destroyed = true;
            Err(RendererError::Vulkan(vk::Result::ERROR_DEVICE_LOST))
        });

        assert!(matches!(
            result,
            Err(RendererError::Vulkan(vk::Result::ERROR_DEVICE_LOST))
        ));
        assert_eq!(
            unsafe { dear_imgui_rs::sys::igGetCurrentContext() },
            foreign.as_raw()
        );
        foreign_state.validate().unwrap();
        assert!(renderer.consumer.is_none());
        assert!(
            !renderer
                .textures
                .external_textures
                .contains_key(&texture.id())
        );
        assert!(context.io().backend_renderer_user_data().is_null());
        assert!(context.io().backend_renderer_name().is_none());
        assert!(!context.io().backend_flags().intersects(
            BackendFlags::RENDERER_HAS_TEXTURES | BackendFlags::RENDERER_HAS_VTX_OFFSET
        ));
        foreign_state.unpublish(&mut foreign);
    }

    #[test]
    fn terminal_resource_destruction_clears_external_descriptor_bookkeeping() {
        let mut context = Context::create();
        let mut renderer = renderer_for_test(&mut context);
        let texture = seed_external_texture(&mut renderer);

        renderer.destroy_unsubmitted_internal().unwrap();

        assert!(renderer.destroyed);
        assert!(
            !renderer
                .textures
                .external_textures
                .contains_key(&texture.id())
        );
        renderer.context_state.unpublish(&mut context);
        renderer.consumer.take();
    }

    #[test]
    fn renderer_drop_clears_publication_and_allows_reattach() {
        let mut context = Context::create();
        let renderer = renderer_for_test(&mut context);
        let texture = dear_imgui_rs::texture::OwnedTextureData::from_pixels(
            ImGuiTextureFormat::RGBA32,
            1,
            1,
            &[255; 4],
        )
        .unwrap();
        let texture = context.register_texture(texture);
        context.prepare_frame(
            dear_imgui_rs::FramePrepareOptions::new([128.0, 128.0], 1.0 / 60.0)
                .renderer_has_textures(),
        );
        let frame = context.begin_frame();
        frame.ui().image(texture, [16.0, 16.0]);
        let pending = frame.render(renderer.renderer_consumer().unwrap());
        let feedback = pending
            .texture_requests()
            .iter()
            .enumerate()
            .map(|(i, request)| {
                request
                    .uploaded(TextureId::from(u64::try_from(i + 1).unwrap()))
                    .unwrap()
            })
            .collect::<Vec<_>>();
        drop(pending.reconcile_texture_feedback(feedback).unwrap());
        assert!(
            context
                .with_texture(texture, |data| !data.texture_id().is_null())
                .unwrap()
        );

        drop(renderer);
        assert!(context.io().backend_renderer_user_data().is_null());
        assert!(context.io().backend_renderer_name().is_none());
        context.poll_snapshot_completions().unwrap();
        assert!(
            context
                .with_texture(texture, |data| {
                    data.texture_id().is_null() && data.status() == TextureStatus::WantCreate
                })
                .unwrap()
        );
        let mut replacement = renderer_for_test(&mut context);
        replacement.shutdown(&mut context).unwrap();
    }

    #[test]
    fn context_first_teardown_clears_ash_publication() {
        let mut context = Context::create();
        let renderer = renderer_for_test(&mut context);

        drop(context);
        assert_eq!(
            renderer.context_state.binding().lifecycle(),
            dear_imgui_rs::ContextLifecycle::NativeDestroyed
        );
        drop(renderer);
    }
}
