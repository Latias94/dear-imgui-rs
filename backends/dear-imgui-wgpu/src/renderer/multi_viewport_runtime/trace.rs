use dear_imgui_rs::Id;

/// Secondary-viewport GPU submissions collected while preparing one frame.
///
/// A viewport appears in [`Self::render_submitted_viewport_ids`] only after its command buffer
/// was submitted and its acquired surface frame was retained for presentation. It appears in
/// [`Self::present_submitted_viewport_ids`] only after the backend called the WGPU presentation
/// API for that retained frame. Both ID sets are sorted and contain no duplicates.
#[derive(Debug, Default, Eq, PartialEq)]
pub struct WgpuViewportFrameReport {
    render_submitted_viewport_ids: Vec<Id>,
    present_submitted_viewport_ids: Vec<Id>,
}

impl WgpuViewportFrameReport {
    /// Returns sorted, unique viewport IDs whose render commands were submitted.
    pub fn render_submitted_viewport_ids(&self) -> &[Id] {
        &self.render_submitted_viewport_ids
    }

    /// Returns sorted, unique viewport IDs whose surface frames were submitted for presentation.
    pub fn present_submitted_viewport_ids(&self) -> &[Id] {
        &self.present_submitted_viewport_ids
    }

    fn finish(mut self) -> Self {
        normalize_ids(&mut self.render_submitted_viewport_ids);
        normalize_ids(&mut self.present_submitted_viewport_ids);
        self
    }
}

fn normalize_ids(ids: &mut Vec<Id>) {
    ids.sort_unstable_by_key(|id| id.raw());
    ids.dedup();
}

#[derive(Debug, Default)]
pub(super) struct FrameTraceState {
    active: Option<WgpuViewportFrameReport>,
}

impl FrameTraceState {
    pub(super) fn begin(&mut self) -> bool {
        if self.active.is_some() {
            return false;
        }
        self.active = Some(WgpuViewportFrameReport::default());
        true
    }

    pub(super) fn record_render_submitted(&mut self, viewport_id: Id) {
        if let Some(trace) = self.active.as_mut() {
            trace.render_submitted_viewport_ids.push(viewport_id);
        }
    }

    pub(super) fn record_present_submitted(&mut self, viewport_id: Id) {
        if let Some(trace) = self.active.as_mut() {
            trace.present_submitted_viewport_ids.push(viewport_id);
        }
    }

    pub(super) fn finish(&mut self) -> WgpuViewportFrameReport {
        self.active
            .take()
            .expect("a live WGPU frame-trace guard owns the active trace")
            .finish()
    }

    pub(super) fn abort(&mut self) {
        self.active = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trace_is_non_nested_and_normalizes_id_sets() {
        let mut state = FrameTraceState::default();
        let low = Id::from(3_u32);
        let high = Id::from(7_u32);

        assert!(state.begin());
        assert!(!state.begin());
        state.record_render_submitted(high);
        state.record_render_submitted(low);
        state.record_render_submitted(high);
        state.record_present_submitted(high);
        state.record_present_submitted(low);
        state.record_present_submitted(low);

        let report = state.finish();
        assert_eq!(report.render_submitted_viewport_ids(), &[low, high]);
        assert_eq!(report.present_submitted_viewport_ids(), &[low, high]);
    }

    #[test]
    fn abort_discards_observations_and_inactive_recording_is_ignored() {
        let mut state = FrameTraceState::default();
        let viewport_id = Id::from(9_u32);

        state.record_render_submitted(viewport_id);
        assert!(state.begin());
        state.record_render_submitted(viewport_id);
        state.abort();

        assert!(state.begin());
        let report = state.finish();
        assert!(report.render_submitted_viewport_ids().is_empty());
        assert!(report.present_submitted_viewport_ids().is_empty());
    }
}
