use super::*;

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
    /// compatible with every target used by this renderer. The queue and command pool must remain
    /// live for the renderer lifetime, and all host access to them must be externally synchronized.
    #[cfg(not(any(feature = "gpu-allocator", feature = "vk-mem")))]
    pub unsafe fn with_default_allocator(
        instance: &Instance,
        physical_device: vk::PhysicalDevice,
        config: AshRendererConfig,
        imgui: &mut Context,
    ) -> RendererResult<Self> {
        let memory_properties =
            unsafe { instance.get_physical_device_memory_properties(physical_device) };
        let allocator = Allocator::new(memory_properties);

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
        #[cfg(all(feature = "gpu-allocator", not(feature = "vk-mem")))]
        let allocator = Allocator::new(allocator);
        #[cfg(all(feature = "gpu-allocator", feature = "vk-mem"))]
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
        #[cfg(all(feature = "vk-mem", not(feature = "gpu-allocator")))]
        let allocator = Allocator::new(allocator);
        #[cfg(all(feature = "vk-mem", feature = "gpu-allocator"))]
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
            #[cfg(not(feature = "dynamic-rendering"))]
            render_pass,
            #[cfg(feature = "dynamic-rendering")]
            dynamic_rendering,
            options,
        } = config;
        if options.in_flight_frames == 0 {
            return Err(RendererError::InvalidRenderState(
                "Options::in_flight_frames must be >= 1".to_string(),
            ));
        }
        let context_state = RendererContextState::prepare(imgui)?;

        let resources = VulkanRendererResources::create(
            &device,
            #[cfg(not(feature = "dynamic-rendering"))]
            render_pass,
            #[cfg(feature = "dynamic-rendering")]
            dynamic_rendering,
            options,
        )?;

        let mut renderer = Self {
            device,
            allocator,
            queue,
            command_pool,
            resources,
            textures: TextureManager::new(),
            consumer: None,
            context_state,
            default_texture_id: 0,
            options,
            frames: Frames::new(options.in_flight_frames),
            destroyed: false,
            in_flight_uploads: VecDeque::new(),
            managed_uploads: ManagedUploadTracker::default(),
            #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
            viewport_pipelines: HashMap::new(),
            #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
            viewport_clear_color: [0.0, 0.0, 0.0, 1.0],
        };

        let consumer = match imgui.create_renderer_consumer() {
            Ok(consumer) => consumer,
            Err(error) => {
                renderer.destroy_unsubmitted_internal()?;
                return Err(error.into());
            }
        };
        renderer.consumer = Some(consumer);
        let reset = match imgui.prepare_renderer_texture_reset(
            renderer
                .consumer
                .as_ref()
                .expect("renderer consumer was just attached"),
        ) {
            Ok(reset) => reset,
            Err(error) => {
                renderer.destroy_unsubmitted_internal()?;
                renderer.consumer.take();
                return Err(error.into());
            }
        };
        // `create_default_texture` is renderer-private and has not created a Context-managed
        // texture mapping. The new consumer has not submitted an epoch, so this is an empty
        // transaction that completes before renderer state is published.
        let _ = reset.commit();
        if let Err(error) = renderer.context_state.publish(imgui) {
            renderer.destroy_unsubmitted_internal()?;
            renderer.consumer.take();
            return Err(error);
        }

        renderer.default_texture_id = match renderer.create_default_texture() {
            Ok(texture_id) => texture_id,
            Err(error) => {
                let rollback = renderer.shutdown_with_destroy(imgui, |renderer| {
                    renderer.destroy_unsubmitted_internal()
                });
                return match rollback {
                    Ok(()) => Err(error),
                    Err(rollback_error) => Err(rollback_error),
                };
            }
        };
        Ok(renderer)
    }
}

impl AshRenderer {
    pub(super) fn ensure_operational(&self) -> RendererResult<()> {
        if self.destroyed {
            return Err(RendererError::RendererDestroyed);
        }
        self.context_state.validate()
    }

