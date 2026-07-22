use bevy_app::App;
#[cfg(feature = "render")]
use bevy_ecs::schedule::ScheduleLabel;
#[cfg(feature = "render")]
use bevy_render::{Render, RenderApp, extract_plugin::ExtractPlugin};
use dear_imgui_bevy::{
    BEVY_TARGET_COMMIT, BEVY_TARGET_VERSION, ImguiBackendConfig, ImguiBackendStatus, ImguiContext,
    ImguiPlugin, RUST_TARGET_VERSION, WGPU_TARGET_VERSION,
};
use std::sync::{Arc, Mutex, OnceLock};

fn imgui_context_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(())).lock().unwrap()
}

#[derive(Clone)]
struct TestClipboardBackend {
    value: Arc<Mutex<Option<String>>>,
}

impl dear_imgui_rs::ClipboardBackend for TestClipboardBackend {
    fn get(&mut self) -> Option<String> {
        self.value.lock().unwrap().clone()
    }

    fn set(&mut self, text: &str) {
        *self.value.lock().unwrap() = Some(text.to_owned());
    }
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

unsafe extern "C" fn stale_platform_open_in_shell_callback(
    _ctx: *mut dear_imgui_rs::sys::ImGuiContext,
    _path: *const std::ffi::c_char,
) -> bool {
    false
}

unsafe extern "C" fn stale_platform_ime_callback(
    _ctx: *mut dear_imgui_rs::sys::ImGuiContext,
    _viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
    _data: *mut dear_imgui_rs::sys::ImGuiPlatformImeData,
) {
}

unsafe extern "C" fn stale_platform_window_callback(
    _viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
) {
}

unsafe extern "C" fn stale_platform_vec2_callback(
    _viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
) -> dear_imgui_rs::sys::ImVec2 {
    dear_imgui_rs::sys::ImVec2 { x: 1.0, y: 2.0 }
}

unsafe extern "C" fn stale_platform_size_callback(
    _viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
    _size: dear_imgui_rs::sys::ImVec2,
) {
}

unsafe extern "C" fn stale_platform_title_callback(
    _viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
    _title: *const std::ffi::c_char,
) {
}

unsafe extern "C" fn stale_platform_alpha_callback(
    _viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
    _alpha: f32,
) {
}

unsafe extern "C" fn stale_platform_render_callback(
    _viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
    _render_arg: *mut std::ffi::c_void,
) {
}

unsafe extern "C" fn stale_platform_bool_callback(
    _viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
) -> bool {
    true
}

unsafe extern "C" fn stale_platform_f32_callback(
    _viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
) -> f32 {
    1.0
}

unsafe extern "C" fn stale_platform_vec4_callback(
    _viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
) -> dear_imgui_rs::sys::ImVec4 {
    dear_imgui_rs::sys::ImVec4 {
        x: 1.0,
        y: 2.0,
        z: 3.0,
        w: 4.0,
    }
}

unsafe extern "C" fn stale_platform_vk_surface_callback(
    _viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
    _vk_inst: dear_imgui_rs::sys::ImU64,
    _vk_allocators: *const std::ffi::c_void,
    _out_vk_surface: *mut dear_imgui_rs::sys::ImU64,
) -> std::os::raw::c_int {
    0
}

fn install_stale_platform_backend_handlers(context: &mut dear_imgui_rs::Context) {
    let platform_io = context.platform_io_mut();
    unsafe {
        let raw = platform_io.as_raw_mut();
        (*raw).Platform_OpenInShellFn = Some(stale_platform_open_in_shell_callback);
        (*raw).Platform_OpenInShellUserData = std::ptr::dangling_mut::<u8>().cast();
        (*raw).Platform_SetImeDataFn = Some(stale_platform_ime_callback);
        (*raw).Platform_ImeUserData = std::ptr::dangling_mut::<u8>().cast();
        (*raw).Platform_CreateWindow = Some(stale_platform_window_callback);
        (*raw).Platform_DestroyWindow = Some(stale_platform_window_callback);
        (*raw).Platform_ShowWindow = Some(stale_platform_window_callback);
        (*raw).Platform_SetWindowPos = Some(stale_platform_size_callback);
        (*raw).Platform_GetWindowPos = Some(stale_platform_vec2_callback);
        (*raw).Platform_SetWindowSize = Some(stale_platform_size_callback);
        (*raw).Platform_GetWindowSize = Some(stale_platform_vec2_callback);
        (*raw).Platform_GetWindowFramebufferScale = Some(stale_platform_vec2_callback);
        (*raw).Platform_SetWindowFocus = Some(stale_platform_window_callback);
        (*raw).Platform_GetWindowFocus = Some(stale_platform_bool_callback);
        (*raw).Platform_GetWindowMinimized = Some(stale_platform_bool_callback);
        (*raw).Platform_SetWindowTitle = Some(stale_platform_title_callback);
        (*raw).Platform_SetWindowAlpha = Some(stale_platform_alpha_callback);
        (*raw).Platform_UpdateWindow = Some(stale_platform_window_callback);
        (*raw).Platform_RenderWindow = Some(stale_platform_render_callback);
        (*raw).Platform_SwapBuffers = Some(stale_platform_render_callback);
        (*raw).Platform_GetWindowDpiScale = Some(stale_platform_f32_callback);
        (*raw).Platform_OnChangedViewport = Some(stale_platform_window_callback);
        (*raw).Platform_GetWindowWorkAreaInsets = Some(stale_platform_vec4_callback);
        (*raw).Platform_CreateVkSurface = Some(stale_platform_vk_surface_callback);
    }
}

fn assert_stale_platform_backend_handlers_preserved(context: &dear_imgui_rs::Context) {
    let raw = unsafe { &*context.platform_io().as_raw() };
    assert_eq!(
        raw.Platform_OpenInShellFn.map(|f| f as usize),
        Some(stale_platform_open_in_shell_callback as *const () as usize)
    );
    assert_eq!(
        raw.Platform_OpenInShellUserData,
        std::ptr::dangling_mut::<u8>().cast()
    );
    assert_eq!(
        raw.Platform_SetImeDataFn.map(|f| f as usize),
        Some(stale_platform_ime_callback as *const () as usize)
    );
    assert_eq!(
        raw.Platform_ImeUserData,
        std::ptr::dangling_mut::<u8>().cast()
    );
    assert_eq!(
        raw.Platform_CreateWindow.map(|f| f as usize),
        Some(stale_platform_window_callback as *const () as usize)
    );
    assert_eq!(
        raw.Platform_DestroyWindow.map(|f| f as usize),
        Some(stale_platform_window_callback as *const () as usize)
    );
    assert_eq!(
        raw.Platform_ShowWindow.map(|f| f as usize),
        Some(stale_platform_window_callback as *const () as usize)
    );
    assert_eq!(
        raw.Platform_SetWindowPos.map(|f| f as usize),
        Some(stale_platform_size_callback as *const () as usize)
    );
    assert_eq!(
        raw.Platform_GetWindowPos.map(|f| f as usize),
        Some(stale_platform_vec2_callback as *const () as usize)
    );
    assert_eq!(
        raw.Platform_SetWindowSize.map(|f| f as usize),
        Some(stale_platform_size_callback as *const () as usize)
    );
    assert_eq!(
        raw.Platform_GetWindowSize.map(|f| f as usize),
        Some(stale_platform_vec2_callback as *const () as usize)
    );
    assert_eq!(
        raw.Platform_GetWindowFramebufferScale.map(|f| f as usize),
        Some(stale_platform_vec2_callback as *const () as usize)
    );
    assert_eq!(
        raw.Platform_SetWindowFocus.map(|f| f as usize),
        Some(stale_platform_window_callback as *const () as usize)
    );
    assert_eq!(
        raw.Platform_GetWindowFocus.map(|f| f as usize),
        Some(stale_platform_bool_callback as *const () as usize)
    );
    assert_eq!(
        raw.Platform_GetWindowMinimized.map(|f| f as usize),
        Some(stale_platform_bool_callback as *const () as usize)
    );
    assert_eq!(
        raw.Platform_SetWindowTitle.map(|f| f as usize),
        Some(stale_platform_title_callback as *const () as usize)
    );
    assert_eq!(
        raw.Platform_SetWindowAlpha.map(|f| f as usize),
        Some(stale_platform_alpha_callback as *const () as usize)
    );
    assert_eq!(
        raw.Platform_UpdateWindow.map(|f| f as usize),
        Some(stale_platform_window_callback as *const () as usize)
    );
    assert_eq!(
        raw.Platform_RenderWindow.map(|f| f as usize),
        Some(stale_platform_render_callback as *const () as usize)
    );
    assert_eq!(
        raw.Platform_SwapBuffers.map(|f| f as usize),
        Some(stale_platform_render_callback as *const () as usize)
    );
    assert_eq!(
        raw.Platform_GetWindowDpiScale.map(|f| f as usize),
        Some(stale_platform_f32_callback as *const () as usize)
    );
    assert_eq!(
        raw.Platform_OnChangedViewport.map(|f| f as usize),
        Some(stale_platform_window_callback as *const () as usize)
    );
    assert_eq!(
        raw.Platform_GetWindowWorkAreaInsets.map(|f| f as usize),
        Some(stale_platform_vec4_callback as *const () as usize)
    );
    assert_eq!(
        raw.Platform_CreateVkSurface.map(|f| f as usize),
        Some(stale_platform_vk_surface_callback as *const () as usize)
    );
}

fn install_stale_renderer_backend_handlers(context: &mut dear_imgui_rs::Context) {
    let platform_io = context.platform_io_mut();
    unsafe {
        platform_io.set_draw_callback_reset_render_state_raw(Some(stale_draw_callback));
        platform_io.set_draw_callback_set_sampler_linear_raw(Some(stale_draw_callback));
        platform_io.set_draw_callback_set_sampler_nearest_raw(Some(stale_draw_callback));
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

fn clear_test_foreign_backend_state(context: &mut dear_imgui_rs::Context) {
    unsafe {
        let platform_io = dear_imgui_rs::sys::igGetPlatformIO_ContextPtr(context.as_raw());
        dear_imgui_rs::sys::ImGuiPlatformIO_ClearPlatformHandlers(platform_io);
        dear_imgui_rs::sys::ImGuiPlatformIO_ClearRendererHandlers(platform_io);
        let io = dear_imgui_rs::sys::igGetIO_ContextPtr(context.as_raw());
        (*io).BackendPlatformUserData = std::ptr::null_mut();
        (*io).BackendRendererUserData = std::ptr::null_mut();
        let main_viewport = dear_imgui_rs::sys::igGetMainViewport();
        if !main_viewport.is_null() {
            (*main_viewport).PlatformUserData = std::ptr::null_mut();
            (*main_viewport).PlatformHandle = std::ptr::null_mut();
            (*main_viewport).PlatformHandleRaw = std::ptr::null_mut();
            (*main_viewport).PlatformWindowCreated = false;
        }
    }
    context
        .set_platform_name::<String>(None)
        .expect("test cleanup should clear platform name");
    context
        .set_renderer_name::<String>(None)
        .expect("test cleanup should clear renderer name");
    let flags_to_clear = dear_imgui_rs::BackendFlags::RENDERER_HAS_TEXTURES
        | dear_imgui_rs::BackendFlags::RENDERER_HAS_VTX_OFFSET;
    #[cfg(feature = "multi-viewport")]
    let flags_to_clear = flags_to_clear
        | dear_imgui_rs::BackendFlags::PLATFORM_HAS_VIEWPORTS
        | dear_imgui_rs::BackendFlags::RENDERER_HAS_VIEWPORTS
        | dear_imgui_rs::BackendFlags::HAS_MOUSE_HOVERED_VIEWPORT;
    let mut flags = context.io().backend_flags();
    flags.remove(flags_to_clear);
    context.io_mut().set_backend_flags(flags);
}

fn assert_bevy_platform_window_callbacks_cleared(context: &dear_imgui_rs::Context) {
    let raw = unsafe { &*context.platform_io().as_raw() };
    assert!(raw.Platform_CreateWindow.is_none());
    assert!(raw.Platform_DestroyWindow.is_none());
    assert!(raw.Platform_ShowWindow.is_none());
    assert!(raw.Platform_SetWindowPos.is_none());
    assert!(raw.Platform_GetWindowPos.is_none());
    assert!(raw.Platform_SetWindowSize.is_none());
    assert!(raw.Platform_GetWindowSize.is_none());
    assert!(raw.Platform_GetWindowFramebufferScale.is_none());
    assert!(raw.Platform_SetWindowFocus.is_none());
    assert!(raw.Platform_GetWindowFocus.is_none());
    assert!(raw.Platform_GetWindowMinimized.is_none());
    assert!(raw.Platform_SetWindowTitle.is_none());
    assert!(raw.Platform_SetWindowAlpha.is_none());
    assert!(raw.Platform_UpdateWindow.is_none());
    assert!(raw.Platform_RenderWindow.is_none());
    assert!(raw.Platform_SwapBuffers.is_none());
    assert!(raw.Platform_GetWindowDpiScale.is_none());
    assert!(raw.Platform_OnChangedViewport.is_none());
    assert!(raw.Platform_GetWindowWorkAreaInsets.is_none());
    assert!(raw.Platform_CreateVkSurface.is_none());
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

fn assert_stale_renderer_backend_handlers_preserved(context: &dear_imgui_rs::Context) {
    let platform_io = context.platform_io();
    assert!(
        platform_io
            .draw_callback_reset_render_state_raw()
            .is_some_and(|callback| {
                std::ptr::fn_addr_eq(
                    callback,
                    stale_draw_callback
                        as unsafe extern "C" fn(
                            *const dear_imgui_rs::sys::ImDrawList,
                            *const dear_imgui_rs::sys::ImDrawCmd,
                        ),
                )
            })
    );
    assert!(
        platform_io
            .draw_callback_set_sampler_linear_raw()
            .is_some_and(|callback| {
                std::ptr::fn_addr_eq(
                    callback,
                    stale_draw_callback
                        as unsafe extern "C" fn(
                            *const dear_imgui_rs::sys::ImDrawList,
                            *const dear_imgui_rs::sys::ImDrawCmd,
                        ),
                )
            })
    );
    assert!(
        platform_io
            .draw_callback_set_sampler_nearest_raw()
            .is_some_and(|callback| {
                std::ptr::fn_addr_eq(
                    callback,
                    stale_draw_callback
                        as unsafe extern "C" fn(
                            *const dear_imgui_rs::sys::ImDrawList,
                            *const dear_imgui_rs::sys::ImDrawCmd,
                        ),
                )
            })
    );
    assert_eq!(
        unsafe { platform_io.renderer_render_state() },
        std::ptr::dangling_mut::<u8>().cast()
    );
    let raw = unsafe { &*platform_io.as_raw() };
    assert_eq!(raw.Renderer_TextureMaxWidth, 1234);
    assert_eq!(raw.Renderer_TextureMaxHeight, 5678);
    assert!(raw.Renderer_CreateWindow.is_some_and(|callback| {
        std::ptr::fn_addr_eq(
            callback,
            stale_renderer_window_callback
                as unsafe extern "C" fn(*mut dear_imgui_rs::sys::ImGuiViewport),
        )
    }));
    assert!(raw.Renderer_DestroyWindow.is_some_and(|callback| {
        std::ptr::fn_addr_eq(
            callback,
            stale_renderer_window_callback
                as unsafe extern "C" fn(*mut dear_imgui_rs::sys::ImGuiViewport),
        )
    }));
    assert!(raw.Renderer_SetWindowSize.is_some_and(|callback| {
        std::ptr::fn_addr_eq(
            callback,
            stale_renderer_size_callback
                as unsafe extern "C" fn(
                    *mut dear_imgui_rs::sys::ImGuiViewport,
                    dear_imgui_rs::sys::ImVec2,
                ),
        )
    }));
    assert!(raw.Renderer_RenderWindow.is_some_and(|callback| {
        std::ptr::fn_addr_eq(
            callback,
            stale_renderer_render_callback
                as unsafe extern "C" fn(
                    *mut dear_imgui_rs::sys::ImGuiViewport,
                    *mut std::ffi::c_void,
                ),
        )
    }));
    assert!(raw.Renderer_SwapBuffers.is_some_and(|callback| {
        std::ptr::fn_addr_eq(
            callback,
            stale_renderer_render_callback
                as unsafe extern "C" fn(
                    *mut dear_imgui_rs::sys::ImGuiViewport,
                    *mut std::ffi::c_void,
                ),
        )
    }));
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
    assert_eq!(BEVY_TARGET_VERSION, "0.19.0");
    assert_eq!(
        BEVY_TARGET_COMMIT,
        "c6f634ca9f406d68ba5109d921247b654cb42c10"
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
        viewport_window: Default::default(),
    });
    let mut existing_context = ImguiContext::new(dear_imgui_rs::Context::create());
    unsafe {
        // These dangling values model stale foreign ownership and are never dereferenced.
        existing_context
            .context_mut()
            .io_mut()
            .set_backend_renderer_user_data(std::ptr::dangling_mut::<u8>().cast());
        existing_context
            .context_mut()
            .io_mut()
            .set_backend_platform_user_data(std::ptr::dangling_mut::<u8>().cast());
    }
    install_stale_platform_backend_handlers(existing_context.context_mut());
    install_stale_renderer_backend_handlers(existing_context.context_mut());
    app.insert_non_send(existing_context);

    let plugin_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        app.add_plugins(ImguiPlugin::default());
    }));

