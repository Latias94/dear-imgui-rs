use bevy_app::{App, MainScheduleOrder, PreUpdate};
use bevy_ecs::{
    prelude::{IntoScheduleConfigs, SystemSet},
    schedule::ScheduleLabel,
};

/// UI schedule driven for the primary Dear ImGui Context.
#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ImguiPrimaryContextPass;

/// Dedicated UI schedule namespace for an additional Dear ImGui Context.
///
/// Wrapping the caller's label prevents an application schedule such as `Update` from being
/// removed and run recursively by the Context driver. Use the same inner label when registering
/// systems and constructing [`crate::ImguiContextConfig`].
///
/// ```no_run
/// use bevy_app::App;
/// use bevy_ecs::schedule::ScheduleLabel;
/// use dear_imgui_bevy::{ImguiContextPass, ImguiUi};
///
/// #[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
/// struct InspectorUi;
///
/// fn inspector(_ui: ImguiUi<'_>) {}
///
/// let mut app = App::new();
/// app.add_systems(ImguiContextPass::new(InspectorUi), inspector);
/// ```
#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ImguiContextPass<L>(L)
where
    L: ScheduleLabel + Clone + Eq + std::hash::Hash;

impl<L> ImguiContextPass<L>
where
    L: ScheduleLabel + Clone + Eq + std::hash::Hash,
{
    /// Place `label` in the dedicated Dear ImGui Context schedule namespace.
    #[must_use]
    pub const fn new(label: L) -> Self {
        Self(label)
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
