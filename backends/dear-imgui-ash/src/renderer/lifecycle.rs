use super::*;

impl AshRenderer {
    pub fn configure_imgui_context(&self, imgui_context: &mut Context) {
        let should_set_name = imgui_context.io().backend_renderer_name().is_none();
        if should_set_name {
            let _ = imgui_context.set_renderer_name(Some(format!(
                "dear-imgui-ash {}",
                env!("CARGO_PKG_VERSION")
            )));
        }

        let io = imgui_context.io_mut();
        let mut flags = io.backend_flags();
        flags.insert(BackendFlags::RENDERER_HAS_VTX_OFFSET);
        flags.insert(BackendFlags::RENDERER_HAS_TEXTURES);
        #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
        {
            flags.insert(BackendFlags::RENDERER_HAS_VIEWPORTS);
        }
        io.set_backend_flags(flags);

        imgui_context
            .platform_io_mut()
            .set_draw_callback_reset_render_state_raw(Some(draw_callback_reset_render_state));
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
            default_texture_id: 0,
            options,
            frames: Frames::new(options.in_flight_frames),
            destroyed: false,
            in_flight_uploads: VecDeque::new(),
            #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
            viewport_pipelines: HashMap::new(),
            #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
            viewport_clear_color: [0.0, 0.0, 0.0, 1.0],
        };

        renderer.default_texture_id = renderer.create_default_texture()?;
        renderer.configure_imgui_context(imgui);
        Ok(renderer)
    }
}

impl AshRenderer {
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
        let render_pass = create_viewport_render_pass(&self.device, format)?;

        let pipeline = match create_vulkan_pipeline(
            &self.device,
            self.pipeline_layout,
            #[cfg(not(feature = "dynamic-rendering"))]
            render_pass,
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
                    self.device.destroy_render_pass(render_pass, None);
                }
                return Err(err);
            }
        };

        let vp = ViewportPipeline {
            pipeline,
            #[cfg(not(feature = "dynamic-rendering"))]
            render_pass,
        };

        self.viewport_pipelines.insert(format, vp);
        Ok(self.viewport_pipelines.get(&format).expect("just inserted"))
    }
}

impl AshRenderer {
    pub(super) fn destroy_internal(&mut self) {
        if self.destroyed {
            return;
        }
        self.destroyed = true;

        // Best-effort: ensure in-flight uploads are complete before freeing staging memory.
        let _ = unsafe { self.device.device_wait_idle() };
        let _ = self.reap_all_uploads();

        let textures = std::mem::take(&mut self.textures.textures);
        for (_, tex) in textures {
            tex.destroy(&self.device, &mut self.allocator, self.descriptor_pool);
        }

        unsafe {
            #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
            {
                // Ensure callbacks cannot reach this renderer during teardown.
                #[cfg(feature = "multi-viewport-winit")]
                multi_viewport::clear_for_drop(self as *mut _);
                #[cfg(feature = "multi-viewport-sdl3")]
                multi_viewport_sdl3::clear_for_drop(self as *mut _);

                let viewport_pipelines = std::mem::take(&mut self.viewport_pipelines);
                for (_, vp) in viewport_pipelines {
                    self.device.destroy_pipeline(vp.pipeline, None);
                    #[cfg(not(feature = "dynamic-rendering"))]
                    self.device.destroy_render_pass(vp.render_pass, None);
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
    }
}

impl Drop for AshRenderer {
    fn drop(&mut self) {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.destroy_internal();
        }));
    }
}
