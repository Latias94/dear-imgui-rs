use std::marker::PhantomData;
use std::ops::Deref;
use std::ptr::NonNull;

use crate::Context;
use crate::render::DrawData;

use super::snapshot::{
    RendererConsumerError, SnapshotCompletionProgress, SnapshotEpoch, TextureFeedback,
    TextureRequest,
};

/// Context-borrowed synchronous render lease.
///
/// Managed texture requests are bound to this frame's consumer generation and epoch. A renderer
/// must reconcile its feedback before the lease is dropped. Dropping an unreconciled managed
/// frame abandons the epoch without acknowledging destroy requests.
#[must_use = "render or explicitly drop the frame; managed requests are abandoned on drop"]
pub struct RenderedFrame<'ctx> {
    context: &'ctx mut Context,
    draw_data: NonNull<DrawData>,
    epoch: Option<SnapshotEpoch>,
    texture_requests: Vec<TextureRequest>,
    reconciled: bool,
}

/// Proof that one synchronous render lease completed managed-texture reconciliation.
///
/// The proof keeps the originating Context mutably borrowed until it is consumed or dropped. Safe
/// renderer integrations cannot construct it without consuming the corresponding
/// [`RenderedFrame`] through [`RenderedFrame::into_reconciled`]. It proves texture reconciliation,
/// not that a GPU submission or operating-system presentation completed.
#[must_use = "return the proof to the presentation owner before presenting the frame"]
pub struct ReconciledFrame<'ctx> {
    context_id: crate::ContextId,
    epoch: Option<SnapshotEpoch>,
    _context_borrow: PhantomData<&'ctx mut Context>,
}

impl std::fmt::Debug for ReconciledFrame<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ReconciledFrame")
            .field("context", &self.context_id)
            .field("epoch", &self.epoch)
            .finish()
    }
}

impl ReconciledFrame<'_> {
    /// Context that owned the reconciled render lease.
    #[must_use]
    pub const fn context_id(&self) -> crate::ContextId {
        self.context_id
    }

    /// Ordered managed-texture epoch, or `None` for a legacy renderer Context.
    #[must_use]
    pub const fn epoch(&self) -> Option<SnapshotEpoch> {
        self.epoch
    }
}

impl std::fmt::Debug for RenderedFrame<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RenderedFrame")
            .field("context", &self.context.id())
            .field("epoch", &self.epoch)
            .field("texture_requests", &self.texture_requests.len())
            .field("reconciled", &self.reconciled)
            .finish()
    }
}

impl<'ctx> RenderedFrame<'ctx> {
    pub(crate) fn new(
        context: &'ctx mut Context,
        draw_data: NonNull<DrawData>,
        epoch: Option<SnapshotEpoch>,
        texture_requests: Vec<TextureRequest>,
    ) -> Self {
        Self {
            context,
            draw_data,
            epoch,
            texture_requests,
            reconciled: false,
        }
    }

    /// Context that owns this frame.
    #[must_use]
    pub fn context_id(&self) -> crate::ContextId {
        self.context.id()
    }

    /// Ordered managed-texture epoch, or `None` for a legacy renderer context.
    #[must_use]
    pub const fn epoch(&self) -> Option<SnapshotEpoch> {
        self.epoch
    }

    /// Owned managed texture requests for this synchronous frame.
    #[must_use]
    pub fn texture_requests(&self) -> &[TextureRequest] {
        &self.texture_requests
    }

    /// Returns whether managed-texture feedback for this lease was already reconciled.
    ///
    /// Backends with a separate no-surface preparation phase may use this to make that phase
    /// idempotent before consuming the lease for drawing.
    #[must_use]
    pub const fn is_texture_feedback_reconciled(&self) -> bool {
        self.reconciled
    }

    /// Read the native draw data while this Context borrow is active.
    #[must_use]
    pub fn draw_data(&self) -> &DrawData {
        unsafe { self.draw_data.as_ref() }
    }

    /// Updates and renders native platform windows while this rendered-frame lease owns Context.
    ///
    /// This combined operation does not render or present the main viewport. The default platform
    /// pump may render and present secondary surfaces through installed callbacks. WSI backends can
    /// call it before acquiring the main surface, while OpenGL integrations can call it after the
    /// main draw and before the main swap. The installed platform and renderer callbacks must
    /// satisfy the same contract required by [`Context::render_platform_windows_default`].
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

    fn with_owner_context<R>(&mut self, f: impl FnOnce(&mut Context) -> R) -> R {
        let binding = self.context.binding();
        binding.with_bound_context(|| f(self.context))
    }

    /// Apply renderer feedback before drawing commands that depend on new texture identifiers.
    pub fn reconcile_texture_feedback(
        &mut self,
        feedback: impl IntoIterator<Item = TextureFeedback>,
    ) -> Result<SnapshotCompletionProgress, RendererConsumerError> {
        if self.reconciled {
            return Err(RendererConsumerError::EpochAlreadyCompleted {
                epoch: self.epoch.map_or(0, SnapshotEpoch::sequence),
            });
        }
        let Some(epoch) = self.epoch else {
            let feedback = feedback.into_iter().collect::<Vec<_>>();
            if feedback.is_empty() {
                self.reconciled = true;
                return Ok(SnapshotCompletionProgress::default());
            }
            return Err(RendererConsumerError::NoActiveConsumer);
        };
        let feedback = feedback.into_iter().collect();
        let progress = self
            .with_owner_context(|context| context.complete_synchronous_render(epoch, feedback))?;
        self.reconciled = true;
        Ok(progress)
    }

    /// Consumes this lease and returns proof that managed-texture feedback was reconciled.
    ///
    /// Renderer integrations should return this proof to any owner that separates rendering from
    /// presentation. Calling this before [`Self::reconcile_texture_feedback`] fails and abandons an
    /// active managed-texture epoch when the lease is dropped.
    pub fn into_reconciled(self) -> Result<ReconciledFrame<'ctx>, RendererConsumerError> {
        if !self.reconciled {
            return Err(RendererConsumerError::FrameNotReconciled {
                pending_requests: self.texture_requests.len(),
            });
        }
        let reconciled = ReconciledFrame {
            context_id: self.context.id(),
            epoch: self.epoch,
            _context_borrow: PhantomData,
        };
        drop(self);
        Ok(reconciled)
    }
}

impl Deref for RenderedFrame<'_> {
    type Target = DrawData;

    fn deref(&self) -> &Self::Target {
        self.draw_data()
    }
}

impl Drop for RenderedFrame<'_> {
    fn drop(&mut self) {
        let abandoned_epoch = (!self.reconciled).then_some(self.epoch).flatten();
        self.with_owner_context(|context| {
            if let Some(epoch) = abandoned_epoch {
                context.abandon_synchronous_render(epoch);
            }
            context.collect_retired_textures();
        });
    }
}
