use std::sync::{Mutex, OnceLock};

use bevy_app::{App, MainScheduleOrder, PreUpdate};
use bevy_ecs::message::Messages;
#[cfg(feature = "render")]
use bevy_ecs::prelude::{ResMut, Resource};
#[cfg(feature = "render")]
use bevy_ecs::schedule::ScheduleLabel;
use bevy_ecs::schedule::Schedules;
#[cfg(feature = "render")]
use bevy_render::{Render, RenderApp, extract_plugin::ExtractPlugin};
#[cfg(feature = "render")]
use bevy_window::{PrimaryWindow, Window};
use dear_imgui_bevy::{
    BEVY_TARGET_COMMIT, BEVY_TARGET_VERSION, ImguiAppExt, ImguiContextConfig, ImguiContextError,
    ImguiContextRetired, ImguiContexts, ImguiDriverScheduleError, ImguiPlugin, ImguiPluginConfig,
    ImguiPluginInstallError, ImguiViewportWindowConfig, WGPU_TARGET_VERSION,
};
#[cfg(feature = "render")]
use dear_imgui_bevy::{ContextId, ImguiFrame, ImguiShutdownError};

fn imgui_context_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

struct AdditionalPass;

fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    match payload.downcast::<String>() {
        Ok(message) => *message,
        Err(payload) => payload
            .downcast::<&'static str>()
            .map(|message| (*message).to_owned())
            .unwrap_or_else(|_| "non-string panic payload".to_owned()),
    }
}

fn install_error(
    result: Result<&mut App, ImguiPluginInstallError>,
    message: &'static str,
) -> ImguiPluginInstallError {
    match result {
        Ok(_) => panic!("{message}"),
        Err(error) => error,
    }
}

#[cfg(feature = "render")]
#[derive(Resource, Default)]
struct UiErrorTrace {
    expected: Option<ContextId>,
    saw_expected: bool,
}

#[cfg(feature = "render")]
fn observe_additional_context(
    frame: ImguiFrame<'_, AdditionalPass>,
    mut trace: ResMut<UiErrorTrace>,
) {
    trace.saw_expected = Some(frame.context_id()) == trace.expected;
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
        .expect("default registry must be active")
        .expect("default registry needs a primary");
    assert_eq!(contexts.ids().unwrap().collect::<Vec<_>>(), vec![primary]);

    let schedules = app.world().resource::<Schedules>();
    assert!(
        schedules.iter().count() >= 1,
        "the plugin must install its private serial driver schedule"
    );

    assert_eq!(BEVY_TARGET_VERSION, "0.19.1");
    assert_eq!(
        BEVY_TARGET_COMMIT,
        "b56fc29d3016e641754765244b5ba3f9cc504671"
    );
    assert_eq!(WGPU_TARGET_VERSION, "29.0.3");
}

#[test]
fn fallible_installation_registers_the_plugin_and_rejects_a_duplicate_transaction() {
    let _guard = imgui_context_guard();
    let mut app = App::new();
    app.try_install_imgui(ImguiPlugin::default())
        .expect("the default integration must install successfully");
    let primary = app
        .world()
        .get_non_send::<ImguiContexts>()
        .expect("successful installation must retain its Context registry")
        .primary_id()
        .expect("successful installation must retain an active Context registry")
        .expect("successful installation must retain its primary Context");

    let error = install_error(
        app.try_install_imgui(ImguiPlugin::default()),
        "a second installation transaction must be rejected",
    );

    assert!(matches!(error, ImguiPluginInstallError::AlreadyInstalled));
    assert!(app.is_plugin_added::<ImguiPlugin>());
    assert_eq!(
        app.world()
            .get_non_send::<ImguiContexts>()
            .expect("duplicate validation must preserve the Context registry")
            .primary_id()
            .expect("duplicate validation must preserve an active registry"),
        Some(primary),
        "duplicate validation must leave the installed registry unchanged"
    );

    app.shutdown_imgui()
        .expect("the duplicate-install fixture must still shut down cleanly");
    let terminal = install_error(
        app.try_install_imgui(ImguiPlugin::default()),
        "a terminal App must reject reinstallation",
    );
    assert!(matches!(terminal, ImguiPluginInstallError::AppTerminated));
}