    if cfg!(all(feature = "multi-viewport", not(target_arch = "wasm32"))) {
        assert!(
            plugin_result.is_err(),
            "native multi-viewport must reject an already-owned PlatformIO table"
        );
        let context = app
            .world()
            .get_non_send::<ImguiContext>()
            .expect("the rejected plugin build must preserve the existing context");
        assert_eq!(
            context.context().io().backend_platform_user_data(),
            std::ptr::dangling_mut::<u8>().cast(),
            "failed attachment must preserve foreign BackendPlatformUserData"
        );
        assert_stale_platform_backend_handlers_preserved(context.context());
        let mut context = app
            .world_mut()
            .get_non_send_mut::<ImguiContext>()
            .expect("the rejected plugin build must preserve the existing context");
        clear_test_foreign_backend_state(context.context_mut());
        return;
    }
    plugin_result.expect("plugin installation should succeed without the native viewport bridge");

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
    assert!(
        io.backend_platform_name().is_none(),
        "plugin must not claim BackendPlatformName while foreign platform state is present"
    );
    assert_eq!(
        io.backend_renderer_user_data(),
        std::ptr::dangling_mut::<u8>().cast(),
        "plugin must preserve foreign renderer user data when no renderer integration is installed"
    );
    assert_stale_renderer_backend_handlers_preserved(context.context());
    assert_eq!(
        io.backend_platform_user_data(),
        std::ptr::dangling_mut::<u8>().cast(),
        "plugin must preserve foreign platform user data when bridge installation is rejected"
    );
    assert_stale_platform_backend_handlers_preserved(context.context());
    let mut context = app
        .world_mut()
        .get_non_send_mut::<ImguiContext>()
        .expect("the existing context should remain owned by the app");
    clear_test_foreign_backend_state(context.context_mut());
}

