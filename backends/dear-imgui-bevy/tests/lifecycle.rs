use std::sync::{Mutex, OnceLock};

use bevy_app::{App, Update};
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::{ScheduleLabel, Schedules};
use bevy_time::{Real, Time};
use bevy_window::{PrimaryWindow, Window, WindowResolution};
use dear_imgui_bevy::{
    ContextId, ImguiContextConfig, ImguiContextError, ImguiContexts, ImguiPlugin,
    ImguiPrimaryContextPass, ImguiUi,
};
use std::time::Duration;

fn imgui_context_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
struct ContextPassA;

#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
struct ContextPassB;

#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
struct MissingContextPass;

#[derive(Resource, Default)]
struct LifecycleTrace {
    expected: Vec<ContextId>,
    visits: Vec<(ContextId, u64)>,
    wrong_schedule: bool,
    outside_frame: bool,
    mutation_rejected: bool,
    expected_raw: Vec<(ContextId, usize)>,
    wrong_current_context: bool,
    delta_times: Vec<f32>,
    display_metrics: Vec<([f32; 2], [f32; 2])>,
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

fn record_ui(ui: ImguiUi, mut trace: ResMut<LifecycleTrace>) {
    let context_id = ui
        .context_id()
        .expect("UI schedule must expose its Context");
    let frame_index = ui.frame_index().expect("UI schedule must expose its frame");
    let current_ui = ui.ui().expect("UI schedule must expose a live Ui");
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

fn capture_primary_metrics(ui: ImguiUi, mut trace: ResMut<LifecycleTrace>) {
    let ui = ui.ui().expect("primary metrics require a live Ui");
    trace.delta_times.push(ui.io().delta_time());
    trace
        .display_metrics
        .push((ui.io().display_size(), ui.io().display_framebuffer_scale()));
}

fn reject_cross_context_access(ui: ImguiUi, mut trace: ResMut<LifecycleTrace>) {
    let active = ui.context_id().expect("Context A must be active");
    let other = trace
        .expected
        .iter()
        .copied()
        .find(|context_id| *context_id != active)
        .expect("the test installs multiple Contexts");
    trace.wrong_schedule = matches!(
        ui.ui_for(other),
        Err(ImguiContextError::WrongSchedule {
            requested,
            active: actual,
            ..
        }) if requested == other && actual == active
    );
}

fn reject_raw_mutation_during_ui(
    mut contexts: NonSendMut<ImguiContexts>,
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
        contexts.create(ImguiContextConfig::new(MissingContextPass)),
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

fn observe_ui_outside_context_schedule(ui: ImguiUi, mut trace: ResMut<LifecycleTrace>) {
    trace.outside_frame = matches!(ui.ui(), Err(ImguiContextError::NoOpenFrame));
}

#[test]
fn primary_and_two_additional_contexts_run_in_stable_order_with_independent_frames() {
    let _guard = imgui_context_guard();
    let mut app = app_with_primary_window();
    app.init_resource::<LifecycleTrace>()
        .add_systems(ImguiPrimaryContextPass, record_ui)
        .add_systems(ContextPassA, record_ui)
        .add_systems(ContextPassB, record_ui)
        .add_plugins(ImguiPlugin::default());

    let (primary, context_a, context_b, expected_raw) = {
        let mut contexts = app.world_mut().get_non_send_mut::<ImguiContexts>().unwrap();
        let primary = contexts.primary_id().unwrap();
        let context_a = contexts
            .create(ImguiContextConfig::new(ContextPassA))
            .unwrap();
        let context_b = contexts
            .create(ImguiContextConfig::new(ContextPassB))
            .unwrap();
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

    app.update();
    app.update();

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
        "every UI schedule must run with its own native Context current"
    );
}

#[test]
fn primary_schedule_receives_bevy_time_and_logical_window_metrics() {
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
        .add_systems(ImguiPrimaryContextPass, capture_primary_metrics)
        .add_plugins(ImguiPlugin::default());

    app.update();

    let trace = app.world().resource::<LifecycleTrace>();
    assert_eq!(trace.delta_times.len(), 1);
    assert!((trace.delta_times[0] - 0.042).abs() < f32::EPSILON);
    assert_eq!(trace.display_metrics, [([640.0, 360.0], [2.0, 2.0])]);
}

#[test]
fn primary_schedule_sanitizes_invalid_window_metrics_before_begin_frame() {
    let _guard = imgui_context_guard();
    let mut app = App::new();
    let mut window = Window::default();
    window.resolution.set(f32::NAN, -10.0);
    window.resolution.set_scale_factor(f32::NAN);
    app.world_mut().spawn((window, PrimaryWindow));
    app.init_resource::<LifecycleTrace>()
        .add_systems(ImguiPrimaryContextPass, capture_primary_metrics)
        .add_plugins(ImguiPlugin::default());

    app.update();

    assert_eq!(
        app.world().resource::<LifecycleTrace>().display_metrics,
        [([1.0, 1.0], [1.0, 1.0])]
    );
}

#[test]
fn ui_access_reports_wrong_schedule_and_is_revoked_outside_the_context_pass() {
    let _guard = imgui_context_guard();
    let mut app = app_with_primary_window();
    app.init_resource::<LifecycleTrace>()
        .add_systems(ImguiPrimaryContextPass, reject_cross_context_access)
        .add_systems(ContextPassA, record_ui)
        .add_systems(Update, observe_ui_outside_context_schedule)
        .add_plugins(ImguiPlugin::default());

    let (primary, context_a) = {
        let mut contexts = app.world_mut().get_non_send_mut::<ImguiContexts>().unwrap();
        let primary = contexts.primary_id().unwrap();
        let context_a = contexts
            .create(ImguiContextConfig::new(ContextPassA))
            .unwrap();
        (primary, context_a)
    };
    app.world_mut().resource_mut::<LifecycleTrace>().expected = vec![primary, context_a];

    app.update();

    let trace = app.world().resource::<LifecycleTrace>();
    assert!(trace.wrong_schedule);
    assert!(trace.outside_frame);
}

#[test]
fn any_live_ui_blocks_raw_mutation_of_every_registered_context() {
    let _guard = imgui_context_guard();
    let mut app = app_with_primary_window();
    app.init_resource::<LifecycleTrace>()
        .add_systems(ImguiPrimaryContextPass, reject_raw_mutation_during_ui)
        .add_systems(ContextPassA, record_ui)
        .add_plugins(ImguiPlugin::default());

    let (primary, context_a) = {
        let mut contexts = app.world_mut().get_non_send_mut::<ImguiContexts>().unwrap();
        let primary = contexts.primary_id().unwrap();
        let context_a = contexts
            .create(ImguiContextConfig::new(ContextPassA))
            .unwrap();
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
        .expect("configuration must become available after the UI schedule");
}

#[test]
fn duplicate_schedule_and_stale_context_errors_are_typed_and_recover_ownership() {
    let _guard = imgui_context_guard();
    let mut contexts = ImguiContexts::with_primary(dear_imgui_rs::SuspendedContext::create());
    let context_a = contexts
        .create(ImguiContextConfig::new(ContextPassA))
        .unwrap();
    let rejected = dear_imgui_rs::SuspendedContext::create();
    let rejected_id = rejected.id();

    let error = contexts
        .insert_suspended(rejected, ImguiContextConfig::new(ContextPassA))
        .expect_err("duplicate schedule ownership must be rejected");
    assert!(matches!(
        error.error(),
        ImguiContextError::DuplicateSchedule { owner, .. } if *owner == context_a
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
fn additional_multi_viewport_admission_fails_without_consuming_the_context() {
    let _guard = imgui_context_guard();
    let mut contexts = ImguiContexts::with_primary(dear_imgui_rs::SuspendedContext::create());
    let additional = dear_imgui_rs::SuspendedContext::create();
    let additional_id = additional.id();

    let error = contexts
        .insert_suspended(
            additional,
            ImguiContextConfig::new(ContextPassA).with_multi_viewport(true),
        )
        .expect_err("additional native viewport state is not Context-namespaced yet");

    assert!(matches!(
        error.error(),
        ImguiContextError::AdditionalMultiViewportUnsupported
    ));
    assert_eq!(error.into_context().id(), additional_id);
    assert!(!contexts.contains(additional_id));
}

#[test]
fn missing_schedule_is_context_local_and_does_not_stop_later_contexts() {
    let _guard = imgui_context_guard();
    let mut app = app_with_primary_window();
    app.init_resource::<LifecycleTrace>()
        .add_systems(ImguiPrimaryContextPass, record_ui)
        .add_systems(ContextPassB, record_ui)
        .add_plugins(ImguiPlugin::default());

    let (primary, missing, healthy) = {
        let mut contexts = app.world_mut().get_non_send_mut::<ImguiContexts>().unwrap();
        let primary = contexts.primary_id().unwrap();
        let missing = contexts
            .create(ImguiContextConfig::new(MissingContextPass))
            .unwrap();
        let healthy = contexts
            .create(ImguiContextConfig::new(ContextPassB))
            .unwrap();
        (primary, missing, healthy)
    };
    app.world_mut().resource_mut::<LifecycleTrace>().expected = vec![primary, missing, healthy];

    app.update();

    let contexts = app.world().get_non_send::<ImguiContexts>().unwrap();
    assert!(matches!(
        contexts.last_error(missing).unwrap(),
        Some(ImguiContextError::MissingSchedule { context_id, .. }) if *context_id == missing
    ));
    assert_eq!(contexts.frame_index(missing).unwrap(), 0);
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

fn panic_once(mut state: ResMut<PanicOnce>) {
    if !state.0 {
        state.0 = true;
        panic!("intentional Context schedule panic");
    }
}

#[test]
fn schedule_panic_reinserts_the_schedule_and_restores_context_ownership() {
    let _guard = imgui_context_guard();
    let foreign = dear_imgui_rs::Context::create();
    let foreign_raw = foreign.as_raw();

    let mut app = app_with_primary_window();
    app.init_resource::<PanicOnce>()
        .init_resource::<LifecycleTrace>()
        .add_systems(ImguiPrimaryContextPass, panic_once)
        .add_plugins(ImguiPlugin::default());
    let primary = app
        .world()
        .get_non_send::<ImguiContexts>()
        .unwrap()
        .primary_id()
        .unwrap();

    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| app.update()));
    assert!(panic.is_err());
    assert_eq!(
        unsafe { dear_imgui_rs::sys::igGetCurrentContext() },
        foreign_raw
    );
    {
        let contexts = app.world().get_non_send::<ImguiContexts>().unwrap();
        assert!(contexts.contains(primary));
        assert_eq!(contexts.frame_index(primary).unwrap(), 0);
    }
    assert!(
        app.world()
            .resource::<Schedules>()
            .contains(ImguiPrimaryContextPass),
        "the nested Context schedule must survive unwinding"
    );
    assert_eq!(
        unsafe { dear_imgui_rs::sys::igGetCurrentContext() },
        foreign_raw
    );
    drop(app);
    assert_eq!(
        unsafe { dear_imgui_rs::sys::igGetCurrentContext() },
        foreign_raw
    );
    drop(foreign);
}

#[test]
fn primary_without_a_window_does_not_advance_or_replay_a_frame() {
    let _guard = imgui_context_guard();
    let mut app = App::new();
    app.init_resource::<LifecycleTrace>()
        .add_systems(ImguiPrimaryContextPass, record_ui)
        .add_plugins(ImguiPlugin::default());
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
    app.init_resource::<LifecycleTrace>()
        .add_systems(ContextPassA, record_ui)
        .add_plugins(ImguiPlugin::default());

    let (primary, additional) = {
        let mut contexts = app.world_mut().get_non_send_mut::<ImguiContexts>().unwrap();
        let primary = contexts.primary_id().unwrap();
        let additional = contexts
            .create(ImguiContextConfig::new(ContextPassA))
            .unwrap();
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
