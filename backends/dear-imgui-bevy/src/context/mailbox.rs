#[cfg(feature = "render")]
use dear_imgui_rs as imgui;
#[cfg(feature = "render")]
use std::collections::HashMap;
#[cfg(feature = "render")]
use std::sync::{Arc, Mutex, MutexGuard};

#[cfg(feature = "render")]
#[derive(Debug)]
pub(crate) struct PendingFrame {
    #[cfg(test)]
    pub(crate) frame_index: u64,
    pub(crate) include_platform_viewports: bool,
    pub(crate) render_routes: crate::route::ImguiRenderRouteEpoch,
    pub(crate) snapshot: imgui::render::snapshot::FrameSnapshot,
}

#[cfg(feature = "render")]
#[derive(bevy_ecs::resource::Resource, Clone, Debug, Default)]
pub(crate) struct ImguiFrameMailbox {
    pending: Arc<Mutex<HashMap<imgui::ContextId, PendingFrame>>>,
    completion_watermarks: Arc<Mutex<HashMap<imgui::ContextId, u64>>>,
    snapshot_commit_errors: Arc<
        Mutex<
            Vec<(
                imgui::ContextId,
                imgui::render::snapshot::SnapshotCommitError,
            )>,
        >,
    >,
}

#[cfg(feature = "render")]
impl ImguiFrameMailbox {
    fn pending(&self) -> MutexGuard<'_, HashMap<imgui::ContextId, PendingFrame>> {
        self.pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub(crate) fn publish(&self, context_id: imgui::ContextId, frame: PendingFrame) {
        debug_assert_eq!(frame.snapshot.epoch().context_id(), context_id);
        let previous = self.pending().insert(context_id, frame);
        drop(previous);
    }

    pub(crate) fn take_all(&self) -> HashMap<imgui::ContextId, PendingFrame> {
        std::mem::take(&mut *self.pending())
    }

    pub(crate) fn clear(&self, context_id: imgui::ContextId) {
        let previous = self.pending().remove(&context_id);
        drop(previous);
    }

    pub(crate) fn update_completion_watermark(&self, context_id: imgui::ContextId, watermark: u64) {
        let mut watermarks = self
            .completion_watermarks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let current = watermarks.entry(context_id).or_default();
        *current = (*current).max(watermark);
    }

    pub(crate) fn completion_watermarks(&self) -> HashMap<imgui::ContextId, u64> {
        self.completion_watermarks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    pub(crate) fn record_snapshot_commit_error(
        &self,
        context_id: imgui::ContextId,
        source: imgui::render::snapshot::SnapshotCommitError,
    ) {
        self.snapshot_commit_errors
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push((context_id, source));
    }

    pub(crate) fn take_snapshot_commit_errors(
        &self,
    ) -> Vec<(
        imgui::ContextId,
        imgui::render::snapshot::SnapshotCommitError,
    )> {
        std::mem::take(
            &mut *self
                .snapshot_commit_errors
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        )
    }

    pub(crate) fn remove_context(&self, context_id: imgui::ContextId) {
        self.clear(context_id);
        self.completion_watermarks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&context_id);
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.pending().len()
    }
}
