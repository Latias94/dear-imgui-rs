#[cfg(feature = "multi-viewport")]
use bevy_app::App;
#[cfg(all(feature = "multi-viewport", feature = "render"))]
use bevy_camera::{Camera, Camera2d, RenderTarget, visibility::RenderLayers};
#[cfg(feature = "multi-viewport")]
use bevy_ecs::message::Messages;
#[cfg(all(feature = "multi-viewport", feature = "render"))]
use bevy_ecs::prelude::Entity;
#[cfg(feature = "multi-viewport")]
use bevy_math::IVec2;
#[cfg(feature = "multi-viewport")]
use bevy_window::Monitor;
#[cfg(feature = "multi-viewport")]
use bevy_window::WindowCloseRequested;
use bevy_window::WindowLevel;
#[cfg(feature = "multi-viewport")]
use bevy_window::WindowMoved;
#[cfg(feature = "multi-viewport")]
use bevy_window::WindowOccluded;
use bevy_window::WindowPosition;
#[cfg(all(feature = "multi-viewport", feature = "render"))]
use bevy_window::WindowRef;
#[cfg(feature = "multi-viewport")]
use bevy_window::WindowResized;
#[cfg(feature = "multi-viewport")]
use bevy_window::{PrimaryWindow, Window};
#[cfg(all(feature = "multi-viewport", feature = "render"))]
use dear_imgui_bevy::ImguiViewportCamera;
use dear_imgui_bevy::ImguiViewportSnapshot;
#[cfg(feature = "multi-viewport")]
use dear_imgui_bevy::{
    ImguiBackendConfig, ImguiBackendStatus, ImguiContext, ImguiPlugin, ImguiViewportBridge,
    ImguiViewportCommand, ImguiViewportFeedback, ImguiViewportWindow,
};
use dear_imgui_rs as imgui;
#[cfg(feature = "multi-viewport")]
use imgui::sys;
#[cfg(feature = "multi-viewport")]
use std::sync::{Mutex, OnceLock};

#[cfg(feature = "multi-viewport")]
static DESTROY_CALLBACK_SAW_NULL_BACKEND_USER_DATA: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

#[cfg(feature = "multi-viewport")]
fn imgui_context_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(())).lock().unwrap()
}

fn viewport_snapshot(id: u32) -> ImguiViewportSnapshot {
    ImguiViewportSnapshot {
        id: imgui::Id::from(id),
        pos: [32.0, 48.0],
        size: [640.0, 360.0],
        dpi_scale: 2.0,
        flags: imgui::ViewportFlags::IS_PLATFORM_WINDOW,
    }
}

#[cfg(feature = "multi-viewport")]
fn app_with_multi_viewport_bridge(name: &str) -> App {
    let mut app = App::new();
    app.add_message::<WindowCloseRequested>();
    app.add_plugins(ImguiPlugin::new(ImguiBackendConfig {
        name: name.to_owned(),
        docking: true,
        multi_viewport: true,
    }));
    {
        let mut context = app.world_mut().get_non_send_mut::<ImguiContext>().unwrap();
        let _ = context.context_mut().font_atlas_mut().build();
    }
    app
}

#[cfg(feature = "multi-viewport")]
fn with_test_platform_viewport(
    app: &mut App,
    id: imgui::Id,
    f: impl FnOnce(&mut App, *mut sys::ImGuiViewport),
) {
    let raw_viewport = unsafe { sys::ImGuiViewport_ImGuiViewport() };
    assert!(
        !raw_viewport.is_null(),
        "ImGuiViewport_ImGuiViewport() returned null"
    );
    unsafe {
        (*raw_viewport).ID = id.raw();
    }

    let mut viewport_ptrs = [raw_viewport];
    let original = {
        let mut context = app.world_mut().get_non_send_mut::<ImguiContext>().unwrap();
        let platform_io = context.context_mut().platform_io_mut().as_raw_mut();
        unsafe {
            let original = (*platform_io).Viewports;
            (*platform_io).Viewports = sys::ImVector_ImGuiViewportPtr {
                Size: 1,
                Capacity: 1,
                Data: viewport_ptrs.as_mut_ptr(),
            };
            original
        }
    };

    f(app, raw_viewport);

    {
        let mut context = app.world_mut().get_non_send_mut::<ImguiContext>().unwrap();
        let platform_io = context.context_mut().platform_io_mut().as_raw_mut();
        unsafe {
            (*platform_io).Viewports = original;
            sys::ImGuiViewport_destroy(raw_viewport);
        }
    }
}

#[cfg(feature = "multi-viewport")]
fn context_backend_platform_user_data(app: &App) -> *mut std::ffi::c_void {
    app.world()
        .get_non_send::<ImguiContext>()
        .expect("plugin should install ImGui context")
        .context()
        .io()
        .backend_platform_user_data()
}

#[cfg(all(feature = "multi-viewport", feature = "render"))]
fn spawn_secondary_viewport(app: &mut App, id: imgui::Id) -> (Entity, Entity) {
    app.world_mut()
        .get_non_send_mut::<ImguiViewportBridge>()
        .expect("bridge should be installed")
        .queue(ImguiViewportCommand::Create(viewport_snapshot(id.raw())));
    app.update();

    let bridge = app
        .world()
        .get_non_send::<ImguiViewportBridge>()
        .expect("bridge should still exist");
    let window = bridge
        .viewport_window(id)
        .expect("create command should spawn a secondary Bevy window");
    let camera = bridge
        .viewport_camera(id)
        .expect("create command should spawn a secondary viewport overlay camera");
    (window, camera)
}

