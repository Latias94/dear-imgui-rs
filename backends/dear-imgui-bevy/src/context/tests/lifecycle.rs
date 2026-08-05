#[cfg(feature = "render")]
use std::{cell::Cell, rc::Rc};

use crate::test_util::imgui_context_guard;
#[cfg(feature = "render")]
use bevy_app::Main;
use bevy_app::{App, Last, PostUpdate, PreUpdate, Update};
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::{ScheduleLabel, Schedules};
#[cfg(feature = "render")]
use bevy_render::{Render, RenderApp, extract_plugin::ExtractPlugin};
use bevy_time::{Real, Time};
use bevy_window::{PrimaryWindow, Window, WindowResolution};
#[cfg(feature = "render")]
use dear_imgui_bevy::ImguiContextRemovalPendingReason;
use dear_imgui_bevy::{
    ContextId, ImguiAppExt, ImguiContextConfig, ImguiContextError, ImguiContexts, ImguiFrame,
    ImguiPass, ImguiPlugin, ImguiPrimaryPass,
};
use std::time::Duration;

struct ContextPassA;

struct ContextPassB;

#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
struct DynamicallyRegisteredPass;

struct MissingContextPass;

#[derive(Resource, Default)]
struct LifecycleTrace {
    expected: Vec<ContextId>,
    visits: Vec<(ContextId, u64)>,
    mutation_rejected: bool,
    expected_raw: Vec<(ContextId, usize)>,
    wrong_current_context: bool,
    delta_times: Vec<f32>,
    display_metrics: Vec<([f32; 2], [f32; 2])>,
}

#[derive(Resource, Default)]
struct ScheduleNamespaceTrace {
    update_runs: u32,
    ui_runs: u32,
}

#[derive(Resource, Default)]
struct NestedScheduleTrace {
    pass_a_runs: u32,
    pass_b_runs: u32,
    update_runs: u32,
    nested_update_ran: bool,
    pass_b_contexts: Vec<ContextId>,
}

#[derive(Resource, Default)]
struct DynamicScheduleTrace(u32);

#[derive(Component)]
struct DeferredPassEntity(u64);

#[derive(Resource, Default)]
struct PassSystemSemantics {
    local_values: Vec<u32>,
    change_flags: Vec<bool>,
}

#[derive(Resource, Default)]
struct TrackedPassResource;

#[derive(Resource, Default)]
struct ConfiguredPassTrace(Vec<&'static str>);

#[derive(Component)]
struct ConfiguredPassDeferredEntity;

#[derive(SystemSet, Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum ConfiguredPassSet {
    Produce,
    Observe,
    Final,
}

#[derive(Resource, Default)]
struct MainScheduleTrace(Vec<&'static str>);

#[cfg(feature = "render")]
struct RetirementProbeMarker;

#[cfg(feature = "render")]
struct RetirementProbe {
    destroyed: Rc<Cell<bool>>,
}

#[cfg(feature = "render")]
impl dear_imgui_rs::ContextAttachment for RetirementProbe {
    fn context_destroyed(&self, _context: dear_imgui_rs::ContextDestroyed) {
        self.destroyed.set(true);
    }
}

fn app_with_primary_window() -> App {
    let mut app = App::new();
    app.world_mut().spawn((
        Window {
            resolution: WindowResolution::new(640, 480),
            ..Default::default()
        },
        PrimaryWindow,
    ));
    app
}

fn record_ui<P: 'static>(frame: ImguiFrame<'_, P>, mut trace: ResMut<LifecycleTrace>) {
    let context_id = frame.context_id();
    let frame_index = frame.frame_index();
    let current_ui = frame.ui();
    assert_eq!(current_ui.context_id(), context_id);
    let current_raw = unsafe { dear_imgui_rs::sys::igGetCurrentContext() } as usize;
    if let Some(expected_raw) = trace
        .expected_raw
        .iter()
        .find(|(expected_id, _)| *expected_id == context_id)
        .map(|(_, raw)| *raw)
    {
        trace.wrong_current_context |= current_raw != expected_raw;
    }
    current_ui.text(format!(
        "Context {:?}, frame {frame_index}",
        context_id.get()
    ));
    trace.visits.push((context_id, frame_index));
}

fn capture_primary_metrics(frame: ImguiFrame<'_>, mut trace: ResMut<LifecycleTrace>) {
    let ui = frame.ui();
    trace.delta_times.push(ui.io().delta_time());
    trace
        .display_metrics
        .push((ui.io().display_size(), ui.io().display_framebuffer_scale()));
}

fn reject_raw_mutation_during_ui(
    _frame: ImguiFrame<'_>,
    mut contexts: NonSendMut<ImguiContexts>,
    missing_pass: Res<ImguiPass<MissingContextPass>>,
    mut trace: ResMut<LifecycleTrace>,
) {
    let target = trace
        .expected
        .last()
        .copied()
        .expect("the test installs an additional Context");
    let current_before_create = unsafe { dear_imgui_rs::sys::igGetCurrentContext() };
    let configure_rejected = matches!(
        contexts.configure(target, |_| ()),
        Err(ImguiContextError::RawMutationWhileFrameOpen { .. })
    );
    let create_rejected = matches!(
        contexts.create(ImguiContextConfig::new(&missing_pass)),
        Err(ImguiContextError::RawMutationWhileFrameOpen { .. })
    );
    let removal_rejected = matches!(
        contexts.remove(target),
        Err(ImguiContextError::RawMutationWhileFrameOpen { .. })
    );
    let current_after_create = unsafe { dear_imgui_rs::sys::igGetCurrentContext() };
    trace.mutation_rejected = configure_rejected
        && create_rejected
        && removal_rejected
        && current_after_create == current_before_create;
}

fn record_update_schedule(mut trace: ResMut<ScheduleNamespaceTrace>) {
    trace.update_runs += 1;
}

fn record_update_named_ui_pass(
    frame: ImguiFrame<'_, ContextPassA>,
    mut trace: ResMut<ScheduleNamespaceTrace>,
) {
    let _ = frame.ui();
    trace.ui_runs += 1;
}

