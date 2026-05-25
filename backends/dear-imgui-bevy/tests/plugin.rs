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

unsafe extern "C" fn stale_draw_callback(
    _parent_list: *const dear_imgui_rs::sys::ImDrawList,
    _cmd: *const dear_imgui_rs::sys::ImDrawCmd,
) {
}

unsafe extern "C" fn stale_renderer_window_callback(
    _viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
) {
}

unsafe extern "C" fn stale_renderer_size_callback(
    _viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
    _size: dear_imgui_rs::sys::ImVec2,
) {
}

unsafe extern "C" fn stale_renderer_render_callback(
    _viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
    _render_arg: *mut std::ffi::c_void,
) {
}

fn install_stale_renderer_backend_handlers(context: &mut dear_imgui_rs::Context) {
    let platform_io = context.platform_io_mut();
    platform_io.set_draw_callback_reset_render_state_raw(Some(stale_draw_callback));
    platform_io.set_draw_callback_set_sampler_linear_raw(Some(stale_draw_callback));
    platform_io.set_draw_callback_set_sampler_nearest_raw(Some(stale_draw_callback));
    unsafe {
        platform_io.set_renderer_render_state(std::ptr::dangling_mut::<u8>().cast());
        let raw = platform_io.as_raw_mut();
        (*raw).Renderer_TextureMaxWidth = 1234;
        (*raw).Renderer_TextureMaxHeight = 5678;
        (*raw).Renderer_CreateWindow = Some(stale_renderer_window_callback);
        (*raw).Renderer_DestroyWindow = Some(stale_renderer_window_callback);
        (*raw).Renderer_SetWindowSize = Some(stale_renderer_size_callback);
        (*raw).Renderer_RenderWindow = Some(stale_renderer_render_callback);
        (*raw).Renderer_SwapBuffers = Some(stale_renderer_render_callback);
    }
}

