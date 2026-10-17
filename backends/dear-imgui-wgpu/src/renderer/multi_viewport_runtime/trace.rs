use super::work::RecordedTarget;
use dear_imgui_rs::NativeWindowTarget;

use crate::{
    WgpuDrawDependencies, WgpuDrawRecording, WgpuFrameOutcome, WgpuFrameScope, WgpuFrameSkipReason,
    WgpuFrameStage, WgpuViewportObservation,
};

/// Owned per-target observations from one secondary-viewport preparation attempt.
///
/// Reports contain no native handles or GPU resource leases. Save them in editor history, including
/// reports returned with route errors. Their stages prove API operations, never GPU completion or
/// visible pixels. Multiple observations of the same target remain separate attempts.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WgpuViewportFrameReport {
    pub(super) scope: Option<WgpuFrameScope>,
    pub(super) observations: Vec<WgpuViewportObservation>,
}

impl WgpuViewportFrameReport {
    /// UI scope, absent when preparation failed before a frame was reconciled.
    pub fn scope(&self) -> Option<WgpuFrameScope> {
        self.scope
    }

    /// Target attempts in observation order, including skips and failures.
    pub fn observations(&self) -> &[WgpuViewportObservation] {
        &self.observations
    }

    /// Consumes the report without retaining the Context or any live window.
    pub fn into_observations(self) -> Vec<WgpuViewportObservation> {
        self.observations
    }
}

impl WgpuViewportFrameReport {
    fn observation_mut(
        &mut self,
        key: Option<ObservationKey>,
    ) -> Option<&mut WgpuViewportObservation> {
        let key = key?;
        if self.scope != Some(key.scope) {
            return None;
        }
        self.observations.get_mut(key.index)
    }
    pub(super) fn stage(&mut self, key: Option<ObservationKey>, stage: WgpuFrameStage) {
        if let Some(observation) = self.observation_mut(key) {
            observation.stage = observation.stage.max(stage);
        }
    }
    pub(super) fn fail(
        &mut self,
        key: Option<ObservationKey>,
        operation: &'static str,
        error: &impl std::fmt::Display,
    ) {
        if let Some(observation) = self.observation_mut(key) {
            observation.outcome = WgpuFrameOutcome::Failed {
                operation,
                message: error.to_string(),
            };
        }
    }
    pub(super) fn discard(&mut self, key: Option<ObservationKey>) {
        if let Some(observation) = self.observation_mut(key) {
            observation.outcome = WgpuFrameOutcome::Discarded;
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ObservationKey {
    scope: WgpuFrameScope,
    index: usize,
}

#[derive(Default)]
pub(super) struct FrameTraceState {
    active: Option<WgpuViewportFrameReport>,
    recorded: Vec<RecordedTarget>,
}

impl std::fmt::Debug for FrameTraceState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FrameTraceState")
            .field("active", &self.active)
            .field("recorded_count", &self.recorded.len())
            .finish()
    }
}

impl FrameTraceState {
    pub(super) fn begin(&mut self, scope: WgpuFrameScope) -> bool {
        if self.active.is_some() {
            return false;
        }
        self.recorded.clear();
        self.active = Some(WgpuViewportFrameReport {
            scope: Some(scope),
            observations: Vec::new(),
        });
        true
    }

    pub(super) fn start(&mut self, target: NativeWindowTarget) -> Option<ObservationKey> {
        let report = self.active.as_mut()?;
        let scope = report.scope?;
        let index = report.observations.len();
        report.observations.push(WgpuViewportObservation {
            target,
            scope,
            dependencies: WgpuDrawDependencies::default(),
            stage: WgpuFrameStage::Unrecorded,
            outcome: WgpuFrameOutcome::Observed,
        });
        Some(ObservationKey { scope, index })
    }

    fn observation_mut(
        &mut self,
        key: Option<ObservationKey>,
    ) -> Option<&mut WgpuViewportObservation> {
        let key = key?;
        let report = self.active.as_mut()?;
        if report.scope != Some(key.scope) {
            return None;
        }
        report.observations.get_mut(key.index)
    }

    pub(super) fn target(&mut self, key: Option<ObservationKey>, target: NativeWindowTarget) {
        if let Some(observation) = self.observation_mut(key) {
            if observation.target.instance_id == target.instance_id {
                observation.target = target;
            }
        }
    }

    pub(super) fn recording(&mut self, key: Option<ObservationKey>, recording: WgpuDrawRecording) {
        if let Some(observation) = self.observation_mut(key) {
            *observation = WgpuViewportObservation::from_recording(observation.target, recording);
        }
    }

    #[cfg(test)]
    pub(super) fn stage(&mut self, key: Option<ObservationKey>, stage: WgpuFrameStage) {
        if let Some(observation) = self.observation_mut(key) {
            observation.stage = observation.stage.max(stage);
        }
    }