#[cfg(feature = "multi-viewport")]
#[test]
fn multi_viewport_feature_does_not_install_bridge_until_requested() {
    let _guard = imgui_context_guard();
    let mut app = App::new();
    app.add_plugins(ImguiPlugin::default());

    assert!(app.world().get_non_send::<ImguiViewportBridge>().is_none());

    let context = app
        .world()
        .get_non_send::<ImguiContext>()
        .expect("plugin should install ImGui context");
    assert!(
        context
            .context()
            .io()
            .backend_platform_user_data()
            .is_null(),
        "BackendPlatformUserData should stay unset when multi_viewport is not requested"
    );
}

#[cfg(feature = "multi-viewport")]
#[test]
fn multi_viewport_feature_installs_bridge_but_does_not_advertise_full_support_yet() {
    let _guard = imgui_context_guard();
    let mut app = App::new();
    app.add_plugins(ImguiPlugin::new(ImguiBackendConfig {
        name: "viewport-lifecycle".to_owned(),
        docking: true,
        multi_viewport: true,
    }));

    assert!(app.world().get_non_send::<ImguiViewportBridge>().is_some());

    let context = app
        .world()
        .get_non_send::<ImguiContext>()
        .expect("plugin should install ImGui context");
    let bridge = app
        .world()
        .get_non_send::<ImguiViewportBridge>()
        .expect("multi-viewport feature should install bridge");
    assert!(
        !context
            .context()
            .io()
            .backend_platform_user_data()
            .is_null(),
        "BackendPlatformUserData should point at the bridge's stable boxed state"
    );
    assert!(bridge.commands().is_empty());

    let status = app.world().resource::<ImguiBackendStatus>();
    assert!(status.multi_viewport_requested);
    assert!(status.multi_viewport_feature_enabled);
    assert!(status.native_platform_target);
    assert!(status.viewport_lifecycle_bridge_enabled);
    assert!(status.viewport_input_feedback_enabled);
    assert_eq!(status.render_feature_enabled, cfg!(feature = "render"));
    assert!(!status.render_integration_installed);
    assert!(!status.viewport_render_routing_enabled);
    assert!(!status.multi_viewport_supported);
}

#[cfg(feature = "multi-viewport")]
#[test]
fn viewport_prepare_refreshes_main_platform_user_data_to_live_handle() {
    let _guard = imgui_context_guard();
    let mut app = app_with_multi_viewport_bridge("viewport-main-handle-refresh");
    app.world_mut().spawn((Window::default(), PrimaryWindow));

    let mut stale_marker = 0usize;
    let stale = (&mut stale_marker as *mut usize).cast::<std::ffi::c_void>();
    {
        let mut context = app.world_mut().get_non_send_mut::<ImguiContext>().unwrap();
        let main_viewport = context.context_mut().main_viewport();
        main_viewport.set_platform_user_data(stale);
        main_viewport.set_platform_handle(std::ptr::null_mut());
    }

    app.update();

    let (handle, user_data) = {
        let mut context = app.world_mut().get_non_send_mut::<ImguiContext>().unwrap();
        let main_viewport = context.context_mut().main_viewport();
        (
            main_viewport.platform_handle(),
            main_viewport.platform_user_data(),
        )
    };
    assert!(!handle.is_null());
    assert_eq!(user_data, handle);
    assert_ne!(
        user_data, stale,
        "main viewport PlatformUserData must not keep a stale backend handle"
    );
}

#[cfg(feature = "multi-viewport")]
#[test]
fn viewport_primary_cleanup_clears_imgui_platform_handles() {
    let _guard = imgui_context_guard();
    let mut app = app_with_multi_viewport_bridge("viewport-primary-cleanup-handles");
    let primary = app
        .world_mut()
        .spawn((Window::default(), PrimaryWindow))
        .id();

    app.update();

    {
        let mut context = app.world_mut().get_non_send_mut::<ImguiContext>().unwrap();
        let main_viewport = context.context_mut().main_viewport();
        assert!(!main_viewport.platform_handle().is_null());
        assert_eq!(
            main_viewport.platform_handle(),
            main_viewport.platform_user_data()
        );
    }

    app.world_mut()
        .resource_mut::<Messages<WindowCloseRequested>>()
        .write(WindowCloseRequested { window: primary });
    app.update();

    let (handle, user_data) = {
        let mut context = app.world_mut().get_non_send_mut::<ImguiContext>().unwrap();
        let main_viewport = context.context_mut().main_viewport();
        (
            main_viewport.platform_handle(),
            main_viewport.platform_user_data(),
        )
    };
    assert!(
        handle.is_null(),
        "primary cleanup must clear ImGui's main viewport PlatformHandle before releasing backend handles"
    );
    assert!(
        user_data.is_null(),
        "primary cleanup must clear ImGui's main viewport PlatformUserData before releasing backend handles"
    );
}

#[test]
fn viewport_window_factory_maps_snapshot_to_hidden_secondary_window() {
    let snapshot = ImguiViewportSnapshot {
        flags: imgui::ViewportFlags::IS_PLATFORM_WINDOW
            | imgui::ViewportFlags::NO_DECORATION
            | imgui::ViewportFlags::NO_TASK_BAR_ICON
            | imgui::ViewportFlags::TOP_MOST
            | imgui::ViewportFlags::NO_FOCUS_ON_APPEARING,
        ..viewport_snapshot(0x42)
    };

    let window = dear_imgui_bevy::viewport::window_from_snapshot(&snapshot);

    assert_eq!(window.title, "Dear ImGui Viewport 66");
    assert_eq!(
        window.position,
        WindowPosition::At(bevy_math::IVec2::new(64, 96))
    );
    assert_eq!(window.resolution.physical_width(), 1280);
    assert_eq!(window.resolution.physical_height(), 720);
    assert_eq!(window.resolution.scale_factor(), 2.0);
    assert!(!window.decorations);
    assert!(window.skip_taskbar);
    assert_eq!(window.window_level, WindowLevel::AlwaysOnTop);
    assert!(!window.visible);
    assert!(!window.focused);
}

