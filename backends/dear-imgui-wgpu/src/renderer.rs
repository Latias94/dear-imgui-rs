//! Main WGPU renderer implementation
//!
//! This module contains the main WgpuRenderer struct and its implementation,
//! following the pattern from imgui_impl_wgpu.cpp
//!
//! Texture Updates Flow (ImGui 1.92+)
//! - During `Context::render()`, Dear ImGui emits a list of textures to be processed in
//!   `DrawData::textures()` (see `dear_imgui::render::DrawData::textures`). Each item is an
//!   `ImTextureData*` with a `Status` field:
//!   - `WantCreate`: create a GPU texture, upload all pixels, set `TexID`, then set status `OK`.
//!   - `WantUpdates`: upload `UpdateRect` (and any queued rects) then set `OK`.
//!   - `WantDestroy`: schedule/destroy GPU texture; if unused for some frames, set `Destroyed`.
//! - This backend honors these transitions in its texture module; users can simply pass
//!   `&mut TextureData` to UI/draw calls and let the backend handle the rest.

use crate::GammaMode;
use crate::{
    FrameResources, RenderResources, RendererError, RendererResult, ShaderManager, Uniforms,
    WgpuBackendData, WgpuInitInfo, WgpuTextureManager,
};
use dear_imgui::{BackendFlags, Context, render::DrawData};
use wgpu::*;

// Debug logging helper (off by default). Enable by building this crate with
// `--features mv-log` to see multi-viewport renderer traces.
#[allow(unused_macros)]
macro_rules! mvlog {
    ($($arg:tt)*) => {
        if cfg!(feature = "mv-log") { eprintln!($($arg)*); }
    }
}

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
    /// Registered font atlas texture id (if created via font-atlas fallback)
    font_texture_id: Option<u64>,
    /// Gamma mode: automatic (by format), force linear (1.0), or force 2.2
    gamma_mode: GammaMode,
}

impl WgpuRenderer {
    /// Create a new WGPU renderer with full initialization (recommended)
    ///
    /// This is the preferred way to create a WGPU renderer as it ensures proper
    /// initialization order and is consistent with other backends.
    ///
    /// # Arguments
    /// * `init_info` - WGPU initialization information (device, queue, format)
    /// * `imgui_ctx` - Dear ImGui context to configure
    ///
    /// # Example
    /// ```rust,no_run
    /// use dear_imgui_wgpu::{WgpuRenderer, WgpuInitInfo};
    ///
    /// let init_info = WgpuInitInfo::new(device, queue, surface_format);
    /// let mut renderer = WgpuRenderer::new(init_info, &mut imgui_context)?;
    /// ```
    pub fn new(init_info: WgpuInitInfo, imgui_ctx: &mut Context) -> RendererResult<Self> {
        let mut renderer = Self::empty();
        renderer.init_with_context(init_info, imgui_ctx)?;
        Ok(renderer)
    }

