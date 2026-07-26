use std::sync::{Mutex, OnceLock};

use bevy_app::App;
#[cfg(feature = "render")]
use bevy_ecs::prelude::{ResMut, Resource};
use bevy_ecs::schedule::{ScheduleLabel, Schedules};
#[cfg(feature = "render")]
use bevy_render::{Render, RenderApp, extract_plugin::ExtractPlugin};
#[cfg(feature = "render")]
use bevy_window::{PrimaryWindow, Window};
use dear_imgui_bevy::{
    BEVY_TARGET_COMMIT, BEVY_TARGET_VERSION, ImguiBackendConfig, ImguiBackendStatus,
    ImguiContextConfig, ImguiContextError, ImguiContexts, ImguiPlugin, ImguiPrimaryContextPass,
    RUST_TARGET_VERSION, WGPU_TARGET_VERSION,
};
#[cfg(feature = "render")]
use dear_imgui_bevy::{ContextId, ImguiUi};

fn imgui_context_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
struct AdditionalPass;

#[cfg(feature = "render")]
#[derive(Resource, Default)]
struct UiErrorTrace {
    teardown: Option<ContextId>,
    unknown: Option<ContextId>,
    saw_teardown: bool,
    saw_unknown: bool,
}

#[cfg(feature = "render")]
fn observe_ui_error_kinds(ui: ImguiUi, mut trace: ResMut<UiErrorTrace>) {
    let teardown = trace
        .teardown
        .expect("test must install a teardown Context");
    let unknown = trace.unknown.expect("test must install an unknown Context");
    trace.saw_teardown = matches!(
        ui.ui_for(teardown),
        Err(ImguiContextError::TeardownInProgress { context_id }) if context_id == teardown
    );
    trace.saw_unknown = matches!(
        ui.ui_for(unknown),
        Err(ImguiContextError::UnknownContext { context_id }) if context_id == unknown
    );
}

#[test]
fn plugin_registers_the_primary_registry_and_private_driver_schedule() {
    let _guard = imgui_context_guard();
    let mut app = App::new();
    app.add_plugins(ImguiPlugin::default());

    let contexts = app
        .world()
        .get_non_send::<ImguiContexts>()
        .expect("plugin must install a non-send Context registry");
    let primary = contexts
        .primary_id()
        .expect("default registry needs a primary");
    assert_eq!(contexts.ids().collect::<Vec<_>>(), vec![primary]);

    let schedules = app.world().resource::<Schedules>();
    assert!(schedules.contains(ImguiPrimaryContextPass));
    assert!(
        schedules.iter().count() >= 2,
        "the plugin must install its private serial driver schedule"
    );

    let status = app.world().resource::<ImguiBackendStatus>();
    assert_eq!(status.bevy_target, BEVY_TARGET_VERSION);
    assert_eq!(status.rust_target, RUST_TARGET_VERSION);
    assert_eq!(BEVY_TARGET_VERSION, "0.19.0");
    assert_eq!(
        BEVY_TARGET_COMMIT,
        "c6f634ca9f406d68ba5109d921247b654cb42c10"
    );
    assert_eq!(WGPU_TARGET_VERSION, "29.0.3");
}

#[test]
fn plugin_adopts_a_preinserted_registry_without_replacing_its_primary_context() {
    let _guard = imgui_context_guard();
    let primary = dear_imgui_rs::SuspendedContext::create();
    let primary_id = primary.id();
    let mut contexts = ImguiContexts::with_primary(primary);
    let additional_id = contexts
        .create(ImguiContextConfig::new(AdditionalPass))
        .unwrap();

    let config = ImguiBackendConfig {
        name: "custom\0backend".to_owned(),
        docking: false,
        multi_viewport: false,
        viewport_window: Default::default(),
    };
    let mut app = App::new();
    app.insert_non_send(contexts);
    app.insert_resource(config.clone());
    app.add_plugins(ImguiPlugin::default());

    let contexts = app.world().get_non_send::<ImguiContexts>().unwrap();
    assert_eq!(contexts.primary_id(), Some(primary_id));
    assert_eq!(
        contexts.ids().collect::<Vec<_>>(),
        vec![primary_id, additional_id]
    );
    assert_eq!(app.world().resource::<ImguiBackendConfig>(), &config);

    let mut contexts = app.world_mut().get_non_send_mut::<ImguiContexts>().unwrap();
    for context_id in [primary_id, additional_id] {
        contexts
            .configure(context_id, |context| {
                assert_eq!(
                    context.io().backend_platform_name().unwrap().to_bytes(),
                    b"custom?backend"
                );
                assert_eq!(
                    context
                        .io()
                        .config_flags()
                        .contains(dear_imgui_rs::ConfigFlags::DOCKING_ENABLE),
                    context_id == additional_id
                );
            })
            .unwrap();
    }
}