#[test]
fn viewport_window_factory_sanitizes_non_finite_platform_values() {
    let snapshot = ImguiViewportSnapshot {
        pos: [f32::NAN, f32::INFINITY],
        size: [f32::NAN, f32::NEG_INFINITY],
        dpi_scale: f32::INFINITY,
        ..viewport_snapshot(0x43)
    };

    let window = dear_imgui_bevy::viewport::window_from_snapshot(&snapshot);

    assert_eq!(
        window.position,
        WindowPosition::At(bevy_math::IVec2::new(0, 0))
    );
    assert_eq!(window.resolution.width(), 1.0);
    assert_eq!(window.resolution.height(), 1.0);
    assert_eq!(window.resolution.scale_factor(), 1.0);
}

#[cfg(feature = "multi-viewport")]
#[test]
fn viewport_platform_monitors_use_real_monitor_space_not_primary_window_space() {
    let primary = Monitor {
        name: Some("primary".to_owned()),
        physical_width: 2560,
        physical_height: 1600,
        physical_position: bevy_math::IVec2::new(2560, 0),
        refresh_rate_millihertz: Some(60_000),
        scale_factor: 2.0,
        video_modes: Vec::new(),
    };
    let secondary = Monitor {
        name: Some("secondary".to_owned()),
        physical_width: 1920,
        physical_height: 1080,
        physical_position: bevy_math::IVec2::new(0, 0),
        refresh_rate_millihertz: Some(144_000),
        scale_factor: 1.0,
        video_modes: Vec::new(),
    };

    let monitors = dear_imgui_bevy::viewport::platform_monitors_from_bevy_monitors([
        (secondary, false),
        (primary, true),
    ]);

    assert_eq!(monitors.len(), 2);
    assert_eq!(monitors[0].MainPos.x, 1280.0);
    assert_eq!(monitors[0].MainPos.y, 0.0);
    assert_eq!(monitors[0].MainSize.x, 1280.0);
    assert_eq!(monitors[0].MainSize.y, 800.0);
    assert_eq!(monitors[0].DpiScale, 2.0);
    assert_eq!(monitors[1].MainPos.x, 0.0);
    assert_eq!(monitors[1].MainPos.y, 0.0);
    assert_eq!(monitors[1].MainSize.x, 1920.0);
    assert_eq!(monitors[1].MainSize.y, 1080.0);
    assert_eq!(monitors[1].DpiScale, 1.0);
}

#[cfg(feature = "multi-viewport")]
#[test]
fn viewport_platform_io_callbacks_capture_commands_and_bevy_system_applies_them() {
    let _guard = imgui_context_guard();
    let mut app = App::new();
    app.add_plugins(ImguiPlugin::new(ImguiBackendConfig {
        name: "viewport-callbacks".to_owned(),
        docking: true,
        multi_viewport: true,
    }));

    let id = imgui::Id::from(0x200);
    let raw_viewport = unsafe { sys::ImGuiViewport_ImGuiViewport() };
    assert!(
        !raw_viewport.is_null(),
        "ImGuiViewport_ImGuiViewport() returned null"
    );

    unsafe {
        let viewport = imgui::Viewport::from_raw_mut(raw_viewport);
        (*raw_viewport).ID = id.raw();
        viewport.set_pos([10.0, 20.0]);
        viewport.set_size([400.0, 240.0]);
        viewport.set_dpi_scale(1.0);
        viewport.set_flags(imgui::ViewportFlags::IS_PLATFORM_WINDOW);
    }

    let (create_window, set_window_pos, destroy_window) = {
        let context = app
            .world()
            .get_non_send::<ImguiContext>()
            .expect("plugin should install ImGui context");
        let platform_io = context.context().platform_io().as_raw();
        unsafe {
            (
                (*platform_io)
                    .Platform_CreateWindow
                    .expect("bridge should install Platform_CreateWindow"),
                (*platform_io)
                    .Platform_SetWindowPos
                    .expect("bridge should install Platform_SetWindowPos"),
                (*platform_io)
                    .Platform_DestroyWindow
                    .expect("bridge should install Platform_DestroyWindow"),
            )
        }
    };

    unsafe {
        create_window(raw_viewport);
        set_window_pos(raw_viewport, sys::ImVec2 { x: 88.0, y: 99.0 });
        assert!(!(*raw_viewport).PlatformHandle.is_null());
        assert_eq!(
            (*raw_viewport).PlatformHandle,
            (*raw_viewport).PlatformUserData
        );
        assert_ne!(
            (*raw_viewport).PlatformHandle,
            context_backend_platform_user_data(&app),
            "each Dear ImGui viewport needs its own platform handle; the global bridge pointer is only backend state"
        );
    }

    {
        let bridge = app
            .world()
            .get_non_send::<ImguiViewportBridge>()
            .expect("bridge should still exist");
        assert_eq!(
            bridge.commands(),
            [
                ImguiViewportCommand::Create(ImguiViewportSnapshot {
                    id,
                    pos: [10.0, 20.0],
                    size: [400.0, 240.0],
                    dpi_scale: 1.0,
                    flags: imgui::ViewportFlags::IS_PLATFORM_WINDOW,
                }),
                ImguiViewportCommand::SetPos {
                    id,
                    pos: [88.0, 99.0],
                },
            ]
        );
    }

    app.update();

    let bridge = app
        .world()
        .get_non_send::<ImguiViewportBridge>()
        .expect("bridge should still exist");
    let entity = bridge
        .viewport_window(id)
        .expect("captured create command should spawn a Bevy window entity");
    let window = app
        .world()
        .get::<Window>(entity)
        .expect("spawned entity should contain Window");
    assert_eq!(
        window.position,
        WindowPosition::At(bevy_math::IVec2::new(88, 99))
    );

    unsafe {
        destroy_window(raw_viewport);
        sys::ImGuiViewport_destroy(raw_viewport);
    }
}