fn attempt_nested_schedules(_frame: ImguiFrame<'_, ContextPassA>, world: &mut World) {
    let update_before = world.resource::<NestedScheduleTrace>().update_runs;
    world.run_schedule(Update);
    let update_after = world.resource::<NestedScheduleTrace>().update_runs;
    let mut trace = world.resource_mut::<NestedScheduleTrace>();
    trace.pass_a_runs += 1;
    trace.nested_update_ran = update_after == update_before + 1;
}

fn record_nested_context_b(
    frame: ImguiFrame<'_, ContextPassB>,
    mut trace: ResMut<NestedScheduleTrace>,
) {
    trace.pass_b_runs += 1;
    trace.pass_b_contexts.push(frame.context_id());
}

fn record_nested_update(mut trace: ResMut<NestedScheduleTrace>) {
    trace.update_runs += 1;
}

fn record_dynamic_schedule(mut trace: ResMut<DynamicScheduleTrace>) {
    trace.0 += 1;
}

fn register_and_run_dynamic_schedule(_frame: ImguiFrame<'_>, world: &mut World) {
    let mut schedule = bevy_ecs::schedule::Schedule::new(DynamicallyRegisteredPass);
    schedule.add_systems(record_dynamic_schedule);
    world.add_schedule(schedule);
    world.run_schedule(DynamicallyRegisteredPass);
}

fn exercise_pass_system_semantics(
    frame: ImguiFrame<'_>,
    mut commands: Commands,
    mut local: Local<u32>,
    tracked: Res<TrackedPassResource>,
    mut semantics: ResMut<PassSystemSemantics>,
) {
    *local += 1;
    semantics.local_values.push(*local);
    semantics.change_flags.push(tracked.is_changed());
    commands.spawn(DeferredPassEntity(frame.frame_index()));
}

fn produce_deferred_entity(
    _frame: ImguiFrame<'_>,
    mut commands: Commands,
    mut trace: ResMut<ConfiguredPassTrace>,
) {
    trace.0.push("produce");
    commands.spawn(ConfiguredPassDeferredEntity);
}

fn observe_deferred_entity(
    _frame: ImguiFrame<'_>,
    entities: Query<Entity, With<ConfiguredPassDeferredEntity>>,
    mut commands: Commands,
    mut trace: ResMut<ConfiguredPassTrace>,
) {
    assert_eq!(entities.iter().count(), 1);
    for entity in &entities {
        commands.entity(entity).despawn();
    }
    trace.0.push("observe");
}

fn finish_configured_pass(_frame: ImguiFrame<'_>, mut trace: ResMut<ConfiguredPassTrace>) {
    trace.0.push("final");
}

fn record_dynamically_registered_pass(
    _frame: ImguiFrame<'_>,
    mut trace: ResMut<ConfiguredPassTrace>,
) {
    trace.0.push("dynamic");
}

fn trace_update(mut trace: ResMut<MainScheduleTrace>) {
    trace.0.push("update");
}

fn trace_pre_update(mut trace: ResMut<MainScheduleTrace>) {
    trace.0.push("pre_update");
}

fn trace_post_update(mut trace: ResMut<MainScheduleTrace>) {
    trace.0.push("post_update");
}

fn trace_last(mut trace: ResMut<MainScheduleTrace>) {
    trace.0.push("last");
}

fn trace_imgui(_frame: ImguiFrame<'_>, mut trace: ResMut<MainScheduleTrace>) {
    trace.0.push("imgui");
}

#[cfg(feature = "render")]
fn force_input_route_epoch_mismatch(
    routes: Res<crate::route::ImguiResolvedRoutes>,
    mut input_metrics: ResMut<crate::input::ImguiContextInputMetrics>,
) {
    input_metrics.set_epoch_for_test(routes.epoch().wrapping_add(1));
}

#[test]
fn private_context_pass_does_not_collide_with_update() {
    let _guard = imgui_context_guard();
    let mut app = app_with_primary_window();
    let pass = app.declare_imgui_pass::<ContextPassA>();
    app.init_resource::<ScheduleNamespaceTrace>()
        .add_systems(Update, record_update_schedule)
        .add_plugins(ImguiPlugin::default());
    app.add_imgui_systems(&pass, pass.system(record_update_named_ui_pass));

    app.world_mut()
        .non_send_mut::<ImguiContexts>()
        .create(ImguiContextConfig::new(&pass))
        .unwrap();

    app.update();

    let trace = app.world().resource::<ScheduleNamespaceTrace>();
    assert_eq!(trace.update_runs, 1);
    assert_eq!(trace.ui_runs, 1);
}

#[test]
fn public_schedule_can_nest_without_obtaining_another_context_frame() {
    let _guard = imgui_context_guard();
    let mut app = app_with_primary_window();
    let pass_a = app.declare_imgui_pass::<ContextPassA>();
    let pass_b = app.declare_imgui_pass::<ContextPassB>();
    app.init_resource::<NestedScheduleTrace>()
        .add_systems(Update, record_nested_update)
        .add_plugins(ImguiPlugin::default());
    app.add_imgui_systems(&pass_a, pass_a.system(attempt_nested_schedules));
    app.add_imgui_systems(&pass_b, pass_b.system(record_nested_context_b));

    let context_b = {
        let mut contexts = app.world_mut().non_send_mut::<ImguiContexts>();
        contexts.create(ImguiContextConfig::new(&pass_a)).unwrap();
        contexts.create(ImguiContextConfig::new(&pass_b)).unwrap()
    };

    app.update();

    let trace = app.world().resource::<NestedScheduleTrace>();
    assert_eq!(trace.pass_a_runs, 1);
    assert_eq!(trace.pass_b_runs, 1);
    assert_eq!(trace.update_runs, 2);
    assert!(trace.nested_update_ran);
    assert_eq!(trace.pass_b_contexts, [context_b]);
}

#[test]
fn dynamic_public_schedule_registration_does_not_expose_imgui_frame() {
    let _guard = imgui_context_guard();
    let mut app = app_with_primary_window();
    app.init_resource::<DynamicScheduleTrace>()
        .add_plugins(ImguiPlugin::default());
    let primary = app.imgui_primary_pass();
    app.add_imgui_systems(&primary, primary.system(register_and_run_dynamic_schedule));

    app.update();

    assert_eq!(app.world().resource::<DynamicScheduleTrace>().0, 1);
    assert!(
        app.world()
            .resource::<Schedules>()
            .contains(DynamicallyRegisteredPass)
    );
}

