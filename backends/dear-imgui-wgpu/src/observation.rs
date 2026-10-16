//! Detached metadata describing observed renderer work, without GPU-completion claims.

use dear_imgui_rs::{ContextId, NativeWindowTarget, render::SnapshotEpoch};

use crate::WgpuDrawDependencies;

/// The UI scope in which renderer work was observed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WgpuFrameScope {
    /// Context that produced the draw data.
    pub context_id: ContextId,
    /// Managed renderer epoch; absent only for explicitly legacy draw data.
    pub epoch: Option<SnapshotEpoch>,
    /// Dear ImGui native frame counter when the draw data was captured.
    pub frame_count: usize,
}

/// Why a target did not produce Dear ImGui draw commands during this attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum WgpuFrameSkipReason {
    /// The framebuffer has at least one zero dimension.
    ZeroSize,
    /// No valid draw data was available.
    NoDrawData,
    /// Every command was empty or clipped away.
    NoDrawCommands,
    /// Surface acquisition timed out.
    Timeout,
    /// The outdated surface was reconfigured for a later attempt.
    Outdated,
    /// The lost surface was recreated for a later attempt.
    Lost,
    /// The compositor reported the surface as occluded.
    Occluded,
    /// The native allocation was no longer owned by the captured platform owner.
    RetiredTarget,
    /// A previous recorded or pending frame still owns this surface acquisition.
    TargetBusy,
}

/// Successful recording metadata returned for an application-owned render pass.
///
/// The render pass does not reveal its native window or queue. This value therefore proves only
/// which commands were recorded; it cannot establish submission, presentation, or GPU completion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WgpuDrawRecording {
    /// UI frame associated with this recording attempt.
    pub scope: WgpuFrameScope,
    /// External view mappings captured during command preparation.
    pub dependencies: WgpuDrawDependencies,
    /// Whether Dear ImGui commands were successfully recorded into the supplied render pass.
    pub recorded: bool,
    /// Reason for an attempt with no Dear ImGui commands.
    pub skip_reason: Option<WgpuFrameSkipReason>,
}

impl WgpuDrawRecording {
    pub(crate) fn skipped(scope: WgpuFrameScope, reason: WgpuFrameSkipReason) -> Self {
        Self {
            scope,
            dependencies: WgpuDrawDependencies::default(),
            recorded: false,
            skip_reason: Some(reason),
        }
    }
}

/// Highest operation actually observed for one native target in one attempt.
///
/// Submission is a host queue operation. PresentationRequested means the backend called the
/// presentation API. Neither stage proves GPU completion, compositor acceptance, scanout, or
/// visible pixels. A submitted clear-only pass can have no external texture dependencies.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum WgpuFrameStage {
    /// No successful recording was observed.
    Unrecorded,
    /// Commands were recorded successfully.
    Recorded,
    /// The target's command buffer was passed to Queue::submit.
    Submitted,
    /// The target's retained surface frame was passed to the presentation API.
    PresentationRequested,
}

/// Result of one target attempt, independent of the highest operation already observed.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum WgpuFrameOutcome {
    /// The backend completed the operations represented by `stage`.
    Observed,
    /// Drawing or acquisition was skipped for a stated reason.
    Skipped(WgpuFrameSkipReason),
    /// Recorded or submitted work was explicitly abandoned without requesting presentation.
    Discarded,
    /// An operation failed; earlier recorded/submitted stages remain valid evidence.
    Failed {
        /// Operation being attempted when the error was observed.
        operation: &'static str,
        /// Owned diagnostic text, without retaining native resources or errors.
        message: String,
    },
}

/// Immutable-by-ownership association between native target, frame, dependencies and observed work.
///
/// Save this value directly in editor history. Resolve live windows separately by `instance_id`;
/// the stored viewport routing ID must never be reinterpreted through a newer routing table.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WgpuViewportObservation {
    /// Native registration and routing association captured for this attempt.
    pub target: NativeWindowTarget,
    /// UI frame associated with the observation.
    pub scope: WgpuFrameScope,
    /// External mappings used by successfully prepared commands.
    pub dependencies: WgpuDrawDependencies,
    /// Highest completed operation observed by this backend.
    pub stage: WgpuFrameStage,
    /// Skip or failure information, retaining any earlier successful stage.
    pub outcome: WgpuFrameOutcome,
}

impl WgpuViewportObservation {
    #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
    pub(crate) fn from_recording(target: NativeWindowTarget, recording: WgpuDrawRecording) -> Self {
        Self {
            target,
            scope: recording.scope,
            dependencies: recording.dependencies,
            stage: if recording.recorded {
                WgpuFrameStage::Recorded
            } else {
                WgpuFrameStage::Unrecorded
            },
            outcome: recording
                .skip_reason
                .map_or(WgpuFrameOutcome::Observed, WgpuFrameOutcome::Skipped),
        }
    }
}