#[test]
fn removing_a_context_clears_only_backend_state_owned_by_bevy() {
    let _guard = imgui_context_guard();
    let primary = dear_imgui_rs::SuspendedContext::create();
    let primary_id = primary.id();
    let mut app = App::new();
    app.insert_non_send(ImguiContexts::with_primary(primary));
    app.add_plugins(ImguiPlugin::default());

    let mut removed = app
        .world_mut()
        .get_non_send_mut::<ImguiContexts>()
        .unwrap()
        .remove(primary_id)
        .expect("headless backend teardown should complete immediately");
    removed
        .try_with_active(|context| {
            assert!(context.io().backend_platform_name().is_none());
            assert!(context.io().backend_platform_user_data().is_null());
            assert!(context.io().backend_renderer_name().is_none());
            assert!(context.io().backend_renderer_user_data().is_null());
            Ok::<_, ()>(())
        })
        .unwrap();
    assert_eq!(
        app.world()
            .get_non_send::<ImguiContexts>()
            .unwrap()
            .primary_id(),
        None
    );
}

#[test]
fn foreign_platform_name_is_not_overwritten_when_bevy_does_not_own_the_platform_contract() {
    let _guard = imgui_context_guard();
    let mut primary = dear_imgui_rs::SuspendedContext::create();
    primary
        .try_with_active(|context| {
            context.set_platform_name(Some("foreign-platform")).unwrap();
            Ok::<_, ()>(())
        })
        .unwrap();
    let primary_id = primary.id();

    let mut app = App::new();
    app.insert_non_send(ImguiContexts::with_primary(primary));
    app.add_plugins(ImguiPlugin::default());

    app.world_mut()
        .get_non_send_mut::<ImguiContexts>()
        .unwrap()
        .configure(primary_id, |context| {
            assert_eq!(
                context.io().backend_platform_name().unwrap().to_bytes(),
                b"foreign-platform"
            );
        })
        .unwrap();
}

#[cfg(feature = "render")]
fn app_with_render_schedule() -> App {
    let mut app = App::new();
    app.add_plugins(ExtractPlugin::default());
    app.sub_app_mut(RenderApp).update_schedule = Some(Render.intern());
    app
}

#[cfg(feature = "render")]
#[test]
fn managed_shared_font_atlas_admission_fails_before_backend_fields_are_mutated() {
    let _guard = imgui_context_guard();
    let atlas = dear_imgui_rs::SharedFontAtlas::create();
    let mut primary =
        dear_imgui_rs::SuspendedContext::try_create_with_shared_font_atlas(atlas.clone()).unwrap();
    let primary_id = primary.id();
    let primary_font_texture_id = primary
        .try_with_active(|context| Ok::<_, ()>(context.font_atlas().texture_id()))
        .unwrap();
    let mut additional =
        dear_imgui_rs::SuspendedContext::try_create_with_shared_font_atlas(atlas).unwrap();
    let additional_id = additional.id();
    let additional_font_texture_id = additional
        .try_with_active(|context| Ok::<_, ()>(context.font_atlas().texture_id()))
        .unwrap();
    let mut contexts = ImguiContexts::with_primary(primary);
    contexts
        .insert_suspended(additional, ImguiContextConfig::new(AdditionalPass))
        .unwrap();

    let mut app = app_with_render_schedule();
    app.insert_non_send(contexts);
    let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        app.add_plugins(ImguiPlugin::default());
    }))
    .expect_err("a managed renderer must reject a shared multi-Context font atlas");
    let message = failure
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| failure.downcast_ref::<&str>().copied())
        .unwrap_or_default();
    assert!(message.contains("managed font-atlas rendering requires exactly one"));

    let mut contexts = app.world_mut().get_non_send_mut::<ImguiContexts>().unwrap();
    for (context_id, expected_font_texture_id) in [
        (primary_id, primary_font_texture_id),
        (additional_id, additional_font_texture_id),
    ] {
        contexts
            .configure(context_id, |context| {
                assert!(context.io().backend_platform_name().is_none());
                assert!(context.io().backend_renderer_name().is_none());
                assert_eq!(
                    context.font_atlas().texture_id(),
                    expected_font_texture_id,
                    "failed renderer admission must not alter the font-atlas texture ID"
                );
                assert!(!context.io().backend_flags().intersects(
                    dear_imgui_rs::BackendFlags::RENDERER_HAS_TEXTURES
                        | dear_imgui_rs::BackendFlags::RENDERER_HAS_VTX_OFFSET
                ));
            })
            .unwrap();
    }
}