#[test]
fn private_pass_preserves_commands_local_state_and_change_detection() {
    let _guard = imgui_context_guard();
    let mut app = app_with_primary_window();
    app.init_resource::<PassSystemSemantics>()
        .init_resource::<TrackedPassResource>()
        .add_plugins(ImguiPlugin::default());
    let primary = app.imgui_primary_pass();
    app.add_imgui_systems(&primary, primary.system(exercise_pass_system_semantics));

    app.update();
    app.update();
    app.world_mut()
        .resource_mut::<TrackedPassResource>()
        .set_changed();
    app.update();

    let semantics = app.world().resource::<PassSystemSemantics>();
    assert_eq!(semantics.local_values, [1, 2, 3]);
    assert_eq!(semantics.change_flags, [true, false, true]);
    let mut query = app.world_mut().query::<&DeferredPassEntity>();
    let mut deferred_frames = query
        .iter(app.world())
        .map(|entity| entity.0)
        .collect::<Vec<_>>();
    deferred_frames.sort_unstable();
    assert_eq!(deferred_frames, [1, 2, 3]);
}

#[test]
fn private_pass_preserves_bevy_system_configs_and_intermediate_deferred_barriers() {
    let _guard = imgui_context_guard();
    let mut app = app_with_primary_window();
    app.init_resource::<ConfiguredPassTrace>()
        .add_plugins(ImguiPlugin::default());
    let pass = app.imgui_primary_pass();
    app.configure_imgui_sets(
        &pass,
        (
            ConfiguredPassSet::Produce,
            ConfiguredPassSet::Observe,
            ConfiguredPassSet::Final,
        ),
    );
    app.add_imgui_systems(
        &pass,
        (
            pass.system(produce_deferred_entity)
                .in_set(ConfiguredPassSet::Produce),
            pass.system(observe_deferred_entity)
                .in_set(ConfiguredPassSet::Observe),
        )
            .chain()
            .run_if(|| true)
            .before(ConfiguredPassSet::Final),
    );
    app.add_imgui_systems(
        &pass,
        pass.system(finish_configured_pass)
            .in_set(ConfiguredPassSet::Final)
            .after(ConfiguredPassSet::Observe),
    );

    app.update();

    assert_eq!(
        app.world().resource::<ConfiguredPassTrace>().0,
        ["produce", "observe", "final"]
    );

    app.add_imgui_systems(
        &pass,
        pass.system(record_dynamically_registered_pass)
            .after(ConfiguredPassSet::Final),
    );
    app.update();

    assert_eq!(
        app.world().resource::<ConfiguredPassTrace>().0,
        [
            "produce", "observe", "final", "produce", "observe", "final", "dynamic"
        ]
    );
}

#[test]
fn driver_defaults_after_pre_update_and_supports_an_explicit_main_schedule_anchor() {
    let _guard = imgui_context_guard();
    let mut default_app = app_with_primary_window();
    default_app
        .init_resource::<MainScheduleTrace>()
        .add_systems(PreUpdate, trace_pre_update)
        .add_systems(Update, trace_update)
        .add_systems(PostUpdate, trace_post_update)
        .add_systems(Last, trace_last)
        .add_plugins(ImguiPlugin::default());
    let default_pass = default_app.imgui_primary_pass();
    default_app.add_imgui_systems(&default_pass, default_pass.system(trace_imgui));

    default_app.update();

    assert_eq!(
        default_app.world().resource::<MainScheduleTrace>().0,
        ["pre_update", "imgui", "update", "post_update", "last"]
    );

    let mut anchored_app = app_with_primary_window();
    anchored_app
        .init_resource::<MainScheduleTrace>()
        .add_systems(Update, trace_update)
        .add_plugins(ImguiPlugin::new(
            crate::ImguiPluginConfig::default().with_driver_after(Update),
        ));
    let anchored_pass = anchored_app.imgui_primary_pass();
    anchored_app.add_imgui_systems(&anchored_pass, anchored_pass.system(trace_imgui));

    anchored_app.update();

    assert_eq!(
        anchored_app.world().resource::<MainScheduleTrace>().0,
        ["update", "imgui"]
    );
}

#[cfg(feature = "render")]
#[test]
fn driver_fails_the_complete_frame_closed_when_input_and_render_epochs_differ() {
    let _guard = imgui_context_guard();
    let mut app = app_with_primary_window();
    let additional_pass = app.declare_imgui_pass::<ContextPassA>();
    app.init_resource::<LifecycleTrace>()
        .add_plugins(ImguiPlugin::default())
        .add_systems(
            PreUpdate,
            force_input_route_epoch_mismatch.after(crate::input::ImguiInputSystems),
        );
    let primary_pass = app.imgui_primary_pass();
    app.add_imgui_systems(
        &primary_pass,
        primary_pass.system(record_ui::<ImguiPrimaryPass>),
    );
    app.add_imgui_systems(
        &additional_pass,
        additional_pass.system(record_ui::<ContextPassA>),
    );

    let (primary, additional) = {
        let mut contexts = app.world_mut().non_send_mut::<ImguiContexts>();
        let primary = contexts.primary_id().unwrap();
        let additional = contexts
            .create(ImguiContextConfig::new(&additional_pass))
            .unwrap();
        (primary, additional)
    };

    app.update();

    let contexts = app.world().non_send_resource::<ImguiContexts>();
    assert_eq!(contexts.frame_index(primary).unwrap(), 0);
    assert_eq!(contexts.frame_index(additional).unwrap(), 0);
    assert!(app.world().resource::<LifecycleTrace>().visits.is_empty());
    assert_eq!(
        app.world()
            .resource::<crate::context::ImguiFrameMailbox>()
            .len(),
        0,
        "an epoch mismatch must not publish a partial Context frame"
    );
}

