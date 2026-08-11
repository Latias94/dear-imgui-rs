//! Frame lifecycle APIs.
//!
//! Use [`Context::frame_with_result`] when a callback should also close and render the frame.
//! The callback-only `Context::frame_with` API is intentionally unavailable because it leaves
//! frame completion implicit:
//!
//! ```compile_fail
//! # use dear_imgui_rs::Context;
//! # let mut context = Context::create();
//! let _ = context.frame_with(|ui| ui.text("frame"));
//! ```

use crate::sys;

use super::Context;
use super::binding::{CTX_MUTEX, with_bound_context};

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

/// Comparable identity and native progress marker for a Context frame boundary.
///
/// Integrations can retain a stamp before invoking application code and require the same stamp
/// afterward. Equality covers the Context generation, native frame sequence, and current lifecycle
/// state, so an `Idle -> Idle` frame round trip is still observable.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct FrameLifecycleStamp {
    context: super::ContextId,
    native_frame: i32,
    state: FrameLifecycleState,
}

impl FrameLifecycleStamp {
    /// Return the Context generation captured by this stamp.
    #[must_use]
    pub const fn context_id(self) -> super::ContextId {
        self.context
    }

    /// Return the frame lifecycle state captured by this stamp.
    #[must_use]
    pub const fn state(self) -> FrameLifecycleState {
        self.state
    }
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
#[must_use = "dropping FrameToken ends the frame without rendering; call render() or render_snapshot() to produce draw data"]
#[doc(alias = "EndFrame")]
pub struct FrameToken<'ctx> {
    ctx: &'ctx mut Context,
}

/// Result returned by [`Context::frame_with_result`].
#[must_use = "use the closure result and reconcile the pending frame before drawing"]
pub struct FrameResult<'ctx, T> {
    /// Value returned by the UI-building closure.
    pub value: T,
    /// Context-borrowed pending frame produced after the closure returned.
    pub pending_frame: crate::render::PendingFrame<'ctx>,
}