#[cfg(feature = "render")]
#[test]
fn later_renderer_admission_failure_leaves_earlier_font_atlas_unclaimed() {
    let _guard = imgui_context_guard();
    let primary_atlas = dear_imgui_rs::SharedFontAtlas::create();
    let primary =
        dear_imgui_rs::SuspendedContext::try_create_with_shared_font_atlas(primary_atlas.clone())
            .unwrap();
    let primary_id = primary.id();

    let blocked_atlas = dear_imgui_rs::SharedFontAtlas::create();
    let additional =
        dear_imgui_rs::SuspendedContext::try_create_with_shared_font_atlas(blocked_atlas.clone())
            .unwrap();
    let blocked_peer =
        dear_imgui_rs::SuspendedContext::try_create_with_shared_font_atlas(blocked_atlas).unwrap();
    let additional_id = additional.id();
    let mut contexts = ImguiContexts::with_primary(primary);
    contexts
        .insert_suspended(additional, ImguiContextConfig::new(AdditionalPass))
        .unwrap();

    let mut app = app_with_render_schedule();
    app.insert_non_send(contexts);
    let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        app.add_plugins(ImguiPlugin::default());
    }))
    .expect_err("the later shared atlas must reject managed renderer admission");
    let message = failure
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| failure.downcast_ref::<&str>().copied())
        .unwrap_or_default();
    assert!(message.contains("managed font-atlas rendering requires exactly one"));

    let primary_peer =
        dear_imgui_rs::SuspendedContext::try_create_with_shared_font_atlas(primary_atlas)
            .expect("the earlier Context atlas must remain in legacy-unclaimed mode");
    drop(primary_peer);
    drop(blocked_peer);

    let mut contexts = app.world_mut().get_non_send_mut::<ImguiContexts>().unwrap();
    for context_id in [primary_id, additional_id] {
        contexts
            .configure(context_id, |context| {
                assert!(context.io().backend_platform_name().is_none());
                assert!(context.io().backend_renderer_name().is_none());
            })
            .unwrap();
    }
    contexts
        .configure(primary_id, |context| {
            let consumer = context
                .create_renderer_consumer()
                .expect("failed registry admission must not bind the snapshot hub");
            let _ = context
                .prepare_renderer_texture_reset(&consumer)
                .unwrap()
                .commit();
            drop(consumer);
            context.poll_snapshot_completions().unwrap();
        })
        .unwrap();
}

