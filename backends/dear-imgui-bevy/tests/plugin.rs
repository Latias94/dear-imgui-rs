use bevy_app::App;
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

    assert!(app.world().get_non_send::<ImguiContext>().is_some());
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
    assert_eq!(
        status.viewport_render_routing_enabled,
        cfg!(all(
            feature = "render",
            feature = "multi-viewport",
            not(target_arch = "wasm32")
        ))
    );
    assert_eq!(
        status.multi_viewport_supported,
        cfg!(all(
            feature = "render",
            feature = "multi-viewport",
            not(target_arch = "wasm32")
        ))
    );
    assert!(app.world().get_non_send::<ImguiContext>().is_some());
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
    assert_eq!(
        status.viewport_render_routing_enabled,
        cfg!(all(
            feature = "render",
            feature = "multi-viewport",
            not(target_arch = "wasm32")
        )),
        "DMV-060 enables secondary viewport render routing for native render,multi-viewport builds"
    );
    assert_eq!(
        status.multi_viewport_supported,
        cfg!(all(
            feature = "render",
            feature = "multi-viewport",
            not(target_arch = "wasm32")
        ))
    );
}
