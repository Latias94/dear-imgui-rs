#[cfg(all(feature = "multi-viewport", feature = "render"))]
use crate::context::ImguiFrameMailbox;
#[cfg(feature = "multi-viewport")]
use crate::test_util::imgui_context_guard;
#[cfg(feature = "multi-viewport")]
use bevy_app::App;
#[cfg(all(feature = "multi-viewport", feature = "render"))]
use bevy_app::Main;
#[cfg(all(feature = "multi-viewport", feature = "render"))]
use bevy_camera::{
    Camera, Camera2d, CameraOutputMode, ClearColorConfig, RenderTarget, visibility::RenderLayers,
};
#[cfg(feature = "multi-viewport")]
use bevy_ecs::message::Messages;
#[cfg(feature = "multi-viewport")]
use bevy_ecs::prelude::{Entity, Res, Resource, With};
#[cfg(feature = "multi-viewport")]
use bevy_ecs::schedule::ScheduleLabel;
#[cfg(feature = "multi-viewport")]
use bevy_math::IVec2;
#[cfg(all(feature = "multi-viewport", feature = "render"))]
use bevy_math::{Rect, Vec2};
#[cfg(all(feature = "multi-viewport", feature = "render"))]
use bevy_render::camera::CameraRenderGraph;
#[cfg(all(feature = "multi-viewport", feature = "render"))]
use bevy_render::{Render, RenderApp, extract_plugin::ExtractPlugin};
#[cfg(feature = "multi-viewport")]
use bevy_window::CompositeAlphaMode;
#[cfg(feature = "multi-viewport")]
use bevy_window::Monitor;
#[cfg(feature = "multi-viewport")]
use bevy_window::WindowCloseRequested;
use bevy_window::WindowLevel;
#[cfg(feature = "multi-viewport")]
use bevy_window::WindowOccluded;
use bevy_window::WindowPosition;
#[cfg(all(feature = "multi-viewport", feature = "render"))]
use bevy_window::WindowRef;
#[cfg(feature = "multi-viewport")]
use bevy_window::{PrimaryWindow, Window};
#[cfg(feature = "multi-viewport")]
use dear_imgui_bevy::ImguiContextConfig;
#[cfg(all(feature = "multi-viewport", feature = "render"))]
use dear_imgui_bevy::ImguiViewportCamera;
use dear_imgui_bevy::ImguiViewportSnapshot;
#[cfg(all(feature = "multi-viewport", feature = "render"))]
use dear_imgui_bevy::route::ImguiInputRoute;
#[cfg(feature = "multi-viewport")]
use dear_imgui_bevy::{
    ImguiContextError, ImguiContexts, ImguiNativeViewportStatus, ImguiNativeViewportSupport,
    ImguiPlugin, ImguiPluginConfig, ImguiPrimaryContextPass, ImguiUi, ImguiViewportBridge,
    ImguiViewportFeedback, ImguiViewportWindow, ImguiViewportWindowConfig,
};
use dear_imgui_rs as imgui;
#[cfg(feature = "multi-viewport")]
use imgui::sys;
#[cfg(feature = "multi-viewport")]
use std::{cell::Cell, rc::Rc};

#[cfg(feature = "multi-viewport")]
#[path = "geometry.rs"]
mod geometry_tests;

#[cfg(feature = "multi-viewport")]
static FOREIGN_DESTROY_SAW_BEVY_BACKEND_USER_DATA: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

#[cfg(feature = "multi-viewport")]
static FOREIGN_DROP_BACKEND_FLAGS: std::sync::atomic::AtomicI32 =
    std::sync::atomic::AtomicI32::new(0);
#[cfg(feature = "multi-viewport")]
static FOREIGN_DROP_CONFIG_FLAGS: std::sync::atomic::AtomicI32 =
    std::sync::atomic::AtomicI32::new(0);
#[cfg(feature = "multi-viewport")]
static FOREIGN_DROP_EXPECTED_USER_DATA: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);
#[cfg(feature = "multi-viewport")]
static FOREIGN_DROP_EXPECTED_MAIN_USER_DATA: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);
#[cfg(feature = "multi-viewport")]
static FOREIGN_DROP_EXPECTED_MAIN_HANDLE: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);
#[cfg(feature = "multi-viewport")]
static FOREIGN_DROP_EXPECTED_MAIN_HANDLE_RAW: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);
#[cfg(feature = "multi-viewport")]
static FOREIGN_DROP_FIELDS_PRESERVED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

#[cfg(feature = "multi-viewport")]
#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
struct SecondaryViewportPass;

#[cfg(feature = "multi-viewport")]
struct ForeignDropObserver;

#[cfg(feature = "multi-viewport")]
struct ForeignDropObserverMarker;

#[cfg(all(feature = "multi-viewport", feature = "render"))]
struct RetirementDropProbeMarker;

#[cfg(all(feature = "multi-viewport", feature = "render"))]
struct RetirementDropProbe {
    destroyed: Rc<Cell<bool>>,
}

#[cfg(all(feature = "multi-viewport", feature = "render"))]
impl imgui::ContextAttachment for RetirementDropProbe {
    fn context_destroyed(&self, _context: imgui::ContextDestroyed) {
        self.destroyed.set(true);
    }
}