#[test]
fn explicit_headless_shutdown_is_terminal_and_idempotent() {
    let _guard = imgui_context_guard();
    let mut app = app_with_primary_window();
    app.add_plugins(ImguiPlugin::default());

    app.shutdown_imgui().unwrap();

    assert!(app.world().get_non_send::<ImguiContexts>().is_none());
    app.shutdown_imgui().unwrap();
}

#[test]
fn terminal_shutdown_invalidates_retained_registries_and_rejects_app_scoped_readmission() {
    let _guard = imgui_context_guard();
    let mut app = app_with_primary_window();
    app.add_plugins(ImguiPlugin::default());
    let primary = app
        .world()
        .get_non_send::<ImguiContexts>()
        .unwrap()
        .primary_id()
        .unwrap();
    let mut retained = app.world_mut().remove_non_send::<ImguiContexts>().unwrap();

    app.shutdown_imgui().unwrap();

    assert_eq!(retained.primary_id(), None);
    assert_eq!(retained.ids().len(), 0);
    assert!(matches!(
        retained.configure(primary, |_| ()),
        Err(ImguiContextError::AppTerminated)
    ));
    let rejected = dear_imgui_rs::SuspendedContext::create();
    let rejected_id = rejected.id();
    let error = match app.adopt_imgui_primary_context(rejected) {
        Err(error) => error,
        Ok(_) => panic!("a terminal App must reject a replacement registry"),
    };
    assert!(matches!(error.error(), ImguiContextError::AppTerminated));
    assert_eq!(error.into_context().id(), rejected_id);

    app.insert_non_send(retained);
    app.shutdown_imgui()
        .expect("terminal retry must still retire a retained inert registry");
}

#[test]
fn shutdown_before_plugin_installation_still_closes_context_admission() {
    let _guard = imgui_context_guard();
    let mut app = app_with_primary_window();

    app.shutdown_imgui().unwrap();

    let rejected = dear_imgui_rs::SuspendedContext::create();
    let rejected_id = rejected.id();
    let error = match app.adopt_imgui_primary_context(rejected) {
        Err(error) => error,
        Ok(_) => panic!("explicit shutdown must permanently close Context admission"),
    };
    assert!(matches!(error.error(), ImguiContextError::AppTerminated));
    assert_eq!(error.into_context().id(), rejected_id);
}

#[test]
fn removing_the_registry_does_not_open_a_second_context_admission() {
    let _guard = imgui_context_guard();
    let mut app = app_with_primary_window();
    app.add_plugins(ImguiPlugin::default());
    let retained = app.world_mut().remove_non_send::<ImguiContexts>().unwrap();

    let rejected = dear_imgui_rs::SuspendedContext::create();
    let rejected_id = rejected.id();
    let error = match app.adopt_imgui_primary_context(rejected) {
        Err(error) => error,
        Ok(_) => panic!("an App must never own two Context registries"),
    };
    assert!(matches!(
        error.error(),
        ImguiContextError::ContextRegistryAlreadyInstalled
    ));
    assert_eq!(error.into_context().id(), rejected_id);

    app.insert_non_send(retained);
    app.shutdown_imgui().unwrap();
}

#[test]
fn primary_promotion_and_replacement_are_transactional() {
    let _guard = imgui_context_guard();
    let mut app = app_with_primary_window();
    let pass_a = app.declare_imgui_pass::<ContextPassA>();
    let pass_b = app.declare_imgui_pass::<ContextPassB>();
    app.add_plugins(ImguiPlugin::default());
    let (initial_primary, context_a) = {
        let mut contexts = app.world_mut().non_send_mut::<ImguiContexts>();
        let initial_primary = contexts.primary_id().unwrap();
        let context_a = contexts.create(ImguiContextConfig::new(&pass_a)).unwrap();
        (initial_primary, context_a)
    };

    let promoted = app
        .world_mut()
        .non_send_mut::<ImguiContexts>()
        .promote_primary(context_a)
        .unwrap();
    assert_eq!(promoted.previous(), Some(initial_primary));
    assert_eq!(promoted.current(), context_a);

    let duplicate = dear_imgui_rs::SuspendedContext::create();
    let duplicate_id = duplicate.id();
    let duplicate_error = app
        .world_mut()
        .non_send_mut::<ImguiContexts>()
        .replace_primary(duplicate, ImguiContextConfig::new(&pass_a))
        .expect_err("duplicate pass admission must leave the promoted primary unchanged");
    assert!(matches!(
        duplicate_error.error(),
        ImguiContextError::DuplicatePass { owner, .. } if *owner == context_a
    ));
    assert_eq!(duplicate_error.into_context().id(), duplicate_id);
    assert_eq!(
        app.world().non_send::<ImguiContexts>().primary_id(),
        Some(context_a)
    );

    let replacement = dear_imgui_rs::SuspendedContext::create();
    let replacement_id = replacement.id();
    let replaced = app
        .world_mut()
        .non_send_mut::<ImguiContexts>()
        .replace_primary(replacement, ImguiContextConfig::new(&pass_b))
        .unwrap();
    assert_eq!(replaced.previous(), Some(context_a));
    assert_eq!(replaced.current(), replacement_id);
    let contexts = app.world().non_send::<ImguiContexts>();
    assert_eq!(contexts.primary_id(), Some(replacement_id));
    assert_eq!(
        contexts.ids().collect::<Vec<_>>(),
        [initial_primary, context_a, replacement_id]
    );
}