#[test]
fn plugin_preserves_existing_context_clipboard_backend() {
    let _guard = imgui_context_guard();

    let mut app = App::new();
    let clipboard_value = Arc::new(Mutex::new(None));
    let mut existing_context = dear_imgui_rs::Context::create();
    existing_context.set_clipboard_backend(TestClipboardBackend {
        value: clipboard_value.clone(),
    });
    existing_context.set_clipboard_text("before-plugin");
    app.insert_non_send(ImguiContext::new(existing_context));

    app.add_plugins(ImguiPlugin::default());

    let context = app
        .world()
        .get_non_send::<ImguiContext>()
        .expect("plugin should preserve the existing Dear ImGui context");
    context.context().set_clipboard_text("after-plugin");
    assert_eq!(
        context.context().clipboard_text().as_deref(),
        Some("after-plugin")
    );
    assert_eq!(
        clipboard_value.lock().unwrap().as_deref(),
        Some("after-plugin")
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
        viewport_window: Default::default(),
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
        viewport_window: Default::default(),
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

#[test]
fn plugin_preserves_stale_platform_handlers_when_bridge_is_not_installed() {
    let _guard = imgui_context_guard();

    let mut app = App::new();
    app.insert_resource(ImguiBackendConfig {
        name: "clear-platform-handlers".to_owned(),
        docking: true,
        multi_viewport: false,
        viewport_window: Default::default(),
    });
    let mut existing_context = ImguiContext::new(dear_imgui_rs::Context::create());
    unsafe {
        // This dangling value models stale foreign ownership and is never dereferenced.
        existing_context
            .context_mut()
            .io_mut()
            .set_backend_platform_user_data(std::ptr::dangling_mut::<u8>().cast());
    }
    install_stale_platform_backend_handlers(existing_context.context_mut());
    app.insert_non_send(existing_context);

    app.add_plugins(ImguiPlugin::default());

    let context = app
        .world()
        .get_non_send::<ImguiContext>()
        .expect("plugin should preserve the existing Dear ImGui context");
    assert_eq!(
        context.context().io().backend_platform_user_data(),
        std::ptr::dangling_mut::<u8>().cast(),
        "plugin must not clear a platform backend it does not own"
    );
    assert_stale_platform_backend_handlers_preserved(context.context());
    let mut context = app
        .world_mut()
        .get_non_send_mut::<ImguiContext>()
        .expect("plugin should preserve the existing Dear ImGui context");
    clear_test_foreign_backend_state(context.context_mut());
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
        viewport_window: Default::default(),
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
fn plugin_rejects_foreign_renderer_state_when_render_app_is_installed() {
    let _guard = imgui_context_guard();

    let mut app = App::new();
    app.add_plugins(ExtractPlugin::default());
    app.sub_app_mut(RenderApp).update_schedule = Some(Render.intern());
    app.insert_resource(ImguiBackendConfig {
        name: "replace-render-callbacks".to_owned(),
        docking: true,
        multi_viewport: false,
        viewport_window: Default::default(),
    });
    let mut existing_context = ImguiContext::new(dear_imgui_rs::Context::create());
    install_stale_renderer_backend_handlers(existing_context.context_mut());
    app.insert_non_send(existing_context);

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        app.add_plugins(ImguiPlugin::default());
    }));
    assert!(
        result.is_err(),
        "render integration must reject renderer state it does not own"
    );

    let mut context = app
        .world_mut()
        .get_non_send_mut::<ImguiContext>()
        .expect("rejected installation must preserve the existing Dear ImGui context");
    assert_stale_renderer_backend_handlers_preserved(context.context());

    // The claim must fail before `ensure_renderer_consumer` touches the snapshot hub or font
    // atlas. A fresh consumer is therefore still claimable from the preserved context.
    let consumer = context
        .context_mut()
        .create_renderer_consumer()
        .expect("renderer preflight failure must not claim the consumer");
    let _ = context
        .context_mut()
        .prepare_renderer_texture_reset(&consumer)
        .expect("the probe consumer should be idle")
        .commit();
    drop(consumer);
    clear_test_foreign_backend_state(context.context_mut());
}

