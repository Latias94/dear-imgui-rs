use crate::sys;

use super::Context;
use super::binding::CTX_MUTEX;

/// Runtime state for a Dear ImGui frame owned by an external engine schedule.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum FrameLifecycleState {
    /// No Dear ImGui frame is currently open for this context.
    Idle,
    /// A frame was opened and can accept UI commands.
    InFrame,
    /// The last opened frame has been rendered and draw data is available until the next frame.
    Rendered,
}

/// Options used by [`Context::prepare_frame`].
#[derive(Copy, Clone, Debug)]
pub struct FramePrepareOptions {
    /// Main display size in pixels.
    pub display_size: [f32; 2],
    /// Time elapsed since the previous frame, in seconds.
    pub delta_time: f32,
    /// Optional framebuffer scale for HiDPI render targets.
    pub framebuffer_scale: Option<[f32; 2]>,
    /// Backend capability flags to OR into the context before opening the frame.
    pub backend_flags: crate::BackendFlags,
}

impl FramePrepareOptions {
    /// Create frame preparation options with display size and delta time.
    pub fn new(display_size: [f32; 2], delta_time: f32) -> Self {
        Self {
            display_size,
            delta_time,
            framebuffer_scale: None,
            backend_flags: crate::BackendFlags::empty(),
        }
    }

    /// Set the framebuffer scale used by the frame.
    #[must_use]
    pub fn framebuffer_scale(mut self, scale: [f32; 2]) -> Self {
        self.framebuffer_scale = Some(scale);
        self
    }

    /// OR backend capability flags into the context before opening the frame.
    #[must_use]
    pub fn backend_flags(mut self, flags: crate::BackendFlags) -> Self {
        self.backend_flags |= flags;
        self
    }

    /// Convenience for modern renderers that support ImGui 1.92 texture requests.
    #[must_use]
    pub fn renderer_has_textures(self) -> Self {
        self.backend_flags(crate::BackendFlags::RENDERER_HAS_TEXTURES)
    }
}

/// A frame opened by [`Context::begin_frame`].
///
/// This token is intended for engine integrations that need to make the Dear ImGui frame boundary
/// explicit: one system opens the frame, several user systems draw through [`Self::ui`], and one
/// system consumes the token to render or snapshot the frame. The existing [`Context::frame`] and
/// [`Context::render`] calls remain available for traditional immediate-mode loops.
pub struct FrameToken<'ctx> {
    ctx: &'ctx mut Context,
}

/// Result returned by [`Context::frame_with_result`].
pub struct FrameResult<'ctx, T> {
    /// Value returned by the UI-building closure.
    pub value: T,
    /// Draw data produced by rendering the frame after the closure returned.
    pub draw_data: &'ctx mut crate::render::DrawData,
}

impl Context {
    /// Prepare IO values commonly needed before starting a frame.
    ///
    /// This does not call `NewFrame()`. Engine backends can call it from their input/window update
    /// stage and then open the actual Dear ImGui frame later in the schedule with
    /// [`Context::begin_frame`].
    pub fn prepare_frame(&mut self, options: FramePrepareOptions) {
        let io = self.io_mut();
        io.set_display_size(options.display_size);
        io.set_delta_time(options.delta_time);
        if let Some(scale) = options.framebuffer_scale {
            io.set_display_framebuffer_scale(scale);
        }
        if !options.backend_flags.is_empty() {
            io.set_backend_flags(io.backend_flags() | options.backend_flags);
        }
    }

    /// Return the current frame lifecycle state for this context.
    pub fn frame_lifecycle_state(&self) -> FrameLifecycleState {
        let _guard = CTX_MUTEX.lock();
        self.assert_current_context("Context::frame_lifecycle_state()");
        self.frame_lifecycle_state_unlocked()
    }

