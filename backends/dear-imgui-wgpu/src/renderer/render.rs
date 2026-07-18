#[cfg(feature = "mv-log")]
use std::sync::{Mutex, OnceLock};

use super::{ActiveSampler, RendererRenderStateGuard, WgpuRenderer};
use crate::wgpu;
use crate::{GammaMode, RendererError, RendererResult, Uniforms};
use dear_imgui_rs::{
    Context, ContextBinding, TextureId,
    render::{DrawData, RenderedFrame, TextureOp},
    sys,
};
use wgpu::RenderPass;

#[allow(unused_macros)]
macro_rules! mvlog {
    ($($arg:tt)*) => {
        if cfg!(feature = "mv-log") { eprintln!($($arg)*); }
    }
}

fn with_bound_context<R>(
    binding: &ContextBinding,
    f: impl FnOnce() -> RendererResult<R>,
) -> RendererResult<R> {
    binding
        .try_with_bound_context(f)
        .map_err(|_| RendererError::ContextDropped)?
}

fn platform_io_for_current_context() -> RendererResult<*mut sys::ImGuiPlatformIO> {
    let context = unsafe { sys::igGetCurrentContext() };
    let platform_io = unsafe { sys::igGetPlatformIO_ContextPtr(context) };
    if platform_io.is_null() {
        Err(RendererError::InvalidRenderState(
            "bound Dear ImGui context has no PlatformIO".to_owned(),
        ))
    } else {
        Ok(platform_io)
    }
}

impl WgpuRenderer {
    /// Render one Context-borrowed Dear ImGui frame.
    ///
    /// Managed texture requests are applied and reconciled before draw commands resolve their
    /// renderer texture IDs. Consuming the frame prevents native draw data from escaping its
    /// owning Context borrow.
    pub fn render(
        &mut self,
        mut frame: RenderedFrame<'_>,
        render_pass: &mut RenderPass,
    ) -> RendererResult<()> {
        self.ensure_frame_matches(&frame)?;
        let binding = self.bound_context()?;
        with_bound_context(&binding, || {
            let platform_io = platform_io_for_current_context()?;
            self.reconcile_frame_textures(&mut frame)?;
            self.render_read_only_draw_data(frame.draw_data(), render_pass, platform_io)
        })
    }

    /// Finalize and render the frame for this renderer's bound ImGui context.
    ///
    /// Returns [`RendererError::ContextMismatch`] when `ctx` is not the context used to initialize
    /// this renderer. Multi-context applications must create one renderer per context.
    pub fn render_context(
        &mut self,
        ctx: &mut Context,
        render_pass: &mut RenderPass,
    ) -> RendererResult<()> {
        self.ensure_context_matches(ctx)?;
        let frame = ctx.render();
        self.render(frame, render_pass)
    }

    fn reconcile_frame_textures(&mut self, frame: &mut RenderedFrame<'_>) -> RendererResult<()> {
        let destroyed = frame
            .texture_requests()
            .iter()
            .filter_map(|request| {
                matches!(request.operation(), TextureOp::Destroy).then_some(request.texture())
            })
            .collect::<Vec<_>>();
        let backend_data = self.backend_data.as_mut().ok_or_else(|| {
            RendererError::InvalidRenderState("Renderer not initialized".to_owned())
        })?;
        let feedback = self.texture_manager.handle_texture_requests(
            frame.texture_requests(),
            &backend_data.device,
            &backend_data.queue,
            &mut backend_data.render_resources,
        )?;
        frame.reconcile_texture_feedback(feedback)?;
        self.texture_manager
            .acknowledge_destroyed_textures(destroyed);
        Ok(())
    }

