use dear_imgui_rs as imgui;

#[cfg(feature = "render")]
use std::sync::{Arc, Mutex, MutexGuard};

/// Output produced by the latest completed primary-Context frame.
///
/// Context-local frame indices live in [`super::ImguiContexts`]. This primary projection remains
/// until the render-world mailbox becomes fully Context-keyed in the renderer partitioning step.
#[derive(bevy_ecs::resource::Resource, Debug, Default)]
pub struct ImguiFrameOutput {
    frame_index: u64,
    snapshot_epoch: Option<imgui::render::snapshot::SnapshotEpoch>,
    snapshot_error: Option<String>,
}

impl ImguiFrameOutput {
    /// Monotonic primary-Context frame index for the latest completed frame.
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

    #[cfg(feature = "render")]
    pub(crate) fn set_snapshot(
        &mut self,
        mailbox: &ImguiFrameMailbox,
        renderer_release: &crate::render::ImguiRendererRelease,
        frame_index: u64,
        snapshot: Result<
            imgui::render::snapshot::FrameSnapshot,
            imgui::render::snapshot::SnapshotError,
        >,
    ) {
        self.frame_index = frame_index;
        if renderer_release.release_requested() {
            self.snapshot_epoch = None;
            mailbox.clear();
            self.snapshot_error = Some("Bevy renderer shutdown is in progress".to_owned());
            return;
        }
        match snapshot {
            Ok(snapshot) => {
                self.snapshot_epoch = Some(snapshot.epoch());
                mailbox.publish(frame_index, snapshot);
                self.snapshot_error = None;
            }
            Err(error) => {
                self.snapshot_epoch = None;
                mailbox.clear();
                self.snapshot_error = Some(error.to_string());
            }
        }
    }

    pub(crate) fn clear_snapshot(&mut self) {
        self.snapshot_epoch = None;
        self.snapshot_error = None;
    }

    pub(crate) fn complete_without_snapshot(&mut self, frame_index: u64) {
        self.frame_index = frame_index;
        self.snapshot_epoch = None;
        self.snapshot_error = None;
    }
}

/// Read-only primary frame state retained for renderer migration.
#[derive(Default)]
pub struct ImguiFrameState {
    frame_index: u64,
    frame_open: bool,
}

impl ImguiFrameState {
    /// Current or most recently opened primary frame index.
    #[must_use]
    pub fn frame_index(&self) -> u64 {
        self.frame_index
    }

    /// Whether the serial driver currently exposes the primary frame.
    #[must_use]
    pub fn is_frame_open(&self) -> bool {
        self.frame_open
    }

    pub(crate) fn begin(&mut self, frame_index: u64) {
        self.frame_index = frame_index;
        self.frame_open = true;
    }

    pub(crate) fn end(&mut self) {
        self.frame_open = false;
    }
}

#[cfg(feature = "render")]
#[derive(bevy_ecs::resource::Resource, Clone, Debug, Default)]
pub(crate) struct ImguiFrameMailbox {
    pending: Arc<Mutex<Option<(u64, imgui::render::snapshot::FrameSnapshot)>>>,
}

#[cfg(feature = "render")]
impl ImguiFrameMailbox {
    fn pending(&self) -> MutexGuard<'_, Option<(u64, imgui::render::snapshot::FrameSnapshot)>> {
        self.pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub(crate) fn publish(
        &self,
        frame_index: u64,
        snapshot: imgui::render::snapshot::FrameSnapshot,
    ) {
        let previous = self.pending().replace((frame_index, snapshot));
        drop(previous);
    }

    pub(crate) fn take(&self) -> Option<(u64, imgui::render::snapshot::FrameSnapshot)> {
        self.pending().take()
    }

    pub(crate) fn clear(&self) {
        let previous = self.pending().take();
        drop(previous);
    }
}