#[cfg(feature = "multi-viewport")]
#[test]
fn viewport_destroy_callback_ignores_owned_by_app_main_viewport() {
    let _guard = imgui_context_guard();
    let mut app = app_with_multi_viewport_bridge("viewport-main-destroy-callback");

    let destroy_window = {
        let context = app
            .world()
            .get_non_send::<ImguiContext>()
            .expect("plugin should install ImGui context");
        let platform_io = context.context().platform_io().as_raw();
        unsafe {
            (*platform_io)
                .Platform_DestroyWindow
                .expect("bridge should install Platform_DestroyWindow")
        }
    };

    {
        let mut context = app.world_mut().get_non_send_mut::<ImguiContext>().unwrap();
        let main_viewport = context.context_mut().main_viewport();
        unsafe {
            (*main_viewport.as_raw_mut()).Flags = imgui::ViewportFlags::OWNED_BY_APP.bits();
            destroy_window(main_viewport.as_raw_mut());
        }
    }

    let bridge = app
        .world()
        .get_non_send::<ImguiViewportBridge>()
        .expect("bridge should still exist");
    assert!(
        bridge.commands().is_empty(),
        "destroying the application-owned main viewport must not enqueue a secondary-window destroy"
    );
}

#[cfg(feature = "multi-viewport")]
#[test]
fn context_drop_clears_backend_user_data_before_destroying_platform_windows() {
    let _guard = imgui_context_guard();

    unsafe extern "C" fn assert_backend_user_data_is_cleared(viewport: *mut sys::ImGuiViewport) {
        let io = unsafe { sys::igGetIO_Nil() };
        let cleared = io.is_null() || unsafe { (*io).BackendPlatformUserData.is_null() };
        DESTROY_CALLBACK_SAW_NULL_BACKEND_USER_DATA
            .store(cleared, std::sync::atomic::Ordering::SeqCst);
        if let Some(viewport) = unsafe { viewport.as_mut() } {
            viewport.PlatformUserData = std::ptr::null_mut();
            viewport.PlatformHandle = std::ptr::null_mut();
        }
    }

    DESTROY_CALLBACK_SAW_NULL_BACKEND_USER_DATA.store(false, std::sync::atomic::Ordering::SeqCst);
    let mut app = app_with_multi_viewport_bridge("viewport-context-drop");
    {
        let mut context = app.world_mut().get_non_send_mut::<ImguiContext>().unwrap();
        let main_viewport = context.context_mut().main_viewport();
        main_viewport.set_platform_user_data(std::ptr::dangling_mut::<u8>().cast());
        main_viewport.set_platform_handle(std::ptr::dangling_mut::<u8>().cast());
        main_viewport.set_platform_window_created(true);
        unsafe {
            let platform_io = context.context_mut().platform_io_mut().as_raw_mut();
            (*platform_io).Platform_DestroyWindow = Some(assert_backend_user_data_is_cleared);
        }
    }

    drop(
        app.world_mut()
            .remove_non_send::<ImguiContext>()
            .expect("ImguiContext should be removable for direct shutdown testing"),
    );

    assert!(
        DESTROY_CALLBACK_SAW_NULL_BACKEND_USER_DATA.load(std::sync::atomic::Ordering::SeqCst),
        "ImguiContext shutdown must not leave Platform_DestroyWindow callbacks with a dangling bridge pointer"
    );
}

