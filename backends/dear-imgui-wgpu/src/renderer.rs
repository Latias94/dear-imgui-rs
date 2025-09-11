//! Main WGPU renderer implementation
//!
//! This module contains the main WgpuRenderer struct and its implementation,
//! following the pattern from imgui_impl_wgpu.cpp

use crate::{
    FrameResources, RenderResources, RendererError, RendererResult, ShaderManager, Uniforms,
    WgpuBackendData, WgpuInitInfo, WgpuTextureManager,
};
use dear_imgui::{render::DrawData, BackendFlags, Context};
use wgpu::*;

/// Main WGPU renderer for Dear ImGui
///
/// This corresponds to the main renderer functionality in imgui_impl_wgpu.cpp
pub struct WgpuRenderer {
    /// Backend data
    backend_data: Option<WgpuBackendData>,
    /// Shader manager
    shader_manager: ShaderManager,
    /// Texture manager
    texture_manager: WgpuTextureManager,
    /// Default texture for fallback
    default_texture: Option<TextureView>,
}

impl WgpuRenderer {
    /// Create a new WGPU renderer
    pub fn new() -> Self {
        Self {
            backend_data: None,
            shader_manager: ShaderManager::new(),
            texture_manager: WgpuTextureManager::new(),
            default_texture: None,
        }
    }

    /// Initialize the renderer
    ///
    /// This corresponds to ImGui_ImplWGPU_Init in the C++ implementation
    pub fn init(&mut self, init_info: WgpuInitInfo) -> RendererResult<()> {
        // Create backend data
        let mut backend_data = WgpuBackendData::new(init_info);

        // Initialize render resources
        backend_data
            .render_resources
            .initialize(&backend_data.device)?;

        // Initialize shaders
        self.shader_manager.initialize(&backend_data.device)?;

        // Create default texture (1x1 white pixel)
        let default_texture =
            self.create_default_texture(&backend_data.device, &backend_data.queue)?;
        self.default_texture = Some(default_texture);

        // Create device objects (pipeline, etc.)
        self.create_device_objects(&mut backend_data)?;

        self.backend_data = Some(backend_data);
        Ok(())
    }

    /// Configure Dear ImGui context with WGPU backend capabilities
    pub fn configure_imgui_context(&self, imgui_context: &mut Context) {
        let io = imgui_context.io_mut();
        let mut flags = io.backend_flags();

        // Set WGPU renderer capabilities
        // We can honor the ImDrawCmd::VtxOffset field, allowing for large meshes.
        flags.insert(BackendFlags::RENDERER_HAS_VTX_OFFSET);
        // We can honor ImGuiPlatformIO::Textures[] requests during render.
        flags.insert(BackendFlags::RENDERER_HAS_TEXTURES);

        io.set_backend_flags(flags);
        // Note: Dear ImGui doesn't expose set_backend_renderer_name in the Rust bindings yet
    }

    /// Prepare font atlas for rendering
    pub fn prepare_font_atlas(&mut self, imgui_ctx: &mut Context) -> RendererResult<()> {
        if let Some(backend_data) = &self.backend_data {
            let device = backend_data.device.clone();
            let queue = backend_data.queue.clone();
            self.reload_font_texture(imgui_ctx, &device, &queue)?;
        }
        Ok(())
    }

