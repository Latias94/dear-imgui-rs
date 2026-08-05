use bevy_app::{App, MainScheduleOrder, PostUpdate, PreUpdate};
use bevy_ecs::{
    prelude::{IntoScheduleConfigs, SystemSet},
    schedule::{InternedScheduleLabel, ScheduleLabel},
};

/// Placement of the serial Dear ImGui driver in Bevy's main schedule order.
///
/// The default is [`ImguiDriverSchedulePlacement::After`] [`PreUpdate`]. Bevy input is therefore
/// mapped before Dear ImGui opens its frame, and gameplay systems in `Update` can observe the
/// current UI output and capture state. Custom placements must remain after `PreUpdate` completes
/// and before `PostUpdate` begins so input mapping and route publication cannot straddle a frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImguiDriverSchedulePlacement {
    /// Run the Dear ImGui driver immediately before the anchor schedule.
    Before(InternedScheduleLabel),
    /// Run the Dear ImGui driver immediately after the anchor schedule.
    After(InternedScheduleLabel),
}

impl ImguiDriverSchedulePlacement {
    /// Place the driver immediately before `anchor`.
    #[must_use]
    pub fn before(anchor: impl ScheduleLabel) -> Self {
        Self::Before(anchor.intern())
    }

    /// Place the driver immediately after `anchor`.
    #[must_use]
    pub fn after(anchor: impl ScheduleLabel) -> Self {
        Self::After(anchor.intern())
    }
}

impl Default for ImguiDriverSchedulePlacement {
    fn default() -> Self {
        Self::after(PreUpdate)
    }
}

/// Private exclusive schedule that serially activates every registered Context.
#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ImguiContextDriver;

/// Internal ordering for serial Context frames and deferred platform work.
#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum ImguiContextDriverSystems {
    Drive,
    RetirementBegin,
    Platform,
    RetirementFinish,
}

pub(crate) fn install_imgui_schedules(app: &mut App, placement: ImguiDriverSchedulePlacement) {
    app.init_schedule(ImguiContextDriver)
        .configure_sets(
            ImguiContextDriver,
            (
                ImguiContextDriverSystems::Drive,
                ImguiContextDriverSystems::RetirementBegin,
                ImguiContextDriverSystems::Platform,
                ImguiContextDriverSystems::RetirementFinish,
            )
                .chain(),
        )
        .add_systems(
            ImguiContextDriver,
            (
                crate::context::drive_imgui_contexts.in_set(ImguiContextDriverSystems::Drive),
                crate::context::ownership::begin_context_retirements
                    .in_set(ImguiContextDriverSystems::RetirementBegin),
                crate::context::ownership::finish_context_retirements
                    .in_set(ImguiContextDriverSystems::RetirementFinish),
            ),
        );

    let mut order = app.world_mut().resource_mut::<MainScheduleOrder>();
    let driver = ImguiContextDriver.intern();
    order.labels.retain(|label| *label != driver);
    let (anchor, after_anchor) = match placement {
        ImguiDriverSchedulePlacement::Before(anchor) => (anchor, false),
        ImguiDriverSchedulePlacement::After(anchor) => (anchor, true),
    };
    let anchor_index = order
        .labels
        .iter()
        .position(|label| *label == anchor)
        .unwrap_or_else(|| {
            panic!("Dear ImGui driver anchor {anchor:?} is not in MainScheduleOrder")
        });
    let insertion_index = anchor_index + usize::from(after_anchor);
    let pre_update_index = order
        .labels
        .iter()
        .position(|label| *label == PreUpdate.intern())
        .expect("PreUpdate must be present in MainScheduleOrder");
    let post_update_index = order
        .labels
        .iter()
        .position(|label| *label == PostUpdate.intern())
        .expect("PostUpdate must be present in MainScheduleOrder");
    assert!(
        insertion_index > pre_update_index && insertion_index <= post_update_index,
        "Dear ImGui driver must run after PreUpdate completes and before PostUpdate begins"
    );
    order.labels.insert(insertion_index, driver);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic(
        expected = "Dear ImGui driver must run after PreUpdate completes and before PostUpdate begins"
    )]
    fn driver_rejects_placement_before_pre_update() {
        let mut app = App::new();
        install_imgui_schedules(&mut app, ImguiDriverSchedulePlacement::before(PreUpdate));
    }

    #[test]
    #[should_panic(
        expected = "Dear ImGui driver must run after PreUpdate completes and before PostUpdate begins"
    )]
    fn driver_rejects_placement_after_post_update() {
        let mut app = App::new();
        install_imgui_schedules(&mut app, ImguiDriverSchedulePlacement::after(PostUpdate));
    }
}
