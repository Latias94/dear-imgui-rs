use std::fmt;

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

/// Invalid placement of the private Dear ImGui Context driver schedule.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ImguiDriverScheduleError {
    /// The App no longer contains Bevy's main schedule ordering resource.
    MainScheduleOrderMissing,
    /// Bevy's required Dear ImGui frame interval boundary is absent.
    FrameBoundaryMissing {
        /// Missing `PreUpdate` or `PostUpdate` boundary.
        boundary: InternedScheduleLabel,
    },
    /// The configured anchor is not present in Bevy's main schedule order.
    AnchorMissing {
        /// Missing schedule anchor.
        anchor: InternedScheduleLabel,
    },
    /// The requested placement would run before input mapping completed or after frame output.
    OutsideFrameInterval,
}

impl fmt::Display for ImguiDriverScheduleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MainScheduleOrderMissing => {
                formatter.write_str("MainScheduleOrder is missing from the Bevy App")
            }
            Self::FrameBoundaryMissing { boundary } => write!(
                formatter,
                "required Dear ImGui frame boundary {boundary:?} is not in MainScheduleOrder"
            ),
            Self::AnchorMissing { anchor } => write!(
                formatter,
                "Dear ImGui driver anchor {anchor:?} is not in MainScheduleOrder"
            ),
            Self::OutsideFrameInterval => formatter.write_str(
                "the Dear ImGui driver must run after PreUpdate completes and before PostUpdate begins",
            ),
        }
    }
}

impl std::error::Error for ImguiDriverScheduleError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ValidatedImguiDriverSchedulePlacement {
    insertion_index: usize,
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

pub(crate) fn validate_imgui_schedule_placement(
    app: &App,
    placement: ImguiDriverSchedulePlacement,
) -> Result<ValidatedImguiDriverSchedulePlacement, ImguiDriverScheduleError> {
    let order = app
        .world()
        .get_resource::<MainScheduleOrder>()
        .ok_or(ImguiDriverScheduleError::MainScheduleOrderMissing)?;
    let driver = ImguiContextDriver.intern();
    let labels = order
        .labels
        .iter()
        .copied()
        .filter(|label| *label != driver)
        .collect::<Vec<_>>();
    let (anchor, after_anchor) = match placement {
        ImguiDriverSchedulePlacement::Before(anchor) => (anchor, false),
        ImguiDriverSchedulePlacement::After(anchor) => (anchor, true),
    };
    let anchor_index = labels
        .iter()
        .position(|label| *label == anchor)
        .ok_or(ImguiDriverScheduleError::AnchorMissing { anchor })?;
    let insertion_index = anchor_index + usize::from(after_anchor);
    let pre_update_index = labels
        .iter()
        .position(|label| *label == PreUpdate.intern())
        .ok_or(ImguiDriverScheduleError::FrameBoundaryMissing {
            boundary: PreUpdate.intern(),
        })?;
    let post_update_index = labels
        .iter()
        .position(|label| *label == PostUpdate.intern())
        .ok_or(ImguiDriverScheduleError::FrameBoundaryMissing {
            boundary: PostUpdate.intern(),
        })?;
    if insertion_index <= pre_update_index || insertion_index > post_update_index {
        return Err(ImguiDriverScheduleError::OutsideFrameInterval);
    }
    Ok(ValidatedImguiDriverSchedulePlacement { insertion_index })
}

pub(crate) fn install_imgui_schedules(
    app: &mut App,
    placement: ValidatedImguiDriverSchedulePlacement,
) {
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
                crate::context::begin_context_retirements
                    .in_set(ImguiContextDriverSystems::RetirementBegin),
                crate::context::finish_context_retirements
                    .in_set(ImguiContextDriverSystems::RetirementFinish),
            ),
        );

    let mut order = app.world_mut().resource_mut::<MainScheduleOrder>();
    let driver = ImguiContextDriver.intern();
    order.labels.retain(|label| *label != driver);
    order.labels.insert(placement.insertion_index, driver);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn driver_rejects_placement_before_pre_update() {
        let app = App::new();
        assert_eq!(
            validate_imgui_schedule_placement(
                &app,
                ImguiDriverSchedulePlacement::before(PreUpdate)
            ),
            Err(ImguiDriverScheduleError::OutsideFrameInterval)
        );
    }

    #[test]
    fn driver_rejects_placement_after_post_update() {
        let app = App::new();
        assert_eq!(
            validate_imgui_schedule_placement(
                &app,
                ImguiDriverSchedulePlacement::after(PostUpdate)
            ),
            Err(ImguiDriverScheduleError::OutsideFrameInterval)
        );
    }

    #[test]
    fn driver_reports_a_missing_main_schedule_order() {
        let mut app = App::new();
        assert!(
            app.world_mut()
                .remove_resource::<MainScheduleOrder>()
                .is_some()
        );

        assert_eq!(
            validate_imgui_schedule_placement(&app, ImguiDriverSchedulePlacement::after(PreUpdate)),
            Err(ImguiDriverScheduleError::MainScheduleOrderMissing)
        );
    }

    #[test]
    fn driver_reports_a_missing_frame_boundary() {
        let mut app = App::new();
        app.world_mut()
            .resource_mut::<MainScheduleOrder>()
            .labels
            .retain(|label| *label != PostUpdate.intern());

        assert_eq!(
            validate_imgui_schedule_placement(&app, ImguiDriverSchedulePlacement::after(PreUpdate)),
            Err(ImguiDriverScheduleError::FrameBoundaryMissing {
                boundary: PostUpdate.intern(),
            })
        );
    }
}
