use std::sync::{Mutex, OnceLock};

use bevy_app::App;
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
    ImguiContexts, ImguiPlugin, ImguiPluginConfig, WGPU_TARGET_VERSION,
};
#[cfg(feature = "render")]
use dear_imgui_bevy::{ContextId, ImguiFrame};

fn imgui_context_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

struct AdditionalPass;

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
        .expect("default registry needs a primary");
    assert_eq!(contexts.ids().collect::<Vec<_>>(), vec![primary]);

    let schedules = app.world().resource::<Schedules>();
    assert!(
        schedules.iter().count() >= 1,
        "the plugin must install its private serial driver schedule"
    );

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
    let mut app = App::new();
    let primary_pass = app.imgui_primary_pass();
    let additional_pass = app.declare_imgui_pass::<AdditionalPass>();
    let primary = dear_imgui_rs::SuspendedContext::create();
    let primary_id = primary.id();
    let mut contexts = ImguiContexts::with_primary(primary, primary_pass);
    let additional_id = contexts
        .create(ImguiContextConfig::new(additional_pass))
        .unwrap();

    let config = ImguiPluginConfig::default().with_docking(false);
    app.insert_non_send(contexts);
    app.add_plugins(ImguiPlugin::new(config));

    let contexts = app.world().get_non_send::<ImguiContexts>().unwrap();
    assert_eq!(contexts.primary_id(), Some(primary_id));
    assert_eq!(
        contexts.ids().collect::<Vec<_>>(),
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
    let primary_pass = app.imgui_primary_pass();
    app.insert_non_send(ImguiContexts::with_primary(primary, primary_pass));
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
    let primary_pass = app.imgui_primary_pass();
    app.insert_non_send(ImguiContexts::with_primary(primary, primary_pass));
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
    let primary_pass = app.imgui_primary_pass();
    let additional_pass = app.declare_imgui_pass::<AdditionalPass>();
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
    let mut contexts = ImguiContexts::with_primary(primary, primary_pass);
    contexts
        .insert_suspended(additional, ImguiContextConfig::new(additional_pass))
        .unwrap();

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
    let mut app = app_with_render_schedule();
    let primary_pass = app.imgui_primary_pass();
    let additional_pass = app.declare_imgui_pass::<AdditionalPass>();
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
    let mut contexts = ImguiContexts::with_primary(primary, primary_pass);
    contexts
        .insert_suspended(additional, ImguiContextConfig::new(additional_pass))
        .unwrap();

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
    let additional_pass = app.declare_imgui_pass::<AdditionalPass>();
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    app.init_resource::<UiErrorTrace>();
    app.add_imgui_system(&additional_pass, observe_additional_context);
    app.add_plugins(ImguiPlugin::default());
    let stale = dear_imgui_rs::SuspendedContext::create();
    let stale_id = stale.id();
    drop(stale);
    let (primary_id, additional_id) = {
        let mut contexts = app.world_mut().get_non_send_mut::<ImguiContexts>().unwrap();
        let primary_id = contexts.primary_id().unwrap();
        let additional_id = contexts
            .create(ImguiContextConfig::new(additional_pass))
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
            contexts.remove(primary_id),
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
        .remove(primary_id)
        .expect("renderer acknowledgement must complete the repaired teardown");
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
    let primary_pass = app.imgui_primary_pass();
    app.insert_non_send(ImguiContexts::with_primary(primary, primary_pass));
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
    let mut app = App::new();
    let primary_pass = app.imgui_primary_pass();
    let mut contexts = ImguiContexts::with_primary(primary, primary_pass);

    assert!(matches!(
        contexts.configure(stale_id, |_| ()),
        Err(ImguiContextError::UnknownContext { context_id }) if context_id == stale_id
    ));
}