    /// Create device objects (pipeline, etc.)
    ///
    /// This corresponds to ImGui_ImplWGPU_CreateDeviceObjects in the C++ implementation
    fn create_device_objects(&mut self, backend_data: &mut WgpuBackendData) -> RendererResult<()> {
        let device = &backend_data.device;

        // Create bind group layouts
        let (common_layout, image_layout) = crate::shaders::create_bind_group_layouts(device);

        // Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Dear ImGui Pipeline Layout"),
            bind_group_layouts: &[&common_layout, &image_layout],
            push_constant_ranges: &[],
        });

        // Get shader module
        let shader_module = self.shader_manager.get_shader_module()?;

        // Create vertex buffer layout
        let vertex_buffer_layout = crate::shaders::create_vertex_buffer_layout();
        let vertex_buffer_layouts = [vertex_buffer_layout];

        // Create vertex state
        let vertex_state =
            crate::shaders::create_vertex_state(shader_module, &vertex_buffer_layouts);

        // Create color target state
        let color_target = ColorTargetState {
            format: backend_data.render_target_format,
            blend: Some(BlendState {
                color: BlendComponent {
                    src_factor: BlendFactor::SrcAlpha,
                    dst_factor: BlendFactor::OneMinusSrcAlpha,
                    operation: BlendOperation::Add,
                },
                alpha: BlendComponent {
                    src_factor: BlendFactor::One,
                    dst_factor: BlendFactor::OneMinusSrcAlpha,
                    operation: BlendOperation::Add,
                },
            }),
            write_mask: ColorWrites::ALL,
        };

        // Determine if we need gamma correction based on format
        let use_gamma_correction = matches!(
            backend_data.render_target_format,
            TextureFormat::Rgba8UnormSrgb
                | TextureFormat::Bgra8UnormSrgb
                | TextureFormat::Bc1RgbaUnormSrgb
                | TextureFormat::Bc2RgbaUnormSrgb
                | TextureFormat::Bc3RgbaUnormSrgb
                | TextureFormat::Bc7RgbaUnormSrgb
        );

        // Create fragment state
        let color_targets = [Some(color_target)];
        let fragment_state = crate::shaders::create_fragment_state(
            shader_module,
            &color_targets,
            use_gamma_correction,
        );

        // Create depth stencil state if needed (matches imgui_impl_wgpu.cpp depth-stencil setup)
        let depth_stencil = backend_data
            .depth_stencil_format
            .map(|format| DepthStencilState {
                format,
                depth_write_enabled: false, // matches WGPUOptionalBool_False in C++
                depth_compare: CompareFunction::Always, // matches WGPUCompareFunction_Always
                stencil: StencilState {
                    front: StencilFaceState {
                        compare: CompareFunction::Always, // matches WGPUCompareFunction_Always
                        fail_op: StencilOperation::Keep,  // matches WGPUStencilOperation_Keep
                        depth_fail_op: StencilOperation::Keep, // matches WGPUStencilOperation_Keep
                        pass_op: StencilOperation::Keep,  // matches WGPUStencilOperation_Keep
                    },
                    back: StencilFaceState {
                        compare: CompareFunction::Always, // matches WGPUCompareFunction_Always
                        fail_op: StencilOperation::Keep,  // matches WGPUStencilOperation_Keep
                        depth_fail_op: StencilOperation::Keep, // matches WGPUStencilOperation_Keep
                        pass_op: StencilOperation::Keep,  // matches WGPUStencilOperation_Keep
                    },
                    read_mask: 0xff,  // default value
                    write_mask: 0xff, // default value
                },
                bias: DepthBiasState::default(),
            });

        // Create render pipeline
        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Dear ImGui Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: vertex_state,
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: FrontFace::Cw,
                cull_mode: None,
                polygon_mode: PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil,
            multisample: backend_data.init_info.pipeline_multisample_state,
            fragment: Some(fragment_state),
            multiview: None,
            cache: None,
        });

        backend_data.pipeline_state = Some(pipeline);
        Ok(())
    }

    /// Create a default 1x1 white texture
    fn create_default_texture(
        &self,
        device: &Device,
        queue: &Queue,
    ) -> RendererResult<TextureView> {
        let texture = device.create_texture(&TextureDescriptor {
            label: Some("Dear ImGui Default Texture"),
            size: Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Upload white pixel
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[255u8, 255u8, 255u8, 255u8], // RGBA white
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );

        Ok(texture.create_view(&TextureViewDescriptor::default()))
    }

    /// Load font texture from Dear ImGui context
    fn reload_font_texture(
        &mut self,
        imgui_ctx: &mut Context,
        device: &Device,
        queue: &Queue,
    ) -> RendererResult<()> {
        let mut fonts = imgui_ctx.font_atlas_mut();

        // Build the font atlas if not already built
        if !fonts.is_built() {
            fonts.build();
        }

        // Get texture data from the font atlas
        unsafe {
            if let Some((pixels_ptr, width, height)) = fonts.get_tex_data_ptr() {
                let bytes_per_pixel = 4; // RGBA format
                let texture_size = (width * height * bytes_per_pixel) as usize;
                let texture_data = std::slice::from_raw_parts(pixels_ptr, texture_size);

                // Create font texture using texture manager
                let mut temp_texture_data = dear_imgui::TextureData::new();
                temp_texture_data.create(
                    dear_imgui::TextureFormat::RGBA32,
                    width as i32,
                    height as i32,
                );
                temp_texture_data.set_data(texture_data);

                let font_texture_id = self.texture_manager.create_texture_from_data(
                    device,
                    queue,
                    &temp_texture_data,
                )?;

                // Set the texture reference in Dear ImGui
                let tex_ref = dear_imgui::create_texture_ref(font_texture_id);
                fonts.set_tex_ref(tex_ref);
            }
        }

        Ok(())
    }

    /// Get the texture manager
    pub fn texture_manager(&self) -> &WgpuTextureManager {
        &self.texture_manager
    }

    /// Get the texture manager mutably
    pub fn texture_manager_mut(&mut self) -> &mut WgpuTextureManager {
        &mut self.texture_manager
    }

    /// Check if the renderer is initialized
    pub fn is_initialized(&self) -> bool {
        self.backend_data.is_some()
    }

    /// Update a single texture manually
    ///
    /// This corresponds to ImGui_ImplWGPU_UpdateTexture in the C++ implementation.
    /// Use this when you need precise control over texture update timing.
    ///
    /// # Returns
    ///
    /// Returns a `TextureUpdateResult` that contains any status/ID updates that need
    /// to be applied to the texture data. This follows Rust's principle of explicit
    /// state management.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui_wgpu::*;
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let mut renderer = WgpuRenderer::new();
    /// # let mut texture_data = dear_imgui::TextureData::new();
    /// let result = renderer.update_texture(&texture_data)?;
    /// result.apply_to(&mut texture_data);
    /// # Ok(())
    /// # }
    /// ```
    pub fn update_texture(
        &mut self,
        texture_data: &dear_imgui::TextureData,
    ) -> RendererResult<crate::TextureUpdateResult> {
        if let Some(backend_data) = &self.backend_data {
            self.texture_manager
                .update_single_texture(texture_data, &backend_data.device, &backend_data.queue)
                .map_err(RendererError::TextureCreationFailed)
        } else {
            Err(RendererError::InvalidRenderState(
                "Renderer not initialized".to_string(),
            ))
        }
    }

    /// Called every frame to prepare for rendering
    ///
    /// This corresponds to ImGui_ImplWGPU_NewFrame in the C++ implementation
    pub fn new_frame(&mut self) -> RendererResult<()> {
        let needs_recreation = if let Some(backend_data) = &self.backend_data {
            backend_data.pipeline_state.is_none()
        } else {
            false
        };

        if needs_recreation {
            // Extract the backend data temporarily to avoid borrow checker issues
            let mut backend_data = self.backend_data.take().unwrap();
            self.create_device_objects(&mut backend_data)?;
            self.backend_data = Some(backend_data);
        }
        Ok(())
    }

    /// Render Dear ImGui draw data
    ///
    /// This corresponds to ImGui_ImplWGPU_RenderDrawData in the C++ implementation
    pub fn render_draw_data(
        &mut self,
        draw_data: &DrawData,
        render_pass: &mut RenderPass,
    ) -> RendererResult<()> {
        let backend_data = self.backend_data.as_mut().ok_or_else(|| {
            RendererError::InvalidRenderState("Renderer not initialized".to_string())
        })?;

        // Avoid rendering when minimized
        let fb_width = (draw_data.display_size[0] * draw_data.framebuffer_scale[0]) as i32;
        let fb_height = (draw_data.display_size[1] * draw_data.framebuffer_scale[1]) as i32;
        if fb_width <= 0 || fb_height <= 0 || !draw_data.valid() {
            return Ok(());
        }

        // Handle texture updates
        self.texture_manager.handle_texture_updates(
            draw_data,
            &backend_data.device,
            &backend_data.queue,
        );

        // Advance to next frame
        backend_data.next_frame();

        // Prepare frame resources
        Self::prepare_frame_resources_static(draw_data, backend_data)?;

        // Setup render state
        Self::setup_render_state_static(draw_data, render_pass, backend_data)?;

        // Setup render state structure (for callbacks and custom texture bindings)
        // Note: We need to be careful with lifetimes here, so we'll set it just before rendering
        // and clear it immediately after
        unsafe {
            let platform_io = dear_imgui::sys::ImGui_GetPlatformIO();

            // Create a temporary render state structure
            let mut render_state = crate::WgpuRenderState::new(&backend_data.device, render_pass);

            // Set the render state pointer
            (*platform_io).Renderer_RenderState =
                &mut render_state as *mut _ as *mut std::ffi::c_void;

            // Render draw lists with the render state exposed
            let result = Self::render_draw_lists_static(
                &mut self.texture_manager,
                &self.default_texture,
                draw_data,
                render_pass,
                backend_data,
            );

            // Clear the render state pointer
            (*platform_io).Renderer_RenderState = std::ptr::null_mut();

            result?;
        }

        Ok(())
    }

    /// Prepare frame resources (buffers)
    fn prepare_frame_resources_static(
        draw_data: &DrawData,
        backend_data: &mut WgpuBackendData,
    ) -> RendererResult<()> {
        // Calculate total vertex and index counts
        let mut total_vtx_count = 0;
        let mut total_idx_count = 0;
        for draw_list in draw_data.draw_lists() {
            total_vtx_count += draw_list.vtx_buffer().len();
            total_idx_count += draw_list.idx_buffer().len();
        }

        if total_vtx_count == 0 || total_idx_count == 0 {
            return Ok(());
        }

        // Collect all vertices and indices first
        let mut vertices = Vec::with_capacity(total_vtx_count);
        let mut indices = Vec::with_capacity(total_idx_count);

        for draw_list in draw_data.draw_lists() {
            vertices.extend_from_slice(draw_list.vtx_buffer());
            indices.extend_from_slice(draw_list.idx_buffer());
        }

        // Get current frame resources and update buffers
        let frame_index = backend_data.frame_index % backend_data.num_frames_in_flight;
        let frame_resources = &mut backend_data.frame_resources[frame_index as usize];

        // Ensure buffer capacity and upload data
        frame_resources
            .ensure_vertex_buffer_capacity(&backend_data.device, total_vtx_count)
            .map_err(RendererError::BufferCreationFailed)?;
        frame_resources
            .ensure_index_buffer_capacity(&backend_data.device, total_idx_count)
            .map_err(RendererError::BufferCreationFailed)?;

        frame_resources
            .upload_vertex_data(&backend_data.queue, &vertices)
            .map_err(RendererError::BufferCreationFailed)?;
        frame_resources
            .upload_index_data(&backend_data.queue, &indices)
            .map_err(RendererError::BufferCreationFailed)?;

        Ok(())
    }

    /// Setup render state
    ///
    /// This corresponds to ImGui_ImplWGPU_SetupRenderState in the C++ implementation
    fn setup_render_state_static(
        draw_data: &DrawData,
        render_pass: &mut RenderPass,
        backend_data: &WgpuBackendData,
    ) -> RendererResult<()> {
        let pipeline = backend_data
            .pipeline_state
            .as_ref()
            .ok_or_else(|| RendererError::InvalidRenderState("Pipeline not created".to_string()))?;

        // Setup viewport
        let fb_width = draw_data.display_size[0] * draw_data.framebuffer_scale[0];
        let fb_height = draw_data.display_size[1] * draw_data.framebuffer_scale[1];
        render_pass.set_viewport(0.0, 0.0, fb_width, fb_height, 0.0, 1.0);

        // Set pipeline
        render_pass.set_pipeline(pipeline);

        // Update uniforms
        let mvp =
            Uniforms::create_orthographic_matrix(draw_data.display_pos, draw_data.display_size);
        let gamma = Uniforms::gamma_for_format(backend_data.render_target_format);
        let mut uniforms = Uniforms::new();
        uniforms.update(mvp, gamma);

        // Update uniform buffer
        if let Some(uniform_buffer) = backend_data.render_resources.uniform_buffer() {
            uniform_buffer.update(&backend_data.queue, &uniforms);
            render_pass.set_bind_group(0, uniform_buffer.bind_group(), &[]);
        }

        // Set vertex and index buffers
        let frame_resources = &backend_data.frame_resources
            [(backend_data.frame_index % backend_data.num_frames_in_flight) as usize];
        if let (Some(vertex_buffer), Some(index_buffer)) = (
            frame_resources.vertex_buffer(),
            frame_resources.index_buffer(),
        ) {
            render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            render_pass.set_index_buffer(index_buffer.slice(..), IndexFormat::Uint16);
        }

        Ok(())
    }

    /// Render all draw lists
    fn render_draw_lists_static(
        texture_manager: &mut WgpuTextureManager,
        default_texture: &Option<TextureView>,
        draw_data: &DrawData,
        render_pass: &mut RenderPass,
        backend_data: &WgpuBackendData,
    ) -> RendererResult<()> {
        let mut global_vtx_offset = 0i32;
        let mut global_idx_offset = 0u32;
        let clip_scale = draw_data.framebuffer_scale;
        let clip_off = draw_data.display_pos;
        let fb_width = draw_data.display_size[0] * draw_data.framebuffer_scale[0];
        let fb_height = draw_data.display_size[1] * draw_data.framebuffer_scale[1];

        for draw_list in draw_data.draw_lists() {
            for cmd in draw_list.commands() {
                match cmd {
                    dear_imgui::render::DrawCmd::Elements { count, cmd_params } => {
                        // Get texture bind group
                        let texture_bind_group = {
                            let tex_id = cmd_params.texture_id.id() as u64;
                            if tex_id == 0 {
                                // Use default texture for null/invalid texture ID
                                if let Some(default_tex) = default_texture {
                                    backend_data.render_resources.create_image_bind_group(
                                        &backend_data.device,
                                        default_tex,
                                    )?
                                } else {
                                    return Err(RendererError::InvalidRenderState(
                                        "Default texture not available".to_string(),
                                    ));
                                }
                            } else if let Some(wgpu_texture) = texture_manager.get_texture(tex_id) {
                                backend_data.render_resources.create_image_bind_group(
                                    &backend_data.device,
                                    wgpu_texture.view(),
                                )?
                            } else {
                                // Fallback to default texture if texture not found
                                if let Some(default_tex) = default_texture {
                                    backend_data.render_resources.create_image_bind_group(
                                        &backend_data.device,
                                        default_tex,
                                    )?
                                } else {
                                    return Err(RendererError::InvalidRenderState(
                                        "Texture not found and no default texture".to_string(),
                                    ));
                                }
                            }
                        };

                        // Set texture bind group
                        render_pass.set_bind_group(1, &texture_bind_group, &[]);

                        // Project scissor/clipping rectangles into framebuffer space
                        let clip_min_x = (cmd_params.clip_rect[0] - clip_off[0]) * clip_scale[0];
                        let clip_min_y = (cmd_params.clip_rect[1] - clip_off[1]) * clip_scale[1];
                        let clip_max_x = (cmd_params.clip_rect[2] - clip_off[0]) * clip_scale[0];
                        let clip_max_y = (cmd_params.clip_rect[3] - clip_off[1]) * clip_scale[1];

                        // Clamp to viewport
                        let clip_min_x = clip_min_x.max(0.0);
                        let clip_min_y = clip_min_y.max(0.0);
                        let clip_max_x = clip_max_x.min(fb_width);
                        let clip_max_y = clip_max_y.min(fb_height);

                        if clip_max_x <= clip_min_x || clip_max_y <= clip_min_y {
                            continue;
                        }

                        // Apply scissor/clipping rectangle
                        render_pass.set_scissor_rect(
                            clip_min_x as u32,
                            clip_min_y as u32,
                            (clip_max_x - clip_min_x) as u32,
                            (clip_max_y - clip_min_y) as u32,
                        );

                        // Draw
                        let start_index = cmd_params.idx_offset as u32 + global_idx_offset;
                        let end_index = start_index + count as u32;
                        let vertex_offset = (cmd_params.vtx_offset as i32) + global_vtx_offset;
                        render_pass.draw_indexed(start_index..end_index, vertex_offset, 0..1);
                    }
                    dear_imgui::render::DrawCmd::ResetRenderState => {
                        Self::setup_render_state_static(draw_data, render_pass, backend_data)?;
                    }
                    dear_imgui::render::DrawCmd::RawCallback { .. } => {
                        // Raw callbacks are not supported in this implementation
                        // They would require access to the raw Dear ImGui draw list
                        eprintln!("Warning: Raw callbacks are not supported in WGPU renderer");
                    }
                }
            }

            global_idx_offset += draw_list.idx_buffer().len() as u32;
            global_vtx_offset += draw_list.vtx_buffer().len() as i32;
        }

        Ok(())
    }

    /// Invalidate device objects
    ///
    /// This corresponds to ImGui_ImplWGPU_InvalidateDeviceObjects in the C++ implementation
    pub fn invalidate_device_objects(&mut self) -> RendererResult<()> {
        if let Some(ref mut backend_data) = self.backend_data {
            backend_data.pipeline_state = None;
            backend_data.render_resources = RenderResources::new();

            // Clear frame resources
            for frame_resources in &mut backend_data.frame_resources {
                *frame_resources = FrameResources::new();
            }
        }

        // Clear texture manager
        self.texture_manager.clear();
        self.default_texture = None;

        Ok(())
    }

    /// Shutdown the renderer
    ///
    /// This corresponds to ImGui_ImplWGPU_Shutdown in the C++ implementation
    pub fn shutdown(&mut self) {
        self.invalidate_device_objects().ok();
        self.backend_data = None;
    }
}

impl Default for WgpuRenderer {
    fn default() -> Self {
        Self::new()
    }
}
