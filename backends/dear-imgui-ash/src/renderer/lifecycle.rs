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
    fn configure_imgui_context(&mut self, imgui_context: &mut Context) {
        let should_set_name = imgui_context.io().backend_renderer_name().is_none();
        if should_set_name {
            let _ = imgui_context.set_renderer_name(Some(format!(
                "dear-imgui-ash {}",
                env!("CARGO_PKG_VERSION")
            )));
        }

        let renderer_flags =
            BackendFlags::RENDERER_HAS_VTX_OFFSET | BackendFlags::RENDERER_HAS_TEXTURES;
        let io = imgui_context.io_mut();
        let flags = io.backend_flags();
        self.renderer_flags_added = renderer_flags & !flags;
        io.set_backend_flags(flags | renderer_flags);

        imgui_context
            .platform_io_mut()
            .set_draw_callback_reset_render_state_raw(Some(draw_callback_reset_render_state));
    }

    pub(super) fn unconfigure_imgui_context(
        imgui_context: &mut Context,
        renderer_flags_added: BackendFlags,
    ) {
        let expected_name = format!("dear-imgui-ash {}", env!("CARGO_PKG_VERSION"));
        if imgui_context
            .io()
            .backend_renderer_name()
            .is_some_and(|name| name.to_bytes() == expected_name.as_bytes())
        {
            let _ = imgui_context.set_renderer_name(None::<String>);
        }

        let io = imgui_context.io_mut();
        io.set_backend_flags(io.backend_flags() & !renderer_flags_added);

        let platform_io = imgui_context.platform_io_mut();
        if platform_io
            .draw_callback_reset_render_state_raw()
            .map(|callback| callback as usize)
            == Some(draw_callback_reset_render_state as *const () as usize)
        {
            platform_io.set_draw_callback_reset_render_state_raw(None);
        }
    }

    #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
    pub(super) fn renderer_name_is_ours(renderer_name: Option<&std::ffi::CStr>) -> bool {
        let expected_name = format!("dear-imgui-ash {}", env!("CARGO_PKG_VERSION"));
        renderer_name.is_some_and(|name| name.to_bytes() == expected_name.as_bytes())
    }

    #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
    pub(super) fn owned_draw_callbacks_match(
        platform_io: &dear_imgui_rs::platform_io::PlatformIo,
    ) -> bool {
        platform_io
            .draw_callback_reset_render_state_raw()
            .is_some_and(|callback| {
                std::ptr::fn_addr_eq(
                    callback,
                    draw_callback_reset_render_state
                        as unsafe extern "C" fn(
                            *const dear_imgui_rs::sys::ImDrawList,
                            *const dear_imgui_rs::sys::ImDrawCmd,
                        ),
                )
            })
    }

    #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
    pub(super) fn clear_owned_draw_callbacks(
        platform_io: &mut dear_imgui_rs::platform_io::PlatformIo,
    ) {
        if Self::owned_draw_callbacks_match(platform_io) {
            platform_io.set_draw_callback_reset_render_state_raw(None);
        }
    }

    /// Create a new renderer using the internal default allocator.
    ///
    /// The provided `command_pool` is used for short-lived upload command buffers.
    #[cfg(not(any(feature = "gpu-allocator", feature = "vk-mem")))]
    pub fn with_default_allocator(
        instance: &Instance,
        physical_device: vk::PhysicalDevice,
        device: Device,
        queue: vk::Queue,
        command_pool: vk::CommandPool,
        #[cfg(not(feature = "dynamic-rendering"))] render_pass: vk::RenderPass,
        #[cfg(feature = "dynamic-rendering")] dynamic_rendering: DynamicRendering,
        imgui: &mut Context,
        options: Option<Options>,
    ) -> RendererResult<Self> {
        let memory_properties =
            unsafe { instance.get_physical_device_memory_properties(physical_device) };
        let allocator = Allocator::new(memory_properties);

        Self::init_renderer(
            device,
            allocator,
            queue,
            command_pool,
            #[cfg(not(feature = "dynamic-rendering"))]
            render_pass,
            #[cfg(feature = "dynamic-rendering")]
            dynamic_rendering,
            imgui,
            options,
        )
    }

    /// Create a new renderer using a shared `gpu-allocator` allocator.
    #[cfg(feature = "gpu-allocator")]
    pub fn with_gpu_allocator(
        allocator: std::sync::Arc<std::sync::Mutex<gpu_allocator::vulkan::Allocator>>,
        device: Device,
        queue: vk::Queue,
        command_pool: vk::CommandPool,
        #[cfg(not(feature = "dynamic-rendering"))] render_pass: vk::RenderPass,
        #[cfg(feature = "dynamic-rendering")] dynamic_rendering: DynamicRendering,
        imgui: &mut Context,
        options: Option<Options>,
    ) -> RendererResult<Self> {
        #[cfg(all(feature = "gpu-allocator", not(feature = "vk-mem")))]
        let allocator = Allocator::new(allocator);
        #[cfg(all(feature = "gpu-allocator", feature = "vk-mem"))]
        let allocator = Allocator::new_gpu(allocator);
        Self::init_renderer(
            device,
            allocator,
            queue,
            command_pool,
            #[cfg(not(feature = "dynamic-rendering"))]
            render_pass,
            #[cfg(feature = "dynamic-rendering")]
            dynamic_rendering,
            imgui,
            options,
        )
    }

    /// Create a new renderer using a shared `vk-mem` allocator.
    #[cfg(feature = "vk-mem")]
    pub fn with_vk_mem_allocator(
        allocator: std::sync::Arc<std::sync::Mutex<vk_mem::Allocator>>,
        device: Device,
        queue: vk::Queue,
        command_pool: vk::CommandPool,
        #[cfg(not(feature = "dynamic-rendering"))] render_pass: vk::RenderPass,
        #[cfg(feature = "dynamic-rendering")] dynamic_rendering: DynamicRendering,
        imgui: &mut Context,
        options: Option<Options>,
    ) -> RendererResult<Self> {
        #[cfg(all(feature = "vk-mem", not(feature = "gpu-allocator")))]
        let allocator = Allocator::new(allocator);
        #[cfg(all(feature = "vk-mem", feature = "gpu-allocator"))]
        let allocator = Allocator::new_vk_mem(allocator);
        Self::init_renderer(
            device,
            allocator,
            queue,
            command_pool,
            #[cfg(not(feature = "dynamic-rendering"))]
            render_pass,
            #[cfg(feature = "dynamic-rendering")]
            dynamic_rendering,
            imgui,
            options,
        )
    }

    fn init_renderer(
        device: Device,
        allocator: Allocator,
        queue: vk::Queue,
        command_pool: vk::CommandPool,
        #[cfg(not(feature = "dynamic-rendering"))] render_pass: vk::RenderPass,
        #[cfg(feature = "dynamic-rendering")] dynamic_rendering: DynamicRendering,
        imgui: &mut Context,
        options: Option<Options>,
    ) -> RendererResult<Self> {
        let options = options.unwrap_or_default();
        if options.in_flight_frames == 0 {
            return Err(RendererError::InvalidRenderState(
                "Options::in_flight_frames must be >= 1".to_string(),
            ));
        }

        let descriptor_set_layout = create_vulkan_descriptor_set_layout(&device)?;
        let pipeline_layout = match create_vulkan_pipeline_layout(&device, descriptor_set_layout) {
            Ok(pipeline_layout) => pipeline_layout,
            Err(err) => {
                unsafe { device.destroy_descriptor_set_layout(descriptor_set_layout, None) };
                return Err(err);
            }
        };
        let pipeline = match create_vulkan_pipeline(
            &device,
            pipeline_layout,
            #[cfg(not(feature = "dynamic-rendering"))]
            render_pass,
            #[cfg(feature = "dynamic-rendering")]
            dynamic_rendering,
            options,
        ) {
            Ok(pipeline) => pipeline,
            Err(err) => {
                unsafe {
                    device.destroy_pipeline_layout(pipeline_layout, None);
                    device.destroy_descriptor_set_layout(descriptor_set_layout, None);
                }
                return Err(err);
            }
        };
        let descriptor_pool = match create_vulkan_descriptor_pool(&device, options.max_textures) {
            Ok(descriptor_pool) => descriptor_pool,
            Err(err) => {
                unsafe {
                    device.destroy_pipeline(pipeline, None);
                    device.destroy_pipeline_layout(pipeline_layout, None);
                    device.destroy_descriptor_set_layout(descriptor_set_layout, None);
                }
                return Err(err);
            }
        };

        let mut renderer = Self {
            device,
            allocator,
            queue,
            command_pool,
            pipeline,
            pipeline_layout,
            descriptor_set_layout,
            descriptor_pool,
            textures: TextureManager::new(),
            consumer: None,
            renderer_flags_added: BackendFlags::empty(),
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

        renderer.default_texture_id = renderer.create_default_texture()?;
        let consumer = match imgui.create_renderer_consumer() {
            Ok(consumer) => consumer,
            Err(error) => {
                let _ = renderer.destroy_internal();
                return Err(error.into());
            }
        };
        renderer.consumer = Some(consumer);
        if let Err(error) = imgui.reset_renderer_texture_bindings(
            renderer
                .consumer
                .as_ref()
                .expect("renderer consumer was just attached"),
        ) {
            let _ = renderer.destroy_internal();
            renderer.consumer.take();
            return Err(error.into());
        }
        renderer.configure_imgui_context(imgui);
        Ok(renderer)
    }
}