#[cfg(feature = "multi-viewport")]
impl imgui::ContextAttachment for ForeignDropObserver {
    fn release_platform_windows(
        &self,
        context: &imgui::ContextTeardown<'_>,
    ) -> Result<(), imgui::ContextAttachmentTeardownError> {
        context.with_bound_context(|| unsafe {
            let io = sys::igGetIO_Nil();
            let main_viewport = sys::igGetMainViewport();
            if io.is_null() || main_viewport.is_null() {
                return;
            }
            FOREIGN_DROP_BACKEND_FLAGS
                .store((*io).BackendFlags, std::sync::atomic::Ordering::SeqCst);
            FOREIGN_DROP_CONFIG_FLAGS.store((*io).ConfigFlags, std::sync::atomic::Ordering::SeqCst);
            let fields_preserved = (*io).BackendPlatformUserData as usize
                == FOREIGN_DROP_EXPECTED_USER_DATA.load(std::sync::atomic::Ordering::SeqCst)
                && (*main_viewport).PlatformUserData as usize
                    == FOREIGN_DROP_EXPECTED_MAIN_USER_DATA
                        .load(std::sync::atomic::Ordering::SeqCst)
                && (*main_viewport).PlatformHandle as usize
                    == FOREIGN_DROP_EXPECTED_MAIN_HANDLE.load(std::sync::atomic::Ordering::SeqCst)
                && (*main_viewport).PlatformHandleRaw as usize
                    == FOREIGN_DROP_EXPECTED_MAIN_HANDLE_RAW
                        .load(std::sync::atomic::Ordering::SeqCst);
            FOREIGN_DROP_FIELDS_PRESERVED
                .store(fields_preserved, std::sync::atomic::Ordering::SeqCst);

            // This simulated foreign backend owns the final native cleanup. The Bevy wrapper has
            // already detached itself by this phase and must not have modified the observed claim.
            (*io).BackendPlatformUserData = std::ptr::null_mut();
            (*main_viewport).PlatformUserData = std::ptr::null_mut();
            (*main_viewport).PlatformHandle = std::ptr::null_mut();
            (*main_viewport).PlatformHandleRaw = std::ptr::null_mut();
            (*main_viewport).PlatformWindowCreated = false;
            sys::ImGuiPlatformIO_ClearPlatformHandlers(sys::igGetPlatformIO_Nil());
        });
        Ok(())
    }
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
fn primary_context_id(app: &App) -> imgui::ContextId {
    app.world()
        .get_non_send::<ImguiContexts>()
        .expect("plugin should install the ImGui Context registry")
        .primary_id()
        .expect("plugin should install a primary ImGui Context")
}

#[cfg(feature = "multi-viewport")]
fn with_primary_context<T>(app: &mut App, operation: impl FnOnce(&mut imgui::Context) -> T) -> T {
    let primary_id = primary_context_id(app);
    app.world_mut()
        .get_non_send_mut::<ImguiContexts>()
        .expect("plugin should install the ImGui Context registry")
        .configure(primary_id, operation)
        .unwrap_or_else(|error| panic!("primary ImGui Context should be configurable: {error}"))
}

#[cfg(feature = "multi-viewport")]
fn remove_primary_context(app: &mut App) -> Result<imgui::SuspendedContext, ImguiContextError> {
    let primary_id = primary_context_id(app);
    app.world_mut()
        .get_non_send_mut::<ImguiContexts>()
        .expect("plugin should install the ImGui Context registry")
        .remove(primary_id)
}

#[cfg(feature = "multi-viewport")]
fn app_with_multi_viewport_bridge() -> App {
    app_with_multi_viewport_window_config(ImguiViewportWindowConfig::default())
}

#[cfg(feature = "multi-viewport")]
fn app_with_multi_viewport_window_config(viewport_window: ImguiViewportWindowConfig) -> App {
    let mut app = App::new();
    app.insert_resource(
        crate::viewport::native_window::DesktopPositionSupportOverride(
            crate::viewport::native_window::DesktopPositionSupport::Available,
        ),
    );
    #[cfg(feature = "render")]
    {
        app.add_plugins(ExtractPlugin::default());
        app.sub_app_mut(RenderApp).update_schedule = Some(Render.intern());
    }
    app.add_message::<WindowCloseRequested>();
    app.add_plugins(ImguiPlugin::new(
        ImguiPluginConfig::default()
            .with_multi_viewport(true)
            .with_viewport_window(viewport_window),
    ));
    with_primary_context(&mut app, |context| {
        let _ = context.font_atlas().build();
    });
    app
}

#[cfg(feature = "multi-viewport")]
#[test]
fn additional_context_can_enable_native_viewports_when_primary_does_not() {
    let _guard = imgui_context_guard();
    let mut app = App::new();
    app.add_plugins(ImguiPlugin::new(ImguiPluginConfig::default()));
    app.add_systems(SecondaryViewportPass, || {});

    let (primary_id, secondary_id) = {
        let mut contexts = app.world_mut().get_non_send_mut::<ImguiContexts>().unwrap();
        let primary_id = contexts.primary_id().unwrap();
        let secondary_id = contexts
            .create(ImguiContextConfig::new(SecondaryViewportPass).with_multi_viewport(true))
            .expect("native viewport infrastructure should be available per Context");
        (primary_id, secondary_id)
    };

    let primary_owns_platform = app
        .world_mut()
        .get_non_send_mut::<ImguiContexts>()
        .unwrap()
        .configure(primary_id, |context| {
            !context.io().backend_platform_user_data().is_null()
        })
        .unwrap();
    let secondary_owns_platform = app
        .world_mut()
        .get_non_send_mut::<ImguiContexts>()
        .unwrap()
        .configure(secondary_id, |context| {
            !context.io().backend_platform_user_data().is_null()
                && unsafe {
                    (*context.platform_io().as_raw())
                        .Platform_CreateWindow
                        .is_some()
                }
        })
        .unwrap();
    assert!(!primary_owns_platform);
    assert!(secondary_owns_platform);

    let removed = app
        .world_mut()
        .get_non_send_mut::<ImguiContexts>()
        .unwrap()
        .remove(secondary_id)
        .expect("an unused Context-local viewport bridge should detach immediately");
    assert_eq!(removed.id(), secondary_id);
}

#[cfg(all(feature = "multi-viewport", feature = "render"))]
#[test]
fn native_viewport_support_is_scoped_by_context_and_drive_state() {
    let _guard = imgui_context_guard();
    let mut app = app_with_multi_viewport_bridge();
    app.add_systems(SecondaryViewportPass, || {});
    let primary_id = primary_context_id(&app);
    let secondary_id = app
        .world_mut()
        .get_non_send_mut::<ImguiContexts>()
        .unwrap()
        .create(ImguiContextConfig::new(SecondaryViewportPass).with_multi_viewport(true))
        .expect("the secondary Context should accept native viewport infrastructure");
    ensure_primary_window(&mut app);

    app.update();

    let support = app.world().resource::<ImguiNativeViewportSupport>();
    assert_eq!(
        support.get(primary_id),
        Some(ImguiNativeViewportStatus::Available)
    );
    assert_eq!(
        support.get(secondary_id),
        Some(ImguiNativeViewportStatus::PendingNativeWindow),
        "a Context without its own routed host must not overwrite another Context's capability"
    );
    assert_eq!(support.iter().len(), 2);
}

#[cfg(all(feature = "multi-viewport", feature = "render"))]
#[test]
fn pending_native_window_defers_the_first_context_frame_until_viewports_are_available() {
    let _guard = imgui_context_guard();
    let mut app = app_with_multi_viewport_bridge();
    let primary_id = primary_context_id(&app);
    ensure_primary_window(&mut app);
    app.insert_resource(
        crate::viewport::native_window::DesktopPositionSupportOverride(
            crate::viewport::native_window::DesktopPositionSupport::PendingWindow,
        ),
    );

    app.update();

    {
        let contexts = app.world().non_send::<ImguiContexts>();
        assert_eq!(contexts.frame_index(primary_id).unwrap(), 0);
        assert!(matches!(
            contexts.last_error(primary_id).unwrap(),
            Some(ImguiContextError::PlatformHostUnavailable { context_id })
                if *context_id == primary_id
        ));
    }
    assert_eq!(
        app.world()
            .resource::<ImguiNativeViewportSupport>()
            .get(primary_id),
        Some(ImguiNativeViewportStatus::PendingNativeWindow)
    );
    with_primary_context(&mut app, |context| {
        assert_eq!(unsafe { (*context.as_raw()).FrameCount }, 0);
        assert!(
            !context
                .io()
                .config_flags()
                .contains(imgui::ConfigFlags::VIEWPORTS_ENABLE)
        );
    });

    app.insert_resource(
        crate::viewport::native_window::DesktopPositionSupportOverride(
            crate::viewport::native_window::DesktopPositionSupport::Available,
        ),
    );
    app.update();

    {
        let contexts = app.world().non_send::<ImguiContexts>();
        assert_eq!(contexts.frame_index(primary_id).unwrap(), 1);
        assert!(contexts.last_error(primary_id).unwrap().is_none());
    }
    assert_eq!(
        app.world()
            .resource::<ImguiNativeViewportSupport>()
            .get(primary_id),
        Some(ImguiNativeViewportStatus::Available)
    );
    with_primary_context(&mut app, |context| {
        assert_eq!(unsafe { (*context.as_raw()).FrameCount }, 1);
        assert!(
            context
                .io()
                .config_flags()
                .contains(imgui::ConfigFlags::VIEWPORTS_ENABLE)
        );
    });
}

#[cfg(feature = "multi-viewport")]
#[derive(Resource)]
struct SubmitLiveSecondaryViewport(bool);

#[cfg(feature = "multi-viewport")]
fn submit_live_secondary_viewport(imgui: ImguiUi, submit: Res<SubmitLiveSecondaryViewport>) {
    if !submit.0 {
        return;
    }
    let ui = imgui.ui().expect("the primary ImGui frame should be open");
    ui.window("viewport-event-source")
        .size([320.0, 240.0], imgui::Condition::Always)
        .build(|| ui.text("secondary viewport event source"));
}

#[cfg(feature = "multi-viewport")]
fn assert_platform_window_update_finished(app: &mut App) {
    with_primary_context(app, |context| {
        let backend_flags = context.io().backend_flags();
        assert!(
            backend_flags.contains(
                imgui::BackendFlags::PLATFORM_HAS_VIEWPORTS
                    | imgui::BackendFlags::RENDERER_HAS_VIEWPORTS,
            ),
            "the fixture must advertise both platform and renderer viewport support"
        );
        assert!(
            context
                .io()
                .config_flags()
                .contains(imgui::ConfigFlags::VIEWPORTS_ENABLE),
            "the fixture must keep Dear ImGui viewport support enabled"
        );
        let raw = context.as_raw();
        assert_eq!(
            unsafe { (*raw).FrameCountEnded },
            unsafe { (*raw).FrameCount },
            "the private Context driver must finish the native frame before platform updates"
        );
        assert_eq!(
            unsafe { (*raw).FrameCountPlatformEnded },
            unsafe { (*raw).FrameCount },
            "the private Context driver must finish platform updates automatically"
        );
    });
}

#[cfg(feature = "multi-viewport")]
fn resolve_live_viewport(app: &mut App, viewport_id: imgui::Id) -> *mut sys::ImGuiViewport {
    let viewport = with_primary_context(app, |context| {
        let binding = context.binding();
        binding.with_bound_context(|| unsafe { sys::igFindViewportByID(viewport_id.raw()) })
    });
    assert!(
        !viewport.is_null(),
        "the secondary viewport must remain in Dear ImGui's internal registry"
    );
    viewport
}

#[cfg(feature = "multi-viewport")]
fn create_live_secondary_viewport(app: &mut App) -> (imgui::Id, Entity) {
    with_primary_context(app, |context| {
        context.io_mut().set_config_viewports_no_auto_merge(true);
    });
    app.insert_resource(SubmitLiveSecondaryViewport(true));
    app.add_systems(ImguiPrimaryContextPass, submit_live_secondary_viewport);
    // NoAutoMerge makes the UI window deterministically request its own native viewport. The
    // first frame creates it; the second lets Dear ImGui publish the platform window mapping.
    app.update();
    assert_platform_window_update_finished(app);
    app.update();
    assert_platform_window_update_finished(app);

    let (viewport_id, published_viewport) = with_primary_context(app, |context| {
        let main_viewport_id = context.main_viewport().id();
        let viewport = context
            .platform_io()
            .viewports_iter()
            .find(|viewport| viewport.id() != main_viewport_id)
            .expect("NoAutoMerge should create a visible secondary viewport");
        (viewport.id(), viewport.as_raw().cast_mut())
    });
    let resolved_viewport = resolve_live_viewport(app, viewport_id);
    assert_eq!(
        resolved_viewport, published_viewport,
        "the visible secondary viewport must resolve through Dear ImGui's complete registry"
    );
    let entity = app
        .world()
        .get_non_send::<ImguiViewportBridge>()
        .expect("bridge should still exist")
        .viewport_window(primary_context_id(app), viewport_id)
        .expect("the real secondary viewport should create a matching Bevy window");
    (viewport_id, entity)
}

#[cfg(feature = "multi-viewport")]
fn destroy_live_secondary_viewport(app: &mut App, viewport_id: imgui::Id) {
    let context_id = primary_context_id(app);
    let entity = app
        .world()
        .get_non_send::<ImguiViewportBridge>()
        .expect("bridge should still exist")
        .viewport_window(context_id, viewport_id)
        .expect("the live viewport should still own a Bevy window");
    app.world_mut()
        .resource_mut::<SubmitLiveSecondaryViewport>()
        .0 = false;
    // An inactive secondary is destroyed by the normal Dear ImGui platform update after two
    // inactive frames. Do not invoke `DestroyPlatformWindows` here: that is a whole-context
    // shutdown transaction, not a frame lifecycle transition.
    app.update();
    assert_platform_window_update_finished(app);
    app.update();
    assert_platform_window_update_finished(app);
    assert!(
        app.world()
            .get_non_send::<ImguiViewportBridge>()
            .expect("bridge should still exist")
            .viewport_window(context_id, viewport_id)
            .is_none(),
        "destroying the live platform viewport must remove its Bevy window mapping"
    );
    assert!(
        app.world().get_entity(entity).is_err(),
        "destroying the live platform viewport must despawn its Bevy window"
    );
    let published = with_primary_context(app, |context| {
        context
            .platform_io()
            .viewports_iter()
            .any(|viewport| viewport.id() == viewport_id)
    });
    assert!(
        !published,
        "an inactive secondary viewport must leave the public PlatformIO snapshot"
    );
    let raw_viewport = resolve_live_viewport(app, viewport_id);
    // Dear ImGui retains the internal viewport for two inactive frames. Its platform fields must
    // already be cleared before the bridge releases the corresponding Bevy entity.
    unsafe {
        assert!((*raw_viewport).PlatformUserData.is_null());
        assert!((*raw_viewport).PlatformHandle.is_null());
        assert!((*raw_viewport).PlatformHandleRaw.is_null());
    }
}

#[cfg(all(feature = "multi-viewport", feature = "render"))]
fn create_callback_viewport(
    app: &mut App,
    context_id: imgui::ContextId,
    snapshot: &ImguiViewportSnapshot,
) -> *mut sys::ImGuiViewport {
    app.world_mut()
        .get_non_send_mut::<ImguiContexts>()
        .expect("plugin should install the Context registry")
        .configure(context_id, |context| {
            let raw_viewport = unsafe { sys::ImGuiViewport_ImGuiViewport() };
            assert!(
                !raw_viewport.is_null(),
                "ImGuiViewport_ImGuiViewport() returned null"
            );
            unsafe {
                let viewport = imgui::Viewport::from_raw_mut(raw_viewport);
                (*raw_viewport).ID = snapshot.id.raw();
                viewport.set_pos(snapshot.pos);
                viewport.set_size(snapshot.size);
                viewport.set_dpi_scale(snapshot.dpi_scale);
                viewport.set_raw_flags_unchecked(snapshot.flags.bits());
                let platform_io = context.platform_io().as_raw();
                (*platform_io)
                    .Platform_CreateWindow
                    .expect("the Context should own Platform_CreateWindow")(
                    raw_viewport
                );
            }
            raw_viewport
        })
        .unwrap_or_else(|error| panic!("Context callback fixture could not be configured: {error}"))
}

#[cfg(all(feature = "multi-viewport", feature = "render"))]
fn update_callback_viewport(
    app: &mut App,
    context_id: imgui::ContextId,
    raw_viewport: *mut sys::ImGuiViewport,
    pos: [f32; 2],
) {
    app.world_mut()
        .get_non_send_mut::<ImguiContexts>()
        .expect("plugin should install the Context registry")
        .configure(context_id, |context| unsafe {
            let platform_io = context.platform_io().as_raw();
            let pos = sys::ImVec2 {
                x: pos[0],
                y: pos[1],
            };
            assert!(sys::ImGuiPlatformIO_InvokePlatformSetWindowPos(
                platform_io,
                raw_viewport,
                &pos,
            ));
            (*platform_io)
                .Platform_ShowWindow
                .expect("the Context should own Platform_ShowWindow")(raw_viewport);
            (*platform_io)
                .Platform_SetWindowFocus
                .expect("the Context should own Platform_SetWindowFocus")(raw_viewport);
        })
        .unwrap_or_else(|error| panic!("Context callback fixture could not be updated: {error}"));
}

#[cfg(all(feature = "multi-viewport", feature = "render"))]
fn destroy_callback_viewport(
    app: &mut App,
    context_id: imgui::ContextId,
    raw_viewport: *mut sys::ImGuiViewport,
) {
    app.world_mut()
        .get_non_send_mut::<ImguiContexts>()
        .expect("plugin should install the Context registry")
        .configure(context_id, |context| unsafe {
            let platform_io = context.platform_io().as_raw();
            (*platform_io)
                .Platform_DestroyWindow
                .expect("the Context should own Platform_DestroyWindow")(raw_viewport);
        })
        .unwrap_or_else(|error| panic!("Context callback fixture could not be destroyed: {error}"));
    unsafe { sys::ImGuiViewport_destroy(raw_viewport) };
}

#[cfg(all(feature = "multi-viewport", feature = "render"))]
fn spawn_secondary_viewport(app: &mut App) -> (imgui::Id, Entity, Entity) {
    ensure_primary_window(app);
    let (id, window) = create_live_secondary_viewport(app);
    let context_id = primary_context_id(app);

    let bridge = app
        .world()
        .get_non_send::<ImguiViewportBridge>()
        .expect("bridge should still exist");
    let camera = bridge
        .viewport_camera(context_id, id)
        .expect("the live secondary viewport should spawn a matching overlay camera");
    (id, window, camera)
}

#[cfg(feature = "multi-viewport")]
fn ensure_primary_window(app: &mut App) -> Entity {
    let mut primary_windows = app
        .world_mut()
        .query_filtered::<Entity, With<PrimaryWindow>>();
    let entity = primary_windows.iter(app.world()).next().unwrap_or_else(|| {
        app.world_mut()
            .spawn((Window::default(), PrimaryWindow))
            .id()
    });
    if app.world().get_non_send::<ImguiContexts>().is_some() {
        with_primary_context(app, |context| {
            let _ = context.font_atlas().build();
        });
    }
    entity
}

#[cfg(feature = "multi-viewport")]
fn foreign_platform_monitor() -> sys::ImGuiPlatformMonitor {
    sys::ImGuiPlatformMonitor {
        MainPos: sys::ImVec2 { x: 80.0, y: 120.0 },
        MainSize: sys::ImVec2 {
            x: 1600.0,
            y: 900.0,
        },
        WorkPos: sys::ImVec2 { x: 80.0, y: 160.0 },
        WorkSize: sys::ImVec2 {
            x: 1600.0,
            y: 860.0,
        },
        DpiScale: 1.25,
        PlatformHandle: std::ptr::dangling_mut::<u16>().cast(),
    }
}

#[cfg(feature = "multi-viewport")]
#[test]
fn multi_viewport_feature_installs_an_inert_bridge_until_requested() {
    let _guard = imgui_context_guard();
    let mut app = App::new();
    app.add_plugins(ImguiPlugin::default());

    assert!(
        app.world().get_non_send::<ImguiViewportBridge>().is_some(),
        "the feature must install shared infrastructure for later Context-local opt-in"
    );

    let backend_platform_user_data = with_primary_context(&mut app, |context| {
        context.io().backend_platform_user_data()
    });
    assert!(
        backend_platform_user_data.is_null(),
        "BackendPlatformUserData should stay unset when multi_viewport is not requested"
    );
}

#[cfg(feature = "multi-viewport")]
#[test]
fn multi_viewport_bridge_stays_inert_without_render_integration() {
    let _guard = imgui_context_guard();
    let mut app = App::new();
    app.add_plugins(ImguiPlugin::new(
        ImguiPluginConfig::default().with_multi_viewport(true),
    ));
    ensure_primary_window(&mut app);
    app.update();

    assert!(app.world().get_non_send::<ImguiViewportBridge>().is_some());

    let (backend_platform_user_data, config_flags, backend_flags) =
        with_primary_context(&mut app, |context| {
            (
                context.io().backend_platform_user_data(),
                context.io().config_flags(),
                context.io().backend_flags(),
            )
        });
    let bridge = app
        .world()
        .get_non_send::<ImguiViewportBridge>()
        .expect("multi-viewport feature should install bridge");
    assert!(
        !backend_platform_user_data.is_null(),
        "BackendPlatformUserData should point at the bridge's stable boxed state"
    );
    assert_eq!(
        bridge.callback_error_for(primary_context_id(&app)),
        None,
        "a newly installed bridge should not report a callback failure"
    );
    assert!(
        !config_flags.contains(imgui::ConfigFlags::VIEWPORTS_ENABLE),
        "native viewports must stay disabled until a renderer is attached"
    );
    assert!(
        !backend_flags.contains(imgui::BackendFlags::RENDERER_HAS_VIEWPORTS),
        "the platform bridge must not claim renderer viewport support"
    );
}

#[cfg(feature = "multi-viewport")]
#[test]
fn viewport_prepare_rejects_replaced_main_platform_user_data() {
    let _guard = imgui_context_guard();
    let mut app = app_with_multi_viewport_bridge();
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    app.update();

    let foreign = std::ptr::dangling_mut::<u16>().cast::<std::ffi::c_void>();
    with_primary_context(&mut app, |context| {
        let main_viewport = context.main_viewport();
        assert_eq!(
            main_viewport.platform_user_data(),
            main_viewport.platform_handle()
        );
        unsafe { main_viewport.set_platform_user_data(foreign) };
    });

    app.update();
    let primary_id = primary_context_id(&app);
    assert!(matches!(
        app.world()
            .get_non_send::<ImguiContexts>()
            .unwrap()
            .last_error(primary_id),
        Ok(Some(ImguiContextError::ViewportBridge {
            source:
                dear_imgui_bevy::viewport::ImguiViewportRuntimeError::CallbackOwnership(
                    dear_imgui_bevy::viewport::ImguiViewportCallbackOwnershipError::
                        ViewportFieldReplaced {
                            field: "PlatformUserData",
                        },
                ),
            ..
        }))
    ));

    with_primary_context(&mut app, |context| {
        let main_viewport = context.main_viewport();
        assert_eq!(
            main_viewport.platform_user_data(),
            foreign,
            "frame preparation must not overwrite a foreign PlatformUserData claim"
        );
        unsafe { main_viewport.set_platform_user_data(std::ptr::null_mut()) };
    });
    assert_eq!(
        app.world()
            .get_non_send::<ImguiViewportBridge>()
            .unwrap()
            .callback_error_for(primary_context_id(&app)),
        Some(
            dear_imgui_bevy::viewport::ImguiViewportRuntimeError::CallbackOwnership(
                dear_imgui_bevy::viewport::ImguiViewportCallbackOwnershipError::
                    ViewportFieldReplaced {
                        field: "PlatformUserData",
                    },
            )
        )
    );
}

#[cfg(feature = "multi-viewport")]
#[test]
fn viewport_primary_cleanup_clears_imgui_platform_handles() {
    let _guard = imgui_context_guard();
    let mut app = app_with_multi_viewport_bridge();
    let primary = app
        .world_mut()
        .spawn((Window::default(), PrimaryWindow))
        .id();

    app.update();

    with_primary_context(&mut app, |context| {
        let main_viewport = context.main_viewport();
        assert!(!main_viewport.platform_handle().is_null());
        assert_eq!(
            main_viewport.platform_handle(),
            main_viewport.platform_user_data()
        );
    });

    app.world_mut()
        .resource_mut::<Messages<WindowCloseRequested>>()
        .write(WindowCloseRequested { window: primary });
    app.update();

    let (handle, user_data) = with_primary_context(&mut app, |context| {
        let main_viewport = context.main_viewport();
        (
            main_viewport.platform_handle(),
            main_viewport.platform_user_data(),
        )
    });
    assert!(
        handle.is_null(),
        "primary cleanup must clear ImGui's main viewport PlatformHandle before releasing backend handles"
    );
    assert!(
        user_data.is_null(),
        "primary cleanup must clear ImGui's main viewport PlatformUserData before releasing backend handles"
    );
}

#[cfg(feature = "multi-viewport")]
#[test]
fn temporary_host_loss_preserves_sticky_callback_fault() {
    let _guard = imgui_context_guard();
    let mut app = app_with_multi_viewport_bridge();
    let primary = ensure_primary_window(&mut app);
    app.update();

    let foreign = std::ptr::dangling_mut::<u16>().cast::<std::ffi::c_void>();
    with_primary_context(&mut app, |context| unsafe {
        context.main_viewport().set_platform_user_data(foreign);
    });
    app.update();

    let context_id = primary_context_id(&app);
    let expected = dear_imgui_bevy::viewport::ImguiViewportRuntimeError::CallbackOwnership(
        dear_imgui_bevy::viewport::ImguiViewportCallbackOwnershipError::ViewportFieldReplaced {
            field: "PlatformUserData",
        },
    );
    assert_eq!(
        app.world()
            .get_non_send::<ImguiViewportBridge>()
            .unwrap()
            .callback_error_for(context_id),
        Some(expected)
    );

    app.world_mut()
        .resource_mut::<Messages<WindowCloseRequested>>()
        .write(WindowCloseRequested { window: primary });
    app.update();

    assert_eq!(
        app.world()
            .get_non_send::<ImguiViewportBridge>()
            .unwrap()
            .callback_error_for(context_id),
        Some(expected),
        "temporary host cleanup must not erase a callback fault before bridge teardown"
    );

    with_primary_context(&mut app, |context| unsafe {
        context
            .main_viewport()
            .set_platform_user_data(std::ptr::null_mut());
    });
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
    #[cfg(not(target_os = "macos"))]
    assert_eq!(
        window.position,
        WindowPosition::At(bevy_math::IVec2::new(32, 48))
    );
    #[cfg(target_os = "macos")]
    assert_eq!(
        window.position,
        WindowPosition::At(bevy_math::IVec2::new(64, 96))
    );
    #[cfg(not(target_os = "macos"))]
    assert_eq!(window.resolution.physical_width(), 640);
    #[cfg(not(target_os = "macos"))]
    assert_eq!(window.resolution.physical_height(), 360);
    #[cfg(target_os = "macos")]
    assert_eq!(window.resolution.physical_width(), 1280);
    #[cfg(target_os = "macos")]
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
fn viewport_platform_monitors_preserve_non_overlapping_native_desktop_space() {
    #[cfg(not(target_os = "macos"))]
    let primary_physical_x = 1920;
    #[cfg(target_os = "macos")]
    let primary_physical_x = 3840;
    let primary = Monitor {
        name: Some("primary".to_owned()),
        physical_width: 2560,
        physical_height: 1600,
        physical_position: bevy_math::IVec2::new(primary_physical_x, 0),
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
    assert_eq!(monitors[0].MainPos.x, 1920.0);
    assert_eq!(monitors[0].MainPos.y, 0.0);
    #[cfg(not(target_os = "macos"))]
    assert_eq!(monitors[0].MainSize.x, 2560.0);
    #[cfg(not(target_os = "macos"))]
    assert_eq!(monitors[0].MainSize.y, 1600.0);
    #[cfg(target_os = "macos")]
    assert_eq!(monitors[0].MainSize.x, 1280.0);
    #[cfg(target_os = "macos")]
    assert_eq!(monitors[0].MainSize.y, 800.0);
    assert_eq!(monitors[0].DpiScale, 2.0);
    assert_eq!(monitors[1].MainPos.x, 0.0);
    assert_eq!(monitors[1].MainPos.y, 0.0);
    assert_eq!(monitors[1].MainSize.x, 1920.0);
    assert_eq!(monitors[1].MainSize.y, 1080.0);
    assert_eq!(monitors[1].DpiScale, 1.0);
    assert_eq!(
        monitors[1].MainPos.x + monitors[1].MainSize.x,
        monitors[0].MainPos.x,
        "adjacent mixed-DPI monitors must not overlap or gain an artificial gap"
    );
}

#[cfg(feature = "multi-viewport")]
#[test]
fn viewport_platform_io_callbacks_capture_commands_and_bevy_system_applies_them() {
    let _guard = imgui_context_guard();
    let mut app = App::new();
    app.add_plugins(ImguiPlugin::new(
        ImguiPluginConfig::default().with_multi_viewport(true),
    ));
    ensure_primary_window(&mut app);

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
        viewport.set_raw_flags_unchecked(imgui::ViewportFlags::IS_PLATFORM_WINDOW.bits());
    }

    with_primary_context(&mut app, |context| {
        let platform_io = context.platform_io().as_raw();
        let backend_platform_user_data = context.io().backend_platform_user_data();
        unsafe {
            (*platform_io)
                .Platform_CreateWindow
                .expect("bridge should install Platform_CreateWindow")(raw_viewport);
            let pos = sys::ImVec2 { x: 88.0, y: 99.0 };
            assert!(sys::ImGuiPlatformIO_InvokePlatformSetWindowPos(
                platform_io.cast_mut(),
                raw_viewport,
                &pos,
            ));
            assert!(!(*raw_viewport).PlatformHandle.is_null());
            assert_eq!(
                (*raw_viewport).PlatformHandle,
                (*raw_viewport).PlatformUserData
            );
            assert_ne!(
                (*raw_viewport).PlatformHandle,
                backend_platform_user_data,
                "each Dear ImGui viewport needs its own platform handle; backend userdata only identifies the owning Context bridge"
            );
        }
    });

    app.update();

    with_primary_context(&mut app, |context| unsafe {
        let platform_io = context.platform_io().as_raw();
        imgui::Viewport::from_raw_mut(raw_viewport).set_raw_flags_unchecked(
            (imgui::ViewportFlags::IS_PLATFORM_WINDOW
                | imgui::ViewportFlags::NO_DECORATION
                | imgui::ViewportFlags::NO_TASK_BAR_ICON
                | imgui::ViewportFlags::TOP_MOST
                | imgui::ViewportFlags::NO_INPUTS)
                .bits(),
        );
        (*platform_io)
            .Platform_UpdateWindow
            .expect("bridge should install Platform_UpdateWindow")(raw_viewport);
    });
    app.update();

    let bridge = app
        .world()
        .get_non_send::<ImguiViewportBridge>()
        .expect("bridge should still exist");
    let entity = bridge
        .viewport_window(primary_context_id(&app), id)
        .expect("captured create command should spawn a Bevy window entity");
    let window = app
        .world()
        .get::<Window>(entity)
        .expect("spawned entity should contain Window");
    assert_eq!(
        window.position,
        WindowPosition::At(bevy_math::IVec2::new(88, 99))
    );
    assert!(!window.decorations);
    assert!(window.skip_taskbar);
    assert_eq!(window.window_level, bevy_window::WindowLevel::AlwaysOnTop);
    let cursor_options = app
        .world()
        .get::<bevy_window::CursorOptions>(entity)
        .expect("viewport windows must retain their cursor policy");
    assert_eq!(
        cursor_options.hit_test,
        !crate::viewport::native_window::supports_pointer_passthrough()
    );

    with_primary_context(&mut app, |context| unsafe {
        let platform_io = context.platform_io().as_raw();
        imgui::Viewport::from_raw_mut(raw_viewport)
            .set_raw_flags_unchecked(imgui::ViewportFlags::IS_PLATFORM_WINDOW.bits());
        (*platform_io)
            .Platform_UpdateWindow
            .expect("bridge should install Platform_UpdateWindow")(raw_viewport);
    });
    app.update();

    let window = app
        .world()
        .get::<Window>(entity)
        .expect("updated viewport should retain its Window");
    assert!(window.decorations);
    assert!(!window.skip_taskbar);
    assert_eq!(window.window_level, bevy_window::WindowLevel::Normal);
    assert!(
        app.world()
            .get::<bevy_window::CursorOptions>(entity)
            .expect("viewport windows must retain their cursor policy")
            .hit_test
    );

    with_primary_context(&mut app, |context| unsafe {
        let platform_io = context.platform_io().as_raw();
        (*platform_io)
            .Platform_DestroyWindow
            .expect("bridge should install Platform_DestroyWindow")(raw_viewport);
    });
    unsafe { sys::ImGuiViewport_destroy(raw_viewport) };
}

#[cfg(all(feature = "multi-viewport", feature = "render"))]
#[test]
fn native_viewport_bridge_isolates_equal_ids_across_two_contexts() {
    let _guard = imgui_context_guard();
    let mut app = app_with_multi_viewport_bridge();
    app.add_systems(SecondaryViewportPass, || {});
    ensure_primary_window(&mut app);
    let primary_id = primary_context_id(&app);
    let secondary_id = app
        .world_mut()
        .get_non_send_mut::<ImguiContexts>()
        .expect("plugin should install the Context registry")
        .create(ImguiContextConfig::new(SecondaryViewportPass).with_multi_viewport(true))
        .expect("an additional Context should receive its own viewport bridge");
    app.world_mut()
        .get_non_send_mut::<ImguiContexts>()
        .expect("plugin should install the Context registry")
        .configure(secondary_id, |context| {
            assert!(context.font_atlas().build());
        })
        .expect("the additional Context should remain configurable");
    let secondary_host = app.world_mut().spawn(Window::default()).id();
    app.world_mut().spawn(ImguiInputRoute::logical(
        secondary_id,
        secondary_host,
        Rect::from_corners(Vec2::ZERO, Vec2::splat(512.0)),
    ));
    let viewport_id = imgui::Id::from(0x20A);
    let snapshot = ImguiViewportSnapshot {
        id: viewport_id,
        pos: [32.0, 48.0],
        size: [320.0, 180.0],
        dpi_scale: 1.0,
        flags: imgui::ViewportFlags::IS_PLATFORM_WINDOW,
    };
    let primary_raw = create_callback_viewport(&mut app, primary_id, &snapshot);
    let secondary_raw = create_callback_viewport(&mut app, secondary_id, &snapshot);
    app.update();

    let (primary_window, secondary_window, primary_camera, secondary_camera) = {
        let bridge = app
            .world()
            .get_non_send::<ImguiViewportBridge>()
            .expect("bridge should remain installed");
        (
            bridge
                .viewport_window(primary_id, viewport_id)
                .expect("primary Context should own its viewport window"),
            bridge
                .viewport_window(secondary_id, viewport_id)
                .expect("secondary Context should own its viewport window"),
            bridge
                .viewport_camera(primary_id, viewport_id)
                .expect("primary Context should own its viewport camera"),
            bridge
                .viewport_camera(secondary_id, viewport_id)
                .expect("secondary Context should own its viewport camera"),
        )
    };
    assert_ne!(primary_window, secondary_window);
    assert_ne!(primary_camera, secondary_camera);
    assert_eq!(
        app.world()
            .get::<ImguiViewportWindow>(primary_window)
            .expect("primary viewport should carry its marker")
            .context_id(),
        primary_id
    );
    assert_eq!(
        app.world()
            .get::<ImguiViewportWindow>(secondary_window)
            .expect("secondary viewport should carry its marker")
            .context_id(),
        secondary_id
    );
    assert_eq!(
        app.world()
            .get::<ImguiViewportCamera>(primary_camera)
            .expect("primary viewport camera should carry its identity")
            .context_id(),
        primary_id
    );
    assert_eq!(
        app.world()
            .get::<ImguiViewportCamera>(secondary_camera)
            .expect("secondary viewport camera should carry its identity")
            .context_id(),
        secondary_id
    );
    assert!(matches!(
        app.world().get::<RenderTarget>(primary_camera),
        Some(RenderTarget::Window(WindowRef::Entity(entity))) if *entity == primary_window
    ));
    assert!(matches!(
        app.world().get::<RenderTarget>(secondary_camera),
        Some(RenderTarget::Window(WindowRef::Entity(entity))) if *entity == secondary_window
    ));

    update_callback_viewport(&mut app, primary_id, primary_raw, [240.0, 160.0]);
    app.update();
    assert_eq!(
        app.world()
            .get::<Window>(primary_window)
            .expect("primary viewport window should remain live")
            .position,
        WindowPosition::At(IVec2::new(240, 160))
    );
    assert_eq!(
        app.world()
            .get::<Window>(secondary_window)
            .expect("secondary viewport window should remain live")
            .position,
        WindowPosition::At(IVec2::new(32, 48)),
        "updating one Context must not move the other Context's equal viewport id"
    );
    {
        let bridge = app
            .world()
            .get_non_send::<ImguiViewportBridge>()
            .expect("bridge should remain installed");
        assert_eq!(
            bridge
                .viewport_feedback(primary_id, viewport_id)
                .expect("primary feedback should remain Context-local")
                .pos,
            [32.0, 48.0],
            "a native position request must not masquerade as observed platform feedback"
        );
        let mut observed = bridge
            .viewport_feedback(primary_id, viewport_id)
            .expect("primary feedback should remain Context-local");
        observed.pos = [240.0, 160.0];
        let reconciliation = bridge
            .context(primary_id)
            .expect("primary Context bridge should remain registered")
            .observe_viewport_feedback(viewport_id, observed);
        assert!(
            !reconciliation.request_move && !reconciliation.request_resize,
            "the matching native observation should acknowledge only the primary Context request"
        );
        assert_eq!(
            bridge
                .viewport_feedback(primary_id, viewport_id)
                .expect("primary feedback should remain Context-local")
                .pos,
            [240.0, 160.0]
        );
        assert_eq!(
            bridge
                .viewport_feedback(secondary_id, viewport_id)
                .expect("secondary feedback should remain Context-local")
                .pos,
            [32.0, 48.0]
        );
    }

    destroy_callback_viewport(&mut app, secondary_id, secondary_raw);
    app.update();
    {
        let bridge = app
            .world()
            .get_non_send::<ImguiViewportBridge>()
            .expect("bridge should remain installed");
        assert!(bridge.viewport_window(secondary_id, viewport_id).is_none());
        assert_eq!(
            bridge.viewport_window(primary_id, viewport_id),
            Some(primary_window)
        );
        assert_eq!(bridge.callback_error_for(primary_id), None);
        assert_eq!(bridge.callback_error_for(secondary_id), None);
    }
    assert!(app.world().get_entity(secondary_window).is_err());
    assert!(app.world().get_entity(secondary_camera).is_err());
    assert!(app.world().get_entity(primary_camera).is_ok());

    let primary_frame_before_fault = app
        .world()
        .get_non_send::<ImguiContexts>()
        .unwrap()
        .frame_index(primary_id)
        .unwrap();
    update_callback_viewport(&mut app, primary_id, primary_raw, [300.0, 220.0]);
    app.world_mut()
        .get_non_send_mut::<ImguiContexts>()
        .unwrap()
        .configure(secondary_id, |context| unsafe {
            context
                .io_mut()
                .set_backend_platform_user_data(std::ptr::null_mut());
        })
        .unwrap();
    app.update();
    assert!(
        app.world()
            .get_non_send::<ImguiContexts>()
            .unwrap()
            .frame_index(primary_id)
            .unwrap()
            > primary_frame_before_fault,
        "one Context's callback fault must not stop another Context"
    );
    assert_eq!(
        app.world().get::<Window>(primary_window).unwrap().position,
        WindowPosition::At(IVec2::new(300, 220)),
        "the healthy Context must continue consuming its own viewport callbacks"
    );
    assert!(matches!(
        app.world()
            .get_non_send::<ImguiContexts>()
            .unwrap()
            .last_error(secondary_id),
        Ok(Some(ImguiContextError::ViewportBridge {
            source:
                dear_imgui_bevy::viewport::ImguiViewportRuntimeError::CallbackOwnership(
                    dear_imgui_bevy::viewport::ImguiViewportCallbackOwnershipError::
                        BackendPlatformUserDataReplaced,
                ),
            ..
        }))
    ));

    assert!(matches!(
        app.world_mut()
            .get_non_send_mut::<ImguiContexts>()
            .unwrap()
            .remove(secondary_id),
        Err(ImguiContextError::RemovalPending {
            reason:
                dear_imgui_bevy::ImguiContextRemovalPendingReason::ViewportCallbackOwnership(
                    dear_imgui_bevy::viewport::ImguiViewportCallbackOwnershipError::
                        BackendPlatformUserDataReplaced,
                ),
            ..
        })
    ));
    app.update();
    let removed = app
        .world_mut()
        .get_non_send_mut::<ImguiContexts>()
        .expect("plugin should install the Context registry")
        .remove(secondary_id)
        .expect("callback-fault teardown should complete on retry");
    assert_eq!(removed.id(), secondary_id);
    let bridge = app
        .world()
        .get_non_send::<ImguiViewportBridge>()
        .expect("removing one Context must preserve the global bridge");
    assert_eq!(
        bridge.viewport_window(primary_id, viewport_id),
        Some(primary_window)
    );
    assert_eq!(
        bridge.viewport_camera(primary_id, viewport_id),
        Some(primary_camera)
    );
    assert_eq!(bridge.callback_error_for(primary_id), None);

    destroy_callback_viewport(&mut app, primary_id, primary_raw);
    app.update();
}

#[cfg(feature = "multi-viewport")]
#[test]
fn viewport_destroy_callback_ignores_owned_by_app_main_viewport() {
    let _guard = imgui_context_guard();
    let mut app = app_with_multi_viewport_bridge();
    ensure_primary_window(&mut app);
    app.update();
    let context_id = primary_context_id(&app);
    let main_viewport_id = with_primary_context(&mut app, |context| context.main_viewport().id());

    let destroy_window = with_primary_context(&mut app, |context| {
        let platform_io = context.platform_io().as_raw();
        unsafe {
            (*platform_io)
                .Platform_DestroyWindow
                .expect("bridge should install Platform_DestroyWindow")
        }
    });

    with_primary_context(&mut app, |context| {
        let main_viewport = context.main_viewport();
        unsafe {
            (*main_viewport.as_raw_mut()).Flags = imgui::ViewportFlags::OWNED_BY_APP.bits();
            destroy_window(main_viewport.as_raw_mut());
        }
    });

    app.update();
    assert!(
        app.world()
            .get_non_send::<ImguiViewportBridge>()
            .expect("bridge should still exist")
            .viewport_window(context_id, main_viewport_id)
            .is_some(),
        "destroying the application-owned main viewport must not remove its main-window mapping"
    );
}

#[cfg(feature = "multi-viewport")]
#[test]
fn context_drop_sanitizes_owned_state_before_foreign_destroy_callbacks_run() {
    let _guard = imgui_context_guard();

    unsafe extern "C" fn observe_live_backend_user_data(viewport: *mut sys::ImGuiViewport) {
        let io = unsafe { sys::igGetIO_Nil() };
        let live = !io.is_null() && unsafe { !(*io).BackendPlatformUserData.is_null() };
        FOREIGN_DESTROY_SAW_BEVY_BACKEND_USER_DATA.store(live, std::sync::atomic::Ordering::SeqCst);
        if let Some(viewport) = unsafe { viewport.as_mut() } {
            viewport.PlatformUserData = std::ptr::null_mut();
            viewport.PlatformHandle = std::ptr::null_mut();
        }
    }

    FOREIGN_DESTROY_SAW_BEVY_BACKEND_USER_DATA.store(false, std::sync::atomic::Ordering::SeqCst);
    let mut app = app_with_multi_viewport_bridge();
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    app.update();
    with_primary_context(&mut app, |context| {
        let main_viewport = context.main_viewport();
        unsafe {
            assert!(!main_viewport.platform_user_data().is_null());
            assert!(!main_viewport.platform_handle().is_null());
            let platform_io = context.platform_io_mut().as_raw_mut();
            (*platform_io).Platform_DestroyWindow = Some(observe_live_backend_user_data);
        }
    });

    drop(
        app.world_mut()
            .remove_non_send::<ImguiViewportBridge>()
            .expect("ImguiViewportBridge should be removable for shutdown-order testing"),
    );

    assert!(matches!(
        remove_primary_context(&mut app),
        Err(ImguiContextError::RemovalPending {
            reason:
                dear_imgui_bevy::ImguiContextRemovalPendingReason::ViewportCallbackOwnership(
                    dear_imgui_bevy::viewport::ImguiViewportCallbackOwnershipError::
                        PlatformCallbackReplaced {
                            slot: "Platform_DestroyWindow",
                        },
                ),
            ..
        })
    ));
    drop(app);

    assert!(
        !FOREIGN_DESTROY_SAW_BEVY_BACKEND_USER_DATA.load(std::sync::atomic::Ordering::SeqCst),
        "foreign destroy callbacks must never observe Bevy-owned BackendPlatformUserData"
    );
}

#[cfg(feature = "multi-viewport")]
#[test]
fn context_drop_preserves_complete_foreign_platform_takeover() {
    let _guard = imgui_context_guard();
    FOREIGN_DROP_BACKEND_FLAGS.store(0, std::sync::atomic::Ordering::SeqCst);
    FOREIGN_DROP_CONFIG_FLAGS.store(0, std::sync::atomic::Ordering::SeqCst);
    FOREIGN_DROP_FIELDS_PRESERVED.store(false, std::sync::atomic::Ordering::SeqCst);

    let mut app = app_with_multi_viewport_bridge();
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    app.update();

    let (expected_backend_flags, expected_config_flags) =
        with_primary_context(&mut app, |context| {
            let foreign_user_data = std::ptr::dangling_mut::<u16>().cast();
            let foreign_main_user_data = std::ptr::dangling_mut::<u32>().cast();
            let foreign_main_handle = std::ptr::dangling_mut::<u64>().cast();
            let foreign_main_handle_raw = std::ptr::dangling_mut::<u8>().cast();
            FOREIGN_DROP_EXPECTED_USER_DATA.store(
                foreign_user_data as usize,
                std::sync::atomic::Ordering::SeqCst,
            );
            FOREIGN_DROP_EXPECTED_MAIN_USER_DATA.store(
                foreign_main_user_data as usize,
                std::sync::atomic::Ordering::SeqCst,
            );
            FOREIGN_DROP_EXPECTED_MAIN_HANDLE.store(
                foreign_main_handle as usize,
                std::sync::atomic::Ordering::SeqCst,
            );
            FOREIGN_DROP_EXPECTED_MAIN_HANDLE_RAW.store(
                foreign_main_handle_raw as usize,
                std::sync::atomic::Ordering::SeqCst,
            );

            let expected_backend_flags = context.io().backend_flags()
                | imgui::BackendFlags::PLATFORM_HAS_VIEWPORTS
                | imgui::BackendFlags::RENDERER_HAS_VIEWPORTS
                | imgui::BackendFlags::HAS_MOUSE_HOVERED_VIEWPORT;
            let expected_config_flags =
                context.io().config_flags() | imgui::ConfigFlags::VIEWPORTS_ENABLE;
            unsafe {
                context
                    .io_mut()
                    .set_backend_platform_user_data(foreign_user_data);
                context
                    .set_platform_name(Some("foreign-platform-drop"))
                    .unwrap();
                let platform_io = context.platform_io_mut().as_raw_mut();
                sys::ImGuiPlatformIO_ClearPlatformHandlers(platform_io);
                context
                    .platform_io_mut()
                    .set_monitors(&[foreign_platform_monitor()]);
                let main_viewport = context.main_viewport().as_raw_mut();
                (*main_viewport).PlatformUserData = foreign_main_user_data;
                (*main_viewport).PlatformHandle = foreign_main_handle;
                (*main_viewport).PlatformHandleRaw = foreign_main_handle_raw;
                (*main_viewport).PlatformWindowCreated = true;
            }
            context.io_mut().set_backend_flags(expected_backend_flags);
            context.io_mut().set_config_flags(expected_config_flags);
            context
                .register_attachment::<ForeignDropObserverMarker>(
                    imgui::ContextAttachmentRole::Extension,
                    Rc::new(ForeignDropObserver),
                )
                .expect("the foreign test backend should register its teardown observer")
                .defer_to_context();
            (expected_backend_flags, expected_config_flags)
        });
    let mut expected_drop_backend_flags = expected_backend_flags;
    expected_drop_backend_flags.remove(
        imgui::BackendFlags::RENDERER_HAS_VTX_OFFSET | imgui::BackendFlags::RENDERER_HAS_TEXTURES,
    );

    assert!(matches!(
        remove_primary_context(&mut app),
        Err(ImguiContextError::RemovalPending {
            reason: dear_imgui_bevy::ImguiContextRemovalPendingReason::RenderWorldReleasePending,
            ..
        })
    ));
    app.update();
    drop(remove_primary_context(&mut app).expect(
        "the complete foreign takeover should permit removal after renderer acknowledgement",
    ));

    assert_eq!(
        FOREIGN_DROP_BACKEND_FLAGS.load(std::sync::atomic::Ordering::SeqCst),
        expected_drop_backend_flags.bits(),
        "Drop must preserve foreign platform flags after clearing Bevy renderer capabilities"
    );
    assert_eq!(
        FOREIGN_DROP_CONFIG_FLAGS.load(std::sync::atomic::Ordering::SeqCst),
        expected_config_flags.bits(),
        "Drop must not clear foreign viewport configuration"
    );
    assert!(
        FOREIGN_DROP_FIELDS_PRESERVED.load(std::sync::atomic::Ordering::SeqCst),
        "Drop must leave the complete foreign platform claim intact until Dear ImGui dispatches it"
    );
}

#[cfg(all(feature = "multi-viewport", feature = "render"))]
#[test]
fn context_extraction_releases_secondary_entities_before_returning_context() {
    let _guard = imgui_context_guard();
    let mut app = app_with_multi_viewport_bridge();
    let primary = ensure_primary_window(&mut app);
    let (_, window, camera) = spawn_secondary_viewport(&mut app);

    let primary_id = primary_context_id(&app);
    assert!(matches!(
        remove_primary_context(&mut app),
        Err(ImguiContextError::RemovalPending {
            context_id,
            reason:
                dear_imgui_bevy::ImguiContextRemovalPendingReason::ViewportWorldReleasePending,
        }) if context_id == primary_id
    ));

    app.update();
    assert!(app.world().get_entity(primary).is_ok());
    assert!(app.world().get_entity(window).is_err());
    assert!(app.world().get_entity(camera).is_err());

    let _context = remove_primary_context(&mut app)
        .expect("Context removal should finish after secondary entity cleanup");
}

#[cfg(all(feature = "multi-viewport", feature = "render"))]
#[test]
fn viewport_and_render_release_converge_without_pausing_another_context() {
    let _guard = imgui_context_guard();
    let mut app = App::new();
    app.insert_resource(
        crate::viewport::native_window::DesktopPositionSupportOverride(
            crate::viewport::native_window::DesktopPositionSupport::Available,
        ),
    );
    app.add_plugins(ExtractPlugin::default())
        .add_systems(SecondaryViewportPass, || {})
        .add_plugins(ImguiPlugin::new(
            ImguiPluginConfig::default().with_multi_viewport(true),
        ));
    app.sub_app_mut(RenderApp).update_schedule = Some(Render.intern());
    ensure_primary_window(&mut app);

    let context_a = primary_context_id(&app);
    let context_b = app
        .world_mut()
        .get_non_send_mut::<ImguiContexts>()
        .unwrap()
        .create(ImguiContextConfig::new(SecondaryViewportPass).with_multi_viewport(true))
        .unwrap();
    app.world_mut()
        .get_non_send_mut::<ImguiContexts>()
        .unwrap()
        .configure(context_b, |context| {
            assert!(context.font_atlas().build());
        })
        .unwrap();
    let context_b_host = app.world_mut().spawn(Window::default()).id();
    app.world_mut().spawn(ImguiInputRoute::logical(
        context_b,
        context_b_host,
        Rect::from_corners(Vec2::ZERO, Vec2::splat(512.0)),
    ));

    let (viewport_id, viewport_window, viewport_camera) = spawn_secondary_viewport(&mut app);
    let context_b_frame = app
        .world()
        .get_non_send::<ImguiContexts>()
        .unwrap()
        .frame_index(context_b)
        .unwrap();

    app.sub_app_mut(RenderApp).update_schedule = None;
    assert!(matches!(
        remove_primary_context(&mut app),
        Err(ImguiContextError::RemovalPending {
            context_id,
            reason:
                dear_imgui_bevy::ImguiContextRemovalPendingReason::ViewportWorldReleasePending,
        }) if context_id == context_a
    ));

    app.update();
    assert!(app.world().get_entity(viewport_window).is_err());
    assert!(app.world().get_entity(viewport_camera).is_err());
    assert_eq!(
        app.world()
            .get_non_send::<ImguiContexts>()
            .unwrap()
            .frame_index(context_b)
            .unwrap(),
        context_b_frame + 1,
        "Context B must keep framing while Context A drains its viewport world"
    );

    assert!(matches!(
        remove_primary_context(&mut app),
        Err(ImguiContextError::RemovalPending {
            context_id,
            reason:
                dear_imgui_bevy::ImguiContextRemovalPendingReason::RenderWorldReleasePending,
        }) if context_id == context_a
    ));
    assert!(
        app.world()
            .get_non_send::<ImguiViewportBridge>()
            .unwrap()
            .viewport_window(context_a, viewport_id)
            .is_none(),
        "viewport ECS release must complete independently of render-world acknowledgement"
    );

    app.sub_app_mut(RenderApp).update_schedule = Some(Render.intern());
    app.update();
    assert_eq!(
        app.world()
            .get_non_send::<ImguiContexts>()
            .unwrap()
            .frame_index(context_b)
            .unwrap(),
        context_b_frame + 2,
        "Context B must keep framing while Context A waits for renderer acknowledgement"
    );

    let removed = remove_primary_context(&mut app)
        .expect("viewport drain and renderer acknowledgement must converge");
    assert_eq!(removed.id(), context_a);
    assert!(
        app.world()
            .get_non_send::<ImguiViewportBridge>()
            .unwrap()
            .callback_error_for(context_b)
            .is_none(),
        "Context A teardown must preserve Context B's platform bridge state"
    );
}

#[cfg(all(feature = "multi-viewport", feature = "render"))]
#[test]
fn registry_drop_drains_native_viewports_while_renderer_release_is_pending() {
    let _guard = imgui_context_guard();
    let mut app = App::new();
    app.insert_resource(
        crate::viewport::native_window::DesktopPositionSupportOverride(
            crate::viewport::native_window::DesktopPositionSupport::Available,
        ),
    );
    app.add_plugins(ExtractPlugin::default())
        .add_message::<WindowCloseRequested>()
        .add_plugins(ImguiPlugin::new(
            ImguiPluginConfig::default().with_multi_viewport(true),
        ));
    app.sub_app_mut(RenderApp).update_schedule = Some(Render.intern());
    ensure_primary_window(&mut app);

    let context_id = primary_context_id(&app);
    let destroyed = Rc::new(Cell::new(false));
    app.world_mut()
        .get_non_send_mut::<ImguiContexts>()
        .expect("plugin should install the Context registry")
        .configure(context_id, |context| {
            context
                .register_attachment::<RetirementDropProbeMarker>(
                    imgui::ContextAttachmentRole::Extension,
                    Rc::new(RetirementDropProbe {
                        destroyed: Rc::clone(&destroyed),
                    }),
                )
                .expect("the retirement probe should attach")
                .defer_to_context();
        })
        .expect("the primary Context should remain configurable");

    let (viewport_id, viewport_window, viewport_camera) = spawn_secondary_viewport(&mut app);
    app.world_mut().run_schedule(Main);
    assert_eq!(
        app.world().resource::<ImguiFrameMailbox>().len(),
        1,
        "the tombstone fixture must stage a snapshot that has not reached extraction"
    );

    app.sub_app_mut(RenderApp).update_schedule = None;
    let contexts = app
        .world_mut()
        .remove_non_send::<ImguiContexts>()
        .expect("plugin should install the Context registry");
    drop(contexts);
    assert!(!destroyed.get());

    app.update();
    assert!(
        !destroyed.get(),
        "the Context must remain alive while renderer release is unacknowledged"
    );
    assert!(
        app.world().get_entity(viewport_window).is_err(),
        "viewport ECS release must not wait for the render world"
    );
    assert!(
        app.world().get_entity(viewport_camera).is_err(),
        "the viewport camera must retire with its window"
    );
    assert_eq!(
        app.world()
            .get_non_send::<ImguiViewportBridge>()
            .expect("the global viewport bridge should outlive the registry")
            .viewport_window(context_id, viewport_id),
        None,
        "the drained viewport mapping must be removed while renderer release remains pending"
    );

    app.sub_app_mut(RenderApp).update_schedule = Some(Render.intern());
    app.update();
    assert!(
        !destroyed.get(),
        "renderer acknowledgement becomes observable to main-world retirement on the next main schedule"
    );
    app.world_mut().run_schedule(Main);
    assert!(
        destroyed.get(),
        "Context destruction must happen only after renderer and viewport release"
    );
    assert!(
        app.world()
            .get_non_send::<ImguiViewportBridge>()
            .expect("the global viewport bridge should remain installed")
            .viewport_window(context_id, viewport_id)
            .is_none()
    );
}

#[cfg(all(feature = "multi-viewport", feature = "render"))]
#[test]
fn context_extraction_clears_owned_platform_monitors() {
    let _guard = imgui_context_guard();
    let mut app = app_with_multi_viewport_bridge();
    ensure_primary_window(&mut app);
    app.update();
    with_primary_context(&mut app, |context| {
        let monitors = unsafe { (*context.platform_io().as_raw()).Monitors };
        assert!(monitors.Size > 0);
        assert!(!monitors.Data.is_null());
    });

    let primary_id = primary_context_id(&app);
    assert!(matches!(
        remove_primary_context(&mut app),
        Err(ImguiContextError::RemovalPending {
            context_id,
            reason: dear_imgui_bevy::ImguiContextRemovalPendingReason::RenderWorldReleasePending,
        }) if context_id == primary_id
    ));
    app.update();

    let mut context = remove_primary_context(&mut app)
        .expect("renderer acknowledgement should finish Context removal");
    context
        .try_with_active(|context| {
            let monitors = unsafe { (*context.platform_io().as_raw()).Monitors };
            assert_eq!(monitors.Size, 0);
            assert_eq!(monitors.Capacity, 0);
            assert!(monitors.Data.is_null());
            Ok::<_, ()>(())
        })
        .unwrap();
}

#[cfg(all(feature = "multi-viewport", feature = "render"))]
#[test]
fn context_extraction_preserves_replaced_platform_monitors() {
    let _guard = imgui_context_guard();
    let mut app = app_with_multi_viewport_bridge();
    ensure_primary_window(&mut app);
    app.update();

    let foreign_monitor = foreign_platform_monitor();
    let (foreign_storage, platform_io) = with_primary_context(&mut app, |context| unsafe {
        context.platform_io_mut().set_monitors(&[foreign_monitor]);
        let platform_io = context.platform_io().as_raw();
        ((*platform_io).Monitors, platform_io)
    });

    let primary_id = primary_context_id(&app);
    assert!(matches!(
        remove_primary_context(&mut app),
        Err(ImguiContextError::RemovalPending {
            context_id,
            reason:
                dear_imgui_bevy::ImguiContextRemovalPendingReason::ViewportCallbackOwnership(
                    dear_imgui_bevy::viewport::ImguiViewportCallbackOwnershipError::
                        PlatformMonitorsReplaced,
                ),
        }) if context_id == primary_id
    ));
    let preserved = unsafe { (*platform_io).Monitors };
    assert_eq!(preserved, foreign_storage);
    assert_eq!(unsafe { *preserved.Data }, foreign_monitor);

    app.update();
    let mut context = remove_primary_context(&mut app)
        .expect("monitor drift should remain retryable after renderer acknowledgement");
    context
        .try_with_active(|context| {
            let preserved = unsafe { (*context.platform_io().as_raw()).Monitors };
            assert_eq!(preserved, foreign_storage);
            assert_eq!(unsafe { *preserved.Data }, foreign_monitor);
            unsafe { context.platform_io_mut().set_monitors(&[]) };
            Ok::<_, ()>(())
        })
        .unwrap();
}

#[cfg(feature = "multi-viewport")]
#[test]
fn frame_preparation_rejects_replaced_platform_monitors_without_overwriting_them() {
    let _guard = imgui_context_guard();
    let mut app = app_with_multi_viewport_bridge();
    ensure_primary_window(&mut app);
    app.update();

    let foreign_monitor = foreign_platform_monitor();
    let expected = with_primary_context(&mut app, |context| unsafe {
        context.platform_io_mut().set_monitors(&[foreign_monitor]);
        (*context.platform_io().as_raw()).Monitors
    });
    app.update();
    let primary_id = primary_context_id(&app);
    assert!(matches!(
        app.world()
            .get_non_send::<ImguiContexts>()
            .unwrap()
            .last_error(primary_id),
        Ok(Some(ImguiContextError::ViewportBridge {
            source:
                dear_imgui_bevy::viewport::ImguiViewportRuntimeError::CallbackOwnership(
                    dear_imgui_bevy::viewport::ImguiViewportCallbackOwnershipError::
                        PlatformMonitorsReplaced,
                ),
            ..
        }))
    ));

    let actual = with_primary_context(&mut app, |context| unsafe {
        (*context.platform_io().as_raw()).Monitors
    });
    assert_eq!(actual, expected);
    assert_eq!(unsafe { *actual.Data }, foreign_monitor);
    assert_eq!(
        app.world()
            .get_non_send::<ImguiViewportBridge>()
            .unwrap()
            .callback_error_for(primary_context_id(&app)),
        Some(
            dear_imgui_bevy::viewport::ImguiViewportRuntimeError::CallbackOwnership(
                dear_imgui_bevy::viewport::ImguiViewportCallbackOwnershipError::
                    PlatformMonitorsReplaced,
            )
        )
    );
}

#[cfg(feature = "multi-viewport")]
#[test]
fn frame_preparation_rejects_in_place_platform_monitor_tampering() {
    let _guard = imgui_context_guard();
    let mut app = app_with_multi_viewport_bridge();
    ensure_primary_window(&mut app);
    app.update();

    let (data, tampered_scale) = with_primary_context(&mut app, |context| {
        let monitors = unsafe { (*context.platform_io().as_raw()).Monitors };
        assert!(monitors.Size > 0);
        let tampered_scale = unsafe { (*monitors.Data).DpiScale + 0.5 };
        unsafe { (*monitors.Data).DpiScale = tampered_scale };
        (monitors.Data, tampered_scale)
    });
    app.update();
    let primary_id = primary_context_id(&app);
    assert!(matches!(
        app.world()
            .get_non_send::<ImguiContexts>()
            .unwrap()
            .last_error(primary_id),
        Ok(Some(ImguiContextError::ViewportBridge {
            source:
                dear_imgui_bevy::viewport::ImguiViewportRuntimeError::CallbackOwnership(
                    dear_imgui_bevy::viewport::ImguiViewportCallbackOwnershipError::
                        PlatformMonitorsReplaced,
                ),
            ..
        }))
    ));

    let monitors = with_primary_context(&mut app, |context| unsafe {
        (*context.platform_io().as_raw()).Monitors
    });
    assert_eq!(monitors.Data, data);
    assert_eq!(unsafe { (*monitors.Data).DpiScale }, tampered_scale);
    assert_eq!(
        app.world()
            .get_non_send::<ImguiViewportBridge>()
            .unwrap()
            .callback_error_for(primary_context_id(&app)),
        Some(
            dear_imgui_bevy::viewport::ImguiViewportRuntimeError::CallbackOwnership(
                dear_imgui_bevy::viewport::ImguiViewportCallbackOwnershipError::
                    PlatformMonitorsReplaced,
            )
        )
    );
}

#[cfg(all(feature = "multi-viewport", feature = "render"))]
#[test]
fn callback_drift_is_reported_before_world_release_pending() {
    unsafe extern "C" fn foreign_destroy(_viewport: *mut sys::ImGuiViewport) {}

    let _guard = imgui_context_guard();
    let mut app = app_with_multi_viewport_bridge();
    let primary = ensure_primary_window(&mut app);
    let (_, window, camera) = spawn_secondary_viewport(&mut app);
    with_primary_context(&mut app, |context| unsafe {
        context
            .platform_io_mut()
            .set_platform_destroy_window_raw(Some(foreign_destroy));
    });

    let primary_id = primary_context_id(&app);
    assert!(matches!(
        remove_primary_context(&mut app),
        Err(ImguiContextError::RemovalPending {
            context_id,
            reason:
                dear_imgui_bevy::ImguiContextRemovalPendingReason::ViewportCallbackOwnership(
                    dear_imgui_bevy::viewport::ImguiViewportCallbackOwnershipError::
                        PlatformCallbackReplaced {
                            slot: "Platform_DestroyWindow",
                        },
                ),
        }) if context_id == primary_id
    ));
    assert!(matches!(
        remove_primary_context(&mut app),
        Err(ImguiContextError::RemovalPending {
            context_id,
            reason:
                dear_imgui_bevy::ImguiContextRemovalPendingReason::ViewportWorldReleasePending,
        }) if context_id == primary_id
    ));

    app.update();
    assert!(app.world().get_entity(primary).is_ok());
    assert!(app.world().get_entity(window).is_err());
    assert!(app.world().get_entity(camera).is_err());

    let mut context = remove_primary_context(&mut app)
        .expect("World cleanup should make callback-drift removal retryable");
    context
        .try_with_active(|context| {
            unsafe {
                context
                    .platform_io_mut()
                    .set_platform_destroy_window_raw(None);
            }
            Ok::<_, ()>(())
        })
        .unwrap();
}

#[cfg(all(feature = "multi-viewport", feature = "render"))]
#[test]
fn invalid_window_config_is_rejected_before_backend_attachment() {
    let _guard = imgui_context_guard();
    let invalid_window = ImguiViewportWindowConfig {
        transparent: true,
        composite_alpha_mode: CompositeAlphaMode::Opaque,
        ..Default::default()
    };
    let mut app = App::new();
    let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        app.add_plugins(ImguiPlugin::new(
            ImguiPluginConfig::default()
                .with_multi_viewport(true)
                .with_viewport_window(invalid_window),
        ));
    }))
    .expect_err("an invalid native viewport policy must fail during plugin installation");
    let message = failure
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| failure.downcast_ref::<&str>().copied())
        .unwrap_or_default();
    assert!(message.contains("invalid Dear ImGui viewport window policy"));
    assert!(
        app.world().get_non_send::<ImguiContexts>().is_none(),
        "configuration validation must run before the plugin installs backend state"
    );
}