#[cfg(feature = "multi-viewport")]
#[test]
fn viewport_platform_feedback_queries_return_mapped_bevy_window_state() {
    let _guard = imgui_context_guard();
    let mut app = App::new();
    app.add_plugins(ImguiPlugin::new(ImguiBackendConfig {
        name: "viewport-feedback".to_owned(),
        docking: true,
        multi_viewport: true,
    }));
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    {
        let mut context = app.world_mut().get_non_send_mut::<ImguiContext>().unwrap();
        let _ = context.context_mut().font_atlas_mut().build();
    }

    let id = imgui::Id::from(0x201);
    app.world_mut()
        .get_non_send_mut::<ImguiViewportBridge>()
        .expect("bridge should be installed")
        .queue(ImguiViewportCommand::Create(viewport_snapshot(id.raw())));
    app.update();

    let entity = app
        .world()
        .get_non_send::<ImguiViewportBridge>()
        .expect("bridge should still exist")
        .viewport_window(id)
        .expect("create command should spawn a secondary Bevy window");
    {
        let mut window = app
            .world_mut()
            .get_mut::<Window>(entity)
            .expect("spawned entity should contain Window");
        window.position = WindowPosition::At(bevy_math::IVec2::new(150, 225));
        window.resolution.set_scale_factor(1.5);
        window.resolution.set(300.0, 180.0);
        window.focused = false;
    }
    app.update();

    let feedback = app
        .world()
        .get_non_send::<ImguiViewportBridge>()
        .expect("bridge should still exist")
        .viewport_feedback(id)
        .expect("feedback sync should cache secondary window state");
    assert_eq!(
        feedback,
        ImguiViewportFeedback {
            pos: [100.0, 150.0],
            size: [300.0, 180.0],
            framebuffer_scale: [1.5, 1.5],
            dpi_scale: 1.5,
            focused: false,
            minimized: false,
        }
    );

    let raw_viewport = unsafe { sys::ImGuiViewport_ImGuiViewport() };
    assert!(
        !raw_viewport.is_null(),
        "ImGuiViewport_ImGuiViewport() returned null"
    );
    unsafe {
        (*raw_viewport).ID = id.raw();
    }

    let (
        get_window_pos,
        get_window_size,
        get_window_framebuffer_scale,
        get_window_dpi_scale,
        get_window_focus,
        get_window_minimized,
    ) = {
        let context = app
            .world()
            .get_non_send::<ImguiContext>()
            .expect("plugin should install ImGui context");
        let platform_io = context.context().platform_io().as_raw();
        unsafe {
            (
                (*platform_io)
                    .Platform_GetWindowPos
                    .expect("bridge should install Platform_GetWindowPos"),
                (*platform_io)
                    .Platform_GetWindowSize
                    .expect("bridge should install Platform_GetWindowSize"),
                (*platform_io)
                    .Platform_GetWindowFramebufferScale
                    .expect("bridge should install Platform_GetWindowFramebufferScale"),
                (*platform_io)
                    .Platform_GetWindowDpiScale
                    .expect("bridge should install Platform_GetWindowDpiScale"),
                (*platform_io)
                    .Platform_GetWindowFocus
                    .expect("bridge should install Platform_GetWindowFocus"),
                (*platform_io)
                    .Platform_GetWindowMinimized
                    .expect("bridge should install Platform_GetWindowMinimized"),
            )
        }
    };

    unsafe {
        let pos = get_window_pos(raw_viewport);
        let size = get_window_size(raw_viewport);
        let framebuffer_scale = get_window_framebuffer_scale(raw_viewport);
        assert_eq!([pos.x, pos.y], [100.0, 150.0]);
        assert_eq!([size.x, size.y], [300.0, 180.0]);
        assert_eq!([framebuffer_scale.x, framebuffer_scale.y], [1.5, 1.5]);
        assert_eq!(get_window_dpi_scale(raw_viewport), 1.5);
        assert!(!get_window_focus(raw_viewport));
        assert!(!get_window_minimized(raw_viewport));
        sys::ImGuiViewport_destroy(raw_viewport);
    }
}

#[cfg(feature = "multi-viewport")]
#[test]
fn viewport_os_move_and_resize_events_request_imgui_platform_sync() {
    let _guard = imgui_context_guard();
    let mut app = app_with_multi_viewport_bridge("viewport-os-window-events");
    app.world_mut().spawn((Window::default(), PrimaryWindow));

    let id = imgui::Id::from(0x202);
    app.world_mut()
        .get_non_send_mut::<ImguiViewportBridge>()
        .expect("bridge should be installed")
        .queue(ImguiViewportCommand::Create(viewport_snapshot(id.raw())));
    app.update();

    let entity = app
        .world()
        .get_non_send::<ImguiViewportBridge>()
        .expect("bridge should still exist")
        .viewport_window(id)
        .expect("create command should spawn a secondary Bevy window");
    {
        let mut window = app
            .world_mut()
            .get_mut::<Window>(entity)
            .expect("spawned entity should contain Window");
        window.position = WindowPosition::At(IVec2::new(420, 630));
        window.resolution.set_scale_factor(1.5);
        window.resolution.set(420.0, 240.0);
    }

    app.world_mut()
        .resource_mut::<Messages<WindowMoved>>()
        .write(WindowMoved {
            window: entity,
            position: IVec2::new(420, 630),
        });
    app.world_mut()
        .resource_mut::<Messages<WindowResized>>()
        .write(WindowResized {
            window: entity,
            width: 420.0,
            height: 240.0,
        });

    with_test_platform_viewport(&mut app, id, |app, raw_viewport| {
        app.world_mut().run_schedule(bevy_app::PreUpdate);

        let feedback = app
            .world()
            .get_non_send::<ImguiViewportBridge>()
            .expect("bridge should still exist")
            .viewport_feedback(id)
            .expect("OS move/resize events should refresh viewport feedback");
        assert_eq!(feedback.pos, [280.0, 420.0]);
        assert_eq!(feedback.size, [420.0, 240.0]);

        unsafe {
            assert!(
                (*raw_viewport).PlatformRequestMove,
                "OS window moves must tell Dear ImGui to pull the platform position instead of fighting the drag"
            );
            assert!(
                (*raw_viewport).PlatformRequestResize,
                "OS window resizes must tell Dear ImGui to pull the platform size instead of fighting the resize"
            );
        }
    });
}