#[test]
fn primary_and_two_additional_contexts_run_in_stable_order_with_independent_frames() {
    let _guard = imgui_context_guard();
    let mut app = app_with_primary_window();
    let pass_a = app.declare_imgui_pass::<ContextPassA>();
    let pass_b = app.declare_imgui_pass::<ContextPassB>();
    app.init_resource::<LifecycleTrace>()
        .add_plugins(ImguiPlugin::default());
    let primary_pass = app.imgui_primary_pass();
    app.add_imgui_systems(
        &primary_pass,
        primary_pass.system(record_ui::<ImguiPrimaryPass>),
    );
    app.add_imgui_systems(&pass_a, pass_a.system(record_ui::<ContextPassA>));
    app.add_imgui_systems(&pass_b, pass_b.system(record_ui::<ContextPassB>));

    let (primary, context_a, context_b, expected_raw) = {
        let mut contexts = app.world_mut().get_non_send_mut::<ImguiContexts>().unwrap();
        let primary = contexts.primary_id().unwrap();
        let context_a = contexts.create(ImguiContextConfig::new(&pass_a)).unwrap();
        let context_b = contexts.create(ImguiContextConfig::new(&pass_b)).unwrap();
        let mut expected_raw = Vec::new();
        for context_id in [primary, context_a, context_b, primary] {
            let raw = contexts
                .configure(context_id, |context| {
                    let raw = context.as_raw();
                    assert_eq!(
                        unsafe { dear_imgui_rs::sys::igGetCurrentContext() },
                        raw,
                        "configuration must activate exactly the requested Context"
                    );
                    raw as usize
                })
                .unwrap();
            if !expected_raw
                .iter()
                .any(|(expected_id, _)| *expected_id == context_id)
            {
                expected_raw.push((context_id, raw));
            }
        }
        (primary, context_a, context_b, expected_raw)
    };
    {
        let mut trace = app.world_mut().resource_mut::<LifecycleTrace>();
        trace.expected = vec![primary, context_a, context_b];
        trace.expected_raw = expected_raw;
    }

    // Drive two frames to verify that Context order stays stable across frame boundaries.
    for _ in 0..2 {
        app.update();
    }

    let trace = app.world().resource::<LifecycleTrace>();
    assert_eq!(
        trace.visits,
        vec![
            (primary, 1),
            (context_a, 1),
            (context_b, 1),
            (primary, 2),
            (context_a, 2),
            (context_b, 2),
        ]
    );
    let contexts = app.world().get_non_send::<ImguiContexts>().unwrap();
    assert_eq!(contexts.ids().collect::<Vec<_>>(), trace.expected);
    assert_eq!(contexts.frame_index(primary).unwrap(), 2);
    assert_eq!(contexts.frame_index(context_a).unwrap(), 2);
    assert_eq!(contexts.frame_index(context_b).unwrap(), 2);
    assert!(
        !trace.wrong_current_context,
        "every private pass must run with its own native Context current"
    );
}

#[test]
#[should_panic(expected = "is not bound to this exact pass")]
fn private_pass_rejects_a_system_bound_to_another_runtime_pass_during_registration() {
    let mut app = App::new();
    let first_pass = app.declare_imgui_pass::<ContextPassA>();
    let second_pass = app.declare_imgui_pass::<ContextPassA>();

    app.add_imgui_systems(&second_pass, first_pass.system(record_ui::<ContextPassA>));
}

#[test]
#[should_panic(expected = "is not bound to this exact pass")]
fn private_pass_rejects_a_system_bound_to_another_brand_during_registration() {
    let mut app = App::new();
    let pass_a = app.declare_imgui_pass::<ContextPassA>();
    let pass_b = app.declare_imgui_pass::<ContextPassB>();

    app.add_imgui_systems(&pass_b, pass_a.system(record_ui::<ContextPassA>));
}

#[test]
#[should_panic(expected = "is not bound to this exact pass")]
fn private_pass_rejects_an_unbound_bevy_system_during_registration() {
    let mut app = App::new();
    let pass = app.declare_imgui_pass::<ContextPassA>();

    app.add_imgui_systems(&pass, || {});
}

#[test]
fn distinct_passes_with_the_same_brand_drive_independent_contexts() {
    let _guard = imgui_context_guard();
    let mut app = app_with_primary_window();
    let first_pass = app.declare_imgui_pass::<ContextPassA>();
    let second_pass = app.declare_imgui_pass::<ContextPassA>();
    app.init_resource::<LifecycleTrace>()
        .add_plugins(ImguiPlugin::default());
    app.add_imgui_systems(&first_pass, first_pass.system(record_ui::<ContextPassA>));
    app.add_imgui_systems(&second_pass, second_pass.system(record_ui::<ContextPassA>));

    let (first, second) = {
        let mut contexts = app.world_mut().non_send_mut::<ImguiContexts>();
        let first = contexts
            .create(ImguiContextConfig::new(&first_pass))
            .unwrap();
        let second = contexts
            .create(ImguiContextConfig::new(&second_pass))
            .unwrap();
        (first, second)
    };

    app.update();

    assert_eq!(
        app.world().resource::<LifecycleTrace>().visits,
        [(first, 1), (second, 1)]
    );
}

#[test]
fn primary_pass_receives_bevy_time_and_logical_window_metrics() {
    let _guard = imgui_context_guard();
    let mut app = App::new();
    let mut window = Window {
        resolution: WindowResolution::new(1280, 720),
        ..Default::default()
    };
    window.resolution.set_scale_factor(2.0);
    app.world_mut().spawn((window, PrimaryWindow));
    let mut real_time = Time::<Real>::default();
    real_time.advance_by(Duration::from_millis(42));
    app.insert_resource(real_time)
        .init_resource::<LifecycleTrace>()
        .add_plugins(ImguiPlugin::default());
    let primary = app.imgui_primary_pass();
    app.add_imgui_systems(&primary, primary.system(capture_primary_metrics));

    app.update();

    let trace = app.world().resource::<LifecycleTrace>();
    assert_eq!(trace.delta_times.len(), 1);
    assert!((trace.delta_times[0] - 0.042).abs() < f32::EPSILON);
    assert_eq!(trace.display_metrics, [([640.0, 360.0], [2.0, 2.0])]);
}

#[test]
fn primary_pass_sanitizes_invalid_window_metrics_before_begin_frame() {
    let _guard = imgui_context_guard();
    let mut app = App::new();
    let mut window = Window::default();
    window.resolution.set(f32::NAN, -10.0);
    window.resolution.set_scale_factor(f32::NAN);
    app.world_mut().spawn((window, PrimaryWindow));
    app.init_resource::<LifecycleTrace>()
        .add_plugins(ImguiPlugin::default());
    let primary = app.imgui_primary_pass();
    app.add_imgui_systems(&primary, primary.system(capture_primary_metrics));

    app.update();

    assert_eq!(
        app.world().resource::<LifecycleTrace>().display_metrics,
        [([1.0, 1.0], [1.0, 1.0])]
    );
}

