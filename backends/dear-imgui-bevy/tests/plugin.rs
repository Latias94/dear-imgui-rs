use bevy_app::App;
#[cfg(feature = "render")]
use bevy_ecs::schedule::ScheduleLabel;
#[cfg(feature = "render")]
use bevy_render::{Render, RenderApp, extract_plugin::ExtractPlugin};
use dear_imgui_bevy::{
    BEVY_TARGET_COMMIT, BEVY_TARGET_VERSION, ImguiBackendConfig, ImguiBackendStatus, ImguiContext,
    ImguiPlugin, RUST_TARGET_VERSION, WGPU_TARGET_VERSION,
};
use std::sync::{Mutex, OnceLock};

fn imgui_context_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(())).lock().unwrap()
}

#[test]
fn plugin_registers_minimal_imgui_resources() {
    let _guard = imgui_context_guard();

    let mut app = App::new();
    app.add_plugins(ImguiPlugin::default());

    let config = app.world().resource::<ImguiBackendConfig>();
    assert_eq!(config.name, "dear-imgui-bevy");
    assert!(config.docking);
    assert!(!config.multi_viewport);

    let status = app.world().resource::<ImguiBackendStatus>();
    assert_eq!(status.bevy_target, BEVY_TARGET_VERSION);
    assert_eq!(status.rust_target, RUST_TARGET_VERSION);
    assert_eq!(status.render_feature_enabled, cfg!(feature = "render"));
    assert!(!status.render_integration_installed);
    assert!(!status.multi_viewport_requested);
    assert_eq!(
        status.multi_viewport_feature_enabled,
        cfg!(feature = "multi-viewport")
    );
    assert_eq!(status.native_platform_target, !cfg!(target_arch = "wasm32"));
    assert!(!status.viewport_lifecycle_bridge_enabled);
    assert!(!status.viewport_input_feedback_enabled);
    assert!(!status.viewport_render_routing_enabled);
    assert!(!status.multi_viewport_supported);
    assert_eq!(BEVY_TARGET_VERSION, "0.19.0-rc.2");
    assert_eq!(
        BEVY_TARGET_COMMIT,
        "a389b928aee5906928a16a7d4e66cb02c7362901"
    );
    assert_eq!(WGPU_TARGET_VERSION, "29.0.3");

    let context = app
        .world()
        .get_non_send::<ImguiContext>()
        .expect("plugin should install the Dear ImGui context");
    let io = context.context().io();
    assert!(
        io.config_flags()
            .contains(dear_imgui_rs::ConfigFlags::DOCKING_ENABLE)
    );
    assert_eq!(
        io.backend_platform_name()
            .expect("plugin should set BackendPlatformName")
            .to_str()
            .expect("backend name should be valid UTF-8"),
        "dear-imgui-bevy"
    );
    assert!(
        io.backend_renderer_name().is_none(),
        "renderer name should stay unset until render integration is installed"
    );
    assert!(
        !io.backend_flags()
            .contains(dear_imgui_rs::BackendFlags::RENDERER_HAS_TEXTURES)
    );
    assert!(
        !io.backend_flags()
            .contains(dear_imgui_rs::BackendFlags::RENDERER_HAS_VTX_OFFSET)
    );
}

#[test]
fn plugin_preserves_existing_config_and_context() {
    let _guard = imgui_context_guard();

    let mut app = App::new();
    app.insert_resource(ImguiBackendConfig {
        name: "custom-imgui".to_owned(),
        docking: false,
        multi_viewport: true,
    });
    app.insert_non_send(ImguiContext::new(dear_imgui_rs::Context::create()));

    app.add_plugins(ImguiPlugin::default());

    let config = app.world().resource::<ImguiBackendConfig>();
    assert_eq!(config.name, "custom-imgui");
    assert!(!config.docking);
    assert!(config.multi_viewport);

    let status = app.world().resource::<ImguiBackendStatus>();
    assert!(status.multi_viewport_requested);
    assert_eq!(status.render_feature_enabled, cfg!(feature = "render"));
    assert!(!status.render_integration_installed);
    assert_eq!(
        status.multi_viewport_feature_enabled,
        cfg!(feature = "multi-viewport")
    );
    assert_eq!(status.native_platform_target, !cfg!(target_arch = "wasm32"));
    assert_eq!(
        status.viewport_lifecycle_bridge_enabled,
        cfg!(all(feature = "multi-viewport", not(target_arch = "wasm32")))
    );
    assert_eq!(
        status.viewport_input_feedback_enabled,
        cfg!(all(feature = "multi-viewport", not(target_arch = "wasm32")))
    );
    assert!(!status.viewport_render_routing_enabled);
    assert!(!status.multi_viewport_supported);
    let context = app
        .world()
        .get_non_send::<ImguiContext>()
        .expect("plugin should preserve the existing Dear ImGui context");
    let io = context.context().io();
    assert!(
        !io.config_flags()
            .contains(dear_imgui_rs::ConfigFlags::DOCKING_ENABLE)
    );
    assert_eq!(
        io.backend_platform_name()
            .expect("plugin should set BackendPlatformName")
            .to_str()
            .expect("backend name should be valid UTF-8"),
        "custom-imgui"
    );
}