    /// Create an empty WGPU renderer for advanced usage
    ///
    /// This creates an uninitialized renderer that must be initialized later
    /// using `init_with_context()`. Most users should use `new()` instead.
    ///
    /// # Example
    /// ```rust,no_run
    /// use dear_imgui_wgpu::{WgpuRenderer, WgpuInitInfo};
    ///
    /// let mut renderer = WgpuRenderer::empty();
    /// let init_info = WgpuInitInfo::new(device, queue, surface_format);
    /// renderer.init_with_context(init_info, &mut imgui_context)?;
    /// ```
    pub fn empty() -> Self {
        Self {
            backend_data: None,
            shader_manager: ShaderManager::new(),
            texture_manager: WgpuTextureManager::new(),
            default_texture: None,
            font_texture_id: None,
            gamma_mode: GammaMode::Auto,
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

    /// Initialize the renderer with ImGui context configuration (without font atlas for WASM)
    ///
    /// This is a variant of init_with_context that skips font atlas preparation,
    /// useful for WASM builds where font atlas memory sharing is problematic.
    pub fn new_without_font_atlas(
        init_info: WgpuInitInfo,
        imgui_ctx: &mut Context,
    ) -> RendererResult<Self> {
        let mut renderer = Self::empty();

        // First initialize the renderer
        renderer.init(init_info)?;

        // Then configure the ImGui context with backend capabilities
        renderer.configure_imgui_context(imgui_ctx);

        // Skip font atlas preparation for WASM
        // The default font will be used automatically by Dear ImGui

        Ok(renderer)
    }

    /// Initialize the renderer with ImGui context configuration
    ///
    /// This is a convenience method that combines init() and configure_imgui_context()
    /// to ensure proper initialization order, similar to the glow backend approach.
    pub fn init_with_context(
        &mut self,
        init_info: WgpuInitInfo,
        imgui_ctx: &mut Context,
    ) -> RendererResult<()> {
        // First initialize the renderer
        self.init(init_info)?;

        // Then configure the ImGui context with backend capabilities
        // This must be done BEFORE preparing the font atlas
        self.configure_imgui_context(imgui_ctx);

        // Finally prepare the font atlas
        self.prepare_font_atlas(imgui_ctx)?;

        Ok(())
    }

    /// Set gamma mode
    pub fn set_gamma_mode(&mut self, mode: GammaMode) {
        self.gamma_mode = mode;
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

        #[cfg(feature = "multi-viewport")]
        {
            // We can render additional platform windows
            flags.insert(BackendFlags::RENDERER_HAS_VIEWPORTS);
        }

        io.set_backend_flags(flags);
        // Note: Dear ImGui doesn't expose set_backend_renderer_name in the Rust bindings yet
    }

    /// Prepare font atlas for rendering
    pub fn prepare_font_atlas(&mut self, imgui_ctx: &mut Context) -> RendererResult<()> {
        if let Some(backend_data) = &self.backend_data {
            let device = backend_data.device.clone();
            let queue = backend_data.queue.clone();
            self.reload_font_texture(imgui_ctx, &device, &queue)?;
            // Fallback: if draw_data-based texture updates are not triggered for the font atlas
            // on this Dear ImGui version/config, upload the font atlas now and assign a TexID.
            if self.font_texture_id.is_none() {
                if let Some(tex_id) =
                    self.try_upload_font_atlas_legacy(imgui_ctx, &device, &queue)?
                {
                    if cfg!(debug_assertions) {
                        tracing::debug!(
                            target: "dear-imgui-wgpu",
                            "[dear-imgui-wgpu][debug] Font atlas uploaded via fallback (legacy-only) path; user textures use modern ImTextureData. tex_id={}",
                            tex_id
                        );
                    }
                    self.font_texture_id = Some(tex_id);
                }
            } else if cfg!(debug_assertions) {
                tracing::debug!(
                    target: "dear-imgui-wgpu",
                    "[dear-imgui-wgpu][debug] Font atlas tex_id already set: {:?}",
                    self.font_texture_id
                );
            }
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
    ///
    /// With the new texture management system in Dear ImGui 1.92+, font textures are
    /// automatically managed through ImDrawData->Textures[] during rendering.
    /// However, we need to ensure the font atlas is built and ready before the first render.
    fn reload_font_texture(
        &mut self,
        imgui_ctx: &mut Context,
        _device: &Device,
        _queue: &Queue,
    ) -> RendererResult<()> {
        let mut fonts = imgui_ctx.font_atlas_mut();
        // Build the font atlas if not already built
        // This prepares the font data but doesn't create GPU textures yet
        if !fonts.is_built() {
            fonts.build();
        }

        // Do not manually set TexRef/TexID here. With BackendFlags::RENDERER_HAS_TEXTURES,
        // Dear ImGui will emit texture requests (WantCreate/WantUpdates) via DrawData::textures(),
        // and our texture manager will create/upload the font texture on demand during rendering.

        Ok(())
    }

    /// Legacy/fallback path: upload font atlas texture immediately and assign TexID.
    /// Returns Some(tex_id) on success, None if texdata is unavailable.
    fn try_upload_font_atlas_legacy(
        &mut self,
        imgui_ctx: &mut Context,
        device: &Device,
        queue: &Queue,
    ) -> RendererResult<Option<u64>> {
        // SAFETY: Access raw TexData/bytes only to copy pixels. Requires fonts.build() called.
        let fonts = imgui_ctx.font_atlas();
        // Try to read raw texture data to determine bytes-per-pixel
        let raw_tex = fonts.get_tex_data();
        if raw_tex.is_null() {
            if cfg!(debug_assertions) {
                tracing::debug!(
                    target: "dear-imgui-wgpu",
                    "[dear-imgui-wgpu][debug] Font atlas TexData is null; skip legacy upload"
                );
            }
            return Ok(None);
        }
        // Read metadata
        let (width, height, bpp, pixels_slice): (u32, u32, i32, Option<&[u8]>) = unsafe {
            let w = (*raw_tex).Width as u32;
            let h = (*raw_tex).Height as u32;
            let bpp = (*raw_tex).BytesPerPixel;
            let px_ptr = (*raw_tex).Pixels as *const u8;
            if px_ptr.is_null() || w == 0 || h == 0 {
                (w, h, bpp, None)
            } else {
                let size = (w as usize) * (h as usize) * (bpp as usize).max(1);
                (w, h, bpp, Some(std::slice::from_raw_parts(px_ptr, size)))
            }
        };

        if let Some(src) = pixels_slice {
            if cfg!(debug_assertions) {
                tracing::debug!(
                    target: "dear-imgui-wgpu",
                    "[dear-imgui-wgpu][debug] Font atlas texdata: {}x{} bpp={} (fallback upload for font atlas)",
                    width, height, bpp
                );
            }
            // Convert to RGBA8 if needed
            let (format, converted): (wgpu::TextureFormat, Vec<u8>) = if bpp == 4 {
                (wgpu::TextureFormat::Rgba8Unorm, src.to_vec())
            } else if bpp == 1 {
                // Alpha8 -> RGBA8 (white RGB + alpha)
                let mut out = Vec::with_capacity((width as usize) * (height as usize) * 4);
                for &a in src.iter() {
                    out.extend_from_slice(&[255, 255, 255, a]);
                }
                (wgpu::TextureFormat::Rgba8Unorm, out)
            } else {
                // Unexpected format; don't proceed
                if cfg!(debug_assertions) {
                    tracing::debug!(
                        target: "dear-imgui-wgpu",
                        "[dear-imgui-wgpu][debug] Unexpected font atlas bpp={} -> skip",
                        bpp
                    );
                }
                return Ok(None);
            };

            // Create WGPU texture
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Dear ImGui Font Atlas"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            // Write with 256-byte aligned row pitch
            let bpp = 4u32;
            let unpadded = width * bpp;
            let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
            let padded = unpadded.div_ceil(align) * align;
            if padded == unpadded {
                queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    &converted,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(unpadded),
                        rows_per_image: Some(height),
                    },
                    wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                );
            } else {
                let mut padded_buf = vec![0u8; (padded * height) as usize];
                for row in 0..height as usize {
                    let src = row * (unpadded as usize);
                    let dst = row * (padded as usize);
                    padded_buf[dst..dst + (unpadded as usize)]
                        .copy_from_slice(&converted[src..src + (unpadded as usize)]);
                }
                queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    &padded_buf,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(padded),
                        rows_per_image: Some(height),
                    },
                    wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                );
                if cfg!(debug_assertions) {
                    tracing::debug!(
                        target: "dear-imgui-wgpu",
                        "[dear-imgui-wgpu][debug] Upload font atlas with padded row pitch: unpadded={} padded={}",
                        unpadded, padded
                    );
                }
            }

            // Register texture and set IDs so draw commands can bind it
            let tex_id = self
                .texture_manager
                .register_texture(crate::WgpuTexture::new(texture, view));

            // Set atlas texture id + status OK (updates TexRef and TexData)
            {
                let mut fonts_mut = imgui_ctx.font_atlas_mut();
                fonts_mut.set_texture_id(dear_imgui::TextureId::from(tex_id));
            }
            if cfg!(debug_assertions) {
                tracing::debug!(
                    target: "dear-imgui-wgpu",
                    "[dear-imgui-wgpu][debug] Font atlas fallback upload complete: tex_id={}",
                    tex_id
                );
            }

            return Ok(Some(tex_id));
        }
        if cfg!(debug_assertions) {
            tracing::debug!(
                target: "dear-imgui-wgpu",
                "[dear-imgui-wgpu][debug] Font atlas has no CPU pixel buffer; skipping fallback upload (renderer will use modern texture updates)"
            );
        }
        Ok(None)
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
        mvlog!(
            "[wgpu-mv] render_draw_data: valid={} lists={} fb_scale=({:.2},{:.2}) disp=({:.1},{:.1})",
            draw_data.valid(),
            draw_data.draw_lists_count(),
            draw_data.framebuffer_scale()[0],
            draw_data.framebuffer_scale()[1],
            draw_data.display_size()[0],
            draw_data.display_size()[1]
        );
        // Early out if nothing to draw (avoid binding/drawing without buffers)
        let mut total_vtx_count = 0usize;
        let mut total_idx_count = 0usize;
        for dl in draw_data.draw_lists() {
            total_vtx_count += dl.vtx_buffer().len();
            total_idx_count += dl.idx_buffer().len();
        }
        if total_vtx_count == 0 || total_idx_count == 0 {
            mvlog!("[wgpu-mv] no vertices/indices; skipping render");
            return Ok(());
        }

        let backend_data = self.backend_data.as_mut().ok_or_else(|| {
            RendererError::InvalidRenderState("Renderer not initialized".to_string())
        })?;

        // Avoid rendering when minimized
        let fb_width = (draw_data.display_size[0] * draw_data.framebuffer_scale[0]) as i32;
        let fb_height = (draw_data.display_size[1] * draw_data.framebuffer_scale[1]) as i32;
        if fb_width <= 0 || fb_height <= 0 || !draw_data.valid() {
            return Ok(());
        }

        mvlog!("[wgpu-mv] handle_texture_updates");
        self.texture_manager.handle_texture_updates(
            draw_data,
            &backend_data.device,
            &backend_data.queue,
        );

        // Advance to next frame
        mvlog!("[wgpu-mv] next_frame before: {}", backend_data.frame_index);
        backend_data.next_frame();
        mvlog!("[wgpu-mv] next_frame after: {}", backend_data.frame_index);

        // Prepare frame resources
        mvlog!("[wgpu-mv] prepare_frame_resources");
        Self::prepare_frame_resources_static(draw_data, backend_data)?;

        // Compute gamma based on renderer mode
        let gamma = match self.gamma_mode {
            GammaMode::Auto => Uniforms::gamma_for_format(backend_data.render_target_format),
            GammaMode::Linear => 1.0,
            GammaMode::Gamma22 => 2.2,
        };

        // Setup render state
        mvlog!("[wgpu-mv] setup_render_state");
        Self::setup_render_state_static(draw_data, render_pass, backend_data, gamma)?;

        // Setup render state structure (for callbacks and custom texture bindings)
        // Note: We need to be careful with lifetimes here, so we'll set it just before rendering
        // and clear it immediately after
        unsafe {
            // Use _Nil variant as our bindings export it
            let platform_io = dear_imgui::sys::igGetPlatformIO_Nil();

            // Create a temporary render state structure
            mvlog!("[wgpu-mv] create render_state");
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
                gamma,
            );

            // Clear the render state pointer
            (*platform_io).Renderer_RenderState = std::ptr::null_mut();

            if let Err(e) = result {
                eprintln!("[wgpu-mv] render_draw_lists_static error: {:?}", e);
                return Err(e);
            }
        }