#[cfg(all(feature = "multi-viewport", feature = "render"))]
#[test]
fn callback_userdata_drift_fails_before_platform_window_update() {
    let _guard = imgui_context_guard();
    let mut app = App::new();
    app.add_plugins(ExtractPlugin::default());
    app.sub_app_mut(RenderApp).update_schedule = Some(Render.intern());
    app.add_plugins(ImguiPlugin::new(
        ImguiPluginConfig::default().with_multi_viewport(true),
    ));
    ensure_primary_window(&mut app);
    with_primary_context(&mut app, |context| unsafe {
        context
            .io_mut()
            .set_backend_platform_user_data(std::ptr::null_mut());
    });

    app.update();
    let primary_id = primary_context_id(&app);
    assert!(matches!(
        app.world()
            .get_non_send::<ImguiContexts>()
            .unwrap()
            .last_error(primary_id),
        Ok(Some(ImguiContextError::ViewportBridge {
            source:
                dear_imgui_bevy::viewport::ImguiViewportRuntimeError::CallbackOwnership(
                    dear_imgui_bevy::viewport::ImguiViewportCallbackOwnershipError::
                        BackendPlatformUserDataReplaced,
                ),
            ..
        }))
    ));
}

#[cfg(feature = "multi-viewport")]
#[test]
fn transparent_viewport_config_rejects_non_alpha_compositor_modes() {
    for composite_alpha_mode in [
        CompositeAlphaMode::Auto,
        CompositeAlphaMode::Opaque,
        CompositeAlphaMode::Inherit,
    ] {
        let config = ImguiViewportWindowConfig {
            composite_alpha_mode,
            transparent: true,
            ..Default::default()
        };

        assert_eq!(
            config.validate(),
            Err(
                dear_imgui_bevy::viewport::ImguiViewportWindowConfigError::
                    TransparentCompositeAlphaModeUnsupported {
                        composite_alpha_mode,
                    },
            )
        );
    }

    for composite_alpha_mode in [
        CompositeAlphaMode::PreMultiplied,
        CompositeAlphaMode::PostMultiplied,
    ] {
        assert!(
            ImguiViewportWindowConfig {
                composite_alpha_mode,
                transparent: true,
                ..Default::default()
            }
            .validate()
            .is_ok()
        );
    }
}

