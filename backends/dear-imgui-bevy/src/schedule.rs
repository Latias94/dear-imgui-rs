use bevy_app::{App, MainScheduleOrder, PreUpdate};
use bevy_ecs::{
    prelude::{IntoScheduleConfigs, SystemSet},
    schedule::ScheduleLabel,
};

/// UI schedule driven for the primary Dear ImGui Context.
#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ImguiPrimaryContextPass;

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

pub(crate) fn install_imgui_schedules(app: &mut App) {
    app.init_schedule(ImguiPrimaryContextPass)
        .init_schedule(ImguiContextDriver)
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
    if !order.labels.contains(&ImguiContextDriver.intern()) {
        order.insert_after(PreUpdate, ImguiContextDriver);
    }
}