    pub(super) fn ensure_frame_matches(&self, frame: &RenderedFrame<'_>) -> RendererResult<()> {
        self.ensure_operational()?;
        let consumer = self
            .consumer
            .as_ref()
            .ok_or(RendererError::RendererNotAttached)?;
        if frame.context_id() != consumer.context_id() {
            return Err(RendererError::ContextMismatch {
                expected: consumer.context_id(),
                actual: frame.context_id(),
            });
        }
        let epoch = frame.epoch().ok_or_else(|| {
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
        self.options
            .color_gamma_override
            .unwrap_or(if self.options.framebuffer_srgb {
                2.2_f32
            } else {
                1.0_f32
            })
    }

    #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
    pub(super) fn gamma_for_format(&self, format: vk::Format) -> f32 {
        self.options
            .color_gamma_override
            .unwrap_or(if is_srgb_format(format) {
                2.2_f32
            } else {
                1.0_f32
            })
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

        #[cfg(not(feature = "dynamic-rendering"))]
        let clear_render_pass =
            create_viewport_render_pass(&self.device, format, vk::AttachmentLoadOp::CLEAR)?;
        #[cfg(not(feature = "dynamic-rendering"))]
        let discard_render_pass = match create_viewport_render_pass(
            &self.device,
            format,
            vk::AttachmentLoadOp::DONT_CARE,
        ) {
            Ok(render_pass) => render_pass,
            Err(err) => {
                unsafe {
                    self.device.destroy_render_pass(clear_render_pass, None);
                }
                return Err(err);
            }
        };

        let pipeline = match create_vulkan_pipeline(
            &self.device,
            self.resources.pipeline_layout,
            #[cfg(not(feature = "dynamic-rendering"))]
            clear_render_pass,
            #[cfg(feature = "dynamic-rendering")]
            DynamicRendering {
                color_attachment_format: format,
                depth_attachment_format: None,
            },
            options,
        ) {
            Ok(pipeline) => pipeline,
            Err(err) => {
                #[cfg(not(feature = "dynamic-rendering"))]
                unsafe {
                    self.device.destroy_render_pass(discard_render_pass, None);
                    self.device.destroy_render_pass(clear_render_pass, None);
                }
                return Err(err);
            }
        };

        let vp = ViewportPipeline {
            pipeline,
            #[cfg(not(feature = "dynamic-rendering"))]
            clear_render_pass,
            #[cfg(not(feature = "dynamic-rendering"))]
            discard_render_pass,
        };

        self.viewport_pipelines.insert(format, vp);
        Ok(vp)
    }
}

impl AshRenderer {
    /// Wait for the device, release all renderer-owned Vulkan resources, and detach from ImGui.
    ///
    /// Unlike `Drop`, this method can reset Context-owned texture bindings after GPU destruction.
    /// Any recorded but unsubmitted command buffer that references this renderer becomes invalid
    /// and must never be submitted; this is part of [`Self::cmd_draw`]'s safety contract.
    pub fn shutdown(&mut self, imgui_context: &mut Context) -> RendererResult<()> {
        self.shutdown_with_destroy(imgui_context, |renderer| renderer.destroy_internal())
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

        // `commit` is infallible after preparation and must happen even when the terminal GPU
        // result is `ERROR_DEVICE_LOST`: the Vulkan map is no longer reachable in either case.
        let _ = permit.commit();
        self.finalize_shutdown_after_reset(imgui_context);
        destroy_result
    }

    /// Extract the consumer while a caller holds the renderer's complete texture map.
    ///
    /// The caller must either restore it after every retryable failure or commit a matching reset
    /// permit after the map has been destroyed.
    pub(super) fn take_shutdown_consumer(&mut self) -> RendererResult<RendererConsumer> {
        self.consumer
            .take()
            .ok_or(RendererError::RendererNotAttached)
    }

    pub(super) fn restore_shutdown_consumer(&mut self, consumer: RendererConsumer) {
        debug_assert!(self.consumer.is_none());
        self.consumer = Some(consumer);
    }

    /// Completes the Context-facing half of a shutdown after an already-prepared reset commits.
    pub(super) fn finalize_shutdown_after_reset(&mut self, imgui_context: &mut Context) {
        self.textures.clear_destroyed_managed_textures();
        self.context_state.unpublish(imgui_context);
    }

    /// Releases Vulkan resources inside a Context-owned renderer-reset transaction.
    ///
    /// The viewport attachment retains the renderer consumer and commits the native reset only
    /// after this method reaches terminal renderer destruction.
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
            self.consumer.take();
        }
        destroy_result
    }

    pub(super) fn destroy_internal(&mut self) -> RendererResult<()> {
        if self.destroyed {
            return Ok(());
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
        if !self.in_flight_uploads.is_empty() {
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
        let _ = self.reap_all_uploads();

        let textures = std::mem::take(&mut self.textures.textures);
        for (_, tex) in textures {
            tex.destroy(
                &self.device,
                &mut self.allocator,
                self.resources.descriptor_pool,
            );
        }
        let managed_textures = std::mem::take(&mut self.textures.managed_textures);
        for (_, managed) in managed_textures {
            managed.texture.destroy(
                &self.device,
                &mut self.allocator,
                self.resources.descriptor_pool,
            );
        }
        let retiring_textures = self.textures.retiring_textures.drain().collect::<Vec<_>>();
        for (_, managed) in retiring_textures {
            managed.texture.destroy(
                &self.device,
                &mut self.allocator,
                self.resources.descriptor_pool,
            );
        }
        self.textures.managed_ids.clear();
        self.textures.external_textures.clear();

        #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
        {
            unsafe {
                let viewport_pipelines = std::mem::take(&mut self.viewport_pipelines);
                for (_, vp) in viewport_pipelines {
                    self.device.destroy_pipeline(vp.pipeline, None);
                    #[cfg(not(feature = "dynamic-rendering"))]
                    {
                        self.device
                            .destroy_render_pass(vp.discard_render_pass, None);
                        self.device.destroy_render_pass(vp.clear_render_pass, None);
                    }
                }
            }
        }

        let resources = std::mem::replace(&mut self.resources, VulkanRendererResources::empty());
        resources.destroy(&self.device);

        let frames = std::mem::replace(&mut self.frames, Frames::new(0));
        let _ = frames.destroy(&self.device, &mut self.allocator);
        self.destroyed = true;
        completion_result
    }

    fn run_drop_cleanup_if_context_destroyed(&mut self, cleanup: impl FnOnce(&mut Self)) {
        if self.destroyed || !self.context_state.native_context_is_destroyed() {
            return;
        }
        cleanup(self);
    }
}

impl Drop for AshRenderer {
    fn drop(&mut self) {
        // Dropping a live renderer has no mutable Context with which to validate and commit the
        // renderer-texture reset transaction. Keep its Vulkan resources intact instead of leaving
        // Context-managed texture bindings pointing at deleted GPU objects. Once native Context
        // teardown has completed, no such binding can be observed and best-effort cleanup is safe.
        self.run_drop_cleanup_if_context_destroyed(|renderer| {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = renderer.destroy_internal();
            }));
        });
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

#[cfg(all(test, not(any(feature = "gpu-allocator", feature = "vk-mem"))))]
mod shutdown_transaction_tests {
    use std::cell::Cell;