#[test]
fn plugin_sanitizes_backend_names_for_imgui_c_strings() {
    let _guard = imgui_context_guard();

    let mut app = App::new();
    app.add_plugins(ImguiPlugin::new(ImguiBackendConfig {
        name: "bad\0name".to_owned(),
        docking: true,
        multi_viewport: false,
    }));

    let context = app
        .world()
        .get_non_send::<ImguiContext>()
        .expect("plugin should install the Dear ImGui context");
    assert_eq!(
        context
            .context()
            .io()
            .backend_platform_name()
            .expect("plugin should set a sanitized BackendPlatformName")
            .to_str()
            .expect("sanitized backend name should be valid UTF-8"),
        "bad?name"
    );
}

#[test]
fn status_multi_viewport_request_reports_exact_enablement_boundary() {
    let _guard = imgui_context_guard();

    let mut app = App::new();
    app.add_plugins(ImguiPlugin::new(ImguiBackendConfig {
        name: "multi-viewport-status".to_owned(),
        docking: true,
        multi_viewport: true,
    }));

    let status = app.world().resource::<ImguiBackendStatus>();
    assert!(status.multi_viewport_requested);
    assert_eq!(status.render_feature_enabled, cfg!(feature = "render"));
    assert!(!status.render_integration_installed);
    assert_eq!(
        status.multi_viewport_feature_enabled,
        cfg!(feature = "multi-viewport")
    );
    assert_eq!(status.native_platform_target, !cfg!(target_arch = "wasm32"));
    assert_eq!(
        status.viewport_lifecycle_bridge_enabled,
        cfg!(all(feature = "multi-viewport", not(target_arch = "wasm32")))
    );
    assert_eq!(
        status.viewport_input_feedback_enabled,
        cfg!(all(feature = "multi-viewport", not(target_arch = "wasm32"))),
        "DMV-050 proves all-window input/focus/DPI/IME feedback for native multi-viewport builds"
    );
    assert!(
        !status.viewport_render_routing_enabled,
        "Render routing should not be advertised until the Bevy RenderApp integration is installed"
    );
    assert!(!status.multi_viewport_supported);
}

#[cfg(feature = "render")]
#[test]
fn status_reports_render_routing_only_after_render_app_installation() {
    let _guard = imgui_context_guard();

    let mut app = App::new();
    app.add_plugins(ExtractPlugin::default());
    app.sub_app_mut(RenderApp).update_schedule = Some(Render.intern());
    app.add_plugins(ImguiPlugin::new(ImguiBackendConfig {
        name: "render-status".to_owned(),
        docking: true,
        multi_viewport: true,
    }));

    let status = app.world().resource::<ImguiBackendStatus>();
    assert!(status.render_feature_enabled);
    assert!(status.render_integration_installed);
    assert_eq!(
        status.viewport_render_routing_enabled,
        cfg!(all(feature = "multi-viewport", not(target_arch = "wasm32")))
    );
    assert_eq!(
        status.multi_viewport_supported,
        cfg!(all(feature = "multi-viewport", not(target_arch = "wasm32")))
    );

    let context = app
        .world()
        .get_non_send::<ImguiContext>()
        .expect("plugin should install the Dear ImGui context");
    assert_eq!(
        context
            .context()
            .io()
            .backend_renderer_name()
            .expect("render integration should set BackendRendererName")
            .to_str()
            .expect("backend name should be valid UTF-8"),
        "render-status"
    );
    assert!(
        context
            .context()
            .io()
            .backend_flags()
            .contains(dear_imgui_rs::BackendFlags::RENDERER_HAS_TEXTURES),
        "render integration must advertise ImGui 1.92 texture request support"
    );
    assert!(
        context
            .context()
            .io()
            .backend_flags()
            .contains(dear_imgui_rs::BackendFlags::RENDERER_HAS_VTX_OFFSET),
        "render integration must advertise support for draw command vertex offsets"
    );
}
