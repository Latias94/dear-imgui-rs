//! Main WGPU renderer implementation
//!
//! This module contains the main WgpuRenderer struct and its implementation,
//! following the pattern from imgui_impl_wgpu.cpp
//!
//! Texture Updates Flow (ImGui 1.92+)
//! - During `Context::render()`, Dear ImGui emits a list of textures to be processed in
//!   `DrawData::textures()` (see `dear_imgui_rs::render::DrawData::textures`). Each item is an
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
use dear_imgui_rs::{BackendFlags, Context, render::DrawData};
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
    /// Clear color used for secondary viewports (multi-viewport mode)
    viewport_clear_color: Color,
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
        // Native and wasm experimental path: fully configure context, including font atlas.
        #[cfg(any(
            not(target_arch = "wasm32"),
            all(target_arch = "wasm32", feature = "wasm-font-atlas-experimental")
        ))]
        {
            let mut renderer = Self::empty();
            renderer.init_with_context(init_info, imgui_ctx)?;
            Ok(renderer)
        }

        // Default wasm path: skip font atlas manipulation for safety.
        #[cfg(all(target_arch = "wasm32", not(feature = "wasm-font-atlas-experimental")))]
        {
            Self::new_without_font_atlas(init_info, imgui_ctx)
        }
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
            viewport_clear_color: Color::BLACK,
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

    /// Set clear color for secondary viewports (multi-viewport mode).
    ///
    /// This color is used as the load/clear color when rendering ImGui-created
    /// platform windows via `RenderPlatformWindowsDefault`. It is independent
    /// from whatever clear color your main swapchain uses.
    #[cfg(feature = "multi-viewport")]
    pub fn set_viewport_clear_color(&mut self, color: Color) {
        self.viewport_clear_color = color;
    }

    /// Get current clear color for secondary viewports.
    #[cfg(feature = "multi-viewport")]
    pub fn viewport_clear_color(&self) -> Color {
        self.viewport_clear_color
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

    // create_device_objects moved to renderer/pipeline.rs

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
    // reload_font_texture moved to renderer/font_atlas.rs

    /// Legacy/fallback path: upload font atlas texture immediately and assign TexID.
    /// Returns Some(tex_id) on success, None if texdata is unavailable.
    // try_upload_font_atlas_legacy moved to renderer/font_atlas.rs

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
    /// # let mut texture_data = dear_imgui_rs::TextureData::new();
    /// let result = renderer.update_texture(&texture_data)?;
    /// result.apply_to(&mut texture_data);
    /// # Ok(())
    /// # }
    /// ```
    pub fn update_texture(
        &mut self,
        texture_data: &dear_imgui_rs::TextureData,
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
        // Override viewport to the provided framebuffer size to avoid partial viewport issues
        render_pass.set_viewport(0.0, 0.0, fb_width as f32, fb_height as f32, 0.0, 1.0);

        // Setup render state structure (for callbacks and custom texture bindings)
        // Note: We need to be careful with lifetimes here, so we'll set it just before rendering
        // and clear it immediately after
        unsafe {
            // Use _Nil variant as our bindings export it
            let platform_io = dear_imgui_rs::sys::igGetPlatformIO_Nil();

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

    pub fn render_draw_data_with_fb_size(
        &mut self,
        draw_data: &DrawData,
        render_pass: &mut RenderPass,
        fb_width: u32,
        fb_height: u32,
    ) -> RendererResult<()> {
        // Public helper used by the main window: advance frame resources as usual.
        self.render_draw_data_with_fb_size_ex(draw_data, render_pass, fb_width, fb_height, true)
    }

    /// Internal variant that optionally skips advancing the frame index.
    ///
    /// When `advance_frame` is `false`, we reuse the current frame resources.
    fn render_draw_data_with_fb_size_ex(
        &mut self,
        draw_data: &DrawData,
        render_pass: &mut RenderPass,
        fb_width: u32,
        fb_height: u32,
        advance_frame: bool,
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

        if advance_frame {
            backend_data.next_frame();
        }
        Self::prepare_frame_resources_static(draw_data, backend_data)?;

        let gamma = match self.gamma_mode {
            GammaMode::Auto => Uniforms::gamma_for_format(backend_data.render_target_format),
            GammaMode::Linear => 1.0,
            GammaMode::Gamma22 => 2.2,
        };

        Self::setup_render_state_static(draw_data, render_pass, backend_data, gamma)?;

        unsafe {
            let platform_io = dear_imgui_rs::sys::igGetPlatformIO_Nil();
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
                        dear_imgui_rs::render::DrawCmd::Elements {
                            count,
                            cmd_params,
                            raw_cmd,
                        } => {
                            // Texture bind group resolution mirrors render_draw_lists_static
                            let texture_bind_group = {
                                // Resolve effective ImTextureID using raw_cmd (modern texture path)
                                let tex_id = unsafe {
                                    dear_imgui_rs::sys::ImDrawCmd_GetTexID(
                                        raw_cmd as *mut dear_imgui_rs::sys::ImDrawCmd,
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
                        dear_imgui_rs::render::DrawCmd::ResetRenderState => {
                            Self::setup_render_state_static(
                                draw_data,
                                render_pass,
                                backend_data,
                                gamma,
                            )?;
                        }
                        dear_imgui_rs::render::DrawCmd::RawCallback { .. } => {
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
    // prepare_frame_resources_static moved to renderer/draw.rs

    /// Setup render state
    ///
    /// This corresponds to ImGui_ImplWGPU_SetupRenderState in the C++ implementation
    // setup_render_state_static moved to renderer/draw.rs

    /// Render all draw lists
    // render_draw_lists_static moved to renderer/draw.rs

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

// Submodules for renderer features
mod draw;
mod external_textures;
mod font_atlas;
#[cfg(feature = "multi-viewport")]
pub mod multi_viewport;
mod pipeline;

impl Default for WgpuRenderer {
    fn default() -> Self {
        Self::empty()
    }
}