#[cfg(all(feature = "render", feature = "multi-viewport"))]
#[test]
fn combined_renderer_and_viewport_claim_is_zero_mutation_on_renderer_conflict() {
    let _guard = imgui_context_guard();

    let mut app = App::new();
    app.add_plugins(ExtractPlugin::default());
    app.sub_app_mut(RenderApp).update_schedule = Some(Render.intern());
    app.insert_resource(ImguiBackendConfig {
        name: "transactional-combined-claim".to_owned(),
        docking: true,
        multi_viewport: true,
        viewport_window: Default::default(),
    });
    let mut existing_context = ImguiContext::new(dear_imgui_rs::Context::create());
    install_stale_renderer_backend_handlers(existing_context.context_mut());
    app.insert_non_send(existing_context);

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        app.add_plugins(ImguiPlugin::default());
    }));
    assert!(
        result.is_err(),
        "the foreign renderer claim must be rejected"
    );

    let mut context = app.world_mut().get_non_send_mut::<ImguiContext>().unwrap();
    assert_stale_renderer_backend_handlers_preserved(context.context());
    assert!(
        context
            .context()
            .io()
            .backend_platform_user_data()
            .is_null()
    );
    assert!(context.context().io().backend_platform_name().is_none());
    assert_bevy_platform_window_callbacks_cleared(context.context());
    let monitors = unsafe { (*context.context().platform_io().as_raw()).Monitors };
    assert!(monitors.Data.is_null());
    assert_eq!(monitors.Size, 0);
    assert_eq!(monitors.Capacity, 0);
    let main_viewport = context.context_mut().main_viewport();
    assert!(main_viewport.platform_user_data().is_null());
    assert!(main_viewport.platform_handle().is_null());
    assert!(main_viewport.platform_handle_raw().is_null());

    let consumer = context
        .context_mut()
        .create_renderer_consumer()
        .expect("failed combined preflight must not claim the renderer consumer");
    let _ = context
        .context_mut()
        .prepare_renderer_texture_reset(&consumer)
        .unwrap()
        .commit();
    drop(consumer);
    clear_test_foreign_backend_state(context.context_mut());
}