#[cfg(feature = "multi-viewport")]
#[test]
fn viewport_platform_feedback_queries_return_mapped_bevy_window_state() {
    let _guard = imgui_context_guard();
    let mut app = app_with_multi_viewport_bridge();
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    with_primary_context(&mut app, |context| {
        let _ = context.font_atlas().build();
    });

    let context_id = primary_context_id(&app);
    let (id, entity) = create_live_secondary_viewport(&mut app);
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
    let synthetic_feedback = crate::viewport::viewport_feedback_from_window(
        entity,
        app.world()
            .get::<Window>(entity)
            .expect("the synthetic viewport Window must remain live"),
        None,
    );
    app.world()
        .non_send::<ImguiViewportBridge>()
        .set_viewport_feedback_for_test(context_id, id, synthetic_feedback);
    app.update();

    let feedback = app
        .world()
        .get_non_send::<ImguiViewportBridge>()
        .expect("bridge should still exist")
        .viewport_feedback(context_id, id)
        .expect("feedback sync should cache secondary window state");
    assert_eq!(
        feedback,
        ImguiViewportFeedback {
            #[cfg(not(target_os = "macos"))]
            pos: [150.0, 225.0],
            #[cfg(target_os = "macos")]
            pos: [100.0, 150.0],
            #[cfg(not(target_os = "macos"))]
            size: [450.0, 270.0],
            #[cfg(target_os = "macos")]
            size: [300.0, 180.0],
            #[cfg(not(target_os = "macos"))]
            framebuffer_scale: [1.0, 1.0],
            #[cfg(target_os = "macos")]
            framebuffer_scale: [1.5, 1.5],
            dpi_scale: 1.5,
            focused: false,
            minimized: false,
        }
    );

    let raw_viewport = resolve_live_viewport(&mut app, id);

    let (
        has_window_pos,
        has_window_size,
        has_window_framebuffer_scale,
        dpi_scale,
        focused,
        minimized,
    ) = with_primary_context(&mut app, |context| {
        let platform_io = context.platform_io().as_raw();
        unsafe {
            (
                (*platform_io).Platform_GetWindowPos.is_some(),
                (*platform_io).Platform_GetWindowSize.is_some(),
                (*platform_io).Platform_GetWindowFramebufferScale.is_some(),
                (*platform_io)
                    .Platform_GetWindowDpiScale
                    .expect("bridge should install Platform_GetWindowDpiScale")(
                    raw_viewport
                ),
                (*platform_io)
                    .Platform_GetWindowFocus
                    .expect("bridge should install Platform_GetWindowFocus")(
                    raw_viewport
                ),
                (*platform_io)
                    .Platform_GetWindowMinimized
                    .expect("bridge should install Platform_GetWindowMinimized")(
                    raw_viewport
                ),
            )
        }
    });

    assert!(has_window_pos);
    assert!(has_window_size);
    assert!(has_window_framebuffer_scale);
    // The ImVec2 getters are installed through the out-parameter shim in `dear-imgui-rs`.
    // Calling the raw aggregate-return callback directly from Rust re-enters the MSVC ABI edge
    // the shim exists to avoid, so this test verifies the cached aggregate feedback above and
    // directly invokes only scalar callbacks while the owning Context is current.
    assert_eq!(dpi_scale, 1.5);
    assert!(!focused);
    assert!(!minimized);
    destroy_live_secondary_viewport(&mut app, id);
}