#[test]
fn any_live_ui_blocks_raw_mutation_of_every_registered_context() {
    let _guard = imgui_context_guard();
    let mut app = app_with_primary_window();
    let pass_a = app.declare_imgui_pass::<ContextPassA>();
    let missing_pass = app.declare_imgui_pass::<MissingContextPass>();
    app.insert_resource(missing_pass);
    app.init_resource::<LifecycleTrace>()
        .add_plugins(ImguiPlugin::default());
    let primary_pass = app.imgui_primary_pass();
    app.add_imgui_systems(
        &primary_pass,
        primary_pass.system(reject_raw_mutation_during_ui),
    );
    app.add_imgui_systems(&pass_a, pass_a.system(record_ui::<ContextPassA>));

    let (primary, context_a) = {
        let mut contexts = app.world_mut().get_non_send_mut::<ImguiContexts>().unwrap();
        let primary = contexts.primary_id().unwrap();
        let context_a = contexts.create(ImguiContextConfig::new(&pass_a)).unwrap();
        (primary, context_a)
    };
    app.world_mut().resource_mut::<LifecycleTrace>().expected = vec![primary, context_a];

    app.update();
    assert!(app.world().resource::<LifecycleTrace>().mutation_rejected);

    app.world_mut()
        .get_non_send_mut::<ImguiContexts>()
        .unwrap()
        .configure(context_a, |context| {
            context.io_mut().set_delta_time(1.0 / 120.0);
        })
        .expect("configuration must become available after the private pass");
}

#[test]
fn duplicate_pass_and_stale_context_errors_are_typed_and_recover_ownership() {
    let _guard = imgui_context_guard();
    let mut app = App::new();
    let primary_pass = app.imgui_primary_pass();
    let pass_a = app.declare_imgui_pass::<ContextPassA>();
    let lifecycle = crate::context::pass::lifecycle(app.world());
    let mut contexts = ImguiContexts::with_primary(
        dear_imgui_rs::SuspendedContext::create(),
        primary_pass,
        lifecycle,
    );
    let context_a = contexts.create(ImguiContextConfig::new(&pass_a)).unwrap();
    let rejected = dear_imgui_rs::SuspendedContext::create();
    let rejected_id = rejected.id();

    let error = contexts
        .insert_suspended(rejected, ImguiContextConfig::new(&pass_a))
        .expect_err("duplicate pass ownership must be rejected");
    assert!(matches!(
        error.error(),
        ImguiContextError::DuplicatePass { owner, .. } if *owner == context_a
    ));
    let rejected = error.into_context();
    assert_eq!(rejected.id(), rejected_id);

    let removed = contexts.remove(context_a).unwrap();
    assert_eq!(removed.id(), context_a);
    assert!(matches!(
        contexts.configure(context_a, |_| ()),
        Err(ImguiContextError::UnknownContext { context_id }) if context_id == context_a
    ));
}

#[test]
fn pass_from_another_app_is_rejected_without_consuming_the_context() {
    let _guard = imgui_context_guard();
    let mut foreign_app = App::new();
    let foreign_pass = foreign_app.declare_imgui_pass::<ContextPassA>();
    let mut owner_app = App::new();
    let primary_pass = owner_app.imgui_primary_pass();
    let lifecycle = crate::context::pass::lifecycle(owner_app.world());
    let mut contexts = ImguiContexts::with_primary(
        dear_imgui_rs::SuspendedContext::create(),
        primary_pass,
        lifecycle,
    );
    let rejected = dear_imgui_rs::SuspendedContext::create();
    let rejected_id = rejected.id();

    let error = contexts
        .insert_suspended(rejected, ImguiContextConfig::new(&foreign_pass))
        .expect_err("a pass declared by another App must be rejected");

    assert!(matches!(
        error.error(),
        ImguiContextError::ForeignPass { .. }
    ));
    assert_eq!(error.into_context().id(), rejected_id);
    assert!(!contexts.contains(rejected_id));
}

#[test]
fn additional_multi_viewport_config_can_be_registered_before_backend_attachment() {
    let _guard = imgui_context_guard();
    let mut app = App::new();
    let primary_pass = app.imgui_primary_pass();
    let pass_a = app.declare_imgui_pass::<ContextPassA>();
    let lifecycle = crate::context::pass::lifecycle(app.world());
    let mut contexts = ImguiContexts::with_primary(
        dear_imgui_rs::SuspendedContext::create(),
        primary_pass,
        lifecycle,
    );
    let additional = dear_imgui_rs::SuspendedContext::create();
    let additional_id = additional.id();

    let admitted = contexts
        .insert_suspended(
            additional,
            ImguiContextConfig::new(&pass_a).with_multi_viewport(true),
        )
        .expect("multi-viewport configuration should be retained until backend attachment");

    assert_eq!(admitted, additional_id);
    assert!(contexts.contains(additional_id));
    assert_eq!(
        contexts
            .remove(additional_id)
            .expect("an unattached Context should remain removable")
            .id(),
        additional_id
    );
}

#[cfg(not(all(feature = "multi-viewport", not(target_arch = "wasm32"))))]
#[test]
fn attached_backend_rejects_unavailable_native_multi_viewport_without_consuming_context() {
    let _guard = imgui_context_guard();
    let mut app = app_with_primary_window();
    app.add_plugins(ImguiPlugin::default());
    let pass_a = app.declare_imgui_pass::<ContextPassA>();
    let additional = dear_imgui_rs::SuspendedContext::create();
    let additional_id = additional.id();

    let error = app
        .world_mut()
        .non_send_mut::<ImguiContexts>()
        .insert_suspended(
            additional,
            ImguiContextConfig::new(&pass_a).with_multi_viewport(true),
        )
        .expect_err("a build without native viewport support must reject the request");

    assert!(matches!(
        error.error(),
        ImguiContextError::NativeMultiViewportUnavailable { context_id }
            if *context_id == additional_id
    ));
    assert_eq!(error.into_context().id(), additional_id);
    assert!(
        !app.world()
            .non_send::<ImguiContexts>()
            .contains(additional_id)
    );
}