    /// Begin a Dear ImGui frame and return an explicit frame token.
    ///
    /// Engine integrations should prefer this when the frame is owned by a schedule rather than a
    /// single function. Draw UI through [`FrameToken::ui`] and then consume the token with
    /// [`FrameToken::render`] or [`FrameToken::render_snapshot`].
    pub fn begin_frame(&mut self) -> FrameToken<'_> {
        let _ = self.frame();
        FrameToken { ctx: self }
    }

    /// Creates a new frame and returns a Ui object for building the interface.
    ///
    /// Note: you must update `io.DisplaySize` (and usually `io.DeltaTime`) before calling this,
    /// unless you are using a platform backend that does it for you (e.g. `dear-imgui-winit`).
    pub fn frame(&mut self) -> &mut crate::ui::Ui {
        let _guard = CTX_MUTEX.lock();
        self.assert_current_context("Context::frame()");
        self.assert_can_begin_frame_unlocked("Context::frame()");

        unsafe {
            // Dear ImGui initializes DisplaySize to (-1, -1). Calling NewFrame() without a
            // platform backend (or without setting DisplaySize manually) will trip an internal
            // assertion and abort the process. Fail fast with a Rust panic to make the setup
            // requirement obvious.
            let io = sys::igGetIO_Nil();
            if !io.is_null() && ((*io).DisplaySize.x < 0.0 || (*io).DisplaySize.y < 0.0) {
                panic!(
                    "Context::frame() called with invalid io.DisplaySize ({}, {}). \
Set io.DisplaySize (and typically io.DeltaTime) before starting a frame. \
If you are using a windowing/event-loop library, prefer a platform backend such as \
dear-imgui-winit::WinitPlatform::prepare_frame().",
                    (*io).DisplaySize.x,
                    (*io).DisplaySize.y
                );
            }
            sys::igNewFrame();
        }
        &mut self.ui
    }

    /// Create a new frame with a callback
    pub fn frame_with<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce(&crate::ui::Ui) -> R,
    {
        let ui = self.frame();
        f(ui)
    }

    /// Begin a frame, run a UI-building closure, render the frame, and return both values.
    ///
    /// This is a convenience for callers that want the old callback style but also want the draw
    /// data produced by closing the frame. Use [`Context::begin_frame`] when the UI is built across
    /// several engine systems.
    pub fn frame_with_result<F, R>(&mut self, f: F) -> FrameResult<'_, R>
    where
        F: FnOnce(&crate::ui::Ui) -> R,
    {
        let frame = self.begin_frame();
        let value = f(frame.ui());
        let draw_data = frame.render();
        FrameResult { value, draw_data }
    }

    /// Renders the frame and returns a mutable reference to the resulting draw data
    ///
    /// This finalizes the Dear ImGui frame and prepares all draw data for rendering.
    /// The returned draw data contains all the information needed to render the frame.
    ///
    /// Renderer backends receive mutable draw data because ImGui 1.92 texture requests require
    /// backends to write `TexID`/`Status` feedback into `ImTextureData`.
    pub fn render(&mut self) -> &mut crate::render::DrawData {
        let _guard = CTX_MUTEX.lock();
        self.assert_current_context("Context::render()");
        self.assert_can_render_unlocked("Context::render()");

        unsafe {
            sys::igRender();
            let dd = sys::igGetDrawData();
            if dd.is_null() {
                panic!("Context::render() returned null draw data");
            }
            &mut *(dd as *mut crate::render::DrawData)
        }
    }

    /// Gets the draw data for the current frame
    ///
    /// This returns the draw data without calling render. Only valid after
    /// `render()` has been called and before the next `new_frame()`.
    pub fn draw_data(&self) -> Option<&crate::render::DrawData> {
        let _guard = CTX_MUTEX.lock();
        self.assert_current_context("Context::draw_data()");

        unsafe {
            let draw_data = sys::igGetDrawData();
            if draw_data.is_null() {
                None
            } else {
                let data = &*(draw_data as *const crate::render::DrawData);
                if data.valid() { Some(data) } else { None }
            }
        }
    }

    /// Gets mutable draw data for the current frame.
    ///
    /// This returns the draw data without calling render. Only valid after `render()` has been
    /// called and before the next `new_frame()`. Use this when a renderer needs to process texture
    /// updates after draw data has already been produced.
    pub fn draw_data_mut(&mut self) -> Option<&mut crate::render::DrawData> {
        let _guard = CTX_MUTEX.lock();
        self.assert_current_context("Context::draw_data_mut()");

        unsafe {
            let draw_data = sys::igGetDrawData();
            if draw_data.is_null() {
                None
            } else {
                let data = &mut *(draw_data as *mut crate::render::DrawData);
                if data.valid() { Some(data) } else { None }
            }
        }
    }

    fn frame_lifecycle_state_unlocked(&self) -> FrameLifecycleState {
        unsafe {
            let raw = &*self.raw;
            if raw.WithinFrameScope {
                FrameLifecycleState::InFrame
            } else if raw.FrameCountRendered == raw.FrameCount && raw.FrameCount > 0 {
                FrameLifecycleState::Rendered
            } else {
                FrameLifecycleState::Idle
            }
        }
    }

    fn assert_can_begin_frame_unlocked(&self, caller: &str) {
        if self.frame_lifecycle_state_unlocked() == FrameLifecycleState::InFrame {
            panic!("{caller} called while another Dear ImGui frame is already open");
        }
    }

    fn assert_can_render_unlocked(&self, caller: &str) {
        if self.frame_lifecycle_state_unlocked() != FrameLifecycleState::InFrame {
            panic!("{caller} called without an open Dear ImGui frame");
        }
    }
}

impl<'ctx> FrameToken<'ctx> {
    /// Borrow the UI for this frame.
    ///
    /// This can be called repeatedly by an engine-owned frame runner to let multiple systems draw
    /// into the same frame, as long as those systems are scheduled sequentially.
    pub fn ui(&self) -> &crate::ui::Ui {
        &self.ctx.ui
    }

    /// Return the lifecycle state while this token owns the open frame.
    pub fn lifecycle_state(&self) -> FrameLifecycleState {
        self.ctx.frame_lifecycle_state()
    }

    /// Render this frame and return the resulting draw data.
    pub fn render(self) -> &'ctx mut crate::render::DrawData {
        self.ctx.render()
    }

    /// Render this frame and build a thread-safe snapshot from the resulting draw data.
    ///
    /// This is the preferred handoff shape for render-world integrations such as Bevy, where raw
    /// ImGui pointers must not cross the engine extraction boundary.
    pub fn render_snapshot(
        self,
        options: crate::render::snapshot::SnapshotOptions,
    ) -> Result<crate::render::snapshot::FrameSnapshot, crate::render::snapshot::SnapshotError>
    {
        let draw_data = self.ctx.render();
        crate::render::snapshot::FrameSnapshot::from_draw_data(draw_data, options)
    }
}