#[cfg(feature = "multi-viewport")]
#[test]
fn viewport_secondary_window_close_requests_imgui_platform_close() {
    let _guard = imgui_context_guard();
    let mut app = app_with_multi_viewport_bridge();
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    let (id, entity) = create_live_secondary_viewport(&mut app);

    app.world_mut()
        .resource_mut::<Messages<WindowCloseRequested>>()
        .write(WindowCloseRequested { window: entity });

    app.world_mut().run_schedule(bevy_app::PreUpdate);

    let raw_viewport = resolve_live_viewport(&mut app, id);
    unsafe {
        assert!(
            (*raw_viewport).PlatformRequestClose,
            "closing a detached Bevy window must ask Dear ImGui to close the matching platform viewport"
        );
    }
    destroy_live_secondary_viewport(&mut app, id);
}

#[cfg(feature = "multi-viewport")]
#[test]
fn viewport_occlusion_events_update_imgui_minimized_feedback() {
    let _guard = imgui_context_guard();
    let mut app = app_with_multi_viewport_bridge();
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    let (id, entity) = create_live_secondary_viewport(&mut app);

    app.world_mut()
        .resource_mut::<Messages<WindowOccluded>>()
        .write(WindowOccluded {
            window: entity,
            occluded: true,
        });

    app.world_mut().run_schedule(bevy_app::PreUpdate);

    let raw_viewport = resolve_live_viewport(&mut app, id);
    let minimized = with_primary_context(&mut app, |context| {
        let platform_io = context.platform_io().as_raw();
        unsafe {
            (*platform_io)
                .Platform_GetWindowMinimized
                .expect("bridge should install Platform_GetWindowMinimized")(
                raw_viewport
            )
        }
    });
    assert!(
        minimized,
        "occluded detached windows should be reported as minimized to Dear ImGui"
    );

    app.world_mut()
        .resource_mut::<Messages<WindowOccluded>>()
        .write(WindowOccluded {
            window: entity,
            occluded: false,
        });

    app.world_mut().run_schedule(bevy_app::PreUpdate);

    let raw_viewport = resolve_live_viewport(&mut app, id);
    let minimized = with_primary_context(&mut app, |context| {
        let platform_io = context.platform_io().as_raw();
        unsafe {
            (*platform_io)
                .Platform_GetWindowMinimized
                .expect("bridge should install Platform_GetWindowMinimized")(
                raw_viewport
            )
        }
    });
    assert!(
        !minimized,
        "unoccluded detached windows should clear minimized feedback"
    );
    destroy_live_secondary_viewport(&mut app, id);
}