#[cfg(feature = "multi-viewport")]
#[test]
fn viewport_secondary_window_close_requests_imgui_platform_close() {
    let _guard = imgui_context_guard();
    let mut app = app_with_multi_viewport_bridge("viewport-secondary-close-request");
    app.world_mut().spawn((Window::default(), PrimaryWindow));

    let id = imgui::Id::from(0x203);
    app.world_mut()
        .get_non_send_mut::<ImguiViewportBridge>()
        .expect("bridge should be installed")
        .queue(ImguiViewportCommand::Create(viewport_snapshot(id.raw())));
    app.update();

    let entity = app
        .world()
        .get_non_send::<ImguiViewportBridge>()
        .expect("bridge should still exist")
        .viewport_window(id)
        .expect("create command should spawn a secondary Bevy window");

    app.world_mut()
        .resource_mut::<Messages<WindowCloseRequested>>()
        .write(WindowCloseRequested { window: entity });

    with_test_platform_viewport(&mut app, id, |app, raw_viewport| {
        app.world_mut().run_schedule(bevy_app::PreUpdate);

        unsafe {
            assert!(
                (*raw_viewport).PlatformRequestClose,
                "closing a detached Bevy window must ask Dear ImGui to close the matching platform viewport"
            );
        }
    });
}

#[cfg(feature = "multi-viewport")]
#[test]
fn viewport_occlusion_events_update_imgui_minimized_feedback() {
    let _guard = imgui_context_guard();
    let mut app = app_with_multi_viewport_bridge("viewport-occlusion-feedback");
    app.world_mut().spawn((Window::default(), PrimaryWindow));

    let id = imgui::Id::from(0x204);
    app.world_mut()
        .get_non_send_mut::<ImguiViewportBridge>()
        .expect("bridge should be installed")
        .queue(ImguiViewportCommand::Create(viewport_snapshot(id.raw())));
    app.update();

    let entity = app
        .world()
        .get_non_send::<ImguiViewportBridge>()
        .expect("bridge should still exist")
        .viewport_window(id)
        .expect("create command should spawn a secondary Bevy window");

    app.world_mut()
        .resource_mut::<Messages<WindowOccluded>>()
        .write(WindowOccluded {
            window: entity,
            occluded: true,
        });

    with_test_platform_viewport(&mut app, id, |app, raw_viewport| {
        app.world_mut().run_schedule(bevy_app::PreUpdate);

        let minimized = {
            let context = app
                .world()
                .get_non_send::<ImguiContext>()
                .expect("plugin should install ImGui context");
            let platform_io = context.context().platform_io().as_raw();
            unsafe {
                (*platform_io)
                    .Platform_GetWindowMinimized
                    .expect("bridge should install Platform_GetWindowMinimized")
            }
        };

        unsafe {
            assert!(
                minimized(raw_viewport),
                "occluded detached windows should be reported as minimized to Dear ImGui"
            );
        }
    });

    app.world_mut()
        .resource_mut::<Messages<WindowOccluded>>()
        .write(WindowOccluded {
            window: entity,
            occluded: false,
        });

    with_test_platform_viewport(&mut app, id, |app, raw_viewport| {
        app.world_mut().run_schedule(bevy_app::PreUpdate);

        let minimized = {
            let context = app
                .world()
                .get_non_send::<ImguiContext>()
                .expect("plugin should install ImGui context");
            let platform_io = context.context().platform_io().as_raw();
            unsafe {
                (*platform_io)
                    .Platform_GetWindowMinimized
                    .expect("bridge should install Platform_GetWindowMinimized")
            }
        };

        unsafe {
            assert!(
                !minimized(raw_viewport),
                "unoccluded detached windows should clear minimized feedback"
            );
        }
    });
}

#[cfg(feature = "multi-viewport")]
#[test]
fn viewport_commands_spawn_update_show_and_destroy_window_entities() {
    let _guard = imgui_context_guard();
    let mut app = App::new();
    app.add_plugins(ImguiPlugin::new(ImguiBackendConfig {
        name: "viewport-lifecycle".to_owned(),
        docking: true,
        multi_viewport: true,
    }));

    let id = imgui::Id::from(0x100);
    app.world_mut()
        .get_non_send_mut::<ImguiViewportBridge>()
        .expect("bridge should be installed")
        .queue(ImguiViewportCommand::Create(viewport_snapshot(id.raw())));
    app.update();

    let bridge = app
        .world()
        .get_non_send::<ImguiViewportBridge>()
        .expect("bridge should still exist");
    let entity = bridge
        .viewport_window(id)
        .expect("create command should spawn a Bevy window entity");
    let window = app
        .world()
        .get::<Window>(entity)
        .expect("spawned entity should contain Window");
    assert_eq!(
        window.position,
        WindowPosition::At(bevy_math::IVec2::new(64, 96))
    );
    assert!(!window.visible);
    assert!(
        !window.focused,
        "secondary viewport windows are created hidden and focus is requested after the OS window exists"
    );
    let marker = app
        .world()
        .get::<ImguiViewportWindow>(entity)
        .expect("spawned entity should be marked as an ImGui viewport window");
    assert_eq!(marker.viewport_id, id);

    {
        let mut bridge = app
            .world_mut()
            .get_non_send_mut::<ImguiViewportBridge>()
            .expect("bridge should still exist");
        bridge.queue(ImguiViewportCommand::SetPos {
            id,
            pos: [80.0, 96.0],
        });
        bridge.queue(ImguiViewportCommand::SetSize {
            id,
            size: [320.0, 200.0],
        });
        bridge.queue(ImguiViewportCommand::SetTitle {
            id,
            title: "Detached Tools".to_owned(),
        });
        bridge.queue(ImguiViewportCommand::Show { id });
    }
    app.update();

    let window = app
        .world()
        .get::<Window>(entity)
        .expect("window should remain after update commands");
    assert_eq!(
        window.position,
        WindowPosition::At(bevy_math::IVec2::new(160, 192))
    );
    assert_eq!(window.resolution.width(), 320.0);
    assert_eq!(window.resolution.height(), 200.0);
    assert_eq!(window.title, "Detached Tools");
    assert!(window.visible);
    assert!(
        !window.focused,
        "showing a just-created viewport should queue focus for the next ECS pass so Bevy winit sees a false->true transition"
    );

    app.update();

    let window = app
        .world()
        .get::<Window>(entity)
        .expect("window should remain after queued focus command");
    assert!(
        window.focused,
        "secondary viewport show should request focus/bring-to-front after the Bevy OS window has been created"
    );

    app.world_mut()
        .get_non_send_mut::<ImguiViewportBridge>()
        .expect("bridge should still exist")
        .queue(ImguiViewportCommand::Destroy { id });
    app.update();

    let bridge = app
        .world()
        .get_non_send::<ImguiViewportBridge>()
        .expect("bridge should still exist");
    assert!(bridge.viewport_window(id).is_none());
    assert!(bridge.viewport_feedback(id).is_none());
    assert!(
        app.world().get_entity(entity).is_err(),
        "destroy command should despawn the secondary Bevy window entity"
    );
}