    pub(super) fn render_read_only_draw_data(
        &mut self,
        draw_data: &DrawData,
        render_pass: &mut RenderPass,
        platform_io: *mut sys::ImGuiPlatformIO,
    ) -> RendererResult<()> {
        // Early out if nothing to draw (avoid binding/drawing without buffers)
        let mut total_vtx_count = 0usize;
        let mut total_idx_count = 0usize;
        for dl in draw_data.draw_lists() {
            total_vtx_count += dl.vtx_buffer().len();
            total_idx_count += dl.idx_buffer().len();
        }
        if total_vtx_count == 0 || total_idx_count == 0 {
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

        // Advance to next frame
        backend_data.next_frame();

        // Prepare frame resources
        Self::prepare_frame_resources_static(draw_data, backend_data)?;

        // Compute gamma based on renderer mode
        let gamma = match self.gamma_mode {
            GammaMode::Auto => Uniforms::gamma_for_format(backend_data.render_target_format),
            GammaMode::Linear => 1.0,
            GammaMode::Gamma22 => 2.2,
        };

        // Setup render state
        Self::setup_render_state_static(draw_data, render_pass, backend_data, gamma)?;
        // Override viewport to the provided framebuffer size to avoid partial viewport issues
        render_pass.set_viewport(0.0, 0.0, fb_width as f32, fb_height as f32, 0.0, 1.0);

        // Setup render state structure (for callbacks and custom texture bindings)
        // Note: We need to be careful with lifetimes here, so we'll set it just before rendering
        // and clear it immediately after
        unsafe {
            // Create a temporary render state structure
            let mut render_state = crate::WgpuRenderState::new(&backend_data.device, render_pass);
            let _render_state_guard = RendererRenderStateGuard::set(
                platform_io,
                &mut render_state as *mut _ as *mut std::ffi::c_void,
            )?;

            // Render draw lists with the render state exposed
            Self::render_draw_lists_static(
                &mut self.texture_manager,
                &self.default_texture,
                draw_data,
                render_pass,
                backend_data,
                gamma,
            )?;
        }

        Ok(())
    }

    /// Render one Context-borrowed frame with explicit framebuffer dimensions.
    pub fn render_with_fb_size(
        &mut self,
        mut frame: RenderedFrame<'_>,
        render_pass: &mut RenderPass,
        fb_width: u32,
        fb_height: u32,
    ) -> RendererResult<()> {
        self.ensure_frame_matches(&frame)?;
        let binding = self.bound_context()?;
        with_bound_context(&binding, || {
            let platform_io = platform_io_for_current_context()?;
            self.reconcile_frame_textures(&mut frame)?;
            self.render_read_only_draw_data_with_fb_size(
                frame.draw_data(),
                render_pass,
                fb_width,
                fb_height,
                true,
                platform_io,
            )
        })
    }

    /// Finalize and render the frame for the bound ImGui context and framebuffer size.
    ///
    /// Returns [`RendererError::ContextMismatch`] when `ctx` is not the context used to initialize
    /// this renderer.
    pub fn render_context_with_fb_size(
        &mut self,
        ctx: &mut Context,
        render_pass: &mut RenderPass,
        fb_width: u32,
        fb_height: u32,
    ) -> RendererResult<()> {
        self.ensure_context_matches(ctx)?;
        let frame = ctx.render();
        self.render_with_fb_size(frame, render_pass, fb_width, fb_height)
    }

    /// Internal variant that optionally skips advancing the frame index.
    ///
    /// When `advance_frame` is `false`, we reuse the current frame resources.
    pub(super) fn render_read_only_draw_data_with_fb_size(
        &mut self,
        draw_data: &DrawData,
        render_pass: &mut RenderPass,
        fb_width: u32,
        fb_height: u32,
        advance_frame: bool,
        platform_io: *mut sys::ImGuiPlatformIO,
    ) -> RendererResult<()> {
        // Log only when the override framebuffer size doesn't match the draw data scale.
        // This helps diagnose HiDPI/viewport scaling issues without spamming per-frame traces.
        #[cfg(feature = "mv-log")]
        {
            static LAST_MISMATCH: OnceLock<Mutex<Option<(u32, u32, u32, u32, bool)>>> =
                OnceLock::new();
            let last = LAST_MISMATCH.get_or_init(|| Mutex::new(None));
            let expected_w = (draw_data.display_size()[0] * draw_data.framebuffer_scale()[0])
                .round()
                .max(0.0) as u32;
            let expected_h = (draw_data.display_size()[1] * draw_data.framebuffer_scale()[1])
                .round()
                .max(0.0) as u32;
            if expected_w != fb_width || expected_h != fb_height {
                let key = (expected_w, expected_h, fb_width, fb_height, advance_frame);
                let mut guard = last.lock().unwrap();
                if *guard != Some(key) {
                    mvlog!(
                        "[wgpu-mv] fb mismatch expected=({}, {}) override=({}, {}) disp=({:.1},{:.1}) fb_scale=({:.2},{:.2}) main={}",
                        expected_w,
                        expected_h,
                        fb_width,
                        fb_height,
                        draw_data.display_size()[0],
                        draw_data.display_size()[1],
                        draw_data.framebuffer_scale()[0],
                        draw_data.framebuffer_scale()[1],
                        advance_frame
                    );
                    *guard = Some(key);
                }
            }
        }
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
            let mut render_state = crate::WgpuRenderState::new(&backend_data.device, render_pass);
            let _render_state_guard = RendererRenderStateGuard::set(
                platform_io,
                &mut render_state as *mut _ as *mut std::ffi::c_void,
            )?;

            // Reuse core routine but clamp scissor by overriding framebuffer bounds.
            // Extract common bind group handles up front to avoid borrowing conflicts with render_resources.
            let device = backend_data.device.clone();
            let (common_layout, uniform_buffer, default_common_bg, nearest_common_bg) = {
                let ub = backend_data
                    .render_resources
                    .uniform_buffer()
                    .ok_or_else(|| {
                        RendererError::InvalidRenderState(
                            "Uniform buffer not initialized".to_string(),
                        )
                    })?;
                let nearest_bg = backend_data
                    .render_resources
                    .nearest_common_bind_group()
                    .ok_or_else(|| {
                        RendererError::InvalidRenderState(
                            "Nearest sampler bind group not initialized".to_string(),
                        )
                    })?;
                (
                    ub.bind_group_layout().clone(),
                    ub.buffer().clone(),
                    ub.bind_group().clone(),
                    nearest_bg.clone(),
                )
            };
            let mut standard_sampler = ActiveSampler::Linear;
            let mut current_sampler = ActiveSampler::Linear;

            let mut global_idx_offset: u32 = 0;
            let mut global_vtx_offset: i32 = 0;
            let clip_off = draw_data.display_pos();
            let clip_scale = draw_data.framebuffer_scale();
            let fbw = fb_width as f32;
            let fbh = fb_height as f32;

            for draw_list in draw_data.draw_lists() {
                let vtx_buffer = draw_list.vtx_buffer();
                let idx_buffer = draw_list.idx_buffer();
                for cmd in draw_list.commands() {
                    match cmd {
                        dear_imgui_rs::render::DrawCmd::Elements {
                            count,
                            cmd_params,
                            raw_cmd: _,
                        } => {
                            let tex_id = cmd_params.texture_id;

                            // Switch common bind group (sampler) if this texture uses a custom sampler
                            // or a standard sampler callback changed the default.
                            let desired_sampler = if tex_id.is_null() {
                                standard_sampler
                            } else {
                                self.texture_manager
                                    .custom_sampler_id_for_texture(tex_id)
                                    .map(ActiveSampler::Custom)
                                    .unwrap_or(standard_sampler)
                            };
                            if desired_sampler != current_sampler {
                                match desired_sampler {
                                    ActiveSampler::Linear => {
                                        render_pass.set_bind_group(0, &default_common_bg, &[]);
                                    }
                                    ActiveSampler::Nearest => {
                                        render_pass.set_bind_group(0, &nearest_common_bg, &[]);
                                    }
                                    ActiveSampler::Custom(sampler_id) => {
                                        if let Some(bg0) = self
                                            .texture_manager
                                            .get_or_create_common_bind_group_for_sampler(
                                                &device,
                                                &common_layout,
                                                &uniform_buffer,
                                                sampler_id,
                                            )
                                        {
                                            render_pass.set_bind_group(0, &bg0, &[]);
                                        } else {
                                            render_pass.set_bind_group(0, &default_common_bg, &[]);
                                        }
                                    }
                                }
                                current_sampler = desired_sampler;
                            }

                            let texture_bind_group = if tex_id.is_null() {
                                if let Some(default_tex) = &self.default_texture {
                                    backend_data
                                        .render_resources
                                        .get_or_create_image_bind_group(
                                            &backend_data.device,
                                            TextureId::null(),
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
                                        TextureId::null(),
                                        default_tex,
                                    )?
                                    .clone()
                            } else {
                                return Err(RendererError::InvalidRenderState(
                                    "Texture not found and no default texture".to_string(),
                                ));
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
                                continue;
                            }
                            render_pass.set_scissor_rect(
                                clip_min_x as u32,
                                clip_min_y as u32,
                                (clip_max_x - clip_min_x) as u32,
                                (clip_max_y - clip_min_y) as u32,
                            );
                            let Ok(count_u32) = u32::try_from(count) else {
                                continue;
                            };
                            let Ok(idx_offset_u32) = u32::try_from(cmd_params.idx_offset) else {
                                continue;
                            };
                            let Some(start_index) = idx_offset_u32.checked_add(global_idx_offset)
                            else {
                                continue;
                            };
                            let Some(end_index) = start_index.checked_add(count_u32) else {
                                continue;
                            };
                            let Ok(vtx_offset_i32) = i32::try_from(cmd_params.vtx_offset) else {
                                continue;
                            };
                            let Some(vertex_offset) = vtx_offset_i32.checked_add(global_vtx_offset)
                            else {
                                continue;
                            };
                            render_pass.draw_indexed(start_index..end_index, vertex_offset, 0..1);
                        }
                        dear_imgui_rs::render::DrawCmd::ResetRenderState => {
                            Self::setup_render_state_static(
                                draw_data,
                                render_pass,
                                backend_data,
                                gamma,
                            )?;
                            standard_sampler = ActiveSampler::Linear;
                            current_sampler = ActiveSampler::Linear;
                        }
                        dear_imgui_rs::render::DrawCmd::SetSamplerLinear => {
                            standard_sampler = ActiveSampler::Linear;
                            if current_sampler != ActiveSampler::Linear {
                                render_pass.set_bind_group(0, &default_common_bg, &[]);
                                current_sampler = ActiveSampler::Linear;
                            }
                        }
                        dear_imgui_rs::render::DrawCmd::SetSamplerNearest => {
                            standard_sampler = ActiveSampler::Nearest;
                            if current_sampler != ActiveSampler::Nearest {
                                render_pass.set_bind_group(0, &nearest_common_bg, &[]);
                                current_sampler = ActiveSampler::Nearest;
                            }
                        }
                        dear_imgui_rs::render::DrawCmd::RawCallback { .. } => {
                            // Unsupported raw callbacks; skip.
                        }
                    }
                }

                let idx_len_u32 = u32::try_from(idx_buffer.len())
                    .map_err(|_| RendererError::DrawBufferTooLarge { buffer: "index" })?;
                global_idx_offset = global_idx_offset
                    .checked_add(idx_len_u32)
                    .ok_or_else(|| RendererError::DrawBufferOffsetOverflow { buffer: "index" })?;

                let vtx_len_i32 = i32::try_from(vtx_buffer.len())
                    .map_err(|_| RendererError::DrawBufferTooLarge { buffer: "vertex" })?;
                global_vtx_offset = global_vtx_offset
                    .checked_add(vtx_len_i32)
                    .ok_or_else(|| RendererError::DrawBufferOffsetOverflow { buffer: "vertex" })?;
            }
        }

        Ok(())
    }
}