impl<'ctx, T> FrameResult<'ctx, T> {
    /// Split the closure result from the pending renderer capability.
    pub fn into_parts(self) -> (T, crate::render::PendingFrame<'ctx>) {
        (self.value, self.pending_frame)
    }
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
        self.frame_lifecycle_stamp().state()
    }

    /// Capture this Context's identity and native frame progress as one comparable value.
    ///
    /// This is intended for engine callback boundaries that must reject both an open frame and a
    /// frame that was opened and closed entirely inside the callback.
    pub fn frame_lifecycle_stamp(&self) -> FrameLifecycleStamp {
        let _guard = CTX_MUTEX.lock();
        self.assert_current_context("Context::frame_lifecycle_stamp()");
        self.frame_lifecycle_stamp_unlocked()
    }

    /// End an open frame without producing render data.
    ///
    /// Returns `true` when an open native frame was closed and `false` when the Context was
    /// already idle or rendered. The operation is idempotent so engine teardown can revoke UI
    /// access before detaching platform and renderer state.
    #[doc(alias = "EndFrame")]
    pub fn end_frame(&mut self) -> bool {
        let _guard = CTX_MUTEX.lock();
        self.assert_current_context("Context::end_frame()");
        self.end_frame_for_teardown_unlocked()
    }

    pub(super) fn end_frame_for_teardown_unlocked(&mut self) -> bool {
        if self.raw.is_null() || !unsafe { (*self.raw).WithinFrameScope } {
            return false;
        }
        unsafe {
            with_bound_context(self.raw, || {
                let _ = crate::list_clipper::forget_context_clippers(self.raw);
                sys::igEndFrame();
            });
        }
        self.texture_registry
            .borrow_mut()
            .observe_native_texture_list_refresh();
        true
    }

    /// Begin a Dear ImGui frame and return an explicit frame token.
    ///
    /// Engine integrations should prefer this when the frame is owned by a schedule rather than a
    /// single function. Draw UI through [`FrameToken::ui`] and then consume the token with
    /// [`FrameToken::render`] or [`FrameToken::render_snapshot`].
    pub fn begin_frame(&mut self) -> FrameToken<'_> {
        self.try_begin_frame()
            .unwrap_or_else(|error| panic!("Context::begin_frame() rejected completion: {error}"))
    }

    /// Begin a frame while returning detached-completion failures to the caller.
    pub fn try_begin_frame(
        &mut self,
    ) -> Result<FrameToken<'_>, crate::render::RendererConsumerError> {
        let _ = self.try_frame()?;
        Ok(FrameToken { ctx: self })
    }

    /// Creates a new frame and returns a Ui object for building the interface.
    ///
    /// Note: you must update `io.DisplaySize` (and usually `io.DeltaTime`) before calling this,
    /// unless you are using a platform backend that does it for you (e.g. `dear-imgui-winit`).
    #[doc(alias = "NewFrame")]
    pub fn frame(&mut self) -> &mut crate::ui::Ui {
        self.try_frame()
            .unwrap_or_else(|error| panic!("Context::frame() rejected completion: {error}"))
    }

    /// Create a frame while returning detached-completion failures to the caller.
    ///
    /// Native frame-order, display-size, docking, and font-atlas programmer errors retain their
    /// documented panic behavior. This method makes the asynchronous renderer completion boundary
    /// fallible so event-loop and engine integrations can recover without a later panic.
    pub fn try_frame(
        &mut self,
    ) -> Result<&mut crate::ui::Ui, crate::render::RendererConsumerError> {
        let _guard = CTX_MUTEX.lock();
        self.assert_current_context("Context::try_frame()");
        self.assert_can_begin_frame_unlocked("Context::try_frame()");
        self.poll_snapshot_completions_unlocked()?;
        self.collect_retired_textures();

        unsafe {
            // Dear ImGui initializes DisplaySize to (-1, -1). Calling NewFrame() without a
            // platform backend (or without setting DisplaySize manually) will trip an internal
            // assertion and abort the process. Fail fast with a Rust panic to make the setup
            // requirement obvious.
            let io = sys::igGetIO_Nil();
            if !io.is_null() && ((*io).DisplaySize.x < 0.0 || (*io).DisplaySize.y < 0.0) {
                panic!(
                    "Context::try_frame() called with invalid io.DisplaySize ({}, {}). \
Set io.DisplaySize (and typically io.DeltaTime) before starting a frame. \
If you are using a windowing/event-loop library, prefer a platform backend such as \
dear-imgui-winit::WinitPlatform::prepare_frame().",
                    (*io).DisplaySize.x,
                    (*io).DisplaySize.y
                );
            }
            self.assert_docking_config_stable_unlocked("Context::try_frame()");
            #[cfg(feature = "multi-viewport")]
            self.prepare_multi_viewport_new_frame_contract_unlocked("Context::try_frame()");
            crate::fonts::assert_no_font_atlas_texture_borrows((*io).Fonts, "Context::try_frame()");
            let renderer_has_textures =
                ((*io).BackendFlags & sys::ImGuiBackendFlags_RendererHasTextures as i32) != 0;
            crate::fonts::assert_font_atlas_renderer_mode(
                (*io).Fonts,
                renderer_has_textures,
                "Context::try_frame()",
            );
            if let Some(shared_font_atlas) = &self.shared_font_atlas {
                shared_font_atlas.prepare_frame(renderer_has_textures);
            }
            sys::igNewFrame();
        }
        Ok(&mut self.ui)
    }

    fn assert_docking_config_stable_unlocked(&self, caller: &str) {
        let frame_count = unsafe { (*self.raw).FrameCount };
        if frame_count == 0 {
            return;
        }

        let io = self.io_ptr(caller);
        let requested = unsafe { (*io).ConfigFlags } & sys::ImGuiConfigFlags_DockingEnable as i32;
        let active = unsafe { (*self.raw).ConfigFlagsCurrFrame }
            & sys::ImGuiConfigFlags_DockingEnable as i32;
        assert_eq!(
            requested, active,
            "{caller} cannot change ConfigFlags::DOCKING_ENABLE after the first frame; Dear ImGui clears live dock nodes when docking is disabled and cannot restore them safely when it is re-enabled"
        );
    }

    /// Begin a frame, run a UI-building closure, render the frame, and return both values.
    ///
    /// This is a convenience for callers that want the old callback style but also want the draw
    /// data produced by closing the frame. Use [`Context::begin_frame`] when the UI is built across
    /// several engine systems.
    pub fn frame_with_result<F, R>(
        &mut self,
        consumer: &crate::render::SynchronousRendererConsumer,
        f: F,
    ) -> FrameResult<'_, R>
    where
        F: FnOnce(&crate::ui::Ui) -> R,
    {
        let frame = self.begin_frame();
        let value = f(frame.ui());
        let pending_frame = frame.render(consumer);
        FrameResult {
            value,
            pending_frame,
        }
    }

    /// Render a managed-texture frame and return its non-drawable pending capability.
    #[doc(alias = "Render", alias = "GetDrawData")]
    pub fn render(
        &mut self,
        consumer: &crate::render::SynchronousRendererConsumer,
    ) -> crate::render::PendingFrame<'_> {
        self.try_render(consumer)
            .unwrap_or_else(|error| panic!("Context::render() failed: {error}"))
    }

    /// Render a managed-texture frame while returning capture or consumer failures.
    pub fn try_render(
        &mut self,
        consumer: &crate::render::SynchronousRendererConsumer,
    ) -> Result<crate::render::PendingFrame<'_>, crate::render::SnapshotError> {
        self.require_managed_renderer("Context::try_render()")?;
        let draw_data = self.render_raw();
        let (epoch, requests) =
            self.begin_synchronous_render(consumer, draw_data.as_ptr().cast_const())?;
        Ok(crate::render::PendingFrame::new(
            self, draw_data, epoch, requests,
        ))
    }

    /// Render a frame for a legacy renderer that does not implement managed texture requests.
    ///
    /// # Panics
    ///
    /// Panics when the active renderer advertises `RENDERER_HAS_TEXTURES`; managed renderers must
    /// use [`Self::render`] with their synchronous consumer and reconcile every request.
    pub fn render_legacy(&mut self) -> crate::render::ReconciledFrame<'_> {
        let renderer_has_textures = self
            .io()
            .backend_flags()
            .contains(crate::BackendFlags::RENDERER_HAS_TEXTURES);
        assert!(
            !renderer_has_textures,
            "Context::render_legacy() cannot bypass a managed renderer consumer"
        );
        let draw_data = self.render_raw();
        crate::render::ReconciledFrame::new_legacy(self, draw_data)
    }

    fn require_managed_renderer(
        &self,
        caller: &'static str,
    ) -> Result<(), crate::render::RendererConsumerError> {
        if self
            .io()
            .backend_flags()
            .contains(crate::BackendFlags::RENDERER_HAS_TEXTURES)
        {
            Ok(())
        } else {
            Err(crate::render::RendererConsumerError::RendererTexturesUnavailable { caller })
        }
    }

    fn render_raw(&mut self) -> std::ptr::NonNull<crate::render::DrawData> {
        let _guard = CTX_MUTEX.lock();
        self.assert_current_context("Context::render()");
        self.assert_can_render_unlocked("Context::render()");

        let draw_data = unsafe {
            let abandoned_clippers = crate::list_clipper::forget_context_clippers(self.raw);
            if abandoned_clippers != 0 {
                self.end_frame_for_teardown_unlocked();
                panic!(
                    "Context::render() rejected a frame with {abandoned_clippers} list clipper token(s) still active or forgotten"
                );
            }
            sys::igRender();
            let dd = sys::igGetDrawData();
            if dd.is_null() {
                panic!("Context::render() returned null draw data");
            }
            std::ptr::NonNull::new_unchecked(dd as *mut crate::render::DrawData)
        };
        self.texture_registry
            .borrow_mut()
            .observe_native_texture_list_refresh();
        draw_data
    }

    /// Render the current frame and build a thread-safe main-viewport snapshot.
    ///
    /// This Context-level entry point supports engine schedules that open and close the native
    /// frame in separate systems and therefore cannot retain a [`FrameToken`]. The snapshot is
    /// still bound to the supplied renderer consumer, Context, generation, and ordered epoch.
    pub fn render_snapshot(
        &mut self,
        consumer: &crate::render::snapshot::DetachedRendererConsumer,
    ) -> Result<crate::render::snapshot::FrameSnapshot, crate::render::snapshot::SnapshotError>
    {
        let draw_data = self.render_raw();
        self.capture_main_snapshot(consumer, draw_data.as_ptr())
    }

    /// Render the current frame and build a thread-safe snapshot for all platform viewports.
    ///
    /// With the `multi-viewport` feature enabled, Dear ImGui stores draw data on each viewport.
    /// `Context::render()` and `igGetDrawData()` only expose the main viewport draw data, so
    /// engine backends that render secondary OS windows should use this method after opening a
    /// frame. The returned snapshot still keeps [`FrameSnapshot::draw`] as the main viewport for
    /// compatibility and adds per-viewport draw data in
    /// [`FrameSnapshot::viewports`](crate::render::snapshot::FrameSnapshot::viewports).
    #[cfg(feature = "multi-viewport")]
    pub fn render_platform_viewport_snapshot(
        &mut self,
        consumer: &crate::render::snapshot::DetachedRendererConsumer,
    ) -> Result<crate::render::snapshot::FrameSnapshot, crate::render::snapshot::SnapshotError>
    {
        let _ = self.render_raw();
        self.platform_viewport_snapshot(consumer)
    }

    /// Build a thread-safe snapshot from the current platform viewport draw data.
    ///
    /// This does not call `Render()`. Call it only after a frame has already been rendered and
    /// before the next frame starts. Engine integrations can use this when they need to run
    /// platform-window maintenance after `Render()` but before cloning draw data for another
    /// render schedule.
    #[cfg(feature = "multi-viewport")]
    pub fn platform_viewport_snapshot(
        &mut self,
        consumer: &crate::render::snapshot::DetachedRendererConsumer,
    ) -> Result<crate::render::snapshot::FrameSnapshot, crate::render::snapshot::SnapshotError>
    {
        let _guard = CTX_MUTEX.lock();
        self.assert_current_context("Context::platform_viewport_snapshot()");
        if self.frame_lifecycle_state_unlocked() != FrameLifecycleState::Rendered {
            panic!(
                "Context::platform_viewport_snapshot() called before rendering the current frame"
            );
        }
        self.capture_platform_snapshot(consumer)
    }

    pub(super) fn frame_lifecycle_state_unlocked(&self) -> FrameLifecycleState {
        self.frame_lifecycle_stamp_unlocked().state()
    }

    fn frame_lifecycle_stamp_unlocked(&self) -> FrameLifecycleStamp {
        unsafe {
            let raw = &*self.raw;
            let state = if raw.WithinFrameScope {
                FrameLifecycleState::InFrame
            } else if raw.FrameCountRendered == raw.FrameCount && raw.FrameCount > 0 {
                FrameLifecycleState::Rendered
            } else {
                FrameLifecycleState::Idle
            };
            FrameLifecycleStamp {
                context: self.id(),
                native_frame: raw.FrameCount,
                state,
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

    /// Render this managed-texture frame and return its pending capability.
    pub fn render(
        self,
        consumer: &crate::render::SynchronousRendererConsumer,
    ) -> crate::render::PendingFrame<'ctx> {
        self.try_render(consumer)
            .unwrap_or_else(|error| panic!("FrameToken::render() failed: {error}"))
    }

    /// Render this managed-texture frame while returning capture or consumer failures.
    pub fn try_render(
        self,
        consumer: &crate::render::SynchronousRendererConsumer,
    ) -> Result<crate::render::PendingFrame<'ctx>, crate::render::SnapshotError> {
        let ctx = self.ctx as *mut Context;
        match unsafe { (&mut *ctx).try_render(consumer) } {
            Ok(frame) => {
                std::mem::forget(self);
                Ok(frame)
            }
            Err(error) => Err(error),
        }
    }

    /// Render this frame through the explicit legacy renderer path.
    pub fn render_legacy(self) -> crate::render::ReconciledFrame<'ctx> {
        let ctx = self.ctx as *mut Context;
        let frame = unsafe { (&mut *ctx).render_legacy() };
        std::mem::forget(self);
        frame
    }

    /// Render this frame and build a thread-safe snapshot from the resulting draw data.
    ///
    /// This is the preferred handoff shape for render-world integrations such as Bevy, where raw
    /// ImGui pointers must not cross the engine extraction boundary.
    pub fn render_snapshot(
        self,
        consumer: &crate::render::snapshot::DetachedRendererConsumer,
    ) -> Result<crate::render::snapshot::FrameSnapshot, crate::render::snapshot::SnapshotError>
    {
        let ctx = self.ctx as *mut Context;
        let draw_data = unsafe { (&mut *ctx).render_raw().as_ptr() };
        std::mem::forget(self);
        unsafe { (&mut *ctx).capture_main_snapshot(consumer, draw_data) }
    }

    /// Render this frame and build a thread-safe snapshot for all platform viewports.
    #[cfg(feature = "multi-viewport")]
    pub fn render_platform_viewport_snapshot(
        self,
        consumer: &crate::render::snapshot::DetachedRendererConsumer,
    ) -> Result<crate::render::snapshot::FrameSnapshot, crate::render::snapshot::SnapshotError>
    {
        let ctx = self.ctx as *mut Context;
        let snapshot = unsafe { (&mut *ctx).render_platform_viewport_snapshot(consumer) };
        std::mem::forget(self);
        snapshot
    }
}

impl Drop for FrameToken<'_> {
    fn drop(&mut self) {
        let _guard = CTX_MUTEX.lock();
        self.ctx.end_frame_for_teardown_unlocked();
    }
}