#[cfg(feature = "multi-viewport")]
#[test]
fn viewport_recreate_updates_window_level_from_latest_flags() {
    let _guard = imgui_context_guard();
    let mut app = app_with_multi_viewport_bridge("viewport-level-refresh");

    let id = imgui::Id::from(0x106);
    app.world_mut()
        .get_non_send_mut::<ImguiViewportBridge>()
        .expect("bridge should be installed")
        .queue(ImguiViewportCommand::Create(ImguiViewportSnapshot {
            flags: imgui::ViewportFlags::IS_PLATFORM_WINDOW | imgui::ViewportFlags::TOP_MOST,
            ..viewport_snapshot(id.raw())
        }));
    app.update();

    let entity = app
        .world()
        .get_non_send::<ImguiViewportBridge>()
        .expect("bridge should still exist")
        .viewport_window(id)
        .expect("create command should spawn a secondary Bevy window");
    assert_eq!(
        app.world().get::<Window>(entity).unwrap().window_level,
        WindowLevel::AlwaysOnTop
    );

    app.world_mut()
        .get_non_send_mut::<ImguiViewportBridge>()
        .expect("bridge should still exist")
        .queue(ImguiViewportCommand::Create(ImguiViewportSnapshot {
            flags: imgui::ViewportFlags::IS_PLATFORM_WINDOW,
            ..viewport_snapshot(id.raw())
        }));
    app.update();

    assert_eq!(
        app.world().get::<Window>(entity).unwrap().window_level,
        WindowLevel::Normal,
        "refreshing an existing viewport should apply the latest TOP_MOST flag state"
    );
}

#[cfg(feature = "multi-viewport")]
#[test]
fn viewport_show_respects_no_focus_on_appearing() {
    let _guard = imgui_context_guard();
    let mut app = App::new();
    app.add_plugins(ImguiPlugin::new(ImguiBackendConfig {
        name: "viewport-no-focus-on-show".to_owned(),
        docking: true,
        multi_viewport: true,
    }));

    let id = imgui::Id::from(0x104);
    app.world_mut()
        .get_non_send_mut::<ImguiViewportBridge>()
        .expect("bridge should be installed")
        .queue(ImguiViewportCommand::Create(ImguiViewportSnapshot {
            flags: imgui::ViewportFlags::IS_PLATFORM_WINDOW
                | imgui::ViewportFlags::NO_FOCUS_ON_APPEARING,
            ..viewport_snapshot(id.raw())
        }));
    app.update();

    let entity = app
        .world()
        .get_non_send::<ImguiViewportBridge>()
        .expect("bridge should still exist")
        .viewport_window(id)
        .expect("create command should spawn a secondary Bevy window");

    app.world_mut()
        .get_non_send_mut::<ImguiViewportBridge>()
        .expect("bridge should still exist")
        .queue(ImguiViewportCommand::Show { id });
    app.update();
    app.update();

    let window = app
        .world()
        .get::<Window>(entity)
        .expect("window should remain after show command");
    assert!(window.visible);
    assert!(
        !window.focused,
        "NoFocusOnAppearing viewports must not be brought in front when shown"
    );
}

#[cfg(feature = "multi-viewport")]
#[test]
fn viewport_set_focus_requests_focus_on_next_ecs_pass() {
    let _guard = imgui_context_guard();
    let mut app = app_with_multi_viewport_bridge("viewport-set-focus");

    let id = imgui::Id::from(0x105);
    app.world_mut()
        .get_non_send_mut::<ImguiViewportBridge>()
        .expect("bridge should be installed")
        .queue(ImguiViewportCommand::Create(ImguiViewportSnapshot {
            flags: imgui::ViewportFlags::IS_PLATFORM_WINDOW
                | imgui::ViewportFlags::NO_FOCUS_ON_APPEARING,
            ..viewport_snapshot(id.raw())
        }));
    app.update();

    let entity = app
        .world()
        .get_non_send::<ImguiViewportBridge>()
        .expect("bridge should still exist")
        .viewport_window(id)
        .expect("create command should spawn a secondary Bevy window");

    app.world_mut()
        .get_non_send_mut::<ImguiViewportBridge>()
        .expect("bridge should still exist")
        .queue(ImguiViewportCommand::SetFocus { id });
    app.update();
    assert!(
        !app.world().get::<Window>(entity).unwrap().focused,
        "SetFocus first clears focus so Bevy winit can observe a later false->true transition"
    );

    app.update();
    assert!(
        app.world().get::<Window>(entity).unwrap().focused,
        "SetFocus should request focus/bring-to-front on the following ECS pass"
    );
}