    use super::*;
    use dear_imgui_rs::{BackendFlags, FramePrepareOptions};

    fn renderer_for_test(context: &mut Context) -> AshRenderer {
        let device = unsafe { Device::load_with(|_| std::ptr::null(), vk::Device::null()) };
        let context_state = RendererContextState::prepare(context).unwrap();
        let consumer = context.create_renderer_consumer().unwrap();
        // This synthetic renderer has no Vulkan texture map or submitted consumer epoch.
        let reset = context.prepare_renderer_texture_reset(&consumer).unwrap();
        let _ = reset.commit();
        context_state.publish(context).unwrap();
        AshRenderer {
            device,
            allocator: Allocator::new(vk::PhysicalDeviceMemoryProperties::default()),
            queue: vk::Queue::null(),
            command_pool: vk::CommandPool::null(),
            resources: VulkanRendererResources::empty(),
            textures: TextureManager::new(),
            consumer: Some(consumer),
            context_state,
            default_texture_id: 0,
            options: Options::default(),
            frames: Frames::new(0),
            destroyed: false,
            in_flight_uploads: VecDeque::new(),
            managed_uploads: ManagedUploadTracker::default(),
            #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
            viewport_pipelines: HashMap::new(),
            #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
            viewport_clear_color: [0.0, 0.0, 0.0, 1.0],
        }
    }

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
        renderer.destroyed = true;
    }

    #[test]
    fn reset_preparation_failure_does_not_start_gpu_teardown() {
        let mut context = Context::create();
        let mut renderer = renderer_for_test(&mut context);
        let texture = seed_external_texture(&mut renderer);
        context.prepare_frame(
            FramePrepareOptions::new([128.0, 128.0], 1.0 / 60.0).renderer_has_textures(),
        );
        let snapshot = context
            .begin_frame()
            .render_snapshot(renderer.consumer.as_ref().unwrap())
            .unwrap();
        let destroy_called = Cell::new(false);

        let result = renderer.shutdown_with_destroy(&mut context, |_| {
            destroy_called.set(true);
            Ok(())
        });

        assert!(matches!(result, Err(RendererError::RendererConsumer(_))));
        assert!(!destroy_called.get());
        assert!(renderer.consumer.is_some());
        assert!(
            renderer
                .textures
                .external_textures
                .contains_key(&texture.id())
        );
        assert!(!renderer.destroyed);

        drop(snapshot);
        context.poll_snapshot_completions().unwrap();
        finish_retryable_renderer(&mut renderer);
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

        let result = renderer.shutdown_with_destroy(&mut context, |renderer| {
            renderer.textures.external_textures.clear();
            renderer.destroyed = true;
            Err(RendererError::Vulkan(vk::Result::ERROR_DEVICE_LOST))
        });

        assert!(matches!(
            result,
            Err(RendererError::Vulkan(vk::Result::ERROR_DEVICE_LOST))
        ));
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
    fn drop_cleanup_never_runs_while_the_context_is_alive() {
        let mut context = Context::create();
        let mut renderer = renderer_for_test(&mut context);
        let cleanup_calls = Cell::new(0);

        renderer.run_drop_cleanup_if_context_destroyed(|renderer| {
            cleanup_calls.set(cleanup_calls.get() + 1);
            renderer.destroyed = true;
        });

        assert_eq!(cleanup_calls.get(), 0);
        drop(renderer);
        assert!(context.io().backend_renderer_user_data().is_null());
        assert!(context.io().backend_renderer_name().is_none());
    }

    #[test]
    fn drop_cleanup_runs_only_after_native_context_teardown() {
        let mut context = Context::create();
        let mut renderer = renderer_for_test(&mut context);
        let cleanup_calls = Cell::new(0);

        renderer.context_state.unpublish(&mut context);
        drop(context);
        renderer.run_drop_cleanup_if_context_destroyed(|renderer| {
            cleanup_calls.set(cleanup_calls.get() + 1);
            renderer.destroyed = true;
        });

        assert_eq!(cleanup_calls.get(), 1);
    }
}