#[test]
fn empty_pass_is_context_local_and_does_not_stop_later_contexts() {
    let _guard = imgui_context_guard();
    let mut app = app_with_primary_window();
    let empty_pass = app.declare_imgui_pass::<MissingContextPass>();
    let healthy_pass = app.declare_imgui_pass::<ContextPassB>();
    app.init_resource::<LifecycleTrace>()
        .add_plugins(ImguiPlugin::default());
    let primary_pass = app.imgui_primary_pass();
    app.add_imgui_systems(
        &primary_pass,
        primary_pass.system(record_ui::<ImguiPrimaryPass>),
    );
    app.add_imgui_systems(
        &healthy_pass,
        healthy_pass.system(record_ui::<ContextPassB>),
    );

    let (primary, missing, healthy) = {
        let mut contexts = app.world_mut().get_non_send_mut::<ImguiContexts>().unwrap();
        let primary = contexts.primary_id().unwrap();
        let missing = contexts
            .create(ImguiContextConfig::new(&empty_pass))
            .unwrap();
        let healthy = contexts
            .create(ImguiContextConfig::new(&healthy_pass))
            .unwrap();
        (primary, missing, healthy)
    };
    app.world_mut().resource_mut::<LifecycleTrace>().expected = vec![primary, missing, healthy];

    app.update();

    let contexts = app.world().get_non_send::<ImguiContexts>().unwrap();
    assert!(contexts.last_error(missing).unwrap().is_none());
    assert_eq!(contexts.frame_index(missing).unwrap(), 1);
    assert_eq!(contexts.frame_index(healthy).unwrap(), 1);
    assert!(
        app.world()
            .resource::<LifecycleTrace>()
            .visits
            .contains(&(healthy, 1))
    );
}

#[derive(Resource, Default)]
struct PanicOnce(bool);

fn panic_once(_frame: ImguiFrame<'_>, mut state: ResMut<PanicOnce>) {
    if !state.0 {
        state.0 = true;
        panic!("intentional Context pass panic");
    }
}

#[test]
fn pass_panic_reinserts_the_private_runner_and_leaves_no_context_active() {
    let _guard = imgui_context_guard();

    let mut app = app_with_primary_window();
    app.init_resource::<PanicOnce>()
        .init_resource::<LifecycleTrace>()
        .add_plugins(ImguiPlugin::default());
    let primary_pass = app.imgui_primary_pass();
    app.add_imgui_systems(&primary_pass, primary_pass.system(panic_once));
    let primary = app
        .world()
        .get_non_send::<ImguiContexts>()
        .unwrap()
        .primary_id()
        .unwrap();

    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        crate::context::drive_imgui_contexts(app.world_mut());
    }));
    assert!(panic.is_err());
    assert_eq!(
        unsafe { dear_imgui_rs::sys::igGetCurrentContext() },
        std::ptr::null_mut(),
        "the suspended Context must not remain active after unwinding"
    );
    {
        let contexts = app.world().get_non_send::<ImguiContexts>().unwrap();
        assert!(contexts.contains(primary));
        assert_eq!(contexts.frame_index(primary).unwrap(), 0);
    }
    crate::context::drive_imgui_contexts(app.world_mut());
    assert_eq!(
        app.world()
            .get_non_send::<ImguiContexts>()
            .unwrap()
            .frame_index(primary)
            .unwrap(),
        1,
        "the private pass runner must survive unwinding"
    );
    assert_eq!(
        unsafe { dear_imgui_rs::sys::igGetCurrentContext() },
        std::ptr::null_mut(),
        "the next successful frame must suspend its Context"
    );
    drop(app);
    assert_eq!(
        unsafe { dear_imgui_rs::sys::igGetCurrentContext() },
        std::ptr::null_mut(),
        "dropping the App must not install a Context"
    );
}

#[test]
fn primary_without_a_window_does_not_advance_or_replay_a_frame() {
    let _guard = imgui_context_guard();
    let mut app = App::new();
    app.init_resource::<LifecycleTrace>()
        .add_plugins(ImguiPlugin::default());
    let primary_pass = app.imgui_primary_pass();
    app.add_imgui_systems(
        &primary_pass,
        primary_pass.system(record_ui::<ImguiPrimaryPass>),
    );
    let primary = app
        .world()
        .get_non_send::<ImguiContexts>()
        .unwrap()
        .primary_id()
        .unwrap();

    app.update();

    assert_eq!(
        app.world()
            .get_non_send::<ImguiContexts>()
            .unwrap()
            .frame_index(primary)
            .unwrap(),
        0
    );
    assert!(app.world().resource::<LifecycleTrace>().visits.is_empty());
}

#[test]
fn removing_the_primary_context_does_not_stop_an_additional_context() {
    let _guard = imgui_context_guard();
    let mut app = app_with_primary_window();
    let pass_a = app.declare_imgui_pass::<ContextPassA>();
    app.init_resource::<LifecycleTrace>()
        .add_plugins(ImguiPlugin::default());
    app.add_imgui_systems(&pass_a, pass_a.system(record_ui::<ContextPassA>));

    let (primary, additional) = {
        let mut contexts = app.world_mut().get_non_send_mut::<ImguiContexts>().unwrap();
        let primary = contexts.primary_id().unwrap();
        let additional = contexts.create(ImguiContextConfig::new(&pass_a)).unwrap();
        (primary, additional)
    };
    app.world_mut().resource_mut::<LifecycleTrace>().expected = vec![additional];
    app.world_mut()
        .get_non_send_mut::<ImguiContexts>()
        .unwrap()
        .remove(primary)
        .expect("a primary Context without render-world work should detach immediately");

    app.update();

    let contexts = app.world().get_non_send::<ImguiContexts>().unwrap();
    assert_eq!(contexts.primary_id(), None);
    assert_eq!(contexts.frame_index(additional).unwrap(), 1);
    assert_eq!(
        app.world().resource::<LifecycleTrace>().visits,
        vec![(additional, 1)]
    );
}

