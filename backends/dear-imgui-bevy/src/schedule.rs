//! Bevy schedules used by the Dear ImGui backend.
//!
//! The backend keeps the frame boundary explicit by running three small schedules after Bevy input
//! has been translated into Dear ImGui IO:
//!
//! 1. [`ImguiBeginFrame`] opens exactly one primary-context frame.
//! 2. [`ImguiPrimaryContextPass`] runs user UI systems against the already-open frame.
//! 3. [`ImguiEndFrame`] renders the frame and stores a thread-safe snapshot for later render
//!    extraction.

use bevy_app::{App, MainScheduleOrder, PreUpdate};
use bevy_ecs::schedule::ScheduleLabel;

/// Schedule that opens the primary Dear ImGui frame.
#[derive(ScheduleLabel, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ImguiBeginFrame;

/// Schedule where user systems draw into the primary Dear ImGui frame.
#[derive(ScheduleLabel, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ImguiPrimaryContextPass;

/// Schedule that closes the primary Dear ImGui frame and snapshots draw data.
#[derive(ScheduleLabel, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ImguiEndFrame;

pub(crate) fn install_imgui_schedules(app: &mut App) {
    app.init_schedule(ImguiBeginFrame)
        .init_schedule(ImguiPrimaryContextPass)
        .init_schedule(ImguiEndFrame)
        .init_resource::<MainScheduleOrder>();

    let mut order = app.world_mut().resource_mut::<MainScheduleOrder>();
    insert_after_if_missing(&mut order, PreUpdate, ImguiBeginFrame);
    insert_after_if_missing(&mut order, ImguiBeginFrame, ImguiPrimaryContextPass);
    insert_after_if_missing(&mut order, ImguiPrimaryContextPass, ImguiEndFrame);
}

fn insert_after_if_missing(
    order: &mut MainScheduleOrder,
    after: impl ScheduleLabel,
    schedule: impl ScheduleLabel,
) {
    if order.labels.iter().any(|current| (**current).eq(&schedule)) {
        return;
    }
    order.insert_after(after, schedule);
}