impl AshRenderer {
    pub(super) fn ensure_frame_matches(&self, frame: &RenderedFrame<'_>) -> RendererResult<()> {
        if self.destroyed {
            return Err(RendererError::RendererDestroyed);
        }
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
    ) -> RendererResult<&ViewportPipeline> {
        if self.viewport_pipelines.contains_key(&format) {
            return Ok(self
                .viewport_pipelines
                .get(&format)
                .expect("checked contains_key"));
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
            texture_format: self.options.texture_format,
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
            self.pipeline_layout,
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
        Ok(self.viewport_pipelines.get(&format).expect("just inserted"))
    }
}

impl AshRenderer {
    /// Wait for the device, release all renderer-owned Vulkan resources, and detach from ImGui.
    ///
    /// Unlike `Drop`, this method can reset Context-owned texture bindings after GPU destruction.
    pub fn shutdown(&mut self, imgui_context: &mut Context) -> RendererResult<()> {
        self.ensure_context_matches(imgui_context)?;
        let destroy_result = self.destroy_internal();
        if !self.destroyed {
            return destroy_result;
        }
        let consumer = self
            .consumer
            .as_ref()
            .ok_or(RendererError::RendererNotAttached)?;
        imgui_context.reset_renderer_texture_bindings(consumer)?;
        Self::unconfigure_imgui_context(imgui_context, self.renderer_flags_added);
        self.renderer_flags_added = BackendFlags::empty();
        self.consumer.take();
        destroy_result
    }

