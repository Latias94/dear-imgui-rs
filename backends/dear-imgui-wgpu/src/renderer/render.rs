#[cfg(feature = "mv-log")]
use std::sync::{Mutex, OnceLock};

use super::{
    RendererRenderStateGuard, WgpuRenderer, draw::FramebufferExtent,
    map_renderer_render_state_error,
};
use crate::wgpu;
use crate::{GammaMode, RendererError, RendererResult, Uniforms};
use dear_imgui_rs::{
    Context, ContextBinding,
    render::{DrawData, ReconciledFrame, RenderedFrame},
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
    callback: impl FnOnce() -> RendererResult<R>,
) -> RendererResult<R> {
    binding
        .try_with_bound_context(callback)
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
    /// Renders one Context-borrowed Dear ImGui frame.
    ///
    /// Managed texture requests are reconciled before draw commands resolve
    /// renderer texture IDs. Consuming the frame prevents native draw data from
    /// escaping its owning Context borrow.
    pub fn render(
        &mut self,
        frame: RenderedFrame<'_>,
        render_pass: &mut RenderPass<'_>,
    ) -> RendererResult<()> {
        self.render_reconciled(frame, render_pass).map(drop)
    }

    /// Renders one frame and returns its texture-reconciliation proof.
    ///
    /// The proof does not claim that command submission or presentation completed.
    pub fn render_reconciled<'frame>(
        &mut self,
        mut frame: RenderedFrame<'frame>,
        render_pass: &mut RenderPass<'_>,
    ) -> RendererResult<ReconciledFrame<'frame>> {
        self.ensure_renderer_contract()?;
        self.ensure_frame_matches(&frame)?;
        let binding = self.bound_context()?;
        with_bound_context(&binding, || {
            Self::preflight_draw_callback_support(frame.draw_data())?;
            self.reconcile_frame_bound(&mut frame)?;
            let platform_io = platform_io_for_current_context()?;
            self.render_read_only_draw_data(frame.draw_data(), render_pass, platform_io)
        })?;
        frame.into_reconciled().map_err(Into::into)
    }

    /// Finalizes and renders the frame for this renderer's bound Context.
    pub fn render_context(
        &mut self,
        context: &mut Context,
        render_pass: &mut RenderPass<'_>,
    ) -> RendererResult<()> {
        self.ensure_context_matches(context)?;
        let frame = context.render();
        self.render(frame, render_pass)
    }

    /// Applies managed-texture requests without drawing or acquiring a surface.
    ///
    /// Callback capability is checked before texture reconciliation, so an
    /// unsupported callback-bearing frame is not consumed partially.
    pub fn reconcile_frame(&mut self, frame: &mut RenderedFrame<'_>) -> RendererResult<()> {
        self.ensure_renderer_contract()?;
        self.ensure_frame_matches(frame)?;
        let binding = self.bound_context()?;
        with_bound_context(&binding, || {
            Self::preflight_draw_callback_support(frame.draw_data())?;
            self.reconcile_frame_bound(frame)
        })
    }

    fn reconcile_frame_bound(&mut self, frame: &mut RenderedFrame<'_>) -> RendererResult<()> {
        if frame.is_texture_feedback_reconciled() {
            return Ok(());
        }
        let request_epoch = frame.epoch().map_or(0, |epoch| epoch.sequence());
        let backend_data = self.backend_data.as_mut().ok_or_else(|| {
            RendererError::InvalidRenderState("Renderer not initialized".to_owned())
        })?;
        let feedback = self.texture_manager.handle_texture_requests(
            frame.texture_requests(),
            request_epoch,
            &backend_data.device,
            &backend_data.queue,
            &mut backend_data.render_resources,
        )?;
        let progress = frame.reconcile_texture_feedback(feedback)?;
        self.texture_manager
            .prune_destroyed_managed_textures(progress.watermark());
        Ok(())
    }

    pub(super) fn render_read_only_draw_data(
        &mut self,
        draw_data: &DrawData,
        render_pass: &mut RenderPass<'_>,
        platform_io: *mut sys::ImGuiPlatformIO,
    ) -> RendererResult<()> {
        let Some(extent) = FramebufferExtent::from_draw_data(draw_data)? else {
            return Ok(());
        };
        self.render_draw_data_at_extent(draw_data, render_pass, extent, true, platform_io)
    }

    /// Renders one Context-borrowed frame at explicit framebuffer dimensions.
    pub fn render_with_fb_size(
        &mut self,
        frame: RenderedFrame<'_>,
        render_pass: &mut RenderPass<'_>,
        fb_width: u32,
        fb_height: u32,
    ) -> RendererResult<()> {
        self.render_with_fb_size_reconciled(frame, render_pass, fb_width, fb_height)
            .map(drop)
    }

    /// Renders one frame at explicit dimensions and returns its reconciliation proof.
    pub fn render_with_fb_size_reconciled<'frame>(
        &mut self,
        mut frame: RenderedFrame<'frame>,
        render_pass: &mut RenderPass<'_>,
        fb_width: u32,
        fb_height: u32,
    ) -> RendererResult<ReconciledFrame<'frame>> {
        self.ensure_renderer_contract()?;
        self.ensure_frame_matches(&frame)?;
        let binding = self.bound_context()?;
        with_bound_context(&binding, || {
            Self::preflight_draw_callback_support(frame.draw_data())?;
            self.reconcile_frame_bound(&mut frame)?;
            let platform_io = platform_io_for_current_context()?;
            self.render_read_only_draw_data_with_fb_size(
                frame.draw_data(),
                render_pass,
                fb_width,
                fb_height,
                true,
                platform_io,
            )
        })?;
        frame.into_reconciled().map_err(Into::into)
    }

    /// Finalizes and renders a frame for the bound Context at explicit dimensions.
    pub fn render_context_with_fb_size(
        &mut self,
        context: &mut Context,
        render_pass: &mut RenderPass<'_>,
        fb_width: u32,
        fb_height: u32,
    ) -> RendererResult<()> {
        self.ensure_context_matches(context)?;
        let frame = context.render();
        self.render_with_fb_size(frame, render_pass, fb_width, fb_height)
    }

    /// Internal explicit-size variant used by renderer-owned secondary viewports.
    ///
    /// When `advance_frame` is false, the current frame resources are reused.
    pub(super) fn render_read_only_draw_data_with_fb_size(
        &mut self,
        draw_data: &DrawData,
        render_pass: &mut RenderPass<'_>,
        fb_width: u32,
        fb_height: u32,
        advance_frame: bool,
        platform_io: *mut sys::ImGuiPlatformIO,
    ) -> RendererResult<()> {
        self.log_framebuffer_mismatch(draw_data, fb_width, fb_height, advance_frame);
        let Some(extent) = FramebufferExtent::explicit(fb_width, fb_height) else {
            return Ok(());
        };
        self.render_draw_data_at_extent(draw_data, render_pass, extent, advance_frame, platform_io)
    }

    fn render_draw_data_at_extent(
        &mut self,
        draw_data: &DrawData,
        render_pass: &mut RenderPass<'_>,
        extent: FramebufferExtent,
        advance_frame: bool,
        platform_io: *mut sys::ImGuiPlatformIO,
    ) -> RendererResult<()> {
        self.ensure_renderer_contract()?;
        if !draw_data.valid() {
            return Ok(());
        }
        Self::preflight_draw_callback_support(draw_data)?;
        unsafe {
            RendererRenderStateGuard::<crate::WgpuRenderStateStorage>::preflight(platform_io)
        }
        .map_err(map_renderer_render_state_error)?;

        let backend_data = self.backend_data.as_mut().ok_or_else(|| {
            RendererError::InvalidRenderState("Renderer not initialized".to_owned())
        })?;
        let prepared = Self::prepare_draw_data(
            &self.texture_manager,
            &self.default_texture,
            draw_data,
            extent,
            backend_data,
        )?;
        if prepared.is_empty() {
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
        let state = Self::prepare_render_state_static(
            draw_data,
            backend_data,
            gamma,
            prepared.has_elements(),
        )?;
        let device = backend_data.device.clone();
        Self::execute_prepared_draw_data(
            prepared,
            &state,
            extent,
            render_pass,
            platform_io,
            &device,
        )
    }

    #[cfg(feature = "mv-log")]
    fn log_framebuffer_mismatch(
        &self,
        draw_data: &DrawData,
        fb_width: u32,
        fb_height: u32,
        advance_frame: bool,
    ) {
        static LAST_MISMATCH: OnceLock<Mutex<Option<(u32, u32, u32, u32, bool)>>> = OnceLock::new();
        let expected_width = (draw_data.display_size()[0] * draw_data.framebuffer_scale()[0])
            .round()
            .max(0.0) as u32;
        let expected_height = (draw_data.display_size()[1] * draw_data.framebuffer_scale()[1])
            .round()
            .max(0.0) as u32;
        if expected_width == fb_width && expected_height == fb_height {
            return;
        }
        let key = (
            expected_width,
            expected_height,
            fb_width,
            fb_height,
            advance_frame,
        );
        let mut previous = LAST_MISMATCH
            .get_or_init(|| Mutex::new(None))
            .lock()
            .unwrap();
        if *previous != Some(key) {
            mvlog!(
                "[wgpu-mv] fb mismatch expected=({}, {}) override=({}, {}) disp=({:.1},{:.1}) fb_scale=({:.2},{:.2}) main={}",
                expected_width,
                expected_height,
                fb_width,
                fb_height,
                draw_data.display_size()[0],
                draw_data.display_size()[1],
                draw_data.framebuffer_scale()[0],
                draw_data.framebuffer_scale()[1],
                advance_frame
            );
            *previous = Some(key);
        }
    }

    #[cfg(not(feature = "mv-log"))]
    fn log_framebuffer_mismatch(
        &self,
        _draw_data: &DrawData,
        _fb_width: u32,
        _fb_height: u32,
        _advance_frame: bool,
    ) {
    }
}