#[cfg(feature = "render")]
#[test]
fn active_renderer_contract_fails_closed_for_every_reserved_field_group() {
    let _guard = imgui_context_guard();

    fn assert_drift(mutator: fn(&mut dear_imgui_rs::Context)) {
        let mut app = App::new();
        app.add_plugins(ExtractPlugin::default());
        app.sub_app_mut(RenderApp).update_schedule = Some(Render.intern());
        app.add_plugins(ImguiPlugin::default());
        mutator(
            app.world_mut()
                .get_non_send_mut::<ImguiContext>()
                .unwrap()
                .context_mut(),
        );

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| app.update()));
        assert!(
            result.is_err(),
            "renderer ownership drift must stop the frame"
        );
        let mut context = app.world_mut().get_non_send_mut::<ImguiContext>().unwrap();
        assert!(
            !context.context().io().backend_flags().intersects(
                dear_imgui_rs::BackendFlags::RENDERER_HAS_TEXTURES
                    | dear_imgui_rs::BackendFlags::RENDERER_HAS_VTX_OFFSET
            ),
            "the first renderer drift must revoke managed-rendering capabilities"
        );
        clear_test_foreign_backend_state(context.context_mut());
    }

    fn replace_user_data(context: &mut dear_imgui_rs::Context) {
        unsafe {
            context
                .io_mut()
                .set_backend_renderer_user_data(std::ptr::dangling_mut::<u8>().cast());
        }
    }
    fn replace_name_with_equal_bytes(context: &mut dear_imgui_rs::Context) {
        context.set_renderer_name(Some("dear-imgui-bevy")).unwrap();
    }
    fn remove_renderer_flag(context: &mut dear_imgui_rs::Context) {
        let flags =
            context.io().backend_flags() & !dear_imgui_rs::BackendFlags::RENDERER_HAS_TEXTURES;
        context.io_mut().set_backend_flags(flags);
    }
    fn replace_render_state(context: &mut dear_imgui_rs::Context) {
        unsafe {
            context
                .platform_io_mut()
                .set_renderer_render_state(std::ptr::dangling_mut::<u8>().cast());
        }
    }
    fn replace_texture_limit(context: &mut dear_imgui_rs::Context) {
        unsafe { (*context.platform_io_mut().as_raw_mut()).Renderer_TextureMaxWidth = 4096 };
    }
    fn install_renderer_callback(context: &mut dear_imgui_rs::Context) {
        unsafe {
            (*context.platform_io_mut().as_raw_mut()).Renderer_DestroyWindow =
                Some(stale_renderer_window_callback);
        }
    }
    fn replace_draw_callback(context: &mut dear_imgui_rs::Context) {
        unsafe {
            context
                .platform_io_mut()
                .set_draw_callback_set_sampler_nearest_raw(Some(stale_draw_callback));
        }
    }

    for mutator in [
        replace_user_data,
        replace_name_with_equal_bytes,
        remove_renderer_flag,
        replace_render_state,
        replace_texture_limit,
        install_renderer_callback,
        replace_draw_callback,
    ] {
        assert_drift(mutator);
    }
}