    #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
    pub(super) fn shutdown_without_context_reset(&mut self) -> RendererResult<()> {
        let destroy_result = self.destroy_internal();
        if self.destroyed {
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
        let _ = self.reap_all_uploads();

        let textures = std::mem::take(&mut self.textures.textures);
        for (_, tex) in textures {
            tex.destroy(&self.device, &mut self.allocator, self.descriptor_pool);
        }
        let managed_textures = std::mem::take(&mut self.textures.managed_textures);
        for (_, managed) in managed_textures {
            managed
                .texture
                .destroy(&self.device, &mut self.allocator, self.descriptor_pool);
        }
        let retiring_textures = self.textures.retiring_textures.drain().collect::<Vec<_>>();
        for (_, managed) in retiring_textures {
            managed
                .texture
                .destroy(&self.device, &mut self.allocator, self.descriptor_pool);
        }
        self.textures.managed_ids.clear();

        unsafe {
            #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
            {
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

            self.device
                .destroy_descriptor_pool(self.descriptor_pool, None);
            self.device
                .destroy_descriptor_set_layout(self.descriptor_set_layout, None);
            self.device.destroy_pipeline(self.pipeline, None);
            self.device
                .destroy_pipeline_layout(self.pipeline_layout, None);
        }

        let frames = std::mem::replace(&mut self.frames, Frames::new(0));
        let _ = frames.destroy(&self.device, &mut self.allocator);
        self.destroyed = true;
        completion_result
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

impl Drop for AshRenderer {
    fn drop(&mut self) {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = self.destroy_internal();
        }));
    }
}