        Ok(())
    }

    /// Render Dear ImGui draw data with explicit framebuffer size override
    ///
    /// This clones the logic of `render_draw_data` but clamps scissor to the provided
    /// `fb_width`/`fb_height` instead of deriving it from `DrawData`.
    pub fn render_draw_data_with_fb_size(
        &mut self,
        draw_data: &DrawData,
        render_pass: &mut RenderPass,
        fb_width: u32,
        fb_height: u32,
    ) -> RendererResult<()> {
        mvlog!(
            "[wgpu-mv] render_draw_data(with_fb) lists={} override_fb=({}, {}) disp=({:.1},{:.1})",
            draw_data.draw_lists_count(),
            fb_width,
            fb_height,
            draw_data.display_size()[0],
            draw_data.display_size()[1]
        );
        let total_vtx_count: usize = draw_data.draw_lists().map(|dl| dl.vtx_buffer().len()).sum();
        let total_idx_count: usize = draw_data.draw_lists().map(|dl| dl.idx_buffer().len()).sum();
        if total_vtx_count == 0 || total_idx_count == 0 {
            return Ok(());
        }
        let backend_data = self.backend_data.as_mut().ok_or_else(|| {
            RendererError::InvalidRenderState("Renderer not initialized".to_string())
        })?;

        // Skip if invalid/minimized
        if fb_width == 0 || fb_height == 0 || !draw_data.valid() {
            return Ok(());
        }

        self.texture_manager.handle_texture_updates(
            draw_data,
            &backend_data.device,
            &backend_data.queue,
        );

        backend_data.next_frame();
        Self::prepare_frame_resources_static(draw_data, backend_data)?;

        let gamma = match self.gamma_mode {
            GammaMode::Auto => Uniforms::gamma_for_format(backend_data.render_target_format),
            GammaMode::Linear => 1.0,
            GammaMode::Gamma22 => 2.2,
        };

        Self::setup_render_state_static(draw_data, render_pass, backend_data, gamma)?;

        unsafe {
            let platform_io = dear_imgui::sys::igGetPlatformIO_Nil();
            let mut render_state = crate::WgpuRenderState::new(&backend_data.device, render_pass);
            (*platform_io).Renderer_RenderState =
                &mut render_state as *mut _ as *mut std::ffi::c_void;

            // Reuse core routine but clamp scissor by overriding framebuffer bounds.
            let mut global_idx_offset: u32 = 0;
            let mut global_vtx_offset: i32 = 0;
            let clip_off = draw_data.display_pos();
            let clip_scale = draw_data.framebuffer_scale();
            let fbw = fb_width as f32;
            let fbh = fb_height as f32;

            for draw_list in draw_data.draw_lists() {
                let vtx_buffer = draw_list.vtx_buffer();
                let idx_buffer = draw_list.idx_buffer();
                let mut cmd_i = 0;
                for cmd in draw_list.commands() {
                    match cmd {
                        dear_imgui::render::DrawCmd::Elements {
                            count,
                            cmd_params,
                            raw_cmd,
                        } => {
                            // Texture bind group resolution mirrors render_draw_lists_static
                            let texture_bind_group = {
                                // Resolve effective ImTextureID using raw_cmd (modern texture path)
                                let tex_id = unsafe {
                                    dear_imgui::sys::ImDrawCmd_GetTexID(
                                        raw_cmd as *mut dear_imgui::sys::ImDrawCmd,
                                    )
                                } as u64;
                                if tex_id == 0 {
                                    if let Some(default_tex) = &self.default_texture {
                                        backend_data
                                            .render_resources
                                            .get_or_create_image_bind_group(
                                                &backend_data.device,
                                                0,
                                                default_tex,
                                            )?
                                            .clone()
                                    } else {
                                        return Err(RendererError::InvalidRenderState(
                                            "Default texture not available".to_string(),
                                        ));
                                    }
                                } else if let Some(wgpu_texture) =
                                    self.texture_manager.get_texture(tex_id)
                                {
                                    backend_data
                                        .render_resources
                                        .get_or_create_image_bind_group(
                                            &backend_data.device,
                                            tex_id,
                                            &wgpu_texture.texture_view,
                                        )?
                                        .clone()
                                } else if let Some(default_tex) = &self.default_texture {
                                    backend_data
                                        .render_resources
                                        .get_or_create_image_bind_group(
                                            &backend_data.device,
                                            0,
                                            default_tex,
                                        )?
                                        .clone()
                                } else {
                                    return Err(RendererError::InvalidRenderState(
                                        "Texture not found and no default texture".to_string(),
                                    ));
                                }
                            };
                            render_pass.set_bind_group(1, &texture_bind_group, &[]);

                            // Compute clip rect in framebuffer space
                            let mut clip_min_x =
                                (cmd_params.clip_rect[0] - clip_off[0]) * clip_scale[0];
                            let mut clip_min_y =
                                (cmd_params.clip_rect[1] - clip_off[1]) * clip_scale[1];
                            let mut clip_max_x =
                                (cmd_params.clip_rect[2] - clip_off[0]) * clip_scale[0];
                            let mut clip_max_y =
                                (cmd_params.clip_rect[3] - clip_off[1]) * clip_scale[1];
                            // Clamp to override framebuffer bounds
                            clip_min_x = clip_min_x.max(0.0);
                            clip_min_y = clip_min_y.max(0.0);
                            clip_max_x = clip_max_x.min(fbw);
                            clip_max_y = clip_max_y.min(fbh);
                            if clip_max_x <= clip_min_x || clip_max_y <= clip_min_y {
                                cmd_i += 1;
                                continue;
                            }
                            render_pass.set_scissor_rect(
                                clip_min_x as u32,
                                clip_min_y as u32,
                                (clip_max_x - clip_min_x) as u32,
                                (clip_max_y - clip_min_y) as u32,
                            );
                            let start_index = cmd_params.idx_offset as u32 + global_idx_offset;
                            let end_index = start_index + count as u32;
                            let vertex_offset = (cmd_params.vtx_offset as i32) + global_vtx_offset;
                            render_pass.draw_indexed(start_index..end_index, vertex_offset, 0..1);
                        }
                        dear_imgui::render::DrawCmd::ResetRenderState => {
                            Self::setup_render_state_static(
                                draw_data,
                                render_pass,
                                backend_data,
                                gamma,
                            )?;
                        }
                        dear_imgui::render::DrawCmd::RawCallback { .. } => {
                            // Unsupported raw callbacks; skip.
                        }
                    }
                    cmd_i += 1;
                }

                global_idx_offset += idx_buffer.len() as u32;
                global_vtx_offset += vtx_buffer.len() as i32;
            }

            (*platform_io).Renderer_RenderState = std::ptr::null_mut();
        }

        Ok(())
    }

    /// Prepare frame resources (buffers)
    fn prepare_frame_resources_static(
        draw_data: &DrawData,
        backend_data: &mut WgpuBackendData,
    ) -> RendererResult<()> {
        mvlog!("[wgpu-mv] totals start");
        // Calculate total vertex and index counts
        let mut total_vtx_count = 0;
        let mut total_idx_count = 0;
        for draw_list in draw_data.draw_lists() {
            total_vtx_count += draw_list.vtx_buffer().len();
            total_idx_count += draw_list.idx_buffer().len();
        }
        mvlog!(
            "[wgpu-mv] totals vtx={} idx={}",
            total_vtx_count,
            total_idx_count
        );

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
        gamma: f32,
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
        backend_data: &mut WgpuBackendData,
        gamma: f32,
    ) -> RendererResult<()> {
        let mut global_vtx_offset = 0i32;
        let mut global_idx_offset = 0u32;
        let clip_scale = draw_data.framebuffer_scale;
        let clip_off = draw_data.display_pos;
        let fb_width = draw_data.display_size[0] * draw_data.framebuffer_scale[0];
        let fb_height = draw_data.display_size[1] * draw_data.framebuffer_scale[1];

        let mut list_i = 0usize;
        for draw_list in draw_data.draw_lists() {
            mvlog!(
                "[wgpu-mv] list[{}]: vtx={} idx={} cmds~?",
                list_i,
                draw_list.vtx_buffer().len(),
                draw_list.idx_buffer().len()
            );
            let mut cmd_i = 0usize;
            for cmd in draw_list.commands() {
                match cmd {
                    dear_imgui::render::DrawCmd::Elements {
                        count,
                        cmd_params,
                        raw_cmd,
                    } => {
                        mvlog!(
                            "[wgpu-mv] list[{}] cmd[{}]: count={} tex=?",
                            list_i,
                            cmd_i,
                            count
                        );
                        // Get texture bind group
                        //
                        // Dear ImGui 1.92+ (modern texture system): draw commands may carry
                        // an ImTextureRef whose `TexID` is 0 in the raw field, and the effective
                        // id must be resolved via ImDrawCmd_GetTexID using the backend's
                        // Renderer_RenderState. We call it here, after texture updates have been
                        // handled for this frame, so it returns a valid non-zero id.
                        let texture_bind_group = {
                            // Resolve effective ImTextureID now (after texture updates)
                            let tex_id = unsafe {
                                dear_imgui::sys::ImDrawCmd_GetTexID(
                                    raw_cmd as *mut dear_imgui::sys::ImDrawCmd,
                                )
                            } as u64;
                            if tex_id == 0 {
                                // Use default texture for null/invalid texture ID
                                if let Some(default_tex) = default_texture {
                                    backend_data
                                        .render_resources
                                        .get_or_create_image_bind_group(
                                            &backend_data.device,
                                            0,
                                            default_tex,
                                        )?
                                        .clone()
                                } else {
                                    return Err(RendererError::InvalidRenderState(
                                        "Default texture not available".to_string(),
                                    ));
                                }
                            } else if let Some(wgpu_texture) = texture_manager.get_texture(tex_id) {
                                backend_data
                                    .render_resources
                                    .get_or_create_image_bind_group(
                                        &backend_data.device,
                                        tex_id,
                                        wgpu_texture.view(),
                                    )?
                                    .clone()
                            } else {
                                // Fallback to default texture if texture not found
                                if let Some(default_tex) = default_texture {
                                    backend_data
                                        .render_resources
                                        .get_or_create_image_bind_group(
                                            &backend_data.device,
                                            0,
                                            default_tex,
                                        )?
                                        .clone()
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
                        // Re-apply render state using the same gamma
                        Self::setup_render_state_static(
                            draw_data,
                            render_pass,
                            backend_data,
                            gamma,
                        )?;
                    }
                    dear_imgui::render::DrawCmd::RawCallback { .. } => {
                        // Raw callbacks are not supported in this implementation
                        // They would require access to the raw Dear ImGui draw list
                        tracing::warn!(
                            target: "dear-imgui-wgpu",
                            "Warning: Raw callbacks are not supported in WGPU renderer"
                        );
                    }
                }
                cmd_i += 1;
            }

            global_idx_offset += draw_list.idx_buffer().len() as u32;
            global_vtx_offset += draw_list.vtx_buffer().len() as i32;
            list_i += 1;
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
        self.font_texture_id = None;

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
        Self::empty()
    }
}

/// Multi-viewport support (Renderer_* callbacks and helpers)
#[cfg(feature = "multi-viewport")]
pub mod multi_viewport {
    use super::*;
    use dear_imgui::platform_io::Viewport;
    use std::ffi::c_void;
    use std::sync::OnceLock;
    use std::sync::atomic::{AtomicUsize, Ordering};
    #[cfg(not(target_arch = "wasm32"))]
    use winit::window::Window;

    /// Per-viewport WGPU data stored in ImGuiViewport::RendererUserData
    pub struct ViewportWgpuData {
        pub surface: wgpu::Surface<'static>,
        pub config: wgpu::SurfaceConfiguration,
        pub pending_frame: Option<wgpu::SurfaceTexture>,
    }

    static RENDERER_PTR: AtomicUsize = AtomicUsize::new(0);
    static GLOBAL: OnceLock<GlobalHandles> = OnceLock::new();

    struct GlobalHandles {
        instance: Option<wgpu::Instance>,
        adapter: Option<wgpu::Adapter>,
        device: wgpu::Device,
        queue: wgpu::Queue,
        render_target_format: wgpu::TextureFormat,
    }

    /// Enable WGPU multi-viewport: set per-viewport callbacks and capture renderer pointer
    pub fn enable(renderer: &mut WgpuRenderer, imgui_context: &mut Context) {
        // Expose callbacks through PlatformIO
        unsafe {
            let platform_io = imgui_context.platform_io_mut();
            platform_io.set_renderer_create_window(Some(
                renderer_create_window as unsafe extern "C" fn(*mut Viewport),
            ));
            platform_io.set_renderer_destroy_window(Some(
                renderer_destroy_window as unsafe extern "C" fn(*mut Viewport),
            ));
            platform_io.set_renderer_set_window_size(Some(
                renderer_set_window_size
                    as unsafe extern "C" fn(*mut Viewport, dear_imgui::sys::ImVec2),
            ));
            // Route rendering via platform raw callbacks to avoid typed trampolines
            platform_io.set_platform_render_window_raw(Some(platform_render_window_sys));
            platform_io.set_platform_swap_buffers_raw(Some(platform_swap_buffers_sys));
        }

        // Store self pointer for callbacks
        RENDERER_PTR.store(renderer as *mut _ as usize, Ordering::SeqCst);

        // Also store global handles so creation/resizing callbacks don't rely on renderer pointer stability
        if let Some(backend) = renderer.backend_data.as_ref() {
            let _ = GLOBAL.set(GlobalHandles {
                instance: backend.instance.clone(),
                adapter: backend.adapter.clone(),
                device: backend.device.clone(),
                queue: backend.queue.clone(),
                render_target_format: backend.render_target_format,
            });
        }
    }

    unsafe fn get_renderer<'a>() -> &'a mut WgpuRenderer {
        let ptr = RENDERER_PTR.load(Ordering::SeqCst) as *mut WgpuRenderer;
        &mut *ptr
    }

    /// Helper to get or create per-viewport user data
    unsafe fn viewport_user_data_mut<'a>(vp: *mut Viewport) -> Option<&'a mut ViewportWgpuData> {
        let vpm = &mut *vp;
        let data = vpm.renderer_user_data();
        if data.is_null() {
            None
        } else {
            Some(&mut *(data as *mut ViewportWgpuData))
        }
    }

    /// Renderer: create per-viewport resources (surface + config)
    pub unsafe extern "C" fn renderer_create_window(vp: *mut Viewport) {
        if vp.is_null() {
            return;
        }
        mvlog!("[wgpu-mv] Renderer_CreateWindow");

        let global = match GLOBAL.get() {
            Some(g) => g,
            None => return,
        };

        // Obtain window from platform handle
        let vpm = &mut *vp;
        let window_ptr = vpm.platform_handle();
        if window_ptr.is_null() {
            return;
        }

        #[cfg(not(target_arch = "wasm32"))]
        let window: &Window = &*(window_ptr as *const Window);

        #[cfg(not(target_arch = "wasm32"))]
        let instance = match &global.instance {
            Some(i) => i.clone(),
            None => return, // cannot create surfaces without instance
        };

        #[cfg(not(target_arch = "wasm32"))]
        let surface = match instance.create_surface(window) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[wgpu-mv] create_surface error: {:?}", e);
                return;
            }
        };
        // Extend surface lifetime to 'static by tying it to backend-owned instance
        let surface: wgpu::Surface<'static> = std::mem::transmute(surface);

        #[cfg(not(target_arch = "wasm32"))]
        let size = window.inner_size();
        #[cfg(not(target_arch = "wasm32"))]
        let width = size.width.max(1);
        #[cfg(not(target_arch = "wasm32"))]
        let height = size.height.max(1);

        #[cfg(not(target_arch = "wasm32"))]
        let mut config = {
            // Prefer the renderer's main format if the surface supports it; otherwise, bail out gracefully
            if let Some(adapter) = &global.adapter {
                let caps = surface.get_capabilities(adapter);
                let format = if caps.formats.contains(&global.render_target_format) {
                    global.render_target_format
                } else {
                    // If the main pipeline format isn't supported, we cannot render safely with this pipeline.
                    eprintln!(
                        "[wgpu-mv] Surface doesn't support pipeline format {:?}; supported: {:?}. Skipping configure.",
                        global.render_target_format, caps.formats
                    );
                    return;
                };
                let present_mode = if caps.present_modes.contains(&wgpu::PresentMode::Fifo) {
                    wgpu::PresentMode::Fifo
                } else {
                    // Fallback to first supported present mode
                    caps.present_modes
                        .get(0)
                        .cloned()
                        .unwrap_or(wgpu::PresentMode::Fifo)
                };
                let alpha_mode = if caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::Auto) {
                    wgpu::CompositeAlphaMode::Auto
                } else {
                    caps.alpha_modes
                        .get(0)
                        .cloned()
                        .unwrap_or(wgpu::CompositeAlphaMode::Opaque)
                };
                wgpu::SurfaceConfiguration {
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    format,
                    width,
                    height,
                    present_mode,
                    alpha_mode,
                    view_formats: vec![format],
                    desired_maximum_frame_latency: 1,
                }
            } else {
                // No adapter available: assume the same format as main and attempt configure (best-effort)
                wgpu::SurfaceConfiguration {
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    format: global.render_target_format,
                    width,
                    height,
                    present_mode: wgpu::PresentMode::Fifo,
                    alpha_mode: wgpu::CompositeAlphaMode::Auto,
                    view_formats: vec![global.render_target_format],
                    desired_maximum_frame_latency: 1,
                }
            }
        };

        #[cfg(not(target_arch = "wasm32"))]
        {
            // Configure with validated config
            surface.configure(&global.device, &config);
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let data = ViewportWgpuData {
                surface,
                config,
                pending_frame: None,
            };
            vpm.set_renderer_user_data(Box::into_raw(Box::new(data)) as *mut c_void);
        }
    }

    /// Renderer: destroy per-viewport resources
    pub unsafe extern "C" fn renderer_destroy_window(vp: *mut Viewport) {
        if vp.is_null() {
            return;
        }
        mvlog!("[wgpu-mv] Renderer_DestroyWindow");
        if let Some(data) = viewport_user_data_mut(vp) {
            // Drop pending frame if any
            data.pending_frame.take();
            // Free user data box
            let _ = Box::from_raw(data as *mut ViewportWgpuData);
            let vpm = &mut *vp;
            vpm.set_renderer_user_data(std::ptr::null_mut());
        }
    }

    /// Renderer: notify new size
    pub unsafe extern "C" fn renderer_set_window_size(
        vp: *mut Viewport,
        size: dear_imgui::sys::ImVec2,
    ) {
        if vp.is_null() {
            return;
        }
        mvlog!(
            "[wgpu-mv] Renderer_SetWindowSize to ({}, {})",
            size.x,
            size.y
        );
        let global = match GLOBAL.get() {
            Some(g) => g,
            None => return,
        };
        if let Some(data) = viewport_user_data_mut(vp) {
            // Convert ImGui logical size to physical pixels using framebuffer scale
            let vpm_ref = &*vp;
            let scale = vpm_ref.framebuffer_scale();
            let new_w = (size.x * scale[0]).max(1.0) as u32;
            let new_h = (size.y * scale[1]).max(1.0) as u32;
            if data.config.width != new_w || data.config.height != new_h {
                data.config.width = new_w;
                data.config.height = new_h;
                data.surface.configure(&global.device, &data.config);
            }
        }
    }

    /// Renderer: render viewport draw data into its surface
    pub unsafe extern "C" fn renderer_render_window(vp: *mut Viewport, _render_arg: *mut c_void) {
        if vp.is_null() {
            return;
        }
        mvlog!("[wgpu-mv] Renderer_RenderWindow");
        let renderer = match (get_renderer() as *mut WgpuRenderer).as_mut() {
            Some(r) => r,
            None => return,
        };
        // Clone device/queue to avoid borrowing renderer during render
        let (device, queue) = match renderer.backend_data.as_ref() {
            Some(b) => (b.device.clone(), b.queue.clone()),
            None => return,
        };
        let vpm = &mut *vp;
        let raw_dd = vpm.draw_data();
        if raw_dd.is_null() {
            return;
        }
        // SAFETY: Dear ImGui provides a valid ImDrawData during RenderPlatformWindowsDefault
        let draw_data: &dear_imgui::render::DrawData = std::mem::transmute(&*raw_dd);
        mvlog!(
            "[wgpu-mv] draw_data: valid={} lists={} fb_scale=({:.2},{:.2}) disp=({:.1},{:.1})",
            draw_data.valid(),
            draw_data.draw_lists_count(),
            draw_data.framebuffer_scale()[0],
            draw_data.framebuffer_scale()[1],
            draw_data.display_size()[0],
            draw_data.display_size()[1]
        );

        mvlog!("[wgpu-mv] retrieving viewport user data");
        if let Some(data) = unsafe { viewport_user_data_mut(vp) } {
            mvlog!("[wgpu-mv] have viewport user data; acquiring surface frame");
            // Acquire frame
            let frame = match data.surface.get_current_texture() {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("[wgpu-mv] get_current_texture error: {:?}", e);
                    return;
                }
            };
            mvlog!("[wgpu-mv] acquired frame; creating view");
            let view = frame
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());
            // Encode commands and render (catch panics to avoid crashing the whole app)
            mvlog!("[wgpu-mv] creating command encoder");
            let render_block = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("dear-imgui-wgpu::viewport-encoder"),
                });
                mvlog!("[wgpu-mv] begin_render_pass start");
                {
                    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("dear-imgui-wgpu::viewport-pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                                store: wgpu::StoreOp::Store,
                            },
                            depth_slice: None,
                        })],
                        depth_stencil_attachment: None,
                        occlusion_query_set: None,
                        timestamp_writes: None,
                    });
                    mvlog!("[wgpu-mv] begin_render_pass ok");
                    mvlog!("[wgpu-mv] about to render_draw_data for viewport");
                    // Reuse existing draw path with explicit framebuffer size to avoid scissor mismatch
                    let saved_index_opt = renderer.backend_data.as_ref().map(|b| b.frame_index);
                    if let Some(vd) = viewport_user_data_mut(vp) {
                        let fb_w = vd.config.width;
                        let fb_h = vd.config.height;
                        if let Err(e) = renderer.render_draw_data_with_fb_size(
                            &draw_data,
                            &mut render_pass,
                            fb_w,
                            fb_h,
                        ) {
                            eprintln!("[wgpu-mv] render_draw_data(with_fb) error: {:?}", e);
                        }
                    } else if let Err(e) = renderer.render_draw_data(&draw_data, &mut render_pass) {
                        eprintln!("[wgpu-mv] render_draw_data error: {:?}", e);
                    }
                    if let Some(saved_index) = saved_index_opt {
                        if let Some(backend) = renderer.backend_data.as_mut() {
                            // Only restore to a meaningful value; avoid resetting to u32::MAX
                            if saved_index != u32::MAX {
                                backend.frame_index = saved_index;
                            }
                        }
                    }
                    mvlog!("[wgpu-mv] finished render_draw_data");
                }
                mvlog!("[wgpu-mv] submitting queue");
                queue.submit(std::iter::once(encoder.finish()));
                mvlog!("[wgpu-mv] submit ok");
            }));
            if render_block.is_err() {
                eprintln!(
                    "[wgpu-mv] panic during viewport render block; skipping present for this viewport"
                );
                return;
            }
            data.pending_frame = Some(frame);
            mvlog!("[wgpu-mv] submitted and stored pending frame");
        }
    }

    /// Renderer: present frame for viewport surface
    pub unsafe extern "C" fn renderer_swap_buffers(vp: *mut Viewport, _render_arg: *mut c_void) {
        if vp.is_null() {
            return;
        }
        mvlog!("[wgpu-mv] Renderer_SwapBuffers");
        if let Some(data) = viewport_user_data_mut(vp) {
            if let Some(frame) = data.pending_frame.take() {
                frame.present();
            }
        }
    }

    // Raw sys-platform wrappers to avoid typed trampolines
    pub unsafe extern "C" fn platform_render_window_sys(
        vp: *mut dear_imgui::sys::ImGuiViewport,
        arg: *mut c_void,
    ) {
        if vp.is_null() {
            return;
        }
        renderer_render_window(vp as *mut Viewport, arg);
    }

    pub unsafe extern "C" fn platform_swap_buffers_sys(
        vp: *mut dear_imgui::sys::ImGuiViewport,
        arg: *mut c_void,
    ) {
        if vp.is_null() {
            return;
        }
        renderer_swap_buffers(vp as *mut Viewport, arg);
    }
}