#[cfg(all(feature = "multi-viewport", feature = "render"))]
#[test]
fn viewport_commands_spawn_and_destroy_secondary_overlay_camera() {
    let _guard = imgui_context_guard();
    let mut app = app_with_multi_viewport_bridge("viewport-render-target");

    let id = imgui::Id::from(0x101);
    let (window_entity, camera_entity) = spawn_secondary_viewport(&mut app, id);

    let camera_marker = app
        .world()
        .get::<ImguiViewportCamera>(camera_entity)
        .expect("secondary camera should carry the viewport marker");
    assert_eq!(camera_marker.viewport_id, id);
    assert!(
        app.world().get::<Camera2d>(camera_entity).is_some(),
        "secondary viewport camera must enter Bevy's 2D render graph"
    );
    assert!(
        app.world()
            .get::<Camera>(camera_entity)
            .is_some_and(|camera| camera.is_active),
        "secondary viewport camera should be active"
    );
    assert!(matches!(
        app.world()
            .get::<RenderTarget>(camera_entity)
            .expect("secondary camera should target a Bevy window"),
        RenderTarget::Window(WindowRef::Entity(entity)) if *entity == window_entity
    ));
    assert_eq!(
        app.world()
            .get::<RenderLayers>(camera_entity)
            .expect("secondary viewport camera should explicitly opt out of scene layers"),
        &RenderLayers::none(),
        "secondary viewport camera should not render normal Bevy scene entities into detached ImGui windows"
    );

    app.world_mut()
        .get_non_send_mut::<ImguiViewportBridge>()
        .expect("bridge should still exist")
        .queue(ImguiViewportCommand::Destroy { id });
    app.update();

    let bridge = app
        .world()
        .get_non_send::<ImguiViewportBridge>()
        .expect("bridge should still exist");
    assert!(bridge.viewport_window(id).is_none());
    assert!(bridge.viewport_camera(id).is_none());
    assert!(
        app.world().get_entity(window_entity).is_err(),
        "destroy command should despawn the secondary Bevy window entity"
    );
    assert!(
        app.world().get_entity(camera_entity).is_err(),
        "destroy command should despawn the secondary viewport camera entity"
    );
}

#[cfg(all(feature = "multi-viewport", feature = "render"))]
#[test]
fn viewport_orphaned_secondary_overlay_camera_is_despawned_after_dock_back() {
    let _guard = imgui_context_guard();
    let mut app = app_with_multi_viewport_bridge("viewport-dock-back-cleanup");
    app.world_mut().spawn((Window::default(), PrimaryWindow));

    let id = imgui::Id::from(0x103);
    let (window_entity, camera_entity) = spawn_secondary_viewport(&mut app, id);

    app.world_mut().despawn(window_entity);
    app.update();

    let bridge = app
        .world()
        .get_non_send::<ImguiViewportBridge>()
        .expect("bridge should still exist");
    assert!(bridge.viewport_window(id).is_none());
    assert!(bridge.viewport_camera(id).is_none());
    assert!(
        app.world().get_entity(camera_entity).is_err(),
        "when a detached viewport is merged back and its Bevy window disappears, the secondary overlay camera must not keep intercepting or rendering"
    );
}

#[cfg(all(feature = "multi-viewport", feature = "render"))]
#[test]
fn viewport_primary_close_despawns_secondary_viewport_windows_and_cameras() {
    let _guard = imgui_context_guard();
    let mut app = app_with_multi_viewport_bridge("viewport-primary-close");
    let primary = app
        .world_mut()
        .spawn((Window::default(), PrimaryWindow))
        .id();
    let id = imgui::Id::from(0x102);
    let (window_entity, camera_entity) = spawn_secondary_viewport(&mut app, id);

    app.world_mut()
        .resource_mut::<Messages<WindowCloseRequested>>()
        .write(WindowCloseRequested { window: primary });
    app.update();

    let bridge = app
        .world()
        .get_non_send::<ImguiViewportBridge>()
        .expect("bridge should still exist");
    assert!(bridge.viewport_window(id).is_none());
    assert!(bridge.viewport_camera(id).is_none());
    assert!(
        app.world().get_entity(window_entity).is_err(),
        "closing the primary window should despawn detached Dear ImGui viewport windows"
    );
    assert!(
        app.world().get_entity(camera_entity).is_err(),
        "closing the primary window should despawn detached viewport cameras"
    );
}

#[cfg(all(feature = "multi-viewport", feature = "render"))]
#[test]
fn viewport_missing_primary_window_despawns_secondary_viewport_windows_and_cameras() {
    let _guard = imgui_context_guard();
    let mut app = app_with_multi_viewport_bridge("viewport-primary-despawn");
    let primary = app
        .world_mut()
        .spawn((Window::default(), PrimaryWindow))
        .id();
    let id = imgui::Id::from(0x104);
    let (window_entity, camera_entity) = spawn_secondary_viewport(&mut app, id);

    app.world_mut().despawn(primary);
    app.update();

    let bridge = app
        .world()
        .get_non_send::<ImguiViewportBridge>()
        .expect("bridge should still exist");
    assert!(bridge.viewport_window(id).is_none());
    assert!(bridge.viewport_camera(id).is_none());
    assert!(
        app.world().get_entity(window_entity).is_err(),
        "removing the primary window should despawn detached Dear ImGui viewport windows"
    );
    assert!(
        app.world().get_entity(camera_entity).is_err(),
        "removing the primary window should despawn detached viewport cameras"
    );
}