#[cfg(feature = "multi-viewport")]
#[test]
fn direct_context_platform_teardown_preserves_the_bevy_viewport_bridge() {
    let _guard = imgui_context_guard();
    let mut app = app_with_multi_viewport_bridge();
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    let context_id = primary_context_id(&app);
    let (id, entity) = create_live_secondary_viewport(&mut app);

    with_primary_context(&mut app, |context| {
        context
            .destroy_platform_windows()
            .expect("the Bevy bridge should authorize explicit Context platform teardown");
    });
    let raw_viewport = resolve_live_viewport(&mut app, id);
    unsafe {
        assert!((*raw_viewport).PlatformUserData.is_null());
        assert!((*raw_viewport).PlatformHandle.is_null());
        assert!((*raw_viewport).PlatformHandleRaw.is_null());
    }
    assert!(
        app.world()
            .get_non_send::<ImguiViewportBridge>()
            .expect("bridge should still exist")
            .viewport_window(context_id, id)
            .is_some(),
        "the bridge mapping remains visible until the private driver applies deferred platform work"
    );

    app.update();

    assert!(
        app.world().get_entity(entity).is_err(),
        "explicit Context teardown must despawn the secondary Bevy window"
    );
    assert_platform_window_update_finished(&mut app);
    let rebuilt_entity = app
        .world()
        .get_non_send::<ImguiViewportBridge>()
        .expect("bridge should still exist")
        .viewport_window(context_id, id)
        .expect("the private driver must rebuild the viewport mapping on the next frame");
    assert_ne!(
        rebuilt_entity, entity,
        "the rebuilt viewport mapping must not reuse the entity destroyed by explicit teardown"
    );
    let rebuilt_viewport = resolve_live_viewport(&mut app, id);
    unsafe {
        assert!(!(*rebuilt_viewport).PlatformUserData.is_null());
        assert_eq!(
            (*rebuilt_viewport).PlatformUserData,
            (*rebuilt_viewport).PlatformHandle,
            "the delayed destroy command must not release the rebuilt viewport's handle"
        );
    }
    destroy_live_secondary_viewport(&mut app, id);
}