fn assert_stale_renderer_backend_handlers_cleared(context: &dear_imgui_rs::Context) {
    let platform_io = context.platform_io();
    assert!(
        platform_io.draw_callback_reset_render_state_raw().is_none(),
        "stale renderer reset draw callback should be cleared"
    );
    assert!(
        platform_io.draw_callback_set_sampler_linear_raw().is_none(),
        "stale renderer linear sampler draw callback should be cleared"
    );
    assert!(
        platform_io
            .draw_callback_set_sampler_nearest_raw()
            .is_none(),
        "stale renderer nearest sampler draw callback should be cleared"
    );
    assert!(
        unsafe { platform_io.renderer_render_state() }.is_null(),
        "stale renderer render-state pointer should be cleared"
    );
    let raw = unsafe { &*platform_io.as_raw() };
    assert_eq!(raw.Renderer_TextureMaxWidth, 0);
    assert_eq!(raw.Renderer_TextureMaxHeight, 0);
    assert!(raw.Renderer_CreateWindow.is_none());
    assert!(raw.Renderer_DestroyWindow.is_none());
    assert!(raw.Renderer_SetWindowSize.is_none());
    assert!(raw.Renderer_RenderWindow.is_none());
    assert!(raw.Renderer_SwapBuffers.is_none());
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
    let mut existing_context = ImguiContext::new(dear_imgui_rs::Context::create());
    existing_context
        .context_mut()
        .io_mut()
        .set_backend_renderer_user_data(std::ptr::dangling_mut::<u8>().cast());
    existing_context
        .context_mut()
        .io_mut()
        .set_backend_platform_user_data(std::ptr::dangling_mut::<u8>().cast());
    install_stale_renderer_backend_handlers(existing_context.context_mut());
    app.insert_non_send(existing_context);

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
    assert!(
        io.backend_renderer_user_data().is_null(),
        "plugin should clear stale renderer user data before advertising Bevy renderer state"
    );
    assert_stale_renderer_backend_handlers_cleared(context.context());
    assert_eq!(
        io.backend_platform_user_data().is_null(),
        !cfg!(all(feature = "multi-viewport", not(target_arch = "wasm32"))),
        "plugin should clear stale platform user data unless native multi-viewport installs the Bevy bridge"
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

#[cfg(feature = "render")]
#[test]
fn plugin_replaces_stale_renderer_callbacks_when_render_app_is_installed() {
    let _guard = imgui_context_guard();

    let mut app = App::new();
    app.add_plugins(ExtractPlugin::default());
    app.sub_app_mut(RenderApp).update_schedule = Some(Render.intern());
    app.insert_resource(ImguiBackendConfig {
        name: "replace-render-callbacks".to_owned(),
        docking: true,
        multi_viewport: false,
    });
    let mut existing_context = ImguiContext::new(dear_imgui_rs::Context::create());
    install_stale_renderer_backend_handlers(existing_context.context_mut());
    app.insert_non_send(existing_context);

    app.add_plugins(ImguiPlugin::default());

    let context = app
        .world()
        .get_non_send::<ImguiContext>()
        .expect("plugin should preserve the existing Dear ImGui context");
    let platform_io = context.context().platform_io();
    assert!(platform_io.draw_callback_reset_render_state_raw().is_some());
    assert_ne!(
        platform_io
            .draw_callback_reset_render_state_raw()
            .map(|f| f as usize),
        Some(stale_draw_callback as *const () as usize),
        "render integration should replace stale reset callbacks with Bevy callbacks"
    );
    assert_ne!(
        platform_io
            .draw_callback_set_sampler_linear_raw()
            .map(|f| f as usize),
        Some(stale_draw_callback as *const () as usize),
        "render integration should replace stale linear sampler callbacks with Bevy callbacks"
    );
    assert_ne!(
        platform_io
            .draw_callback_set_sampler_nearest_raw()
            .map(|f| f as usize),
        Some(stale_draw_callback as *const () as usize),
        "render integration should replace stale nearest sampler callbacks with Bevy callbacks"
    );
    assert!(
        unsafe { platform_io.renderer_render_state() }.is_null(),
        "render integration should clear stale renderer render-state pointers before installing Bevy callbacks"
    );
    let raw = unsafe { &*platform_io.as_raw() };
    assert_eq!(raw.Renderer_TextureMaxWidth, 0);
    assert_eq!(raw.Renderer_TextureMaxHeight, 0);
    assert!(raw.Renderer_CreateWindow.is_none());
    assert!(raw.Renderer_DestroyWindow.is_none());
    assert!(raw.Renderer_SetWindowSize.is_none());
    assert!(raw.Renderer_RenderWindow.is_none());
    assert!(raw.Renderer_SwapBuffers.is_none());
}

#[cfg(feature = "render")]
#[test]
fn context_into_inner_clears_renderer_backend_state() {
    let _guard = imgui_context_guard();

    let mut app = App::new();
    app.add_plugins(ExtractPlugin::default());
    app.sub_app_mut(RenderApp).update_schedule = Some(Render.intern());
    app.add_plugins(ImguiPlugin::new(ImguiBackendConfig {
        name: "renderer-cleanup".to_owned(),
        docking: true,
        multi_viewport: true,
    }));
    {
        let mut context = app.world_mut().get_non_send_mut::<ImguiContext>().unwrap();
        let context = context.context_mut();
        let backend_flags =
            context.io().backend_flags() | dear_imgui_rs::BackendFlags::HAS_MOUSE_HOVERED_VIEWPORT;
        context.io_mut().set_backend_flags(backend_flags);
    }

    let context = app
        .world_mut()
        .remove_non_send::<ImguiContext>()
        .expect("ImguiContext should be removable for direct shutdown testing");
    let context = context.into_inner();
    let io = context.io();

    assert!(
        io.backend_platform_name().is_none(),
        "releasing the Bevy wrapper must clear BackendPlatformName"
    );
    assert!(
        io.backend_platform_user_data().is_null(),
        "releasing the Bevy wrapper must clear BackendPlatformUserData"
    );
    assert!(
        io.backend_renderer_name().is_none(),
        "releasing the Bevy wrapper must clear BackendRendererName"
    );
    assert!(
        io.backend_renderer_user_data().is_null(),
        "releasing the Bevy wrapper must clear BackendRendererUserData"
    );
    assert_stale_renderer_backend_handlers_cleared(&context);
    assert!(
        !io.backend_flags().intersects(
            dear_imgui_rs::BackendFlags::RENDERER_HAS_TEXTURES
                | dear_imgui_rs::BackendFlags::RENDERER_HAS_VTX_OFFSET
                | dear_imgui_rs::BackendFlags::HAS_MOUSE_HOVERED_VIEWPORT
        ),
        "releasing the Bevy wrapper must clear advertised Bevy backend capabilities"
    );
}
