use std::ops::Deref;
use std::ptr::NonNull;

use crate::Context;
use crate::render::{DrawData, DrawRequirements};

use super::snapshot::{
    RendererConsumerError, SnapshotCompletionProgress, SnapshotEpoch, TextureFeedback,
    TextureRequest,
};

/// Context-borrowed synchronous frame awaiting managed-texture reconciliation.
///
/// This capability intentionally exposes texture requests but not draw data. Reconciliation
/// consumes it and returns the only drawable capability, [`ReconciledFrame`]. Dropping it reports
/// an abandoned epoch without acknowledging destroy requests.
#[must_use = "reconcile the frame before drawing; managed requests are abandoned on drop"]
pub struct PendingFrame<'ctx> {
    context: Option<&'ctx mut Context>,
    draw_data: NonNull<DrawData>,
    draw_requirements: DrawRequirements,
    epoch: SnapshotEpoch,
    texture_requests: Vec<TextureRequest>,
}

/// Drawable proof that one synchronous frame completed managed-texture reconciliation.
///
/// The capability owns the live Context borrow and draw-data pointer. It proves texture
/// reconciliation, not GPU submission or operating-system presentation.
#[must_use = "draw or explicitly drop the reconciled frame before presenting"]
pub struct ReconciledFrame<'ctx> {
    context: &'ctx mut Context,
    draw_data: NonNull<DrawData>,
    epoch: Option<SnapshotEpoch>,
    completion: SnapshotCompletionProgress,
}

impl std::fmt::Debug for PendingFrame<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PendingFrame")
            .field("context", &self.context_id())
            .field("epoch", &self.epoch)
            .field("texture_requests", &self.texture_requests.len())
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for ReconciledFrame<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ReconciledFrame")
            .field("context", &self.context.id())
            .field("epoch", &self.epoch)
            .field("completion", &self.completion)
            .finish_non_exhaustive()
    }
}

impl<'ctx> PendingFrame<'ctx> {
    pub(crate) fn new(
        context: &'ctx mut Context,
        draw_data: NonNull<DrawData>,
        epoch: SnapshotEpoch,
        texture_requests: Vec<TextureRequest>,
    ) -> Self {
        Self {
            context: Some(context),
            draw_data,
            draw_requirements: unsafe { draw_data.as_ref() }.requirements(),
            epoch,
            texture_requests,
        }
    }

    /// Context that owns this frame.
    #[must_use]
    pub fn context_id(&self) -> crate::ContextId {
        self.context
            .as_deref()
            .expect("pending frame retains its Context until reconciliation")
            .id()
    }

    /// Ordered managed-texture epoch.
    #[must_use]
    pub const fn epoch(&self) -> SnapshotEpoch {
        self.epoch
    }

    /// Managed texture requests that must each receive one explicit outcome.
    #[must_use]
    pub fn texture_requests(&self) -> &[TextureRequest] {
        &self.texture_requests
    }

    /// Pointer-free renderer capabilities required by this frame's draw commands.
    ///
    /// This summary is available before reconciliation so a renderer can reject unsupported work
    /// without partially applying managed-texture updates.
    #[must_use]
    pub const fn draw_requirements(&self) -> DrawRequirements {
        self.draw_requirements
    }

    /// Apply request-bound feedback and return the only drawable frame capability.
    ///
    /// Every request must receive exactly one outcome. A `retry` outcome completes this epoch
    /// without changing the binding, and the request remains eligible for a later frame.
    pub fn reconcile_texture_feedback(
        mut self,
        feedback: impl IntoIterator<Item = TextureFeedback>,
    ) -> Result<ReconciledFrame<'ctx>, RendererConsumerError> {
        let feedback = feedback.into_iter().collect::<Vec<_>>();
        let epoch = self.epoch;
        let progress = self
            .with_owner_context(|context| context.complete_synchronous_render(epoch, feedback))?;

        let context = self
            .context
            .take()
            .expect("completed pending frame still owns its Context");
        Ok(ReconciledFrame {
            context,
            draw_data: self.draw_data,
            epoch: Some(self.epoch),
            completion: progress,
        })
    }

    fn with_owner_context<R>(&mut self, f: impl FnOnce(&mut Context) -> R) -> R {
        let context = self
            .context
            .as_deref_mut()
            .expect("pending frame retains its Context until reconciliation");
        let binding = context.binding();
        binding.with_bound_context(|| f(context))
    }
}

impl<'ctx> ReconciledFrame<'ctx> {
    pub(crate) fn new_legacy(context: &'ctx mut Context, draw_data: NonNull<DrawData>) -> Self {
        Self {
            context,
            draw_data,
            epoch: None,
            completion: SnapshotCompletionProgress::default(),
        }
    }

    /// Context that owns this frame.
    #[must_use]
    pub fn context_id(&self) -> crate::ContextId {
        self.context.id()
    }

    /// Ordered managed-texture epoch, or `None` for an explicit legacy render.
    #[must_use]
    pub const fn epoch(&self) -> Option<SnapshotEpoch> {
        self.epoch
    }

    /// Completion progress produced while reconciling this frame.
    #[must_use]
    pub const fn completion_progress(&self) -> SnapshotCompletionProgress {
        self.completion
    }

    /// Read the native draw data while this Context borrow is active.
    #[must_use]
    pub fn draw_data(&self) -> &DrawData {
        unsafe { self.draw_data.as_ref() }
    }

    /// Update and render secondary platform windows while this frame owns the Context.
    ///
    /// This combined operation does not render or present the main viewport. WSI backends can call
    /// it before acquiring the main surface, while OpenGL integrations can call it after the main
    /// draw and before the main swap.
    ///
    /// # Panics
    ///
    /// Panics when multi-viewport is disabled, the operation is repeated for the same frame, or
    /// the installed platform/renderer callback contract is incomplete.
    #[cfg(feature = "multi-viewport")]
    pub fn update_and_render_platform_windows_default(&mut self) {
        self.context.update_platform_windows();
        self.context.render_platform_windows_default();
    }
}

impl Deref for ReconciledFrame<'_> {
    type Target = DrawData;

    fn deref(&self) -> &Self::Target {
        self.draw_data()
    }
}

impl Drop for PendingFrame<'_> {
    fn drop(&mut self) {
        if self.context.is_none() {
            return;
        }
        let epoch = self.epoch;
        self.with_owner_context(|context| {
            context.abandon_synchronous_render(epoch);
            context.collect_retired_textures();
        });
    }
}

impl Drop for ReconciledFrame<'_> {
    fn drop(&mut self) {
        let binding = self.context.binding();
        binding.with_bound_context(|| self.context.collect_retired_textures());
    }
}