#[cfg(feature = "render")]
#[test]
fn context_removal_abandons_unextracted_snapshot_without_pausing_another_context() {
    let _guard = imgui_context_guard();
    let mut app = app_with_primary_window();
    let pass_a = app.declare_imgui_pass::<ContextPassA>();
    app.add_plugins(ExtractPlugin::default())
        .init_resource::<LifecycleTrace>()
        .add_plugins(ImguiPlugin::default());
    let primary_pass = app.imgui_primary_pass();
    app.add_imgui_systems(
        &primary_pass,
        primary_pass.system(record_ui::<ImguiPrimaryPass>),
    );
    app.add_imgui_systems(&pass_a, pass_a.system(record_ui::<ContextPassA>));
    app.sub_app_mut(RenderApp).update_schedule = Some(Render.intern());

    let (context_a, context_b) = {
        let mut contexts = app.world_mut().get_non_send_mut::<ImguiContexts>().unwrap();
        let context_a = contexts.primary_id().unwrap();
        let context_b = contexts.create(ImguiContextConfig::new(&pass_a)).unwrap();
        (context_a, context_b)
    };
    app.world_mut().resource_mut::<LifecycleTrace>().expected = vec![context_a, context_b];

    app.world_mut().run_schedule(Main);

    {
        let mailbox = app
            .world()
            .resource::<crate::context::ImguiFrameMailbox>()
            .clone();
        let pending = mailbox.take_all();
        for context_id in [context_a, context_b] {
            let frame = pending
                .get(&context_id)
                .expect("both Contexts must publish their first render snapshot");
            assert_eq!(frame.frame_index, 1);
            assert_eq!(frame.snapshot.epoch().context_id(), context_id);
        }
        for (context_id, frame) in pending {
            mailbox.publish(context_id, frame);
        }
    }

    let removal = app
        .world_mut()
        .get_non_send_mut::<ImguiContexts>()
        .unwrap()
        .remove(context_a)
        .expect_err("the render world must acknowledge Context A before removal");
    assert!(matches!(
        removal,
        ImguiContextError::RemovalPending {
            context_id,
            reason: ImguiContextRemovalPendingReason::RenderWorldReleasePending,
        } if context_id == context_a
    ));
    let completion = app
        .world_mut()
        .get_non_send_mut::<ImguiContexts>()
        .unwrap()
        .configure(context_a, |context| context.poll_snapshot_completions())
        .expect("a teardown Context must remain configurable")
        .expect("abandoning the staged snapshot must complete cleanly");
    assert_eq!(completion.watermark(), 1);
    assert_eq!(completion.committed(), 0);
    assert_eq!(completion.abandoned(), 1);

    app.update();

    {
        let contexts = app.world().get_non_send::<ImguiContexts>().unwrap();
        assert_eq!(contexts.frame_index(context_a).unwrap(), 1);
        assert_eq!(contexts.frame_index(context_b).unwrap(), 2);
    }
    assert_eq!(
        app.world().resource::<LifecycleTrace>().visits,
        vec![(context_a, 1), (context_b, 1), (context_b, 2)],
        "Context A teardown must not reopen A or interrupt Context B"
    );
    let extracted = app
        .sub_app(RenderApp)
        .world()
        .resource::<crate::render::ImguiExtractedRenderFrame>();
    assert_eq!(extracted.frame_index(context_a), None);
    assert_eq!(extracted.frame_index(context_b), Some(2));

    let removed = app
        .world_mut()
        .get_non_send_mut::<ImguiContexts>()
        .unwrap()
        .remove(context_a)
        .expect("render-world acknowledgement must make Context A removable");
    assert_eq!(removed.id(), context_a);

    app.update();

    let contexts = app.world().get_non_send::<ImguiContexts>().unwrap();
    assert!(!contexts.contains(context_a));
    assert_eq!(contexts.frame_index(context_b).unwrap(), 3);
    let extracted = app
        .sub_app(RenderApp)
        .world()
        .resource::<crate::render::ImguiExtractedRenderFrame>();
    assert_eq!(extracted.frame_index(context_a), None);
    assert_eq!(extracted.frame_index(context_b), Some(3));
    assert_eq!(
        app.world().resource::<LifecycleTrace>().visits,
        vec![
            (context_a, 1),
            (context_b, 1),
            (context_b, 2),
            (context_b, 3),
        ]
    );
}

#[cfg(feature = "render")]
#[test]
fn removed_registry_retires_a_context_with_an_unextracted_snapshot() {
    let _guard = imgui_context_guard();
    let mut app = app_with_primary_window();
    app.add_plugins(ExtractPlugin::default())
        .init_resource::<LifecycleTrace>()
        .add_plugins(ImguiPlugin::default());
    let primary_pass = app.imgui_primary_pass();
    app.add_imgui_systems(
        &primary_pass,
        primary_pass.system(record_ui::<ImguiPrimaryPass>),
    );
    app.sub_app_mut(RenderApp).update_schedule = Some(Render.intern());

    let context_id = app
        .world()
        .get_non_send::<ImguiContexts>()
        .unwrap()
        .primary_id()
        .unwrap();
    let destroyed = Rc::new(Cell::new(false));
    app.world_mut()
        .get_non_send_mut::<ImguiContexts>()
        .unwrap()
        .configure(context_id, |context| {
            context
                .register_attachment::<RetirementProbeMarker>(
                    dear_imgui_rs::ContextAttachmentRole::Extension,
                    Rc::new(RetirementProbe {
                        destroyed: Rc::clone(&destroyed),
                    }),
                )
                .expect("the retirement probe must attach")
                .defer_to_context();
        })
        .unwrap();
    app.world_mut().resource_mut::<LifecycleTrace>().expected = vec![context_id];

    app.world_mut().run_schedule(Main);
    assert_eq!(
        app.world()
            .resource::<crate::context::ImguiFrameMailbox>()
            .len(),
        1,
        "the registry must own a snapshot that has not reached extraction"
    );

    let contexts = app
        .world_mut()
        .remove_non_send::<ImguiContexts>()
        .expect("the plugin must install its Context registry");
    drop(contexts);
    assert!(
        !destroyed.get(),
        "dropping the registry must transfer ownership instead of destroying the Context"
    );

    app.update();
    assert!(
        !destroyed.get(),
        "the Context must remain alive until the render world acknowledges release"
    );

    app.update();
    assert!(
        destroyed.get(),
        "the next main-world update must finish the acknowledged retirement"
    );
}