#[cfg(feature = "render")]
#[test]
fn context_into_inner_preflights_partial_renderer_takeover_before_mutating_owner() {
    let _guard = imgui_context_guard();

    let mut app = App::new();
    app.add_plugins(ExtractPlugin::default());
    app.sub_app_mut(RenderApp).update_schedule = Some(Render.intern());
    app.add_plugins(ImguiPlugin::default());

    {
        let mut owner = app.world_mut().get_non_send_mut::<ImguiContext>().unwrap();
        let context = owner.context_mut();
        let _ = context.font_atlas().build();
        context.prepare_frame(
            dear_imgui_rs::FramePrepareOptions::new([64.0, 64.0], 1.0 / 60.0)
                .renderer_has_textures(),
        );
        let _ = context.frame();
        context.set_renderer_name(Some("foreign-renderer")).unwrap();
    }

    let owner = app
        .world_mut()
        .remove_non_send::<ImguiContext>()
        .expect("ImguiContext should be removable for direct shutdown testing");
    let error = owner
        .into_inner()
        .expect_err("a partial renderer takeover must block Context extraction");
    assert_eq!(
        error.error(),
        dear_imgui_bevy::ImguiContextIntoInnerErrorReason::RendererOwnership(
            dear_imgui_bevy::ImguiRendererOwnershipError::FieldReplaced {
                field: "BackendRendererName",
            },
        )
    );
    let owner = error.into_owner();

    assert_eq!(
        owner.context().frame_lifecycle_state(),
        dear_imgui_rs::FrameLifecycleState::InFrame,
        "renderer ownership preflight must run before ending the native frame"
    );
    assert_eq!(
        owner
            .context()
            .io()
            .backend_renderer_name()
            .unwrap()
            .to_bytes(),
        b"foreign-renderer",
        "failed extraction must preserve the foreign renderer field"
    );
    assert!(
        owner.context().io().backend_flags().contains(
            dear_imgui_rs::BackendFlags::RENDERER_HAS_TEXTURES
                | dear_imgui_rs::BackendFlags::RENDERER_HAS_VTX_OFFSET
        ),
        "failed extraction must not revoke renderer capabilities"
    );
}