#[cfg(feature = "render")]
#[test]
fn renderer_ownership_drift_fails_closed_and_removal_can_be_repaired() {
    let _guard = imgui_context_guard();
    let mut app = app_with_render_schedule();
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    app.init_resource::<UiErrorTrace>()
        .add_systems(AdditionalPass, observe_ui_error_kinds);
    app.add_plugins(ImguiPlugin::default());
    let stale = dear_imgui_rs::SuspendedContext::create();
    let stale_id = stale.id();
    drop(stale);
    let (primary_id, additional_id) = {
        let mut contexts = app.world_mut().get_non_send_mut::<ImguiContexts>().unwrap();
        let primary_id = contexts.primary_id().unwrap();
        let additional_id = contexts
            .create(ImguiContextConfig::new(AdditionalPass))
            .unwrap();
        (primary_id, additional_id)
    };
    let renderer_flags = dear_imgui_rs::BackendFlags::RENDERER_HAS_TEXTURES
        | dear_imgui_rs::BackendFlags::RENDERER_HAS_VTX_OFFSET;
    {
        let mut trace = app.world_mut().resource_mut::<UiErrorTrace>();
        trace.teardown = Some(primary_id);
        trace.unknown = Some(stale_id);
    }

    app.world_mut()
        .get_non_send_mut::<ImguiContexts>()
        .unwrap()
        .configure(primary_id, |context| {
            let mut flags = context.io().backend_flags();
            flags.remove(dear_imgui_rs::BackendFlags::RENDERER_HAS_TEXTURES);
            context.io_mut().set_backend_flags(flags);
        })
        .unwrap();

    app.update();
    assert!(matches!(
        app.world()
            .get_non_send::<ImguiContexts>()
            .unwrap()
            .last_error(primary_id)
            .unwrap(),
        Some(ImguiContextError::RendererOwnership {
            context_id,
            source: dear_imgui_bevy::ImguiRendererOwnershipError::FieldReplaced {
                field: "BackendFlags"
            },
        }) if *context_id == primary_id
    ));
    assert_eq!(
        app.world()
            .resource::<dear_imgui_bevy::ImguiFrameOutput>()
            .frame_index(),
        0,
        "a rejected primary frame must not become the latest completed frame"
    );

    let removal = match app
        .world_mut()
        .get_non_send_mut::<ImguiContexts>()
        .unwrap()
        .remove(primary_id)
    {
        Ok(_) => panic!("partial renderer ownership must fail closed"),
        Err(error) => error,
    };
    assert!(matches!(
        removal,
        ImguiContextError::RemovalPending {
            context_id,
            reason:
                dear_imgui_bevy::ImguiContextIntoInnerErrorReason::RendererOwnership(
                    dear_imgui_bevy::ImguiRendererOwnershipError::FieldReplaced {
                        field: "BackendFlags"
                    }
                ),
        } if context_id == primary_id
    ));

    app.update();
    assert_eq!(
        app.world()
            .get_non_send::<ImguiContexts>()
            .unwrap()
            .frame_index(additional_id)
            .unwrap(),
        2,
        "a pending primary removal must not stop an independent Context"
    );
    let trace = app.world().resource::<UiErrorTrace>();
    assert!(trace.saw_teardown);
    assert!(trace.saw_unknown);

    let mut contexts = app.world_mut().get_non_send_mut::<ImguiContexts>().unwrap();
    contexts
        .configure(primary_id, |context| {
            let flags = context.io().backend_flags() | renderer_flags;
            context.io_mut().set_backend_flags(flags);
        })
        .expect("a pending removal must permit ownership repair");
    let removed = contexts
        .remove(primary_id)
        .expect("repairing the renderer contract must make teardown retryable");
    assert_eq!(removed.id(), primary_id);
}

#[cfg(feature = "render")]
#[test]
fn foreign_renderer_claim_is_rejected_without_mutating_the_context() {
    let _guard = imgui_context_guard();
    let mut primary = dear_imgui_rs::SuspendedContext::create();
    primary
        .try_with_active(|context| {
            context.set_renderer_name(Some("foreign-renderer")).unwrap();
            Ok::<_, ()>(())
        })
        .unwrap();
    let primary_id = primary.id();

    let mut app = app_with_render_schedule();
    app.insert_non_send(ImguiContexts::with_primary(primary));
    let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        app.add_plugins(ImguiPlugin::default());
    }))
    .expect_err("a foreign renderer claim must reject plugin attachment");
    let message = failure
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| failure.downcast_ref::<&str>().copied())
        .unwrap_or_default();
    assert!(message.contains("BackendRendererName"));

    app.world_mut()
        .get_non_send_mut::<ImguiContexts>()
        .unwrap()
        .configure(primary_id, |context| {
            assert_eq!(
                context.io().backend_renderer_name().unwrap().to_bytes(),
                b"foreign-renderer"
            );
            assert!(context.io().backend_platform_name().is_none());
            let consumer = context
                .create_renderer_consumer()
                .expect("foreign backend admission failure must not bind the snapshot hub");
            let _ = context
                .prepare_renderer_texture_reset(&consumer)
                .unwrap()
                .commit();
            drop(consumer);
            context.poll_snapshot_completions().unwrap();
        })
        .unwrap();
}

#[test]
fn unknown_context_error_does_not_alias_the_primary_identity() {
    let _guard = imgui_context_guard();
    let primary = dear_imgui_rs::SuspendedContext::create();
    let stale = dear_imgui_rs::SuspendedContext::create();
    let stale_id = stale.id();
    drop(stale);
    let mut contexts = ImguiContexts::with_primary(primary);

    assert!(matches!(
        contexts.configure(stale_id, |_| ()),
        Err(ImguiContextError::UnknownContext { context_id }) if context_id == stale_id
    ));
}