    pub(super) fn skip(&mut self, key: Option<ObservationKey>, reason: WgpuFrameSkipReason) {
        if let Some(observation) = self.observation_mut(key) {
            observation.outcome = WgpuFrameOutcome::Skipped(reason);
        }
    }

    pub(super) fn fail(
        &mut self,
        key: Option<ObservationKey>,
        operation: &'static str,
        error: &impl std::fmt::Display,
    ) {
        if let Some(observation) = self.observation_mut(key) {
            observation.outcome = WgpuFrameOutcome::Failed {
                operation,
                message: error.to_string(),
            };
        }
    }

    pub(super) fn scope(&self) -> Option<WgpuFrameScope> {
        self.active.as_ref()?.scope
    }

    pub(super) fn finish_recording(&mut self) -> (WgpuViewportFrameReport, Vec<RecordedTarget>) {
        let report = self.finish();
        (report, std::mem::take(&mut self.recorded))
    }

    pub(super) fn push_recorded(&mut self, target: RecordedTarget) {
        if self.active.is_some() {
            self.recorded.push(target);
        }
    }

    pub(super) fn dependencies(
        &mut self,
        key: Option<ObservationKey>,
        dependencies: WgpuDrawDependencies,
    ) {
        if let Some(observation) = self.observation_mut(key) {
            observation.dependencies = dependencies;
        }
    }

    pub(super) fn finish(&mut self) -> WgpuViewportFrameReport {
        self.active
            .take()
            .expect("a live WGPU frame-trace guard owns the active trace")
    }

    pub(super) fn abort(&mut self) {
        self.active = None;
        self.recorded.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dear_imgui_rs::{Context, Id, NativeWindowInstanceId};

    #[test]
    fn observations_keep_native_identity_and_do_not_upgrade_old_scopes() {
        let context = Context::create();
        let scope = WgpuFrameScope {
            context_id: context.id(),
            epoch: None,
            frame_count: 1,
        };
        let target = NativeWindowTarget {
            instance_id: NativeWindowInstanceId::allocate(context.id()).unwrap(),
            viewport_id: Id::from(7),
            is_main: false,
        };
        let mut trace = FrameTraceState::default();
        assert!(trace.begin(scope));
        assert!(!trace.begin(scope));
        let key = trace.start(target);
        trace.stage(key, WgpuFrameStage::Submitted);
        trace.fail(key, "late recovery", &"surface lost after submit");
        let report = trace.finish();
        assert_eq!(report.observations()[0].target, target);
        assert_eq!(report.observations()[0].stage, WgpuFrameStage::Submitted);
        assert!(matches!(
            report.observations()[0].outcome,
            WgpuFrameOutcome::Failed { .. }
        ));

        let next_scope = WgpuFrameScope {
            frame_count: 2,
            ..scope
        };
        assert!(trace.begin(next_scope));
        let rerouted = NativeWindowTarget {
            viewport_id: Id::from(11),
            ..target
        };
        let next = trace.start(rerouted);
        trace.stage(key, WgpuFrameStage::PresentationRequested);
        trace.skip(next, WgpuFrameSkipReason::RetiredTarget);
        let next_report = trace.finish();
        assert_eq!(
            next_report.observations()[0].stage,
            WgpuFrameStage::Unrecorded
        );
        assert_eq!(
            next_report.observations()[0].target.instance_id,
            target.instance_id
        );
        assert_ne!(
            report.observations()[0].target.viewport_id,
            next_report.observations()[0].target.viewport_id
        );
        assert_eq!(report.observations()[0].scope.frame_count, 1);
    }

    #[test]
    fn skipped_targets_have_distinct_reasons_without_submission_claims() {
        let context = Context::create();
        let scope = WgpuFrameScope {
            context_id: context.id(),
            epoch: None,
            frame_count: 1,
        };
        let target = NativeWindowTarget {
            instance_id: NativeWindowInstanceId::allocate(context.id()).unwrap(),
            viewport_id: Id::from(7),
            is_main: false,
        };
        let mut trace = FrameTraceState::default();
        assert!(trace.begin(scope));
        for reason in [
            WgpuFrameSkipReason::ZeroSize,
            WgpuFrameSkipReason::NoDrawData,
            WgpuFrameSkipReason::Timeout,
            WgpuFrameSkipReason::Outdated,
            WgpuFrameSkipReason::Lost,
            WgpuFrameSkipReason::Occluded,
            WgpuFrameSkipReason::RetiredTarget,
        ] {
            let key = trace.start(target);
            trace.skip(key, reason);
        }
        let report = trace.finish();
        assert_eq!(report.observations().len(), 7);
        assert!(
            report
                .observations()
                .iter()
                .all(|entry| entry.stage == WgpuFrameStage::Unrecorded)
        );
    }
}
