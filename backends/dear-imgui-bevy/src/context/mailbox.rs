use dear_imgui_rs as imgui;
use std::collections::HashMap;

#[cfg(feature = "render")]
use std::sync::{Arc, Mutex, MutexGuard};

/// Output produced by the latest completed frame for one Dear ImGui Context.
#[derive(Debug, Default)]
pub struct ImguiContextFrameOutput {
    frame_index: u64,
    snapshot_epoch: Option<imgui::render::snapshot::SnapshotEpoch>,
    snapshot_error: Option<String>,
}

impl ImguiContextFrameOutput {
    /// Monotonic Context-local frame index for the latest completed frame.
    #[must_use]
    pub fn frame_index(&self) -> u64 {
        self.frame_index
    }

    /// Epoch of the latest snapshot handed to the render world.
    #[must_use]
    pub fn snapshot_epoch(&self) -> Option<imgui::render::snapshot::SnapshotEpoch> {
        self.snapshot_epoch
    }

    /// Snapshot error produced by the latest completed frame.
    #[must_use]
    pub fn snapshot_error(&self) -> Option<&str> {
        self.snapshot_error.as_deref()
    }
}

/// Context-keyed output produced by the latest completed Dear ImGui frames.
#[derive(bevy_ecs::resource::Resource, Debug, Default)]
pub struct ImguiFrameOutput {
    contexts: HashMap<imgui::ContextId, ImguiContextFrameOutput>,
}

impl ImguiFrameOutput {
    /// Latest output for `context_id`.
    #[must_use]
    pub fn get(&self, context_id: imgui::ContextId) -> Option<&ImguiContextFrameOutput> {
        self.contexts.get(&context_id)
    }

    /// Iterate over every Context that has produced observable frame output.
    pub fn iter(&self) -> impl Iterator<Item = (imgui::ContextId, &ImguiContextFrameOutput)> + '_ {
        self.contexts
            .iter()
            .map(|(context_id, output)| (*context_id, output))
    }

    #[cfg(feature = "render")]
    pub(crate) fn set_snapshot(
        &mut self,
        context_id: imgui::ContextId,
        mailbox: &ImguiFrameMailbox,
        renderer_releases: &crate::render::ImguiRendererReleases,
        frame_index: u64,
        include_platform_viewports: bool,
        snapshot: Result<
            imgui::render::snapshot::FrameSnapshot,
            imgui::render::snapshot::SnapshotError,
        >,
    ) {
        let output = self.contexts.entry(context_id).or_default();
        output.frame_index = frame_index;
        if renderer_releases.release_requested(context_id) {
            output.snapshot_epoch = None;
            mailbox.clear(context_id);
            output.snapshot_error = Some("Bevy renderer shutdown is in progress".to_owned());
            return;
        }
        match snapshot {
            Ok(snapshot) => {
                debug_assert_eq!(snapshot.epoch().context_id(), context_id);
                output.snapshot_epoch = Some(snapshot.epoch());
                mailbox.publish(
                    context_id,
                    PendingFrame {
                        frame_index,
                        include_platform_viewports,
                        snapshot,
                    },
                );
                output.snapshot_error = None;
            }
            Err(error) => {
                output.snapshot_epoch = None;
                mailbox.clear(context_id);
                output.snapshot_error = Some(error.to_string());
            }
        }
    }

    pub(crate) fn clear_snapshot(&mut self, context_id: imgui::ContextId) {
        let output = self.contexts.entry(context_id).or_default();
        output.snapshot_epoch = None;
        output.snapshot_error = None;
    }

    pub(crate) fn complete_without_snapshot(
        &mut self,
        context_id: imgui::ContextId,
        frame_index: u64,
    ) {
        let output = self.contexts.entry(context_id).or_default();
        output.frame_index = frame_index;
        output.snapshot_epoch = None;
        output.snapshot_error = None;
    }

    pub(crate) fn retain_contexts(&mut self, mut retain: impl FnMut(imgui::ContextId) -> bool) {
        self.contexts.retain(|context_id, _| retain(*context_id));
    }
}

/// Read-only state for the Context currently driven on the main thread.
#[derive(Default)]
pub struct ImguiFrameState {
    active: Option<(imgui::ContextId, u64)>,
}

impl ImguiFrameState {
    /// Context whose UI frame is currently open.
    #[must_use]
    pub fn context_id(&self) -> Option<imgui::ContextId> {
        self.active.map(|(context_id, _)| context_id)
    }

    /// Context-local index of the currently open UI frame.
    #[must_use]
    pub fn frame_index(&self) -> Option<u64> {
        self.active.map(|(_, frame_index)| frame_index)
    }

    /// Whether the serial driver currently exposes a UI frame.
    #[must_use]
    pub fn is_frame_open(&self) -> bool {
        self.active.is_some()
    }

    pub(crate) fn begin(&mut self, context_id: imgui::ContextId, frame_index: u64) {
        self.active = Some((context_id, frame_index));
    }

    pub(crate) fn end(&mut self) {
        self.active = None;
    }
}

#[cfg(feature = "render")]
#[derive(Debug)]
pub(crate) struct PendingFrame {
    pub(crate) frame_index: u64,
    pub(crate) include_platform_viewports: bool,
    pub(crate) snapshot: imgui::render::snapshot::FrameSnapshot,
}

#[cfg(feature = "render")]
#[derive(bevy_ecs::resource::Resource, Clone, Debug, Default)]
pub(crate) struct ImguiFrameMailbox {
    pending: Arc<Mutex<HashMap<imgui::ContextId, PendingFrame>>>,
    completion_watermarks: Arc<Mutex<HashMap<imgui::ContextId, u64>>>,
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
