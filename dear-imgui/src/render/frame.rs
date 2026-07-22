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

    /// Read the native draw data while this Context borrow is active.
    #[must_use]
    pub fn draw_data(&self) -> &DrawData {
        unsafe { self.draw_data.as_ref() }
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