#[test]
fn fallible_installation_rejects_schedule_placement_before_app_mutation() {
    let _guard = imgui_context_guard();
    let mut app = App::new();
    let error = install_error(
        app.try_install_imgui(ImguiPlugin::new(
            ImguiPluginConfig::default().with_driver_before(PreUpdate),
        )),
        "invalid schedule placement must be reported without panic",
    );

    assert!(matches!(
        error,
        ImguiPluginInstallError::DriverSchedule(ImguiDriverScheduleError::OutsideFrameInterval)
    ));
    assert!(app.world().get_non_send::<ImguiContexts>().is_none());
    assert!(!app.is_plugin_added::<ImguiPlugin>());
    assert!(
        !app.world()
            .contains_resource::<Messages<ImguiContextRetired>>(),
        "failed validation must not install retirement resources"
    );
}

#[test]
fn fallible_installation_reports_a_missing_main_schedule_order_without_mutation() {
    let _guard = imgui_context_guard();
    let mut app = App::new();
    assert!(
        app.world_mut()
            .remove_resource::<MainScheduleOrder>()
            .is_some()
    );

    let error = install_error(
        app.try_install_imgui(ImguiPlugin::default()),
        "a missing main schedule order must be reported without panic",
    );

    assert!(matches!(
        error,
        ImguiPluginInstallError::DriverSchedule(ImguiDriverScheduleError::MainScheduleOrderMissing)
    ));
    assert!(app.world().get_non_send::<ImguiContexts>().is_none());
    assert!(!app.is_plugin_added::<ImguiPlugin>());
}

#[test]
fn fallible_installation_rejects_a_closed_bevy_plugin_lifecycle_without_mutation() {
    let _guard = imgui_context_guard();
    let mut app = App::new();
    app.finish();

    let error = install_error(
        app.try_install_imgui(ImguiPlugin::default()),
        "a finished Bevy App must reject new plugins",
    );

    assert!(matches!(
        error,
        ImguiPluginInstallError::PluginLifecycleClosed
    ));
    assert!(app.world().get_non_send::<ImguiContexts>().is_none());
    assert!(!app.is_plugin_added::<ImguiPlugin>());
}

#[test]
fn plugin_convenience_panics_with_the_fallible_installation_cause() {
    let _guard = imgui_context_guard();
    let config = ImguiPluginConfig::default().with_driver_before(PreUpdate);
    let mut fallible_app = App::new();
    let expected = install_error(
        fallible_app.try_install_imgui(ImguiPlugin::new(config.clone())),
        "the fallible path must reject the fixture",
    )
    .to_string();
    let mut app = App::new();
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        app.add_plugins(ImguiPlugin::new(config));
    }))
    .expect_err("the Plugin adapter intentionally panics on invalid configuration");

    assert_eq!(
        panic_message(panic),
        format!("ImguiPlugin installation failed: {expected}")
    );
    assert!(app.world().get_non_send::<ImguiContexts>().is_none());
}

#[test]
fn fallible_installation_rejects_invalid_viewport_policy_before_app_mutation() {
    let _guard = imgui_context_guard();
    let mut app = App::new();
    let viewport = ImguiViewportWindowConfig {
        transparent: true,
        composite_alpha_mode: bevy_window::CompositeAlphaMode::Opaque,
        ..Default::default()
    };
    let error = install_error(
        app.try_install_imgui(ImguiPlugin::new(
            ImguiPluginConfig::default().with_viewport_window(viewport),
        )),
        "invalid secondary-window alpha policy must be rejected",
    );

    assert!(matches!(error, ImguiPluginInstallError::ViewportWindow(_)));
    assert!(app.world().get_non_send::<ImguiContexts>().is_none());
}