#[test]
fn context_into_inner_clears_owned_backend_state() {
    let _guard = imgui_context_guard();

    let mut app = App::new();
    #[cfg(feature = "render")]
    {
        app.add_plugins(ExtractPlugin::default());
        app.sub_app_mut(RenderApp).update_schedule = Some(Render.intern());
    }
    app.add_plugins(ImguiPlugin::new(ImguiBackendConfig {
        name: "renderer-cleanup".to_owned(),
        docking: true,
        multi_viewport: true,
        viewport_window: Default::default(),
    }));
    let context = app
        .world_mut()
        .remove_non_send::<ImguiContext>()
        .expect("ImguiContext should be removable for direct shutdown testing");
    let context = context
        .into_inner()
        .expect("idle renderer state should detach cleanly");
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
    assert_bevy_platform_window_callbacks_cleared(&context);
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

#[cfg(feature = "render")]
#[test]
fn context_into_inner_binds_its_open_frame_and_renderer_teardown_to_its_owner_context() {
    let _guard = imgui_context_guard();

    let foreign = dear_imgui_rs::Context::create();
    let foreign_raw = foreign.as_raw();
    let foreign_binding = foreign.binding();
    let foreign = foreign.suspend();

    let mut app = App::new();
    app.add_plugins(ExtractPlugin::default());
    app.sub_app_mut(RenderApp).update_schedule = Some(Render.intern());
    app.add_plugins(ImguiPlugin::new(ImguiBackendConfig {
        name: "foreign-current-teardown".to_owned(),
        docking: false,
        multi_viewport: false,
        viewport_window: Default::default(),
    }));

    {
        let mut owner = app
            .world_mut()
            .get_non_send_mut::<ImguiContext>()
            .expect("ImguiPlugin should install an ImGui context");
        let context = owner.context_mut();
        let _ = context.font_atlas().build();
        context.io_mut().set_display_size([1.0, 1.0]);
        context.io_mut().set_delta_time(1.0 / 60.0);
        let _ = context.frame();
        assert_eq!(
            context.frame_lifecycle_state(),
            dear_imgui_rs::FrameLifecycleState::InFrame
        );
    }

    foreign_binding.with_bound_context(|| {
        assert_eq!(unsafe { dear_imgui_rs::sys::igGetCurrentContext() }, foreign_raw);
        let owner = app
            .world_mut()
            .remove_non_send::<ImguiContext>()
            .expect("ImguiContext should be removable for explicit shutdown");
        let context = owner
            .into_inner()
            .expect("teardown must bind the Bevy context before ending its frame and resetting renderer state");

        assert_eq!(unsafe { dear_imgui_rs::sys::igGetCurrentContext() }, foreign_raw);
        let binding = context.binding();
        binding.with_bound_context(|| {
            assert_ne!(
                context.frame_lifecycle_state(),
                dear_imgui_rs::FrameLifecycleState::InFrame,
                "explicit teardown must close an open Bevy-owned frame"
            );
        });
        assert_eq!(unsafe { dear_imgui_rs::sys::igGetCurrentContext() }, foreign_raw);
        drop(context);
        assert_eq!(unsafe { dear_imgui_rs::sys::igGetCurrentContext() }, foreign_raw);
    });

    drop(foreign);
}

#[test]
fn context_drop_binds_an_open_frame_to_its_owner_context() {
    let _guard = imgui_context_guard();

    let foreign = dear_imgui_rs::Context::create();
    let foreign_raw = foreign.as_raw();
    let foreign_binding = foreign.binding();
    let foreign = foreign.suspend();

    let mut owner = ImguiContext::new(dear_imgui_rs::Context::create());
    {
        let context = owner.context_mut();
        let _ = context.font_atlas().build();
        context.io_mut().set_display_size([1.0, 1.0]);
        context.io_mut().set_delta_time(1.0 / 60.0);
        let _ = context.frame();
        assert_eq!(
            context.frame_lifecycle_state(),
            dear_imgui_rs::FrameLifecycleState::InFrame
        );
    }

    foreign_binding.with_bound_context(|| {
        drop(owner);
        assert_eq!(
            unsafe { dear_imgui_rs::sys::igGetCurrentContext() },
            foreign_raw
        );
    });

    drop(foreign);
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[test]
fn context_into_inner_preserves_complete_foreign_platform_and_renderer_takeover() {
    let _guard = imgui_context_guard();

    let mut app = App::new();
    #[cfg(feature = "render")]
    {
        app.add_plugins(ExtractPlugin::default());
        app.sub_app_mut(RenderApp).update_schedule = Some(Render.intern());
    }
    app.add_plugins(ImguiPlugin::new(ImguiBackendConfig {
        name: "complete-foreign-takeover".to_owned(),
        docking: true,
        multi_viewport: true,
        viewport_window: Default::default(),
    }));

    let foreign_platform_user_data = std::ptr::dangling_mut::<u16>().cast();
    #[cfg(feature = "render")]
    let foreign_renderer_user_data = std::ptr::dangling_mut::<u32>().cast();
    let foreign_platform_user_data_main = std::ptr::dangling_mut::<u64>().cast();
    let foreign_platform_handle_main = std::ptr::dangling_mut::<u8>().cast();
    let foreign_platform_handle_raw_main = std::ptr::dangling_mut::<usize>().cast();
    let foreign_flags = {
        let mut wrapper = app.world_mut().get_non_send_mut::<ImguiContext>().unwrap();
        let context = wrapper.context_mut();
        install_stale_platform_backend_handlers(context);
        #[cfg(feature = "render")]
        install_stale_renderer_backend_handlers(context);
        context.set_platform_name(Some("foreign-platform")).unwrap();
        #[cfg(feature = "render")]
        context.set_renderer_name(Some("foreign-renderer")).unwrap();
        unsafe {
            context
                .io_mut()
                .set_backend_platform_user_data(foreign_platform_user_data);
            #[cfg(feature = "render")]
            context
                .io_mut()
                .set_backend_renderer_user_data(foreign_renderer_user_data);
            let main = context.main_viewport().as_raw_mut();
            (*main).PlatformUserData = foreign_platform_user_data_main;
            (*main).PlatformHandle = foreign_platform_handle_main;
            (*main).PlatformHandleRaw = foreign_platform_handle_raw_main;
        }
        let flags = context.io().backend_flags()
            | dear_imgui_rs::BackendFlags::PLATFORM_HAS_VIEWPORTS
            | dear_imgui_rs::BackendFlags::RENDERER_HAS_VIEWPORTS
            | dear_imgui_rs::BackendFlags::HAS_MOUSE_HOVERED_VIEWPORT;
        #[cfg(feature = "render")]
        let flags = flags
            | dear_imgui_rs::BackendFlags::RENDERER_HAS_TEXTURES
            | dear_imgui_rs::BackendFlags::RENDERER_HAS_VTX_OFFSET;
        context.io_mut().set_backend_flags(flags);
        let config_flags =
            context.io().config_flags() | dear_imgui_rs::ConfigFlags::VIEWPORTS_ENABLE;
        context.io_mut().set_config_flags(config_flags);
        flags
    };

    let owner = app
        .world_mut()
        .remove_non_send::<ImguiContext>()
        .expect("ImguiContext should be removable for direct shutdown testing");
    let error = owner
        .into_inner()
        .expect_err("the first explicit teardown must report the foreign platform takeover");
    assert_eq!(
        error.error(),
        dear_imgui_bevy::ImguiContextIntoInnerErrorReason::ViewportCallbackOwnership(
            dear_imgui_bevy::viewport::ImguiViewportCallbackOwnershipError::
                BackendPlatformUserDataReplaced,
        )
    );

    let mut context = error
        .into_owner()
        .into_inner()
        .expect("after reporting the takeover, teardown must remain retryable");
    assert_eq!(context.io().backend_flags(), foreign_flags);
    assert!(
        context
            .io()
            .config_flags()
            .contains(dear_imgui_rs::ConfigFlags::VIEWPORTS_ENABLE)
    );
    assert_eq!(
        context.io().backend_platform_user_data(),
        foreign_platform_user_data
    );
    #[cfg(feature = "render")]
    assert_eq!(
        context.io().backend_renderer_user_data(),
        foreign_renderer_user_data
    );
    assert_eq!(
        context.io().backend_platform_name().unwrap().to_bytes(),
        b"foreign-platform"
    );
    #[cfg(feature = "render")]
    assert_eq!(
        context.io().backend_renderer_name().unwrap().to_bytes(),
        b"foreign-renderer"
    );
    unsafe {
        let main = context.main_viewport().as_raw();
        assert_eq!((*main).PlatformUserData, foreign_platform_user_data_main);
        assert_eq!((*main).PlatformHandle, foreign_platform_handle_main);
        assert_eq!((*main).PlatformHandleRaw, foreign_platform_handle_raw_main);
    }
    assert_stale_platform_backend_handlers_preserved(&context);
    #[cfg(feature = "render")]
    assert_stale_renderer_backend_handlers_preserved(&context);

    clear_test_foreign_backend_state(&mut context);
    let mut config_flags = context.io().config_flags();
    config_flags.remove(dear_imgui_rs::ConfigFlags::VIEWPORTS_ENABLE);
    context.io_mut().set_config_flags(config_flags);
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[test]
fn context_into_inner_reports_platform_callback_drift_and_preserves_foreign_callbacks() {
    let _guard = imgui_context_guard();

    let mut app = App::new();
    #[cfg(feature = "render")]
    {
        app.add_plugins(ExtractPlugin::default());
        app.sub_app_mut(RenderApp).update_schedule = Some(Render.intern());
    }
    app.add_plugins(ImguiPlugin::new(ImguiBackendConfig {
        name: "foreign-backend-preservation".to_owned(),
        docking: true,
        multi_viewport: true,
        viewport_window: Default::default(),
    }));
    {
        let mut context = app.world_mut().get_non_send_mut::<ImguiContext>().unwrap();
        let context = context.context_mut();
        install_stale_platform_backend_handlers(context);
    }

    let owner = app
        .world_mut()
        .remove_non_send::<ImguiContext>()
        .expect("ImguiContext should be removable for direct shutdown testing");
    let error = owner
        .into_inner()
        .expect_err("callback ownership drift must be reported before extraction succeeds");
    assert_eq!(
        error.error(),
        dear_imgui_bevy::ImguiContextIntoInnerErrorReason::ViewportCallbackOwnership(
            dear_imgui_bevy::viewport::ImguiViewportCallbackOwnershipError::
                PlatformCallbackReplaced {
                    slot: "Platform_CreateWindow",
                },
        )
    );

    let mut context = error
        .into_owner()
        .into_inner()
        .expect("the safely detached wrapper must remain retryable");
    assert!(
        context.io().backend_platform_user_data().is_null(),
        "Bevy-owned platform user data must be released even when callbacks drift"
    );
    assert!(
        context.io().backend_renderer_user_data().is_null(),
        "Bevy-owned renderer user data must be released"
    );
    assert_stale_platform_backend_handlers_preserved(&context);
    clear_test_foreign_backend_state(&mut context);
}