#[cfg(all(feature = "multi-viewport", feature = "render"))]
#[test]
fn viewport_commands_spawn_and_destroy_secondary_overlay_camera() {
    let _guard = imgui_context_guard();
    let mut app = app_with_multi_viewport_bridge();
    let context_id = primary_context_id(&app);
    let (id, window_entity, camera_entity) = spawn_secondary_viewport(&mut app);

    let camera_marker = app
        .world()
        .get::<ImguiViewportCamera>(camera_entity)
        .expect("secondary camera should carry the viewport marker");
    assert_eq!(camera_marker.context_id(), context_id);
    assert_eq!(camera_marker.viewport_id(), id);
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

    destroy_live_secondary_viewport(&mut app, id);

    let bridge = app
        .world()
        .get_non_send::<ImguiViewportBridge>()
        .expect("bridge should still exist");
    assert!(bridge.viewport_window(context_id, id).is_none());
    assert!(bridge.viewport_camera(context_id, id).is_none());
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
fn viewport_update_synchronizes_existing_camera_clear_policy() {
    let _guard = imgui_context_guard();
    let mut app = app_with_multi_viewport_bridge();
    ensure_primary_window(&mut app);
    let context_id = primary_context_id(&app);
    let snapshot = viewport_snapshot(0x251);
    let raw_viewport = create_callback_viewport(&mut app, context_id, &snapshot);
    app.update();
    let camera = app
        .world()
        .get_non_send::<ImguiViewportBridge>()
        .and_then(|bridge| bridge.viewport_camera(context_id, snapshot.id))
        .expect("the callback-created viewport should own a camera");

    with_primary_context(&mut app, |context| unsafe {
        imgui::Viewport::from_raw_mut(raw_viewport).set_raw_flags_unchecked(
            (imgui::ViewportFlags::IS_PLATFORM_WINDOW | imgui::ViewportFlags::NO_RENDERER_CLEAR)
                .bits(),
        );
        (*context.platform_io().as_raw())
            .Platform_UpdateWindow
            .expect("the Context should own Platform_UpdateWindow")(raw_viewport);
    });
    app.update();
    assert!(matches!(
        app.world()
            .get::<Camera>(camera)
            .expect("the viewport camera should remain live")
            .output_mode,
        CameraOutputMode::Write {
            clear_color: ClearColorConfig::None,
            ..
        }
    ));

    with_primary_context(&mut app, |context| unsafe {
        imgui::Viewport::from_raw_mut(raw_viewport)
            .set_raw_flags_unchecked(imgui::ViewportFlags::IS_PLATFORM_WINDOW.bits());
        (*context.platform_io().as_raw())
            .Platform_UpdateWindow
            .expect("the Context should own Platform_UpdateWindow")(raw_viewport);
    });
    app.update();
    assert!(matches!(
        app.world()
            .get::<Camera>(camera)
            .expect("the viewport camera should remain live")
            .output_mode,
        CameraOutputMode::Write {
            clear_color: ClearColorConfig::Default,
            ..
        }
    ));

    destroy_callback_viewport(&mut app, context_id, raw_viewport);
    app.update();
}

#[cfg(all(feature = "multi-viewport", feature = "render"))]
#[test]
fn viewport_camera_mapping_recovers_after_external_despawn() {
    let _guard = imgui_context_guard();
    let mut app = app_with_multi_viewport_bridge();
    let context_id = primary_context_id(&app);
    let (id, window_entity, original_camera) = spawn_secondary_viewport(&mut app);
    app.world_mut().despawn(original_camera);

    app.update();

    let replacement_camera = app
        .world()
        .get_non_send::<ImguiViewportBridge>()
        .and_then(|bridge| bridge.viewport_camera(context_id, id))
        .expect("a live viewport window should recover its missing overlay camera");
    assert_ne!(replacement_camera, original_camera);
    assert!(app.world().get_entity(replacement_camera).is_ok());
    assert!(matches!(
        app.world().get::<RenderTarget>(replacement_camera),
        Some(RenderTarget::Window(WindowRef::Entity(entity))) if *entity == window_entity
    ));
}

#[cfg(all(feature = "multi-viewport", feature = "render"))]
#[test]
fn live_viewport_recovers_window_camera_and_handle_after_external_window_despawn() {
    let _guard = imgui_context_guard();
    let mut app = app_with_multi_viewport_bridge();
    let context_id = primary_context_id(&app);
    let (id, original_window, original_camera) = spawn_secondary_viewport(&mut app);
    let raw_viewport = resolve_live_viewport(&mut app, id);
    let original_handle = unsafe {
        assert!((*raw_viewport).PlatformWindowCreated);
        assert!(!(*raw_viewport).PlatformHandle.is_null());
        assert_eq!(
            (*raw_viewport).PlatformUserData,
            (*raw_viewport).PlatformHandle
        );
        (*raw_viewport).PlatformHandle
    };

    app.world_mut().despawn(original_window);
    // The first frame detects the missing ECS window and reissues the platform create callback.
    // The second frame proves that the rebuilt mapping remains stable across frame boundaries.
    for _ in 0..2 {
        app.update();
    }

    let bridge = app
        .world()
        .get_non_send::<ImguiViewportBridge>()
        .expect("bridge should still exist");
    let replacement_window = bridge
        .viewport_window(context_id, id)
        .expect("the live viewport should rebuild its externally despawned Bevy window");
    let replacement_camera = bridge
        .viewport_camera(context_id, id)
        .expect("the rebuilt viewport window should own a replacement overlay camera");
    assert_ne!(replacement_window, original_window);
    assert_ne!(replacement_camera, original_camera);
    assert!(app.world().get::<Window>(replacement_window).is_some());
    assert!(matches!(
        app.world().get::<RenderTarget>(replacement_camera),
        Some(RenderTarget::Window(WindowRef::Entity(entity))) if *entity == replacement_window
    ));
    assert!(
        app.world().get_entity(original_camera).is_err(),
        "the camera targeting the externally despawned window must be retired"
    );

    let rebuilt_viewport = resolve_live_viewport(&mut app, id);
    assert_eq!(
        rebuilt_viewport, raw_viewport,
        "recovery must preserve the live Dear ImGui viewport identity"
    );
    unsafe {
        assert!((*rebuilt_viewport).PlatformWindowCreated);
        assert!(!(*rebuilt_viewport).PlatformHandle.is_null());
        assert_eq!(
            (*rebuilt_viewport).PlatformUserData,
            (*rebuilt_viewport).PlatformHandle
        );
        assert_eq!(
            (*rebuilt_viewport).PlatformHandle,
            original_handle,
            "the stable bridge handle allocation should be rebound to the rebuilt Bevy window"
        );
    }
}

#[cfg(all(feature = "multi-viewport", feature = "render"))]
#[test]
fn viewport_camera_contract_recovers_missing_required_components() {
    let _guard = imgui_context_guard();
    let mut app = app_with_multi_viewport_bridge();
    let context_id = primary_context_id(&app);
    let (id, window_entity, camera) = spawn_secondary_viewport(&mut app);

    app.world_mut().entity_mut(camera).remove::<Camera>();
    app.update();
    assert!(
        app.world().get::<Camera>(camera).is_some(),
        "the backend must restore a removed Camera component"
    );

    app.world_mut().entity_mut(camera).remove::<Camera2d>();
    app.update();
    assert!(
        app.world().get::<Camera2d>(camera).is_some(),
        "the backend must restore the viewport camera's 2D graph marker"
    );

    app.world_mut().entity_mut(camera).remove::<RenderTarget>();
    app.update();
    assert!(matches!(
        app.world().get::<RenderTarget>(camera),
        Some(RenderTarget::Window(WindowRef::Entity(entity))) if *entity == window_entity
    ));

    app.world_mut()
        .entity_mut(camera)
        .remove::<CameraRenderGraph>();
    app.update();
    assert!(
        app.world().get::<CameraRenderGraph>(camera).is_some(),
        "the backend must restore a removed render graph"
    );

    app.world_mut().entity_mut(camera).remove::<RenderLayers>();
    app.update();
    assert_eq!(
        app.world()
            .get::<RenderLayers>(camera)
            .expect("the backend must restore explicit render-layer isolation"),
        &RenderLayers::none()
    );

    app.world_mut()
        .entity_mut(camera)
        .remove::<ImguiViewportCamera>();
    app.update();
    let marker = app
        .world()
        .get::<ImguiViewportCamera>(camera)
        .expect("the backend must restore the public viewport identity marker");
    assert_eq!(marker.context_id(), context_id);
    assert_eq!(marker.viewport_id(), id);
    assert_eq!(
        app.world()
            .get_non_send::<ImguiViewportBridge>()
            .and_then(|bridge| bridge.viewport_camera(context_id, id)),
        Some(camera),
        "component recovery should preserve the privately owned camera identity"
    );
}

#[cfg(all(feature = "multi-viewport", feature = "render"))]
#[test]
fn viewport_orphaned_secondary_overlay_camera_is_despawned_after_dock_back() {
    let _guard = imgui_context_guard();
    let mut app = app_with_multi_viewport_bridge();
    app.world_mut().spawn((Window::default(), PrimaryWindow));

    let context_id = primary_context_id(&app);
    let (id, window_entity, camera_entity) = spawn_secondary_viewport(&mut app);

    app.world_mut()
        .resource_mut::<SubmitLiveSecondaryViewport>()
        .0 = false;
    app.world_mut().despawn(window_entity);
    // Dear ImGui retires an inactive platform viewport after two inactive frames.
    for _ in 0..2 {
        app.update();
    }

    let bridge = app
        .world()
        .get_non_send::<ImguiViewportBridge>()
        .expect("bridge should still exist");
    assert!(bridge.viewport_window(context_id, id).is_none());
    assert!(bridge.viewport_camera(context_id, id).is_none());
    assert!(
        app.world().get_entity(camera_entity).is_err(),
        "when a detached viewport is merged back and its Bevy window disappears, the secondary overlay camera must not keep intercepting or rendering"
    );
}

#[cfg(all(feature = "multi-viewport", feature = "render"))]
#[test]
fn viewport_primary_close_despawns_secondary_viewport_windows_and_cameras() {
    let _guard = imgui_context_guard();
    let mut app = app_with_multi_viewport_bridge();
    let primary = app
        .world_mut()
        .spawn((Window::default(), PrimaryWindow))
        .id();
    let context_id = primary_context_id(&app);
    let (id, window_entity, camera_entity) = spawn_secondary_viewport(&mut app);

    app.world_mut()
        .resource_mut::<Messages<WindowCloseRequested>>()
        .write(WindowCloseRequested { window: primary });
    app.update();

    let bridge = app
        .world()
        .get_non_send::<ImguiViewportBridge>()
        .expect("bridge should still exist");
    assert!(bridge.viewport_window(context_id, id).is_none());
    assert!(bridge.viewport_camera(context_id, id).is_none());
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
fn viewport_missing_primary_window_despawns_and_recreates_secondary_viewport_state() {
    let _guard = imgui_context_guard();
    let mut app = app_with_multi_viewport_bridge();
    let primary = app
        .world_mut()
        .spawn((Window::default(), PrimaryWindow))
        .id();
    let context_id = primary_context_id(&app);
    let (id, window_entity, camera_entity) = spawn_secondary_viewport(&mut app);

    app.world_mut().despawn(primary);
    app.update();

    let bridge = app
        .world()
        .get_non_send::<ImguiViewportBridge>()
        .expect("bridge should still exist");
    assert!(bridge.viewport_window(context_id, id).is_none());
    assert!(bridge.viewport_camera(context_id, id).is_none());
    assert!(
        app.world().get_entity(window_entity).is_err(),
        "removing the primary window should despawn detached Dear ImGui viewport windows"
    );
    assert!(
        app.world().get_entity(camera_entity).is_err(),
        "removing the primary window should despawn detached viewport cameras"
    );

    app.world_mut().spawn((Window::default(), PrimaryWindow));
    app.update();
    assert_platform_window_update_finished(&mut app);

    let bridge = app
        .world()
        .get_non_send::<ImguiViewportBridge>()
        .expect("bridge should still exist");
    let rebuilt_window = bridge
        .viewport_window(context_id, id)
        .expect("a live ImGui viewport should recreate its Bevy window after host recovery");
    let rebuilt_camera = bridge
        .viewport_camera(context_id, id)
        .expect("host recovery should recreate the matching viewport camera");
    assert!(app.world().get_entity(rebuilt_window).is_ok());
    assert!(app.world().get_entity(rebuilt_camera).is_ok());
    assert_ne!(rebuilt_window, window_entity);
    assert_ne!(rebuilt_camera, camera_entity);
}