#[cfg(not(all(feature = "multi-viewport", not(target_arch = "wasm32"))))]
#[test]
fn fallible_installation_rejects_unavailable_native_viewports_before_app_mutation() {
    let _guard = imgui_context_guard();
    let mut app = App::new();
    let error = install_error(
        app.try_install_imgui(ImguiPlugin::new(
            ImguiPluginConfig::default().with_multi_viewport(true),
        )),
        "native viewport availability must be validated before installation",
    );

    assert!(matches!(
        error,
        ImguiPluginInstallError::NativeMultiViewportUnavailable
    ));
    assert!(app.world().get_non_send::<ImguiContexts>().is_none());
}

#[test]
fn plugin_adopts_an_app_scoped_registry_without_replacing_its_primary_context() {
    let _guard = imgui_context_guard();
    let mut app = App::new();
    let additional_pass = app.declare_imgui_pass::<AdditionalPass>().unwrap();
    let primary = dear_imgui_rs::SuspendedContext::create();
    let primary_id = primary.id();
    app.adopt_imgui_primary_context(primary).unwrap();
    let additional_id = app
        .world_mut()
        .get_non_send_mut::<ImguiContexts>()
        .unwrap()
        .create(ImguiContextConfig::new(&additional_pass))
        .unwrap();

    let config = ImguiPluginConfig::default().with_docking(false);
    app.add_plugins(ImguiPlugin::new(config));

    let contexts = app.world().get_non_send::<ImguiContexts>().unwrap();
    assert_eq!(contexts.primary_id().unwrap(), Some(primary_id));
    assert_eq!(
        contexts.ids().unwrap().collect::<Vec<_>>(),
        vec![primary_id, additional_id]
    );
    let mut contexts = app.world_mut().get_non_send_mut::<ImguiContexts>().unwrap();
    for context_id in [primary_id, additional_id] {
        contexts
            .configure(context_id, |context| {
                assert_eq!(
                    context.io().backend_platform_name().unwrap().to_bytes(),
                    b"dear-imgui-bevy"
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
    app.adopt_imgui_primary_context(primary).unwrap();
    app.add_plugins(ImguiPlugin::default());

    let mut removed = app
        .world_mut()
        .get_non_send_mut::<ImguiContexts>()
        .unwrap()
        .try_remove_immediately(primary_id)
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
            .primary_id()
            .unwrap(),
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
    app.adopt_imgui_primary_context(primary).unwrap();
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
    let mut app = app_with_render_schedule();
    let additional_pass = app.declare_imgui_pass::<AdditionalPass>().unwrap();
    let atlas = dear_imgui_rs::SharedFontAtlas::create();
    let primary =
        dear_imgui_rs::SuspendedContext::try_create_with_shared_font_atlas(atlas.clone()).unwrap();
    let primary_id = primary.id();
    let additional =
        dear_imgui_rs::SuspendedContext::try_create_with_shared_font_atlas(atlas).unwrap();
    let additional_id = additional.id();
    app.adopt_imgui_primary_context(primary).unwrap();
    app.world_mut()
        .get_non_send_mut::<ImguiContexts>()
        .unwrap()
        .insert_suspended(additional, ImguiContextConfig::new(&additional_pass))
        .unwrap();

    let failure = install_error(
        app.try_install_imgui(ImguiPlugin::default()),
        "a managed renderer must reject a shared multi-Context font atlas",
    );
    assert!(matches!(
        &failure,
        ImguiPluginInstallError::ContextPreflight(_)
    ));
    assert!(
        failure
            .to_string()
            .contains("managed font-atlas rendering requires exactly one")
    );
    assert!(!app.is_plugin_added::<ImguiPlugin>());

    let mut contexts = app.world_mut().get_non_send_mut::<ImguiContexts>().unwrap();
    for context_id in [primary_id, additional_id] {
        contexts
            .configure(context_id, |context| {
                assert!(context.io().backend_platform_name().is_none());
                assert!(context.io().backend_renderer_name().is_none());
                assert!(!context.io().backend_flags().intersects(
                    dear_imgui_rs::BackendFlags::RENDERER_HAS_TEXTURES
                        | dear_imgui_rs::BackendFlags::RENDERER_HAS_VTX_OFFSET
                ));
                drop(
                    context
                        .font_atlas()
                        .try_claim_legacy_renderer()
                        .expect("failed renderer admission must not leave a managed atlas claim"),
                );
            })
            .unwrap();
    }
}

#[cfg(feature = "render")]
#[test]
fn later_renderer_admission_failure_leaves_earlier_font_atlas_unclaimed() {
    let _guard = imgui_context_guard();
    let mut app = app_with_render_schedule();
    let additional_pass = app.declare_imgui_pass::<AdditionalPass>().unwrap();
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
    app.adopt_imgui_primary_context(primary).unwrap();
    app.world_mut()
        .get_non_send_mut::<ImguiContexts>()
        .unwrap()
        .insert_suspended(additional, ImguiContextConfig::new(&additional_pass))
        .unwrap();

    let failure = install_error(
        app.try_install_imgui(ImguiPlugin::default()),
        "the later shared atlas must reject managed renderer admission",
    );
    assert!(matches!(
        &failure,
        ImguiPluginInstallError::ContextPreflight(_)
    ));
    assert!(
        failure
            .to_string()
            .contains("managed font-atlas rendering requires exactly one")
    );
    assert!(!app.is_plugin_added::<ImguiPlugin>());

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
                .create_detached_renderer_consumer()
                .expect("failed registry admission must not bind the snapshot hub");
            context
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
    let additional_pass = app.declare_imgui_pass::<AdditionalPass>().unwrap();
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    app.init_resource::<UiErrorTrace>();
    app.add_imgui_systems(
        &additional_pass,
        additional_pass.system(observe_additional_context),
    )
    .unwrap();
    app.add_plugins(ImguiPlugin::default());
    let stale = dear_imgui_rs::SuspendedContext::create();
    let stale_id = stale.id();
    drop(stale);
    let (primary_id, additional_id) = {
        let mut contexts = app.world_mut().get_non_send_mut::<ImguiContexts>().unwrap();
        let primary_id = contexts.primary_id().unwrap().unwrap();
        let additional_id = contexts
            .create(ImguiContextConfig::new(&additional_pass))
            .unwrap();
        (primary_id, additional_id)
    };
    let renderer_flags = dear_imgui_rs::BackendFlags::RENDERER_HAS_TEXTURES
        | dear_imgui_rs::BackendFlags::RENDERER_HAS_VTX_OFFSET;
    {
        let mut trace = app.world_mut().resource_mut::<UiErrorTrace>();
        trace.expected = Some(additional_id);
    }

    assert!(matches!(
        app.world_mut()
            .non_send_mut::<ImguiContexts>()
            .configure(stale_id, |_| ()),
        Err(ImguiContextError::UnknownContext { context_id }) if context_id == stale_id
    ));

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
            .get_non_send::<ImguiContexts>()
            .expect("the plugin must retain its Context registry")
            .frame_index(primary_id)
            .expect("the rejected Context must retain its frame index"),
        0,
        "a rejected primary frame must not become the latest completed frame"
    );

    let managed_removal = match app
        .world_mut()
        .get_non_send_mut::<ImguiContexts>()
        .unwrap()
        .remove(primary_id)
    {
        Ok(_) => panic!("managed removal must not hide repairable renderer ownership drift"),
        Err(error) => error,
    };
    assert!(matches!(
        managed_removal,
        ImguiContextError::RemovalPending {
            context_id,
            reason:
                dear_imgui_bevy::ImguiContextRemovalPendingReason::RendererOwnership(
                    dear_imgui_bevy::ImguiRendererOwnershipError::FieldReplaced {
                        field: "BackendFlags"
                    }
                ),
        } if context_id == primary_id
    ));

    let removal = match app
        .world_mut()
        .get_non_send_mut::<ImguiContexts>()
        .unwrap()
        .try_remove_immediately(primary_id)
    {
        Ok(_) => panic!("partial renderer ownership must fail closed"),
        Err(error) => error,
    };
    assert!(matches!(
        removal,
        ImguiContextError::RemovalPending {
            context_id,
            reason:
                dear_imgui_bevy::ImguiContextRemovalPendingReason::RendererOwnership(
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
    assert!(trace.saw_expected);

    {
        let mut contexts = app.world_mut().get_non_send_mut::<ImguiContexts>().unwrap();
        contexts
            .configure(primary_id, |context| {
                let flags = context.io().backend_flags() | renderer_flags;
                context.io_mut().set_backend_flags(flags);
            })
            .expect("a pending removal must permit ownership repair");
        assert!(matches!(
            contexts.try_remove_immediately(primary_id),
            Err(ImguiContextError::RemovalPending {
                context_id,
                reason:
                    dear_imgui_bevy::ImguiContextRemovalPendingReason::RenderWorldReleasePending,
            }) if context_id == primary_id
        ));
    }

    app.update();
    let removed = app
        .world_mut()
        .get_non_send_mut::<ImguiContexts>()
        .unwrap()
        .try_remove_immediately(primary_id)
        .expect("renderer acknowledgement must complete the repaired teardown");
    assert_eq!(removed.id(), primary_id);
}

#[cfg(feature = "render")]
#[test]
fn renderer_ownership_drift_blocks_shutdown_before_registry_removal_and_can_be_repaired() {
    let _guard = imgui_context_guard();
    let mut app = app_with_render_schedule();
    let additional_pass = app.declare_imgui_pass::<AdditionalPass>().unwrap();
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    app.add_plugins(ImguiPlugin::default());
    let (primary_id, additional_id) = {
        let mut contexts = app.world_mut().get_non_send_mut::<ImguiContexts>().unwrap();
        let primary_id = contexts.primary_id().unwrap().unwrap();
        let additional_id = contexts
            .create(ImguiContextConfig::new(&additional_pass))
            .unwrap();
        (primary_id, additional_id)
    };

    let expected_flags = app
        .world_mut()
        .get_non_send_mut::<ImguiContexts>()
        .unwrap()
        .configure(additional_id, |context| {
            let expected_flags = context.io().backend_flags();
            let mut drifted_flags = expected_flags;
            drifted_flags.remove(dear_imgui_rs::BackendFlags::RENDERER_HAS_TEXTURES);
            context.io_mut().set_backend_flags(drifted_flags);
            expected_flags
        })
        .unwrap();

    let error = app
        .shutdown_imgui()
        .expect_err("renderer ownership drift must block terminal shutdown preflight");
    assert!(matches!(
        error,
        ImguiShutdownError::ContextTeardownBlocked {
            context_id,
            reason:
                dear_imgui_bevy::ImguiContextRemovalPendingReason::RendererOwnership(
                    dear_imgui_bevy::ImguiRendererOwnershipError::FieldReplaced {
                        field: "BackendFlags"
                    }
                ),
        } if context_id == additional_id
    ));
    assert_eq!(
        app.world()
            .get_non_send::<ImguiContexts>()
            .unwrap()
            .ids()
            .unwrap()
            .collect::<Vec<_>>(),
        vec![primary_id, additional_id],
        "a later Context failure must preserve every preflighted Context"
    );

    app.world_mut()
        .get_non_send_mut::<ImguiContexts>()
        .unwrap()
        .configure(additional_id, |context| {
            context.io_mut().set_backend_flags(expected_flags);
        })
        .expect("the retained registry must allow renderer ownership repair");
    app.shutdown_imgui()
        .expect("repaired renderer ownership must permit terminal shutdown");
    assert!(app.world().get_non_send::<ImguiContexts>().is_none());
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
    app.adopt_imgui_primary_context(primary).unwrap();
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
                .create_detached_renderer_consumer()
                .expect("foreign backend admission failure must not bind the snapshot hub");
            context
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
    let mut app = App::new();
    app.adopt_imgui_primary_context(primary).unwrap();
    let mut contexts = app.world_mut().remove_non_send::<ImguiContexts>().unwrap();

    assert!(matches!(
        contexts.configure(stale_id, |_| ()),
        Err(ImguiContextError::UnknownContext { context_id }) if context_id == stale_id
    ));
}
