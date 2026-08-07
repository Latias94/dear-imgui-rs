#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[path = "support/native_viewport.rs"]
mod native_viewport;

#[cfg(feature = "render")]
use super::{OrderedPointerEvent, append_typed_pointer_event};
#[cfg(all(
    feature = "render",
    feature = "multi-viewport",
    not(target_arch = "wasm32")
))]
use super::{RawWindowPointerEvent, order_raw_pointer_events};
use crate::test_util::imgui_context_guard;
#[cfg(feature = "render")]
use bevy_app::PostUpdate;
#[cfg(feature = "render")]
use bevy_app::Update;
use bevy_app::{App, PreUpdate};
#[cfg(feature = "render")]
use bevy_asset::{Assets, RenderAssetUsages};
#[cfg(feature = "render")]
use bevy_camera::{Camera, RenderTarget, Viewport};
#[cfg(feature = "render")]
use bevy_core_pipeline::Core2d;
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::*;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use bevy_ecs::schedule::ScheduleLabel;
#[cfg(feature = "render")]
use bevy_ecs::system::RunSystemOnce;
#[cfg(feature = "render")]
use bevy_image::Image;
use bevy_input::ButtonState;
use bevy_input::keyboard::{Key as BevyKey, KeyCode, KeyboardFocusLost, KeyboardInput};
use bevy_input::mouse::{
    MouseButton as BevyMouseButton, MouseButtonInput, MouseScrollUnit, MouseWheel,
};
use bevy_input::touch::{TouchInput, TouchPhase};
#[cfg(feature = "render")]
use bevy_math::UVec2;
use bevy_math::{IVec2, Vec2};
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use bevy_render::{Render, extract_plugin::ExtractPlugin};
#[cfg(feature = "render")]
use bevy_render::{
    RenderApp,
    camera::CameraRenderGraph,
    render_resource::{Extent3d, TextureDimension, TextureFormat},
    texture::ManualTextureViews,
};
#[cfg(feature = "render")]
use bevy_window::WindowRef;
use bevy_window::{
    CursorEntered, CursorIcon, CursorLeft, CursorMoved, CursorOptions, Ime, PrimaryWindow,
    SystemCursorIcon, Window, WindowFocused, WindowPosition, WindowResized, WindowResolution,
    WindowScaleFactorChanged,
};
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use bevy_winit::{RawWinitWindowEvent, WINIT_WINDOWS};
#[cfg(feature = "render")]
use dear_imgui_bevy::ContextId;
#[cfg(not(feature = "render"))]
use dear_imgui_bevy::input::{
    imgui_primary_wants_keyboard_input, imgui_primary_wants_pointer_input,
    imgui_primary_wants_text_input,
};
use dear_imgui_bevy::{
    ImguiAppExt, ImguiContexts, ImguiFrame, ImguiPlugin, ImguiPrimaryPass,
    input::{
        ImguiInputCapture, ImguiInputCaptureState, ImguiInputState, imgui_wants_any_input,
        imgui_wants_keyboard_input, imgui_wants_pointer_input,
        imgui_wants_pointer_input_unless_popup_close, imgui_wants_text_input, map_bevy_key_code,
    },
};
#[cfg(feature = "render")]
use dear_imgui_bevy::{
    ImguiContextConfig, ImguiPass,
    input::{
        imgui_context_wants_keyboard_input, imgui_context_wants_pointer_input,
        imgui_window_wants_pointer_input,
    },
    route::{ImguiInputPolicy, ImguiInputRoute, ImguiRenderRoute},
};
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use dear_imgui_bevy::{ImguiPluginConfig, ImguiViewportBridge, ImguiViewportWindow};
use dear_imgui_rs as imgui;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use native_viewport::CallbackViewport;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use winit::{
    dpi::PhysicalPosition,
    event::{DeviceId, ElementState, MouseButton as WinitMouseButton, WindowEvent},
    window::WindowId,
};

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
struct TestWinitWindowMapping {
    window_id: WindowId,
    entity: Entity,
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
impl TestWinitWindowMapping {
    fn install(entity: Entity) -> Self {
        let window_id = WindowId::dummy();
        WINIT_WINDOWS.with_borrow_mut(|windows| {
            assert!(!windows.winit_to_entity.contains_key(&window_id));
            assert!(!windows.entity_to_winit.contains_key(&entity));
            windows.winit_to_entity.insert(window_id, entity);
            windows.entity_to_winit.insert(entity, window_id);
        });
        Self { window_id, entity }
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
impl Drop for TestWinitWindowMapping {
    fn drop(&mut self) {
        WINIT_WINDOWS.with_borrow_mut(|windows| {
            if windows.winit_to_entity.get(&self.window_id) == Some(&self.entity) {
                windows.winit_to_entity.remove(&self.window_id);
            }
            if windows.entity_to_winit.get(&self.entity) == Some(&self.window_id) {
                windows.entity_to_winit.remove(&self.entity);
            }
        });
    }
}

fn app_with_primary_window() -> (App, Entity) {
    app_with_primary_window_plugin(ImguiPlugin::default())
}

fn app_with_primary_window_plugin(plugin: ImguiPlugin) -> (App, Entity) {
    app_with_primary_window_in(App::new(), plugin)
}

fn app_with_primary_window_in(mut app: App, plugin: ImguiPlugin) -> (App, Entity) {
    app.add_plugins(plugin);

    let mut window = Window {
        resolution: WindowResolution::new(1600, 1200),
        ..Default::default()
    };
    window.resolution.set_scale_factor(2.0);

    let primary = app.world_mut().spawn((window, PrimaryWindow)).id();
    #[cfg(feature = "render")]
    {
        let primary_context = app
            .world()
            .non_send::<ImguiContexts>()
            .primary_id()
            .expect("ImguiPlugin should install a primary Context");
        let region = {
            let window = app
                .world()
                .get::<Window>(primary)
                .expect("the primary Window was just spawned");
            bevy_math::Rect::from_corners(Vec2::ZERO, Vec2::new(window.width(), window.height()))
        };
        app.world_mut()
            .spawn(ImguiInputRoute::logical(primary_context, primary, region));
        app.world_mut().run_schedule(PostUpdate);
    }
    prepare_imgui_context(&mut app);
    (app, primary)
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn app_with_primary_window_and_native_viewports() -> (App, Entity) {
    let mut app = App::new();
    app.insert_resource(
        crate::viewport::native_window::DesktopPositionSupportOverride(
            crate::viewport::native_window::DesktopPositionSupport::Available,
        ),
    );
    app.add_plugins(ExtractPlugin::default());
    app.sub_app_mut(RenderApp).update_schedule = Some(Render.intern());
    app_with_primary_window_in(
        app,
        ImguiPlugin::new(ImguiPluginConfig::default().with_multi_viewport(true)),
    )
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn create_native_viewport_window(
    app: &mut App,
    viewport_id: imgui::Id,
    window: Window,
) -> CallbackViewport {
    let context_id = primary_context_id(app);
    let fixture = CallbackViewport::create(app, context_id, viewport_id);
    let feedback = crate::viewport::viewport_feedback_from_window(fixture.window(), &window, None);
    app.world()
        .non_send::<ImguiViewportBridge>()
        .set_viewport_feedback_for_test(context_id, viewport_id, feedback);
    app.world_mut().entity_mut(fixture.window()).insert(window);
    fixture
}

fn prepare_imgui_context(app: &mut App) {
    #[cfg(feature = "render")]
    let uses_managed_renderer = app.get_sub_app(RenderApp).is_some();
    #[cfg(not(feature = "render"))]
    let uses_managed_renderer = false;
    configure_primary(app, |context| {
        context.io_mut().set_delta_time(1.0 / 60.0);
        context.io_mut().set_config_input_trickle_event_queue(false);
        if !uses_managed_renderer {
            let legacy = context
                .font_atlas()
                .try_claim_legacy_renderer()
                .expect("the headless input fixture uses a legacy font atlas");
            if !legacy.is_built() {
                legacy.build();
            }
        }
        let _ = context.set_ini_filename::<std::path::PathBuf>(None);
    });
}

fn configure_primary<T>(app: &mut App, configure: impl FnOnce(&mut imgui::Context) -> T) -> T {
    let mut contexts = app
        .world_mut()
        .get_non_send_mut::<ImguiContexts>()
        .expect("ImguiPlugin should install the Context registry");
    let primary_id = contexts
        .primary_id()
        .expect("ImguiPlugin should install a primary Context");
    contexts
        .configure(primary_id, configure)
        .unwrap_or_else(|error| panic!("primary Context should be configurable: {error}"))
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn primary_context_id(app: &App) -> ContextId {
    app.world()
        .non_send::<ImguiContexts>()
        .primary_id()
        .expect("ImguiPlugin should install a primary Context")
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn expected_hovered_viewport(viewport_id: imgui::Id) -> imgui::Id {
    if cfg!(target_os = "windows") {
        viewport_id
    } else {
        imgui::Id::from(0)
    }
}

#[cfg(feature = "render")]
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct RoutedInputSecondaryUi;

#[cfg(feature = "render")]
fn empty_routed_input_ui(_frame: ImguiFrame<'_, RoutedInputSecondaryUi>) {}

#[cfg(feature = "render")]
#[derive(Resource, Default)]
struct ScopedCaptureRunCount(u32);

#[cfg(feature = "render")]
fn count_scoped_capture(mut count: ResMut<ScopedCaptureRunCount>) {
    count.0 += 1;
}

#[cfg(feature = "render")]
fn request_pointer_capture_next_frame(frame: ImguiFrame<'_, ImguiPrimaryPass>) {
    frame.ui().set_next_frame_want_capture_mouse(true);
}

#[cfg(feature = "render")]
fn configure_context<T>(
    app: &mut App,
    context_id: ContextId,
    configure: impl FnOnce(&mut imgui::Context) -> T,
) -> T {
    let mut contexts = app
        .world_mut()
        .get_non_send_mut::<ImguiContexts>()
        .expect("ImguiPlugin should install the Context registry");
    contexts
        .configure(context_id, configure)
        .unwrap_or_else(|error| panic!("Context should be configurable: {error}"))
}

#[cfg(feature = "render")]
fn prepare_context(app: &mut App, context_id: ContextId) {
    let uses_managed_renderer = app.get_sub_app(RenderApp).is_some();
    configure_context(app, context_id, |context| {
        context.io_mut().set_delta_time(1.0 / 60.0);
        context.io_mut().set_config_input_trickle_event_queue(false);
        if !uses_managed_renderer {
            let legacy = context
                .font_atlas()
                .try_claim_legacy_renderer()
                .expect("the headless routed-input fixture uses a legacy font atlas");
            if !legacy.is_built() {
                legacy.build();
            }
        }
        let _ = context.set_ini_filename::<std::path::PathBuf>(None);
    });
}

#[cfg(feature = "render")]
fn routed_input_app() -> (App, ContextId, Entity, Entity) {
    let mut app = App::new();
    app.init_resource::<Assets<Image>>()
        .init_resource::<ManualTextureViews>()
        .add_plugins(ImguiPlugin::default());

    let primary_context = app
        .world()
        .non_send::<ImguiContexts>()
        .primary_id()
        .expect("ImguiPlugin should install a primary Context");
    let mut primary_window = Window {
        resolution: WindowResolution::new(640, 480),
        ..Default::default()
    };
    primary_window.focused = true;
    let primary_window = app.world_mut().spawn((primary_window, PrimaryWindow)).id();
    let mut secondary_window = Window {
        resolution: WindowResolution::new(640, 480),
        ..Default::default()
    };
    secondary_window.focused = false;
    let secondary_window = app.world_mut().spawn(secondary_window).id();
    prepare_context(&mut app, primary_context);
    (app, primary_context, primary_window, secondary_window)
}

#[cfg(feature = "render")]
fn add_routed_input_context(app: &mut App) -> (ContextId, ImguiPass<RoutedInputSecondaryUi>) {
    let pass = app.declare_imgui_pass::<RoutedInputSecondaryUi>();
    app.add_imgui_systems(&pass, pass.system(empty_routed_input_ui));
    let context_id = app
        .world_mut()
        .non_send_mut::<ImguiContexts>()
        .create(ImguiContextConfig::new(&pass))
        .expect("secondary Context admission must succeed");
    prepare_context(app, context_id);
    (context_id, pass)
}

#[cfg(feature = "render")]
fn logical_window_region(app: &App, window: Entity) -> bevy_math::Rect {
    let window = app
        .world()
        .get::<Window>(window)
        .expect("input host Window must exist");
    bevy_math::Rect::from_corners(Vec2::ZERO, Vec2::new(window.width(), window.height()))
}

#[cfg(feature = "render")]
fn resolve_routed_input(app: &mut App) {
    app.world_mut().run_schedule(PostUpdate);
}

#[cfg(feature = "render")]
fn run_routed_input(app: &mut App) {
    app.world_mut().run_schedule(PreUpdate);
}

#[cfg(feature = "render")]
fn begin_frame_for_context(
    app: &mut App,
    context_id: ContextId,
    assert_ui: impl FnOnce(&imgui::Ui),
) {
    configure_context(app, context_id, |context| {
        let frame = context.begin_frame();
        assert_ui(frame.ui());
        let _ = frame.render_legacy();
    });
}

fn current_frame_input_chars() -> Vec<u32> {
    unsafe {
        let io = imgui::sys::igGetIO_Nil();
        let legacy_queue = &(*io).InputQueueCharacters;
        let mut chars = Vec::new();
        if legacy_queue.Size > 0 && !legacy_queue.Data.is_null() {
            chars.extend(
                std::slice::from_raw_parts(legacy_queue.Data, legacy_queue.Size as usize)
                    .iter()
                    .copied(),
            );
        }
        chars
    }
}

fn begin_frame_and_assert(app: &mut App, assert_ui: impl FnOnce(&imgui::Ui)) {
    configure_primary(app, |context| {
        #[cfg(feature = "multi-viewport")]
        let update_platform_windows = context.io().backend_flags().contains(
            imgui::BackendFlags::PLATFORM_HAS_VIEWPORTS
                | imgui::BackendFlags::RENDERER_HAS_VIEWPORTS,
        );
        let frame = context.begin_frame();
        assert_ui(frame.ui());
        drop(frame);
        #[cfg(feature = "multi-viewport")]
        if update_platform_windows {
            context.update_platform_windows();
        }
    });
}

fn run_input_systems(app: &mut App) {
    app.world_mut().run_schedule(PreUpdate);
}

fn run_condition_value<M>(app: &mut App, system: impl IntoSystem<(), bool, M> + 'static) -> bool {
    app.world_mut().run_system_cached(system).unwrap()
}

#[cfg(feature = "render")]
fn run_condition_once<M>(app: &mut App, system: impl IntoSystem<(), bool, M>) -> bool {
    app.world_mut().run_system_once(system).unwrap()
}

fn request_text_cursor_and_ime<P: 'static>(frame: ImguiFrame<'_, P>) {
    let ui = frame.ui();
    ui.set_mouse_cursor(Some(imgui::MouseCursor::TextInput));
    ui.set_mouse_draw_cursor(false);

    ui.with_bound_context(|| {
        // SAFETY: `ImguiFrame` keeps this exact Context bound for the closure, and the test mutates
        // only its live frame's platform IME output to simulate an active text widget.
        unsafe {
            let raw_context = imgui::sys::igGetCurrentContext();
            let ime_data = &mut (*raw_context).PlatformImeData;
            ime_data.WantTextInput = true;
            ime_data.InputPos = imgui::sys::ImVec2_c { x: 222.0, y: 333.0 };
        }
    });
}

#[cfg(feature = "render")]
fn request_text_cursor_only<P: 'static>(frame: ImguiFrame<'_, P>) {
    let ui = frame.ui();
    ui.set_mouse_cursor(Some(imgui::MouseCursor::TextInput));
    ui.set_mouse_draw_cursor(false);
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn request_text_cursor_and_secondary_viewport_ime(frame: ImguiFrame<'_>) {
    let ui = frame.ui();
    ui.set_mouse_cursor(Some(imgui::MouseCursor::TextInput));
    ui.set_mouse_draw_cursor(false);

    ui.with_bound_context(|| {
        // SAFETY: `ImguiFrame` keeps this exact Context bound while this test updates its live
        // platform IME output.
        unsafe {
            let raw_context = imgui::sys::igGetCurrentContext();
            let ime_data = &mut (*raw_context).PlatformImeData;
            ime_data.WantTextInput = true;
            #[cfg(not(target_os = "macos"))]
            let input_pos = [188.0, 260.0];
            #[cfg(target_os = "macos")]
            let input_pos = [94.0, 130.0];
            ime_data.InputPos = imgui::sys::ImVec2_c {
                x: input_pos[0],
                y: input_pos[1],
            };
            ime_data.ViewportId = 0x501;
        }
    });
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn request_primary_cursor_and_secondary_viewport_ime(frame: ImguiFrame<'_>) {
    let ui = frame.ui();
    ui.set_mouse_cursor(Some(imgui::MouseCursor::TextInput));
    ui.set_mouse_draw_cursor(false);

    ui.with_bound_context(|| {
        // SAFETY: `ImguiFrame` keeps this exact Context bound while this test updates its live
        // platform IME output.
        unsafe {
            let raw_context = imgui::sys::igGetCurrentContext();
            let ime_data = &mut (*raw_context).PlatformImeData;
            ime_data.WantTextInput = true;
            ime_data.InputPos = imgui::sys::ImVec2_c { x: 77.0, y: 88.0 };
            ime_data.ViewportId = 0x502;
        }
    });
}

fn request_software_cursor(frame: ImguiFrame<'_>) {
    let ui = frame.ui();
    ui.set_mouse_cursor(Some(imgui::MouseCursor::Hand));
    ui.set_mouse_draw_cursor(true);

    ui.with_bound_context(|| {
        // SAFETY: `ImguiFrame` keeps this exact Context bound while this test clears its live
        // platform IME output to keep the assertion focused on cursor visibility.
        unsafe {
            let raw_context = imgui::sys::igGetCurrentContext();
            let ime_data = &mut (*raw_context).PlatformImeData;
            ime_data.WantTextInput = false;
            ime_data.InputPos = imgui::sys::ImVec2_c { x: 0.0, y: 0.0 };
        }
    });
}

fn request_hidden_cursor(frame: ImguiFrame<'_>) {
    let ui = frame.ui();
    ui.set_mouse_cursor(None);
    ui.set_mouse_draw_cursor(false);
}

fn key_input(
    window: Entity,
    key_code: KeyCode,
    logical_key: BevyKey,
    state: ButtonState,
    text: Option<&str>,
) -> KeyboardInput {
    KeyboardInput {
        key_code,
        logical_key,
        state,
        text: text.map(Into::into),
        repeat: false,
        window,
    }
}

#[test]
fn primary_window_input_maps_window_mouse_and_scroll_into_imgui_io() {
    let _guard = imgui_context_guard();
    let (mut app, primary) = app_with_primary_window();

    app.world_mut()
        .resource_mut::<Messages<CursorMoved>>()
        .write(CursorMoved {
            window: primary,
            position: Vec2::new(123.0, 45.0),
            delta: None,
        });
    app.world_mut()
        .resource_mut::<Messages<MouseButtonInput>>()
        .write(MouseButtonInput {
            button: BevyMouseButton::Left,
            state: ButtonState::Pressed,
            window: primary,
        });
    app.world_mut()
        .resource_mut::<Messages<MouseWheel>>()
        .write(MouseWheel {
            unit: MouseScrollUnit::Line,
            x: 1.0,
            y: -2.0,
            window: primary,
            phase: TouchPhase::Moved,
        });

    run_input_systems(&mut app);

    configure_primary(&mut app, |context| {
        assert_eq!(context.io().display_size(), [800.0, 600.0]);
        assert_eq!(context.io().display_framebuffer_scale(), [2.0, 2.0]);
    });

    begin_frame_and_assert(&mut app, |ui| {
        assert_eq!(ui.mouse_pos(), [123.0, 45.0]);
        assert!(ui.is_mouse_down(imgui::MouseButton::Left));
        assert_eq!(ui.io().mouse_source(), imgui::MouseSource::Mouse);
        assert_eq!(ui.io().mouse_wheel_h(), 1.0);
        assert_eq!(ui.io().mouse_wheel(), -2.0);
    });
}

#[test]
fn primary_window_input_does_not_self_declare_hovered_viewport_capability() {
    let _guard = imgui_context_guard();
    let (mut app, primary) = app_with_primary_window();
    configure_primary(&mut app, |context| {
        context
            .io_mut()
            .set_config_flags(imgui::ConfigFlags::VIEWPORTS_ENABLE);
    });

    app.world_mut()
        .resource_mut::<Messages<CursorMoved>>()
        .write(CursorMoved {
            window: primary,
            position: Vec2::new(123.0, 45.0),
            delta: None,
        });
    app.world_mut()
        .resource_mut::<Messages<MouseButtonInput>>()
        .write(MouseButtonInput {
            button: BevyMouseButton::Left,
            state: ButtonState::Pressed,
            window: primary,
        });
    run_input_systems(&mut app);

    begin_frame_and_assert(&mut app, |ui| {
        assert_eq!(ui.mouse_pos(), [123.0, 45.0]);
        assert_eq!(ui.io().mouse_hovered_viewport(), imgui::Id::from(0));
        assert!(
            !ui.io()
                .backend_flags()
                .contains(imgui::BackendFlags::HAS_MOUSE_HOVERED_VIEWPORT)
        );
        assert!(ui.is_mouse_down(imgui::MouseButton::Left));
    });
}

#[test]
fn input_keyboard_and_ime_messages_update_imgui_keys_modifiers_and_text_queue() {
    let _guard = imgui_context_guard();
    let (mut app, primary) = app_with_primary_window();

    app.world_mut()
        .resource_mut::<Messages<KeyboardInput>>()
        .write(key_input(
            primary,
            KeyCode::ControlLeft,
            BevyKey::Control,
            ButtonState::Pressed,
            None,
        ));
    app.world_mut()
        .resource_mut::<Messages<KeyboardInput>>()
        .write(key_input(
            primary,
            KeyCode::KeyA,
            BevyKey::Character("a".into()),
            ButtonState::Pressed,
            Some("a"),
        ));
    app.world_mut()
        .resource_mut::<Messages<Ime>>()
        .write(Ime::Commit {
            window: primary,
            value: "好!".to_owned(),
        });

    run_input_systems(&mut app);

    begin_frame_and_assert(&mut app, |ui| {
        let chars = current_frame_input_chars();
        assert!(chars.contains(&('a' as u32)));
        assert!(chars.contains(&('好' as u32)));
        assert!(chars.contains(&('!' as u32)));
        assert!(ui.is_key_down(imgui::Key::A));
    });
}

#[test]
fn input_ime_enable_commit_and_disable_preserves_explicit_ime_state() {
    let _guard = imgui_context_guard();
    let (mut app, primary) = app_with_primary_window();

    app.world_mut()
        .resource_mut::<Messages<Ime>>()
        .write(Ime::Enabled { window: primary });
    run_input_systems(&mut app);
    assert!(app.world().resource::<ImguiInputState>().ime_enabled());

    app.world_mut()
        .resource_mut::<Messages<Ime>>()
        .write(Ime::Commit {
            window: primary,
            value: "好".to_owned(),
        });
    run_input_systems(&mut app);
    assert!(
        app.world().resource::<ImguiInputState>().ime_enabled(),
        "committed text should not imply that the platform IME was disabled"
    );
    begin_frame_and_assert(&mut app, |_ui| {
        assert!(current_frame_input_chars().contains(&('好' as u32)));
    });

    app.world_mut()
        .resource_mut::<Messages<Ime>>()
        .write(Ime::Disabled { window: primary });
    run_input_systems(&mut app);
    assert!(!app.world().resource::<ImguiInputState>().ime_enabled());
}

#[test]
fn input_resize_dpi_and_cursor_leave_messages_update_imgui_io() {
    let _guard = imgui_context_guard();
    let (mut app, primary) = app_with_primary_window();

    app.world_mut()
        .resource_mut::<Messages<WindowResized>>()
        .write(WindowResized {
            window: primary,
            width: 1024.0,
            height: 768.0,
        });
    app.world_mut()
        .resource_mut::<Messages<WindowScaleFactorChanged>>()
        .write(WindowScaleFactorChanged {
            window: primary,
            scale_factor: 1.5,
        });
    app.world_mut()
        .resource_mut::<Messages<CursorMoved>>()
        .write(CursorMoved {
            window: primary,
            position: Vec2::new(8.0, 9.0),
            delta: None,
        });
    app.world_mut()
        .resource_mut::<Messages<CursorLeft>>()
        .write(CursorLeft { window: primary });

    run_input_systems(&mut app);

    configure_primary(&mut app, |context| {
        assert_eq!(context.io().display_size(), [1024.0, 768.0]);
        assert_eq!(context.io().display_framebuffer_scale(), [1.5, 1.5]);
    });

    begin_frame_and_assert(&mut app, |ui| {
        assert!(
            ui.mouse_pos()[0] < -1.0e30 && ui.mouse_pos()[1] < -1.0e30,
            "CursorLeft should move the Dear ImGui mouse position outside every window"
        );
    });
}

#[test]
fn input_invalid_window_metrics_are_sanitized_before_reaching_imgui_io() {
    let _guard = imgui_context_guard();
    let (mut app, primary) = app_with_primary_window();
    app.world_mut()
        .get_mut::<Window>(primary)
        .unwrap()
        .resolution
        .set_scale_factor(f32::NAN);

    app.world_mut()
        .resource_mut::<Messages<WindowResized>>()
        .write(WindowResized {
            window: primary,
            width: f32::NAN,
            height: -10.0,
        });
    app.world_mut()
        .resource_mut::<Messages<WindowScaleFactorChanged>>()
        .write(WindowScaleFactorChanged {
            window: primary,
            scale_factor: f64::INFINITY,
        });

    run_input_systems(&mut app);

    configure_primary(&mut app, |context| {
        assert_eq!(context.io().display_size(), [0.0, 0.0]);
        assert_eq!(context.io().display_framebuffer_scale(), [1.0, 1.0]);
    });
}

#[test]
fn input_platform_feedback_updates_primary_window_cursor_and_ime_state() {
    let _guard = imgui_context_guard();
    let (mut app, primary) = app_with_primary_window();
    app.world_mut().get_mut::<Window>(primary).unwrap().position =
        WindowPosition::At(IVec2::new(100, 150));
    let primary_pass = app.imgui_primary_pass();
    app.add_imgui_systems(
        &primary_pass,
        primary_pass.system(request_text_cursor_and_ime::<ImguiPrimaryPass>),
    );

    app.update();

    let entity = app.world().entity(primary);
    assert!(
        entity.get::<CursorOptions>().unwrap().visible,
        "OS cursor should stay visible when Dear ImGui is not drawing a software cursor"
    );
    assert_eq!(
        entity.get::<CursorIcon>(),
        Some(&CursorIcon::System(SystemCursorIcon::Text))
    );
    let window = entity.get::<Window>().unwrap();
    assert!(window.ime_enabled);
    assert_eq!(window.ime_position, Vec2::new(222.0, 333.0));
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[test]
fn input_platform_feedback_updates_secondary_viewport_window_cursor_and_ime_state() {
    let _guard = imgui_context_guard();
    let (mut app, primary) = app_with_primary_window_and_native_viewports();
    let viewport_id = imgui::Id::from(0x501);
    let fixture = create_native_viewport_window(
        &mut app,
        viewport_id,
        Window {
            position: WindowPosition::At(IVec2::new(100, 150)),
            resolution: WindowResolution::new(640, 480),
            ..Default::default()
        },
    );
    let secondary = fixture.window();
    app.world_mut()
        .get_mut::<Window>(secondary)
        .unwrap()
        .resolution
        .set_scale_factor(2.0);
    app.world_mut()
        .resource_mut::<Messages<CursorMoved>>()
        .write(CursorMoved {
            window: secondary,
            position: Vec2::new(10.0, 20.0),
            delta: None,
        });
    let primary_pass = app.imgui_primary_pass();
    app.add_imgui_systems(
        &primary_pass,
        primary_pass.system(request_text_cursor_and_secondary_viewport_ime),
    );

    app.update();

    let primary_window = app.world().entity(primary).get::<Window>().unwrap();
    assert!(
        !primary_window.ime_enabled,
        "IME feedback for a secondary viewport should not be applied to the primary window"
    );

    let entity = app.world().entity(secondary);
    assert!(
        entity.get::<CursorOptions>().unwrap().visible,
        "OS cursor should stay visible when Dear ImGui is not drawing a software cursor"
    );
    assert_eq!(
        entity.get::<CursorIcon>(),
        Some(&CursorIcon::System(SystemCursorIcon::Text))
    );
    let window = entity.get::<Window>().unwrap();
    assert!(window.ime_enabled);
    assert_eq!(window.ime_position, Vec2::new(44.0, 55.0));
    fixture.destroy(&mut app);
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[test]
fn input_platform_feedback_routes_cursor_independently_from_ime_viewport() {
    let _guard = imgui_context_guard();
    let (mut app, primary) = app_with_primary_window_and_native_viewports();
    let viewport_id = imgui::Id::from(0x502);
    let fixture = create_native_viewport_window(
        &mut app,
        viewport_id,
        Window {
            resolution: WindowResolution::new(640, 480),
            ..Default::default()
        },
    );
    let secondary = fixture.window();
    app.world_mut()
        .resource_mut::<Messages<CursorMoved>>()
        .write(CursorMoved {
            window: primary,
            position: Vec2::new(11.0, 22.0),
            delta: None,
        });
    let primary_pass = app.imgui_primary_pass();
    app.add_imgui_systems(
        &primary_pass,
        primary_pass.system(request_primary_cursor_and_secondary_viewport_ime),
    );

    app.update();

    let primary_entity = app.world().entity(primary);
    assert_eq!(
        primary_entity.get::<CursorIcon>(),
        Some(&CursorIcon::System(SystemCursorIcon::Text)),
        "cursor feedback should follow the hovered Bevy window"
    );
    assert!(
        !primary_entity.get::<Window>().unwrap().ime_enabled,
        "IME feedback for a secondary viewport should not be applied to the primary window"
    );

    let secondary_entity = app.world().entity(secondary);
    assert!(
        secondary_entity.get::<CursorIcon>().is_none(),
        "IME viewport must not pull cursor feedback onto a non-hovered window"
    );
    let secondary_window = secondary_entity.get::<Window>().unwrap();
    assert!(secondary_window.ime_enabled);
    assert_eq!(secondary_window.ime_position, Vec2::new(77.0, 88.0));
    fixture.destroy(&mut app);
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[test]
fn input_disabled_route_suppresses_native_viewport_ime() {
    let _guard = imgui_context_guard();
    let (mut app, primary) = app_with_primary_window_and_native_viewports();
    let viewport_id = imgui::Id::from(0x506);
    let fixture = create_native_viewport_window(&mut app, viewport_id, Window::default());
    let context_id = primary_context_id(&app);
    let route = {
        let mut routes = app.world_mut().query::<(Entity, &ImguiInputRoute)>();
        routes
            .iter(app.world())
            .find(|(_, route)| route.context_id() == context_id)
            .map(|(entity, route)| (entity, *route))
            .expect("the primary Context must retain its explicit input route")
    };
    app.world_mut()
        .entity_mut(route.0)
        .insert(route.1.with_policy(ImguiInputPolicy::Disabled));
    app.world_mut().run_schedule(PostUpdate);
    let primary_pass = app.imgui_primary_pass();
    app.add_imgui_systems(
        &primary_pass,
        primary_pass.system(request_primary_cursor_and_secondary_viewport_ime),
    );

    app.update();

    assert!(
        !app.world()
            .entity(fixture.window())
            .get::<Window>()
            .unwrap()
            .ime_enabled,
        "a disabled Context input route must suppress native viewport IME feedback"
    );
    assert!(
        app.world()
            .resource::<ImguiInputState>()
            .for_context_window(context_id, fixture.window())
            .is_none(),
        "a disabled Context must not synthesize native viewport input state"
    );
    assert_eq!(
        app.world().entity(primary).get::<CursorIcon>(),
        None,
        "a disabled Context must not publish cursor feedback"
    );
    fixture.destroy(&mut app);
}

#[cfg(feature = "render")]
#[test]
fn input_platform_feedback_enables_ime_for_a_focused_secondary_context_route() {
    let _guard = imgui_context_guard();
    let (mut app, primary_context, primary_window, secondary_window) = routed_input_app();
    let (secondary_context, secondary_pass) = add_routed_input_context(&mut app);
    let primary_region = logical_window_region(&app, primary_window);
    let secondary_region = logical_window_region(&app, secondary_window);
    app.world_mut().spawn(ImguiInputRoute::logical(
        primary_context,
        primary_window,
        primary_region,
    ));
    app.world_mut().spawn(ImguiInputRoute::logical(
        secondary_context,
        secondary_window,
        secondary_region,
    ));
    resolve_routed_input(&mut app);
    app.add_imgui_systems(
        &secondary_pass,
        secondary_pass.system(request_text_cursor_and_ime::<RoutedInputSecondaryUi>),
    );

    app.world_mut()
        .resource_mut::<Messages<WindowFocused>>()
        .write(WindowFocused {
            window: primary_window,
            focused: false,
        });
    app.world_mut()
        .resource_mut::<Messages<WindowFocused>>()
        .write(WindowFocused {
            window: secondary_window,
            focused: true,
        });
    app.update();

    assert!(
        !app.world()
            .entity(primary_window)
            .get::<Window>()
            .expect("primary Window must exist")
            .ime_enabled,
        "a secondary Context must not enable IME on the primary Window"
    );
    let secondary = app
        .world()
        .entity(secondary_window)
        .get::<Window>()
        .unwrap();
    assert!(
        secondary.ime_enabled,
        "a focused secondary Context must enable its host Window IME"
    );
    assert_eq!(secondary.ime_position, Vec2::new(222.0, 333.0));

    app.world_mut()
        .resource_mut::<Messages<WindowFocused>>()
        .write(WindowFocused {
            window: secondary_window,
            focused: false,
        });
    app.update();
    assert!(
        !app.world()
            .entity(secondary_window)
            .get::<Window>()
            .expect("secondary Window must exist")
            .ime_enabled,
        "losing the routed focus must clear the previous IME request"
    );
}

#[cfg(feature = "render")]
#[test]
fn input_platform_feedback_follows_the_current_exclusive_pointer_owner() {
    let _guard = imgui_context_guard();
    let (mut app, primary_context, primary_window, _) = routed_input_app();
    let (secondary_context, secondary_pass) = add_routed_input_context(&mut app);
    let left = bevy_math::Rect::from_corners(Vec2::ZERO, Vec2::new(320.0, 480.0));
    let right = bevy_math::Rect::from_corners(Vec2::new(320.0, 0.0), Vec2::new(640.0, 480.0));
    app.world_mut().spawn(ImguiInputRoute::logical(
        primary_context,
        primary_window,
        left,
    ));
    app.world_mut().spawn(ImguiInputRoute::logical(
        secondary_context,
        primary_window,
        right,
    ));
    resolve_routed_input(&mut app);
    let primary_pass = app.imgui_primary_pass();
    app.add_imgui_systems(&primary_pass, primary_pass.system(request_hidden_cursor));
    app.add_imgui_systems(
        &secondary_pass,
        secondary_pass.system(request_text_cursor_only::<RoutedInputSecondaryUi>),
    );

    app.world_mut()
        .resource_mut::<Messages<CursorMoved>>()
        .write(CursorMoved {
            window: primary_window,
            position: Vec2::new(100.0, 120.0),
            delta: None,
        });
    app.update();
    assert!(
        !app.world()
            .entity(primary_window)
            .get::<CursorOptions>()
            .unwrap()
            .visible,
        "the primary Context should own cursor feedback in the left region"
    );

    app.world_mut()
        .resource_mut::<Messages<CursorMoved>>()
        .write(CursorMoved {
            window: primary_window,
            position: Vec2::new(500.0, 120.0),
            delta: None,
        });
    app.update();

    let window = app.world().entity(primary_window);
    assert!(
        window.get::<CursorOptions>().unwrap().visible,
        "a historical primary input slot must not hide the secondary Context's cursor"
    );
    assert_eq!(
        window.get::<CursorIcon>(),
        Some(&CursorIcon::System(SystemCursorIcon::Text))
    );
}

#[cfg(feature = "render")]
#[test]
fn input_platform_feedback_targets_input_host_independently_of_render_host() {
    let _guard = imgui_context_guard();
    let (mut app, primary_context, primary_window, input_window) = routed_input_app();
    let camera = app
        .world_mut()
        .spawn((
            Camera::default(),
            RenderTarget::Window(WindowRef::Entity(primary_window)),
            CameraRenderGraph::new(Core2d),
        ))
        .id();
    app.world_mut()
        .spawn(ImguiRenderRoute::new(primary_context, camera));
    let input_region = logical_window_region(&app, input_window);
    app.world_mut().spawn(ImguiInputRoute::logical(
        primary_context,
        input_window,
        input_region,
    ));
    resolve_routed_input(&mut app);
    let primary_pass = app.imgui_primary_pass();
    app.add_imgui_systems(
        &primary_pass,
        primary_pass.system(request_text_cursor_only::<ImguiPrimaryPass>),
    );
    app.world_mut()
        .resource_mut::<Messages<CursorMoved>>()
        .write(CursorMoved {
            window: input_window,
            position: Vec2::new(24.0, 32.0),
            delta: None,
        });

    app.update();

    assert_eq!(
        app.world().entity(input_window).get::<CursorIcon>(),
        Some(&CursorIcon::System(SystemCursorIcon::Text)),
        "cursor feedback must follow the explicit input host rather than the render target window"
    );
    assert!(
        app.world()
            .entity(primary_window)
            .get::<CursorIcon>()
            .is_none()
    );
}

#[cfg(feature = "render")]
#[test]
fn input_platform_feedback_does_not_enable_ime_for_a_disabled_route() {
    let _guard = imgui_context_guard();
    let (mut app, primary_context, primary_window, _) = routed_input_app();
    let region = logical_window_region(&app, primary_window);
    let camera = app
        .world_mut()
        .spawn((
            Camera::default(),
            RenderTarget::Window(WindowRef::Entity(primary_window)),
            CameraRenderGraph::new(Core2d),
        ))
        .id();
    app.world_mut()
        .spawn(ImguiRenderRoute::new(primary_context, camera));
    app.world_mut().spawn(
        ImguiInputRoute::logical(primary_context, primary_window, region)
            .with_policy(ImguiInputPolicy::Disabled),
    );
    resolve_routed_input(&mut app);
    let primary_pass = app.imgui_primary_pass();
    app.add_imgui_systems(
        &primary_pass,
        primary_pass.system(request_text_cursor_and_ime::<ImguiPrimaryPass>),
    );

    app.update();

    assert!(
        !app.world()
            .entity(primary_window)
            .get::<Window>()
            .unwrap()
            .ime_enabled,
        "a Context with disabled input must not control the host window IME"
    );
}

#[cfg(feature = "render")]
#[test]
fn input_platform_feedback_ignores_render_only_context_without_hover_ownership() {
    let _guard = imgui_context_guard();
    let (mut app, primary_context, primary_window, _secondary_window) = routed_input_app();
    let (secondary_context, _) = add_routed_input_context(&mut app);
    let primary_region = logical_window_region(&app, primary_window);
    app.world_mut().spawn(ImguiInputRoute::logical(
        primary_context,
        primary_window,
        primary_region,
    ));
    app.world_mut().spawn(
        ImguiInputRoute::logical(secondary_context, primary_window, primary_region)
            .with_policy(ImguiInputPolicy::Disabled),
    );
    let camera = app
        .world_mut()
        .spawn((
            Camera::default(),
            RenderTarget::Window(WindowRef::Entity(primary_window)),
            CameraRenderGraph::new(Core2d),
        ))
        .id();
    app.world_mut()
        .spawn(ImguiRenderRoute::new(secondary_context, camera));
    resolve_routed_input(&mut app);
    let primary_pass = app.imgui_primary_pass();
    app.add_imgui_systems(
        &primary_pass,
        primary_pass.system(request_text_cursor_and_ime::<ImguiPrimaryPass>),
    );
    app.world_mut()
        .resource_mut::<Messages<CursorMoved>>()
        .write(CursorMoved {
            window: primary_window,
            position: Vec2::new(24.0, 32.0),
            delta: None,
        });

    app.update();

    let input_state = app.world().resource::<ImguiInputState>();
    assert_eq!(
        input_state.mouse_hovered_window(),
        Some(primary_window),
        "the primary Context must retain the routed pointer owner"
    );
    assert!(
        input_state
            .for_context_window(secondary_context, primary_window)
            .is_none(),
        "a render route must not synthesize an input window state"
    );
    assert_eq!(
        app.world().entity(primary_window).get::<CursorIcon>(),
        Some(&CursorIcon::System(SystemCursorIcon::Text)),
        "a later render-only Context must not overwrite the input owner's cursor feedback"
    );
}

#[test]
fn input_platform_feedback_hides_os_cursor_when_imgui_draws_software_cursor() {
    let _guard = imgui_context_guard();
    let (mut app, primary) = app_with_primary_window();
    app.world_mut()
        .entity_mut(primary)
        .insert(CursorIcon::from(SystemCursorIcon::Pointer));
    let primary_pass = app.imgui_primary_pass();
    app.add_imgui_systems(&primary_pass, primary_pass.system(request_software_cursor));

    app.update();

    let entity = app.world().entity(primary);
    assert!(
        !entity.get::<CursorOptions>().unwrap().visible,
        "OS cursor should be hidden while Dear ImGui draws the software cursor"
    );
    assert!(entity.get::<CursorIcon>().is_none());
    assert!(!entity.get::<Window>().unwrap().ime_enabled);
}

#[test]
fn input_platform_feedback_hides_os_cursor_when_imgui_requests_no_cursor() {
    let _guard = imgui_context_guard();
    let (mut app, primary) = app_with_primary_window();
    app.world_mut()
        .entity_mut(primary)
        .insert(CursorIcon::from(SystemCursorIcon::Pointer));
    let primary_pass = app.imgui_primary_pass();
    app.add_imgui_systems(&primary_pass, primary_pass.system(request_hidden_cursor));

    app.update();

    let entity = app.world().entity(primary);
    assert!(
        !entity.get::<CursorOptions>().unwrap().visible,
        "OS cursor should be hidden when Dear ImGui reports no cursor"
    );
    assert!(entity.get::<CursorIcon>().is_none());
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[test]
fn input_platform_feedback_restores_cursor_on_previous_hovered_window() {
    let _guard = imgui_context_guard();
    let (mut app, primary) = app_with_primary_window_and_native_viewports();
    let fixture =
        create_native_viewport_window(&mut app, imgui::Id::from(0x503), Window::default());
    let secondary = fixture.window();
    let primary_pass = app.imgui_primary_pass();
    app.add_imgui_systems(&primary_pass, primary_pass.system(request_software_cursor));

    app.world_mut()
        .resource_mut::<Messages<CursorMoved>>()
        .write(CursorMoved {
            window: secondary,
            position: Vec2::new(10.0, 20.0),
            delta: None,
        });
    app.update();
    assert!(
        !app.world()
            .entity(secondary)
            .get::<CursorOptions>()
            .unwrap()
            .visible,
        "the hovered secondary window should inherit Dear ImGui's hidden software-cursor state"
    );

    app.world_mut()
        .resource_mut::<Messages<CursorMoved>>()
        .write(CursorMoved {
            window: primary,
            position: Vec2::new(30.0, 40.0),
            delta: None,
        });
    app.update();

    let secondary_entity = app.world().entity(secondary);
    assert!(
        secondary_entity.get::<CursorOptions>().unwrap().visible,
        "moving hover away from a secondary window must restore its OS cursor visibility"
    );
    assert!(
        secondary_entity.get::<CursorIcon>().is_none(),
        "moving hover away from a secondary window must clear stale ImGui cursor icons"
    );
    assert!(
        !app.world()
            .entity(primary)
            .get::<CursorOptions>()
            .unwrap()
            .visible,
        "the newly hovered primary window should now inherit Dear ImGui's hidden software-cursor state"
    );
    fixture.destroy(&mut app);
}

#[test]
fn input_focus_loss_releases_tracked_keyboard_and_mouse_state() {
    let _guard = imgui_context_guard();
    let (mut app, primary) = app_with_primary_window();

    app.world_mut()
        .resource_mut::<Messages<KeyboardInput>>()
        .write(key_input(
            primary,
            KeyCode::ControlLeft,
            BevyKey::Control,
            ButtonState::Pressed,
            None,
        ));
    app.world_mut()
        .resource_mut::<Messages<KeyboardInput>>()
        .write(key_input(
            primary,
            KeyCode::KeyA,
            BevyKey::Character("a".into()),
            ButtonState::Pressed,
            Some("a"),
        ));
    app.world_mut()
        .resource_mut::<Messages<MouseButtonInput>>()
        .write(MouseButtonInput {
            button: BevyMouseButton::Left,
            state: ButtonState::Pressed,
            window: primary,
        });
    run_input_systems(&mut app);

    begin_frame_and_assert(&mut app, |ui| {
        assert!(ui.is_key_down(imgui::Key::A));
        assert!(ui.is_mouse_down(imgui::MouseButton::Left));
    });

    app.world_mut()
        .resource_mut::<Messages<WindowFocused>>()
        .write(WindowFocused {
            window: primary,
            focused: false,
        });
    run_input_systems(&mut app);

    begin_frame_and_assert(&mut app, |ui| {
        assert!(!ui.is_key_down(imgui::Key::A));
        assert!(!ui.is_mouse_down(imgui::MouseButton::Left));
    });
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[test]
fn input_focus_switch_between_viewport_windows_keeps_sticky_input_pressed() {
    let _guard = imgui_context_guard();
    let (mut app, primary) = app_with_primary_window_and_native_viewports();
    let fixture =
        create_native_viewport_window(&mut app, imgui::Id::from(0x560), Window::default());
    let secondary = fixture.window();

    app.world_mut()
        .resource_mut::<Messages<KeyboardInput>>()
        .write(key_input(
            primary,
            KeyCode::ControlLeft,
            BevyKey::Control,
            ButtonState::Pressed,
            None,
        ));
    app.world_mut()
        .resource_mut::<Messages<KeyboardInput>>()
        .write(key_input(
            primary,
            KeyCode::KeyA,
            BevyKey::Character("a".into()),
            ButtonState::Pressed,
            None,
        ));
    app.world_mut()
        .resource_mut::<Messages<MouseButtonInput>>()
        .write(MouseButtonInput {
            button: BevyMouseButton::Left,
            state: ButtonState::Pressed,
            window: primary,
        });
    run_input_systems(&mut app);

    begin_frame_and_assert(&mut app, |ui| {
        assert!(ui.io().key_ctrl());
        assert!(ui.is_key_down(imgui::Key::A));
        assert!(ui.is_mouse_down(imgui::MouseButton::Left));
    });

    app.world_mut()
        .resource_mut::<Messages<WindowFocused>>()
        .write(WindowFocused {
            window: primary,
            focused: false,
        });
    app.world_mut()
        .resource_mut::<Messages<WindowFocused>>()
        .write(WindowFocused {
            window: secondary,
            focused: true,
        });
    app.world_mut()
        .resource_mut::<Messages<KeyboardInput>>()
        .write(key_input(
            secondary,
            KeyCode::KeyC,
            BevyKey::Character("c".into()),
            ButtonState::Pressed,
            None,
        ));
    run_input_systems(&mut app);

    let input_state = app.world().resource::<ImguiInputState>();
    assert_eq!(input_state.primary_window_focused(), Some(false));
    assert_eq!(input_state.focused_window(), Some(secondary));
    begin_frame_and_assert(&mut app, |ui| {
        assert!(
            ui.io().key_ctrl(),
            "modifier state must survive focus transfer within one Context"
        );
        assert!(ui.is_key_down(imgui::Key::C));
        assert!(
            ui.is_key_down(imgui::Key::A),
            "switching focus between mapped ImGui windows must not synthesize a global key release"
        );
        assert!(
            ui.is_mouse_down(imgui::MouseButton::Left),
            "switching focus between mapped ImGui windows must not synthesize a global mouse release"
        );
    });

    app.world_mut()
        .resource_mut::<Messages<KeyboardInput>>()
        .write(key_input(
            secondary,
            KeyCode::ControlLeft,
            BevyKey::Control,
            ButtonState::Released,
            None,
        ));
    run_input_systems(&mut app);
    begin_frame_and_assert(&mut app, |ui| {
        assert!(
            !ui.io().key_ctrl(),
            "a release on the newly focused viewport must clear the Context-wide modifier"
        );
    });

    app.world_mut()
        .resource_mut::<Messages<WindowFocused>>()
        .write(WindowFocused {
            window: secondary,
            focused: false,
        });
    app.world_mut()
        .resource_mut::<Messages<WindowFocused>>()
        .write(WindowFocused {
            window: primary,
            focused: true,
        });
    run_input_systems(&mut app);
    begin_frame_and_assert(&mut app, |ui| {
        assert!(
            !ui.io().key_ctrl(),
            "returning to the original viewport must not resurrect a released modifier"
        );
    });
    fixture.destroy(&mut app);
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[test]
fn input_primary_focus_sync_does_not_blur_while_secondary_viewport_is_focused() {
    let _guard = imgui_context_guard();
    let (mut app, primary) = app_with_primary_window_and_native_viewports();
    app.world_mut().get_mut::<Window>(primary).unwrap().focused = true;
    let fixture =
        create_native_viewport_window(&mut app, imgui::Id::from(0x561), Window::default());
    let secondary = fixture.window();

    app.world_mut()
        .resource_mut::<Messages<KeyboardInput>>()
        .write(key_input(
            primary,
            KeyCode::KeyA,
            BevyKey::Character("a".into()),
            ButtonState::Pressed,
            None,
        ));
    app.world_mut()
        .resource_mut::<Messages<MouseButtonInput>>()
        .write(MouseButtonInput {
            button: BevyMouseButton::Left,
            state: ButtonState::Pressed,
            window: primary,
        });
    run_input_systems(&mut app);

    app.world_mut()
        .resource_mut::<Messages<WindowFocused>>()
        .write(WindowFocused {
            window: secondary,
            focused: true,
        });
    run_input_systems(&mut app);

    app.world_mut().get_mut::<Window>(primary).unwrap().focused = false;
    run_input_systems(&mut app);

    let input_state = app.world().resource::<ImguiInputState>();
    assert_eq!(input_state.primary_window_focused(), Some(false));
    assert_eq!(input_state.focused_window(), Some(secondary));
    begin_frame_and_assert(&mut app, |ui| {
        assert!(
            ui.is_key_down(imgui::Key::A),
            "primary focus sync must not release keys while a secondary ImGui viewport is focused"
        );
        assert!(
            ui.is_mouse_down(imgui::MouseButton::Left),
            "primary focus sync must not release mouse buttons while a secondary ImGui viewport is focused"
        );
    });
    fixture.destroy(&mut app);
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[test]
fn input_stale_focused_viewport_window_releases_sticky_input() {
    let _guard = imgui_context_guard();
    let (mut app, primary) = app_with_primary_window_and_native_viewports();
    app.world_mut().get_mut::<Window>(primary).unwrap().focused = true;
    let fixture =
        create_native_viewport_window(&mut app, imgui::Id::from(0x562), Window::default());
    let secondary = fixture.window();

    app.world_mut()
        .resource_mut::<Messages<KeyboardInput>>()
        .write(key_input(
            primary,
            KeyCode::KeyA,
            BevyKey::Character("a".into()),
            ButtonState::Pressed,
            None,
        ));
    app.world_mut()
        .resource_mut::<Messages<MouseButtonInput>>()
        .write(MouseButtonInput {
            button: BevyMouseButton::Left,
            state: ButtonState::Pressed,
            window: primary,
        });
    run_input_systems(&mut app);

    app.world_mut()
        .resource_mut::<Messages<WindowFocused>>()
        .write(WindowFocused {
            window: primary,
            focused: false,
        });
    app.world_mut()
        .resource_mut::<Messages<WindowFocused>>()
        .write(WindowFocused {
            window: secondary,
            focused: true,
        });
    run_input_systems(&mut app);
    app.world_mut().get_mut::<Window>(primary).unwrap().focused = false;
    app.world_mut().despawn(secondary);
    run_input_systems(&mut app);

    let input_state = app.world().resource::<ImguiInputState>();
    assert_eq!(input_state.focused_window(), None);
    begin_frame_and_assert(&mut app, |ui| {
        assert!(
            !ui.is_key_down(imgui::Key::A),
            "destroying the focused secondary viewport must release sticky keys"
        );
        assert!(
            !ui.is_mouse_down(imgui::MouseButton::Left),
            "destroying the focused secondary viewport must release sticky mouse buttons"
        );
    });
    fixture.destroy(&mut app);
}

#[test]
fn input_keyboard_focus_lost_releases_tracked_state_without_window_message() {
    let _guard = imgui_context_guard();
    let (mut app, primary) = app_with_primary_window();

    app.world_mut()
        .resource_mut::<Messages<KeyboardInput>>()
        .write(key_input(
            primary,
            KeyCode::KeyA,
            BevyKey::Character("a".into()),
            ButtonState::Pressed,
            None,
        ));
    app.world_mut()
        .resource_mut::<Messages<MouseButtonInput>>()
        .write(MouseButtonInput {
            button: BevyMouseButton::Right,
            state: ButtonState::Pressed,
            window: primary,
        });
    run_input_systems(&mut app);

    begin_frame_and_assert(&mut app, |ui| {
        assert!(ui.is_key_down(imgui::Key::A));
        assert!(ui.is_mouse_down(imgui::MouseButton::Right));
    });

    app.world_mut()
        .resource_mut::<Messages<KeyboardFocusLost>>()
        .write(KeyboardFocusLost);
    run_input_systems(&mut app);

    assert_eq!(
        app.world()
            .resource::<ImguiInputState>()
            .primary_window_focused(),
        Some(false)
    );
    begin_frame_and_assert(&mut app, |ui| {
        assert!(!ui.is_key_down(imgui::Key::A));
        assert!(!ui.is_mouse_down(imgui::MouseButton::Right));
    });
}

#[test]
fn input_missing_primary_window_releases_tracked_state_and_clears_window_state() {
    let _guard = imgui_context_guard();
    let (mut app, primary) = app_with_primary_window();
    configure_primary(&mut app, |context| {
        context
            .io_mut()
            .set_config_flags(imgui::ConfigFlags::VIEWPORTS_ENABLE);
    });

    app.world_mut()
        .resource_mut::<Messages<CursorMoved>>()
        .write(CursorMoved {
            window: primary,
            position: Vec2::new(12.0, 34.0),
            delta: None,
        });
    app.world_mut()
        .resource_mut::<Messages<KeyboardInput>>()
        .write(key_input(
            primary,
            KeyCode::KeyA,
            BevyKey::Character("a".into()),
            ButtonState::Pressed,
            None,
        ));
    app.world_mut()
        .resource_mut::<Messages<MouseButtonInput>>()
        .write(MouseButtonInput {
            button: BevyMouseButton::Right,
            state: ButtonState::Pressed,
            window: primary,
        });
    app.world_mut()
        .resource_mut::<Messages<TouchInput>>()
        .write(TouchInput {
            phase: TouchPhase::Started,
            position: Vec2::new(56.0, 78.0),
            window: primary,
            force: None,
            id: 9,
        });
    app.world_mut()
        .resource_mut::<Messages<Ime>>()
        .write(Ime::Enabled { window: primary });
    run_input_systems(&mut app);

    assert_eq!(
        app.world()
            .resource::<ImguiInputState>()
            .mouse_hovered_window(),
        Some(primary)
    );
    assert_eq!(
        app.world().resource::<ImguiInputState>().active_touch_id(),
        Some(9)
    );
    assert!(app.world().resource::<ImguiInputState>().ime_enabled());
    begin_frame_and_assert(&mut app, |ui| {
        assert_eq!(ui.io().mouse_hovered_viewport(), imgui::Id::from(0));
        assert!(ui.is_key_down(imgui::Key::A));
        assert!(ui.is_mouse_down(imgui::MouseButton::Right));
        assert!(ui.is_mouse_down(imgui::MouseButton::Left));
    });

    app.world_mut().despawn(primary);
    run_input_systems(&mut app);

    let input_state = app.world().resource::<ImguiInputState>();
    assert_eq!(input_state.primary_window_focused(), Some(false));
    assert_eq!(input_state.focused_window(), None);
    assert_eq!(input_state.mouse_hovered_window(), None);
    assert_eq!(input_state.active_touch_id(), None);
    assert!(!input_state.ime_enabled());
    begin_frame_and_assert(&mut app, |ui| {
        assert!(!ui.is_key_down(imgui::Key::A));
        assert!(!ui.is_mouse_down(imgui::MouseButton::Right));
        assert!(!ui.is_mouse_down(imgui::MouseButton::Left));
        assert!(
            ui.mouse_pos()[0] < -1.0e30 && ui.mouse_pos()[1] < -1.0e30,
            "removing the primary window must clear the ImGui mouse position"
        );
        assert_eq!(
            ui.io().mouse_hovered_viewport(),
            imgui::Id::from(0),
            "removing the primary window must clear the hovered viewport id"
        );
    });
}

#[test]
fn input_touch_events_drive_first_active_finger_as_touchscreen_mouse() {
    let _guard = imgui_context_guard();
    let (mut app, primary) = app_with_primary_window();

    app.world_mut()
        .resource_mut::<Messages<TouchInput>>()
        .write(TouchInput {
            phase: TouchPhase::Started,
            position: Vec2::new(10.0, 20.0),
            window: primary,
            force: None,
            id: 7,
        });
    run_input_systems(&mut app);

    assert_eq!(
        app.world().resource::<ImguiInputState>().active_touch_id(),
        Some(7)
    );
    begin_frame_and_assert(&mut app, |ui| {
        assert_eq!(ui.mouse_pos(), [10.0, 20.0]);
        assert_eq!(ui.io().mouse_source(), imgui::MouseSource::TouchScreen);
        assert!(ui.is_mouse_down(imgui::MouseButton::Left));
    });

    app.world_mut()
        .resource_mut::<Messages<TouchInput>>()
        .write(TouchInput {
            phase: TouchPhase::Ended,
            position: Vec2::new(15.0, 25.0),
            window: primary,
            force: None,
            id: 7,
        });
    run_input_systems(&mut app);

    assert_eq!(
        app.world().resource::<ImguiInputState>().active_touch_id(),
        None
    );
    begin_frame_and_assert(&mut app, |ui| {
        assert_eq!(ui.mouse_pos(), [15.0, 25.0]);
        assert_eq!(ui.io().mouse_source(), imgui::MouseSource::TouchScreen);
        assert!(!ui.is_mouse_down(imgui::MouseButton::Left));
    });
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[test]
fn input_stale_touched_viewport_window_clears_touch_mouse_state() {
    let _guard = imgui_context_guard();
    let (mut app, primary) = app_with_primary_window_and_native_viewports();
    let viewport_id = imgui::Id::from(0x563);
    let fixture = create_native_viewport_window(
        &mut app,
        viewport_id,
        Window {
            position: WindowPosition::At(IVec2::new(120, 180)),
            resolution: WindowResolution::new(640, 480),
            ..Default::default()
        },
    );
    let secondary = fixture.window();

    app.world_mut()
        .resource_mut::<Messages<TouchInput>>()
        .write(TouchInput {
            phase: TouchPhase::Started,
            position: Vec2::new(15.0, 25.0),
            window: secondary,
            force: None,
            id: 7,
        });
    run_input_systems(&mut app);
    begin_frame_and_assert(&mut app, |ui| {
        assert_eq!(ui.mouse_pos(), [135.0, 205.0]);
        assert_eq!(ui.io().mouse_source(), imgui::MouseSource::TouchScreen);
        assert!(ui.is_mouse_down(imgui::MouseButton::Left));
    });

    app.world_mut().despawn(secondary);
    run_input_systems(&mut app);

    let input_state = app.world().resource::<ImguiInputState>();
    assert_eq!(input_state.active_touch_id(), None);
    begin_frame_and_assert(&mut app, |ui| {
        assert!(
            ui.mouse_pos()[0] < -1.0e30 && ui.mouse_pos()[1] < -1.0e30,
            "destroying the touched secondary viewport must clear the ImGui mouse position"
        );
        assert!(!ui.is_mouse_down(imgui::MouseButton::Left));
        assert_eq!(ui.io().mouse_hovered_viewport(), imgui::Id::from(0));
    });

    assert!(app.world().get::<Window>(primary).is_some());
    fixture.destroy(&mut app);
}

#[test]
fn input_non_primary_window_messages_are_ignored() {
    let _guard = imgui_context_guard();
    let (mut app, _primary) = app_with_primary_window();
    let secondary = app.world_mut().spawn(Window::default()).id();

    app.world_mut()
        .resource_mut::<Messages<KeyboardInput>>()
        .write(key_input(
            secondary,
            KeyCode::KeyX,
            BevyKey::Character("x".into()),
            ButtonState::Pressed,
            Some("x"),
        ));
    app.world_mut()
        .resource_mut::<Messages<CursorMoved>>()
        .write(CursorMoved {
            window: secondary,
            position: Vec2::new(300.0, 400.0),
            delta: None,
        });
    run_input_systems(&mut app);

    begin_frame_and_assert(&mut app, |ui| {
        assert!(current_frame_input_chars().is_empty());
        assert!(!ui.is_key_down(imgui::Key::X));
        assert_ne!(ui.mouse_pos(), [300.0, 400.0]);
    });
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[test]
fn input_relocated_public_viewport_marker_cannot_spoof_backend_identity() {
    let _guard = imgui_context_guard();
    let (mut app, _primary) = app_with_primary_window_and_native_viewports();
    let viewport_id = imgui::Id::from(0x564);
    let fixture = create_native_viewport_window(&mut app, viewport_id, Window::default());
    let viewport_window = fixture.window();
    let marker = app
        .world_mut()
        .entity_mut(viewport_window)
        .take::<ImguiViewportWindow>()
        .expect("the callback-created viewport Window should have a public marker");
    let ordinary_window = app.world_mut().spawn((Window::default(), marker)).id();

    app.world_mut()
        .resource_mut::<Messages<CursorMoved>>()
        .write(CursorMoved {
            window: ordinary_window,
            position: Vec2::new(300.0, 400.0),
            delta: None,
        });
    run_input_systems(&mut app);

    assert_ne!(
        app.world()
            .resource::<ImguiInputState>()
            .mouse_hovered_window(),
        Some(ordinary_window),
        "moving the public marker must not transfer backend ownership"
    );
    begin_frame_and_assert(&mut app, |ui| {
        assert_ne!(
            ui.io().mouse_hovered_viewport(),
            viewport_id,
            "an ordinary Window carrying only the public marker must remain inert"
        );
    });

    app.update();
    let restored_marker = app
        .world()
        .get::<ImguiViewportWindow>(viewport_window)
        .expect("the backend must restore the marker on its privately owned Window");
    assert_eq!(restored_marker.viewport_id(), viewport_id);
    assert!(
        app.world()
            .get::<ImguiViewportWindow>(ordinary_window)
            .is_none(),
        "the backend must remove a relocated marker from an unowned Window"
    );

    fixture.destroy(&mut app);
    app.world_mut().despawn(ordinary_window);
}

#[cfg(all(
    feature = "render",
    feature = "multi-viewport",
    not(target_arch = "wasm32")
))]
#[test]
fn input_equal_viewport_ids_remain_scoped_to_their_context_windows() {
    let _guard = imgui_context_guard();
    let (mut app, primary_host) = app_with_primary_window_and_native_viewports();
    let primary_context = primary_context_id(&app);
    let secondary_pass = app.declare_imgui_pass::<RoutedInputSecondaryUi>();
    app.add_imgui_systems(
        &secondary_pass,
        secondary_pass.system(empty_routed_input_ui),
    );
    let secondary_context = app
        .world_mut()
        .non_send_mut::<ImguiContexts>()
        .create(ImguiContextConfig::new(&secondary_pass).with_multi_viewport(true))
        .expect("the secondary Context should receive its own native viewport bridge");
    prepare_context(&mut app, secondary_context);
    let secondary_host = app.world_mut().spawn(Window::default()).id();
    let secondary_region = logical_window_region(&app, secondary_host);
    app.world_mut().spawn(ImguiInputRoute::logical(
        secondary_context,
        secondary_host,
        secondary_region,
    ));
    resolve_routed_input(&mut app);

    let viewport_id = imgui::Id::from(0x565);
    let primary_viewport = CallbackViewport::create(&mut app, primary_context, viewport_id);
    let secondary_viewport = CallbackViewport::create(&mut app, secondary_context, viewport_id);
    assert_ne!(primary_viewport.window(), secondary_viewport.window());

    app.world_mut()
        .resource_mut::<Messages<CursorMoved>>()
        .write(CursorMoved {
            window: primary_viewport.window(),
            position: Vec2::new(24.0, 32.0),
            delta: None,
        });
    run_routed_input(&mut app);
    let input_state = app.world().resource::<ImguiInputState>();
    assert!(
        input_state
            .for_context_window(primary_context, primary_viewport.window())
            .expect("the primary native viewport should own its input slot")
            .mouse_hovered
    );
    assert!(
        input_state
            .for_context_window(secondary_context, primary_viewport.window())
            .is_none(),
        "an equal viewport ID must not make another Context own this window"
    );
    assert!(
        input_state
            .for_context_window(secondary_context, secondary_viewport.window())
            .is_none_or(|state| !state.mouse_hovered)
    );

    app.world_mut()
        .resource_mut::<Messages<CursorMoved>>()
        .write(CursorMoved {
            window: secondary_viewport.window(),
            position: Vec2::new(48.0, 56.0),
            delta: None,
        });
    run_routed_input(&mut app);
    assert!(
        app.world()
            .resource::<ImguiInputState>()
            .for_context_window(secondary_context, secondary_viewport.window())
            .expect("the secondary native viewport should own its input slot")
            .mouse_hovered
    );

    let secondary_window = secondary_viewport.window();
    primary_viewport.destroy(&mut app);
    assert_eq!(
        app.world()
            .non_send::<ImguiViewportBridge>()
            .viewport_window(secondary_context, viewport_id),
        Some(secondary_window),
        "destroying the primary Context's equal ID must preserve the secondary mapping"
    );
    assert!(app.world().get_entity(secondary_window).is_ok());
    secondary_viewport.destroy(&mut app);
    assert!(app.world().get_entity(primary_host).is_ok());
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[test]
fn input_secondary_viewport_window_messages_use_imgui_platform_coordinates_when_viewports_are_enabled()
 {
    let _guard = imgui_context_guard();
    let (mut app, _primary) = app_with_primary_window_and_native_viewports();
    let viewport_id = imgui::Id::from(0x500);
    let fixture = create_native_viewport_window(
        &mut app,
        viewport_id,
        Window {
            position: WindowPosition::At(IVec2::new(200, 300)),
            resolution: WindowResolution::new(640, 480),
            ..Default::default()
        },
    );
    let secondary = fixture.window();
    {
        let mut window = app.world_mut().get_mut::<Window>(secondary).unwrap();
        window.resolution.set_scale_factor(2.0);
    }
    configure_primary(&mut app, |context| {
        let io = context.io_mut();
        io.set_config_flags(io.config_flags() | imgui::ConfigFlags::VIEWPORTS_ENABLE);
    });

    app.world_mut()
        .resource_mut::<Messages<CursorMoved>>()
        .write(CursorMoved {
            window: secondary,
            position: Vec2::new(300.0, 400.0),
            delta: None,
        });
    app.world_mut()
        .resource_mut::<Messages<MouseButtonInput>>()
        .write(MouseButtonInput {
            button: BevyMouseButton::Right,
            state: ButtonState::Pressed,
            window: secondary,
        });
    app.world_mut()
        .resource_mut::<Messages<MouseWheel>>()
        .write(MouseWheel {
            unit: MouseScrollUnit::Pixel,
            x: -24.0,
            y: 24.0,
            window: secondary,
            phase: TouchPhase::Moved,
        });
    app.world_mut()
        .resource_mut::<Messages<KeyboardInput>>()
        .write(key_input(
            secondary,
            KeyCode::KeyX,
            BevyKey::Character("x".into()),
            ButtonState::Pressed,
            Some("x"),
        ));
    app.world_mut()
        .resource_mut::<Messages<Ime>>()
        .write(Ime::Commit {
            window: secondary,
            value: "界".to_owned(),
        });
    app.world_mut()
        .resource_mut::<Messages<WindowFocused>>()
        .write(WindowFocused {
            window: secondary,
            focused: true,
        });
    run_input_systems(&mut app);

    begin_frame_and_assert(&mut app, |ui| {
        #[cfg(not(target_os = "macos"))]
        assert_eq!(ui.mouse_pos(), [800.0, 1100.0]);
        #[cfg(target_os = "macos")]
        assert_eq!(ui.mouse_pos(), [400.0, 550.0]);
        assert_eq!(
            ui.io().mouse_hovered_viewport(),
            expected_hovered_viewport(viewport_id)
        );
        assert!(ui.is_mouse_down(imgui::MouseButton::Right));
        assert_eq!(ui.io().mouse_wheel_h(), -1.0);
        assert_eq!(ui.io().mouse_wheel(), 1.0);
        assert!(ui.is_key_down(imgui::Key::X));

        let chars = current_frame_input_chars();
        assert!(chars.contains(&('x' as u32)));
        assert!(chars.contains(&('界' as u32)));
    });
    fixture.destroy(&mut app);
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[test]
fn input_cursor_left_from_previous_window_does_not_clear_new_hovered_viewport_position() {
    let _guard = imgui_context_guard();
    let (mut app, primary) = app_with_primary_window_and_native_viewports();
    let viewport_id = imgui::Id::from(0x550);
    let fixture = create_native_viewport_window(
        &mut app,
        viewport_id,
        Window {
            position: WindowPosition::At(IVec2::new(200, 300)),
            resolution: WindowResolution::new(640, 480),
            ..Default::default()
        },
    );
    let secondary = fixture.window();
    configure_primary(&mut app, |context| {
        let io = context.io_mut();
        io.set_config_flags(io.config_flags() | imgui::ConfigFlags::VIEWPORTS_ENABLE);
    });

    app.world_mut()
        .resource_mut::<Messages<CursorMoved>>()
        .write(CursorMoved {
            window: primary,
            position: Vec2::new(10.0, 20.0),
            delta: None,
        });
    run_input_systems(&mut app);
    assert_eq!(
        app.world()
            .resource::<ImguiInputState>()
            .mouse_hovered_window(),
        Some(primary)
    );

    app.world_mut()
        .resource_mut::<Messages<CursorMoved>>()
        .write(CursorMoved {
            window: secondary,
            position: Vec2::new(30.0, 40.0),
            delta: None,
        });
    app.world_mut()
        .resource_mut::<Messages<CursorLeft>>()
        .write(CursorLeft { window: primary });
    run_input_systems(&mut app);

    assert_eq!(
        app.world()
            .resource::<ImguiInputState>()
            .mouse_hovered_window(),
        Some(secondary)
    );
    begin_frame_and_assert(&mut app, |ui| {
        assert_eq!(ui.mouse_pos(), [230.0, 340.0]);
        assert_eq!(
            ui.io().mouse_hovered_viewport(),
            expected_hovered_viewport(viewport_id)
        );
    });
    fixture.destroy(&mut app);
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[test]
fn input_drag_keeps_pointer_across_leave_and_releases_on_another_viewport() {
    let _guard = imgui_context_guard();
    let (mut app, primary) = app_with_primary_window_and_native_viewports();
    let viewport_id = imgui::Id::from(0x552);
    let fixture = create_native_viewport_window(
        &mut app,
        viewport_id,
        Window {
            position: WindowPosition::At(IVec2::new(200, 300)),
            resolution: WindowResolution::new(640, 480),
            ..Default::default()
        },
    );
    let secondary = fixture.window();
    #[cfg(not(target_os = "macos"))]
    let primary_drag_position = [24.0, 68.0];
    #[cfg(target_os = "macos")]
    let primary_drag_position = [12.0, 34.0];

    app.world_mut()
        .resource_mut::<Messages<CursorEntered>>()
        .write(CursorEntered { window: primary });
    app.world_mut()
        .resource_mut::<Messages<CursorMoved>>()
        .write(CursorMoved {
            window: primary,
            position: Vec2::new(12.0, 34.0),
            delta: None,
        });
    app.world_mut()
        .resource_mut::<Messages<MouseButtonInput>>()
        .write(MouseButtonInput {
            button: BevyMouseButton::Left,
            state: ButtonState::Pressed,
            window: primary,
        });
    run_input_systems(&mut app);
    begin_frame_and_assert(&mut app, |ui| {
        assert_eq!(ui.mouse_pos(), primary_drag_position);
        assert!(ui.is_mouse_down(imgui::MouseButton::Left));
    });

    app.world_mut()
        .resource_mut::<Messages<CursorLeft>>()
        .write(CursorLeft { window: primary });
    run_input_systems(&mut app);
    begin_frame_and_assert(&mut app, |ui| {
        assert_eq!(ui.mouse_pos(), primary_drag_position);
        assert!(ui.is_mouse_down(imgui::MouseButton::Left));
    });

    app.world_mut()
        .resource_mut::<Messages<CursorEntered>>()
        .write(CursorEntered { window: secondary });
    app.world_mut()
        .resource_mut::<Messages<CursorMoved>>()
        .write(CursorMoved {
            window: secondary,
            position: Vec2::new(30.0, 40.0),
            delta: None,
        });
    app.world_mut()
        .resource_mut::<Messages<MouseButtonInput>>()
        .write(MouseButtonInput {
            button: BevyMouseButton::Left,
            state: ButtonState::Released,
            window: secondary,
        });
    run_input_systems(&mut app);
    begin_frame_and_assert(&mut app, |ui| {
        assert_eq!(ui.mouse_pos(), [230.0, 340.0]);
        assert!(!ui.is_mouse_down(imgui::MouseButton::Left));
        assert_eq!(
            ui.io().mouse_hovered_viewport(),
            expected_hovered_viewport(viewport_id)
        );
    });

    {
        let state = app.world().resource::<ImguiInputState>();
        assert!(!state.routed.pointer_positions.contains_key(&primary));
        assert!(
            state
                .routed
                .pointer_targets
                .get(&primary)
                .is_none_or(Vec::is_empty)
        );
        assert!(!state.routed.pointer_outside_windows.contains(&primary));
        assert!(
            state.routed.windows.iter().all(|(slot, window_state)| {
                slot.window != primary || !window_state.mouse_hovered
            })
        );
        assert!(state.routed.windows.iter().any(|(slot, window_state)| {
            slot.window == secondary && window_state.mouse_hovered
        }));
    }

    app.world_mut()
        .resource_mut::<Messages<CursorLeft>>()
        .write(CursorLeft { window: secondary });
    run_input_systems(&mut app);
    {
        let state = app.world().resource::<ImguiInputState>();
        assert!(!state.routed.pointer_positions.contains_key(&secondary));
        assert!(
            state
                .routed
                .pointer_targets
                .get(&secondary)
                .is_none_or(Vec::is_empty)
        );
        assert!(state.routed.windows.iter().all(|(slot, window_state)| {
            slot.window != secondary || !window_state.mouse_hovered
        }));
    }

    fixture.destroy(&mut app);
    run_input_systems(&mut app);
}

#[cfg(feature = "render")]
#[test]
fn raw_pointer_dedup_preserves_nonmatching_typed_events() {
    let window = Entity::from_raw_u32(17).expect("test entity index should be valid");
    let raw_move = OrderedPointerEvent::Moved {
        window,
        position: Vec2::new(10.0, 20.0),
        #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
        native_position: Some(Vec2::new(5.0, 10.0)),
    };
    let raw_leave = OrderedPointerEvent::Left { window };
    let synthetic_button = OrderedPointerEvent::Button {
        window,
        button: BevyMouseButton::Left,
        state: ButtonState::Pressed,
    };
    let mut ordered = vec![raw_move, raw_leave];
    let mut duplicates =
        std::collections::HashMap::from([(raw_move.identity(), 1), (raw_leave.identity(), 1)]);

    append_typed_pointer_event(&mut ordered, &mut duplicates, raw_move);
    append_typed_pointer_event(&mut ordered, &mut duplicates, synthetic_button);
    append_typed_pointer_event(&mut ordered, &mut duplicates, raw_leave);

    assert!(duplicates.is_empty());
    assert_eq!(ordered, vec![raw_move, raw_leave, synthetic_button]);
}

#[cfg(all(
    feature = "render",
    feature = "multi-viewport",
    not(target_arch = "wasm32")
))]
#[test]
fn raw_pointer_order_uses_the_scale_active_at_each_event() {
    let window = Entity::from_raw_u32(23).expect("test entity index should be valid");
    let mut scale_factors = std::collections::HashMap::from([(window, 1.0)]);
    let (mut ordered, mut duplicates) = order_raw_pointer_events(
        [
            RawWindowPointerEvent::Moved {
                window,
                physical_position: Vec2::new(300.0, 120.0),
                current_native_scale_factor: 2.0,
                typed_logical_position: None,
            },
            RawWindowPointerEvent::Button {
                window,
                button: BevyMouseButton::Left,
                state: ButtonState::Pressed,
            },
            RawWindowPointerEvent::ScaleFactorChanged {
                window,
                scale_factor: 2.0,
            },
            RawWindowPointerEvent::Moved {
                window,
                physical_position: Vec2::new(300.0, 120.0),
                current_native_scale_factor: 2.0,
                typed_logical_position: None,
            },
        ],
        &mut scale_factors,
    );

    let typed_move_before_scale = OrderedPointerEvent::Moved {
        window,
        position: Vec2::new(300.0, 120.0),
        native_position: None,
    };
    let typed_move_after_scale = OrderedPointerEvent::Moved {
        window,
        position: Vec2::new(150.0, 60.0),
        native_position: None,
    };
    let typed_button = OrderedPointerEvent::Button {
        window,
        button: BevyMouseButton::Left,
        state: ButtonState::Pressed,
    };
    append_typed_pointer_event(&mut ordered, &mut duplicates, typed_move_before_scale);
    append_typed_pointer_event(&mut ordered, &mut duplicates, typed_move_after_scale);
    append_typed_pointer_event(&mut ordered, &mut duplicates, typed_button);

    assert_eq!(scale_factors.get(&window), Some(&2.0));
    assert!(duplicates.is_empty());
    assert_eq!(ordered.len(), 3);
    assert!(matches!(
        ordered[0],
        OrderedPointerEvent::Moved { position, .. }
            if position == Vec2::new(300.0, 120.0)
    ));
    assert_eq!(ordered[1], typed_button);
    assert!(matches!(
        ordered[2],
        OrderedPointerEvent::Moved { position, .. }
            if position == Vec2::new(150.0, 60.0)
    ));
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[test]
fn input_stale_hovered_viewport_window_clears_imgui_mouse_hover() {
    let _guard = imgui_context_guard();
    let (mut app, primary) = app_with_primary_window_and_native_viewports();
    let viewport_id = imgui::Id::from(0x551);
    let fixture = create_native_viewport_window(
        &mut app,
        viewport_id,
        Window {
            position: WindowPosition::At(IVec2::new(200, 300)),
            resolution: WindowResolution::new(640, 480),
            ..Default::default()
        },
    );
    let secondary = fixture.window();
    configure_primary(&mut app, |context| {
        let io = context.io_mut();
        io.set_config_flags(io.config_flags() | imgui::ConfigFlags::VIEWPORTS_ENABLE);
    });

    app.world_mut()
        .resource_mut::<Messages<CursorMoved>>()
        .write(CursorMoved {
            window: secondary,
            position: Vec2::new(30.0, 40.0),
            delta: None,
        });
    run_input_systems(&mut app);
    begin_frame_and_assert(&mut app, |ui| {
        assert_eq!(ui.mouse_pos(), [230.0, 340.0]);
        assert_eq!(
            ui.io().mouse_hovered_viewport(),
            expected_hovered_viewport(viewport_id)
        );
    });

    app.world_mut().despawn(secondary);
    run_input_systems(&mut app);

    assert_eq!(
        app.world()
            .resource::<ImguiInputState>()
            .mouse_hovered_window(),
        None
    );
    begin_frame_and_assert(&mut app, |ui| {
        assert!(
            ui.mouse_pos()[0] < -1.0e30 && ui.mouse_pos()[1] < -1.0e30,
            "destroying the hovered secondary viewport must clear the ImGui mouse position"
        );
        assert_eq!(
            ui.io().mouse_hovered_viewport(),
            imgui::Id::from(0),
            "destroying the hovered secondary viewport must clear the hovered viewport id"
        );
    });

    assert!(app.world().get::<Window>(primary).is_some());
    fixture.destroy(&mut app);
}

#[test]
fn input_cursor_entered_tracks_hovered_window_without_requiring_motion() {
    let _guard = imgui_context_guard();
    let (mut app, primary) = app_with_primary_window();

    app.world_mut()
        .resource_mut::<Messages<CursorEntered>>()
        .write(CursorEntered { window: primary });
    run_input_systems(&mut app);

    assert_eq!(
        app.world()
            .resource::<ImguiInputState>()
            .mouse_hovered_window(),
        Some(primary)
    );
}

#[test]
fn input_key_mapping_covers_modifiers_and_common_keys() {
    assert_eq!(
        map_bevy_key_code(KeyCode::ControlLeft),
        Some(imgui::Key::LeftCtrl)
    );
    assert_eq!(
        map_bevy_key_code(KeyCode::ShiftRight),
        Some(imgui::Key::RightShift)
    );
    assert_eq!(map_bevy_key_code(KeyCode::KeyZ), Some(imgui::Key::Z));
    assert_eq!(
        map_bevy_key_code(KeyCode::NumpadEnter),
        Some(imgui::Key::KeypadEnter)
    );
}

#[test]
fn input_capture_predicates_and_run_conditions_expose_imgui_policy_hints() {
    let mut app = App::new();
    app.init_resource::<ImguiInputCapture>();

    assert!(!run_condition_value(&mut app, imgui_wants_pointer_input));
    assert!(!run_condition_value(
        &mut app,
        imgui_wants_pointer_input_unless_popup_close
    ));
    assert!(!run_condition_value(&mut app, imgui_wants_keyboard_input));
    assert!(!run_condition_value(&mut app, imgui_wants_text_input));
    assert!(!run_condition_value(&mut app, imgui_wants_any_input));

    {
        let mut capture = app.world_mut().resource_mut::<ImguiInputCapture>();
        capture.set_aggregate(ImguiInputCaptureState {
            want_capture_mouse: true,
            want_capture_mouse_unless_popup_close: true,
            want_capture_keyboard: true,
            want_text_input: true,
        });
        assert!(capture.wants_pointer_input());
        assert!(capture.wants_pointer_input_unless_popup_close());
        assert!(capture.wants_keyboard_input());
        assert!(capture.wants_text_input());
        assert!(capture.wants_any_input());
    }

    assert!(run_condition_value(&mut app, imgui_wants_pointer_input));
    assert!(run_condition_value(
        &mut app,
        imgui_wants_pointer_input_unless_popup_close
    ));
    assert!(run_condition_value(&mut app, imgui_wants_keyboard_input));
    assert!(run_condition_value(&mut app, imgui_wants_text_input));
    assert!(run_condition_value(&mut app, imgui_wants_any_input));
}

#[cfg(not(feature = "render"))]
#[test]
fn primary_capture_queries_follow_legacy_primary_input_without_render_routes() {
    let _guard = imgui_context_guard();
    let (mut app, primary_window) = app_with_primary_window();
    let primary_context = app
        .world()
        .non_send::<ImguiContexts>()
        .primary_id()
        .expect("ImguiPlugin should install a primary Context");
    configure_primary(&mut app, |_| unsafe {
        let io = imgui::sys::igGetIO_Nil();
        (*io).WantCaptureMouse = true;
        (*io).WantCaptureKeyboard = true;
        (*io).WantTextInput = true;
    });

    run_input_systems(&mut app);

    let capture = app.world().resource::<ImguiInputCapture>();
    assert_eq!(capture.primary(), capture.aggregate());
    assert_eq!(capture.context(primary_context), capture.aggregate());
    assert_eq!(capture.window(primary_window), capture.aggregate());
    assert!(capture.primary_wants_pointer_input());
    assert!(capture.primary_wants_keyboard_input());
    assert!(capture.primary_wants_text_input());
    assert!(run_condition_value(
        &mut app,
        imgui_primary_wants_pointer_input
    ));
    assert!(run_condition_value(
        &mut app,
        imgui_primary_wants_keyboard_input
    ));
    assert!(run_condition_value(
        &mut app,
        imgui_primary_wants_text_input
    ));
}

#[cfg(feature = "render")]
#[test]
fn input_routes_isolate_contexts_windows_focus_and_sticky_releases() {
    let _guard = imgui_context_guard();
    let (mut app, primary_context, primary_window, secondary_window) = routed_input_app();
    let (secondary_context, _) = add_routed_input_context(&mut app);
    app.world_mut()
        .get_mut::<Window>(primary_window)
        .expect("primary Window must exist")
        .resolution
        .set_scale_factor(2.0);

    let primary_region = logical_window_region(&app, primary_window);
    let secondary_region = logical_window_region(&app, secondary_window);
    app.world_mut().spawn(ImguiInputRoute::logical(
        primary_context,
        primary_window,
        primary_region,
    ));
    app.world_mut().spawn(ImguiInputRoute::logical(
        secondary_context,
        secondary_window,
        secondary_region,
    ));
    resolve_routed_input(&mut app);

    app.world_mut()
        .resource_mut::<Messages<WindowFocused>>()
        .write(WindowFocused {
            window: primary_window,
            focused: true,
        });
    app.world_mut()
        .resource_mut::<Messages<CursorMoved>>()
        .write(CursorMoved {
            window: primary_window,
            position: Vec2::new(20.0, 30.0),
            delta: None,
        });
    app.world_mut()
        .resource_mut::<Messages<MouseButtonInput>>()
        .write(MouseButtonInput {
            button: BevyMouseButton::Left,
            state: ButtonState::Pressed,
            window: primary_window,
        });
    app.world_mut()
        .resource_mut::<Messages<KeyboardInput>>()
        .write(key_input(
            primary_window,
            KeyCode::KeyA,
            BevyKey::Character("a".into()),
            ButtonState::Pressed,
            Some("a"),
        ));
    app.world_mut()
        .resource_mut::<Messages<Ime>>()
        .write(Ime::Enabled {
            window: primary_window,
        });
    app.world_mut()
        .resource_mut::<Messages<Ime>>()
        .write(Ime::Commit {
            window: primary_window,
            value: "A".to_owned(),
        });
    run_routed_input(&mut app);

    let input_state = app.world().resource::<ImguiInputState>();
    assert!(
        input_state
            .for_context_window(primary_context, primary_window)
            .expect("primary route should own input")
            .ime_enabled
    );
    assert!(
        input_state
            .for_context_window(secondary_context, secondary_window)
            .is_none(),
        "events from one host window must not create state for another Context"
    );

    configure_context(&mut app, primary_context, |context| {
        assert_eq!(context.io().display_size(), [320.0, 240.0]);
        assert_eq!(context.io().display_framebuffer_scale(), [2.0, 2.0]);
    });
    configure_context(&mut app, secondary_context, |context| {
        assert_eq!(context.io().display_size(), [640.0, 480.0]);
        assert_eq!(context.io().display_framebuffer_scale(), [1.0, 1.0]);
    });
    begin_frame_for_context(&mut app, primary_context, |ui| {
        assert_eq!(ui.mouse_pos(), [20.0, 30.0]);
        assert!(ui.is_key_down(imgui::Key::A));
        assert!(ui.is_mouse_down(imgui::MouseButton::Left));
        assert!(current_frame_input_chars().contains(&('A' as u32)));
    });
    begin_frame_for_context(&mut app, secondary_context, |ui| {
        assert!(!ui.is_key_down(imgui::Key::A));
        assert!(!ui.is_mouse_down(imgui::MouseButton::Left));
    });

    app.world_mut()
        .resource_mut::<Messages<WindowFocused>>()
        .write(WindowFocused {
            window: primary_window,
            focused: false,
        });
    app.world_mut()
        .resource_mut::<Messages<WindowFocused>>()
        .write(WindowFocused {
            window: secondary_window,
            focused: true,
        });
    app.world_mut()
        .resource_mut::<Messages<CursorMoved>>()
        .write(CursorMoved {
            window: secondary_window,
            position: Vec2::new(50.0, 60.0),
            delta: None,
        });
    app.world_mut()
        .resource_mut::<Messages<MouseButtonInput>>()
        .write(MouseButtonInput {
            button: BevyMouseButton::Right,
            state: ButtonState::Pressed,
            window: secondary_window,
        });
    app.world_mut()
        .resource_mut::<Messages<KeyboardInput>>()
        .write(key_input(
            secondary_window,
            KeyCode::KeyB,
            BevyKey::Character("b".into()),
            ButtonState::Pressed,
            Some("b"),
        ));
    app.world_mut()
        .resource_mut::<Messages<Ime>>()
        .write(Ime::Commit {
            window: secondary_window,
            value: "界".to_owned(),
        });
    run_routed_input(&mut app);

    begin_frame_for_context(&mut app, primary_context, |ui| {
        assert!(!ui.is_key_down(imgui::Key::A));
        assert!(!ui.is_mouse_down(imgui::MouseButton::Left));
        assert!(!ui.is_key_down(imgui::Key::B));
    });
    begin_frame_for_context(&mut app, secondary_context, |ui| {
        assert_eq!(ui.mouse_pos(), [50.0, 60.0]);
        assert!(ui.is_key_down(imgui::Key::B));
        assert!(ui.is_mouse_down(imgui::MouseButton::Right));
        assert!(current_frame_input_chars().contains(&('界' as u32)));
    });

    app.world_mut().despawn(primary_window);
    run_routed_input(&mut app);
    begin_frame_for_context(&mut app, secondary_context, |ui| {
        assert!(
            ui.is_key_down(imgui::Key::B),
            "removing one routed window must not release another Context's keys"
        );
        assert!(ui.is_mouse_down(imgui::MouseButton::Right));
    });
}

#[cfg(feature = "render")]
#[test]
fn input_routes_respect_exclusive_priority_and_explicit_shared_fanout() {
    let _guard = imgui_context_guard();
    let (mut app, primary_context, primary_window, _) = routed_input_app();
    let (secondary_context, _) = add_routed_input_context(&mut app);
    let region = logical_window_region(&app, primary_window);
    app.world_mut().spawn(
        ImguiInputRoute::logical(primary_context, primary_window, region)
            .with_policy(ImguiInputPolicy::exclusive(-3)),
    );
    app.world_mut().spawn(
        ImguiInputRoute::logical(secondary_context, primary_window, region)
            .with_policy(ImguiInputPolicy::exclusive(7)),
    );
    resolve_routed_input(&mut app);

    app.world_mut()
        .resource_mut::<Messages<CursorMoved>>()
        .write(CursorMoved {
            window: primary_window,
            position: Vec2::new(100.0, 120.0),
            delta: None,
        });
    app.world_mut()
        .resource_mut::<Messages<MouseButtonInput>>()
        .write(MouseButtonInput {
            button: BevyMouseButton::Left,
            state: ButtonState::Pressed,
            window: primary_window,
        });
    app.world_mut()
        .resource_mut::<Messages<KeyboardInput>>()
        .write(key_input(
            primary_window,
            KeyCode::KeyP,
            BevyKey::Character("p".into()),
            ButtonState::Pressed,
            Some("p"),
        ));
    run_routed_input(&mut app);

    begin_frame_for_context(&mut app, primary_context, |ui| {
        assert!(!ui.is_key_down(imgui::Key::P));
        assert!(!ui.is_mouse_down(imgui::MouseButton::Left));
    });
    begin_frame_for_context(&mut app, secondary_context, |ui| {
        assert_eq!(ui.mouse_pos(), [100.0, 120.0]);
        assert!(ui.is_key_down(imgui::Key::P));
        assert!(ui.is_mouse_down(imgui::MouseButton::Left));
    });

    let (mut app, primary_context, primary_window, _) = routed_input_app();
    let (secondary_context, _) = add_routed_input_context(&mut app);
    let region = logical_window_region(&app, primary_window);
    app.world_mut().spawn(
        ImguiInputRoute::logical(primary_context, primary_window, region)
            .with_policy(ImguiInputPolicy::Shared),
    );
    app.world_mut().spawn(
        ImguiInputRoute::logical(secondary_context, primary_window, region)
            .with_policy(ImguiInputPolicy::Shared),
    );
    resolve_routed_input(&mut app);

    app.world_mut()
        .resource_mut::<Messages<CursorMoved>>()
        .write(CursorMoved {
            window: primary_window,
            position: Vec2::new(80.0, 90.0),
            delta: None,
        });
    app.world_mut()
        .resource_mut::<Messages<MouseButtonInput>>()
        .write(MouseButtonInput {
            button: BevyMouseButton::Middle,
            state: ButtonState::Pressed,
            window: primary_window,
        });
    app.world_mut()
        .resource_mut::<Messages<KeyboardInput>>()
        .write(key_input(
            primary_window,
            KeyCode::KeyS,
            BevyKey::Character("s".into()),
            ButtonState::Pressed,
            Some("s"),
        ));
    run_routed_input(&mut app);

    for context_id in [primary_context, secondary_context] {
        begin_frame_for_context(&mut app, context_id, |ui| {
            assert_eq!(ui.mouse_pos(), [80.0, 90.0]);
            assert!(ui.is_key_down(imgui::Key::S));
            assert!(ui.is_mouse_down(imgui::MouseButton::Middle));
        });
    }
}

#[cfg(feature = "render")]
#[test]
fn adjacent_exclusive_input_regions_assign_the_shared_edge_once() {
    let _guard = imgui_context_guard();
    let (mut app, primary_context, primary_window, _) = routed_input_app();
    let (secondary_context, _) = add_routed_input_context(&mut app);
    app.world_mut().spawn(ImguiInputRoute::logical(
        primary_context,
        primary_window,
        bevy_math::Rect::from_corners(Vec2::ZERO, Vec2::new(320.0, 480.0)),
    ));
    app.world_mut().spawn(ImguiInputRoute::logical(
        secondary_context,
        primary_window,
        bevy_math::Rect::from_corners(Vec2::new(320.0, 0.0), Vec2::new(640.0, 480.0)),
    ));
    resolve_routed_input(&mut app);
    app.world_mut()
        .resource_mut::<Messages<CursorMoved>>()
        .write(CursorMoved {
            window: primary_window,
            position: Vec2::new(320.0, 120.0),
            delta: None,
        });
    app.world_mut()
        .resource_mut::<Messages<MouseButtonInput>>()
        .write(MouseButtonInput {
            button: BevyMouseButton::Left,
            state: ButtonState::Pressed,
            window: primary_window,
        });

    run_routed_input(&mut app);

    begin_frame_for_context(&mut app, primary_context, |ui| {
        assert!(
            !ui.is_mouse_down(imgui::MouseButton::Left),
            "the maximum edge is exclusive for the left region"
        );
    });
    begin_frame_for_context(&mut app, secondary_context, |ui| {
        assert!(
            ui.is_mouse_down(imgui::MouseButton::Left),
            "the minimum edge is inclusive for the right region"
        );
    });
}

#[cfg(all(
    feature = "render",
    feature = "multi-viewport",
    not(target_arch = "wasm32")
))]
#[test]
fn raw_pointer_before_same_batch_dpi_change_keeps_the_button_on_its_original_route() {
    let _guard = imgui_context_guard();
    let (mut app, primary_context, primary_window, _) = routed_input_app();
    let (secondary_context, _) = add_routed_input_context(&mut app);
    app.world_mut()
        .get_mut::<Window>(primary_window)
        .expect("primary Window must exist")
        .resolution
        .set_scale_factor(2.0);
    app.world_mut().spawn(ImguiInputRoute::logical(
        primary_context,
        primary_window,
        bevy_math::Rect::from_corners(Vec2::ZERO, Vec2::new(200.0, 480.0)),
    ));
    app.world_mut().spawn(ImguiInputRoute::logical(
        secondary_context,
        primary_window,
        bevy_math::Rect::from_corners(Vec2::new(200.0, 0.0), Vec2::new(640.0, 480.0)),
    ));
    resolve_routed_input(&mut app);

    let mapping = TestWinitWindowMapping::install(primary_window);
    let device_id = DeviceId::dummy();
    {
        let mut raw_events = app
            .world_mut()
            .resource_mut::<Messages<RawWinitWindowEvent>>();
        raw_events.write(RawWinitWindowEvent {
            window_id: mapping.window_id,
            event: WindowEvent::CursorMoved {
                device_id,
                position: PhysicalPosition::new(300.0, 120.0),
            },
        });
        raw_events.write(RawWinitWindowEvent {
            window_id: mapping.window_id,
            event: WindowEvent::MouseInput {
                device_id,
                state: ElementState::Pressed,
                button: WinitMouseButton::Left,
            },
        });
    }
    app.world_mut()
        .resource_mut::<Messages<CursorMoved>>()
        .write(CursorMoved {
            window: primary_window,
            position: Vec2::new(300.0, 120.0),
            delta: None,
        });
    app.world_mut()
        .resource_mut::<Messages<MouseButtonInput>>()
        .write(MouseButtonInput {
            button: BevyMouseButton::Left,
            state: ButtonState::Pressed,
            window: primary_window,
        });

    run_routed_input(&mut app);

    assert_eq!(
        app.world()
            .resource::<ImguiInputState>()
            .routed
            .pointer_positions
            .get(&primary_window),
        Some(&Vec2::new(300.0, 120.0)),
        "the raw cursor move must use the scale that was active before the DPI change"
    );
    begin_frame_for_context(&mut app, primary_context, |ui| {
        assert!(
            !ui.is_mouse_down(imgui::MouseButton::Left),
            "the button must not stick to the route selected by the later DPI scale"
        );
    });
    begin_frame_for_context(&mut app, secondary_context, |ui| {
        assert!(
            ui.is_mouse_down(imgui::MouseButton::Left),
            "the button must follow the raw cursor position that preceded it"
        );
    });
    assert_eq!(
        app.world()
            .resource::<ImguiInputState>()
            .routed
            .raw_window_scale_factors
            .get(&primary_window),
        Some(&2.0),
        "the next raw batch must start from the Window's final effective scale"
    );
}

#[cfg(feature = "render")]
#[test]
fn input_routes_recompute_a_stationary_pointer_before_button_dispatch() {
    let _guard = imgui_context_guard();
    let (mut app, primary_context, primary_window, _) = routed_input_app();
    let (secondary_context, _) = add_routed_input_context(&mut app);
    let region = logical_window_region(&app, primary_window);
    let primary_route = app
        .world_mut()
        .spawn(
            ImguiInputRoute::logical(primary_context, primary_window, region)
                .with_policy(ImguiInputPolicy::exclusive(0)),
        )
        .id();
    let secondary_route = app
        .world_mut()
        .spawn(
            ImguiInputRoute::logical(secondary_context, primary_window, region)
                .with_policy(ImguiInputPolicy::exclusive(-1)),
        )
        .id();
    resolve_routed_input(&mut app);

    app.world_mut()
        .resource_mut::<Messages<CursorMoved>>()
        .write(CursorMoved {
            window: primary_window,
            position: Vec2::new(100.0, 120.0),
            delta: None,
        });
    run_routed_input(&mut app);
    begin_frame_for_context(&mut app, primary_context, |ui| {
        assert_eq!(ui.mouse_pos(), [100.0, 120.0]);
    });

    app.world_mut().entity_mut(primary_route).insert(
        ImguiInputRoute::logical(primary_context, primary_window, region)
            .with_policy(ImguiInputPolicy::exclusive(-2)),
    );
    app.world_mut().entity_mut(secondary_route).insert(
        ImguiInputRoute::logical(secondary_context, primary_window, region)
            .with_policy(ImguiInputPolicy::exclusive(1)),
    );
    resolve_routed_input(&mut app);

    app.world_mut()
        .resource_mut::<Messages<MouseButtonInput>>()
        .write(MouseButtonInput {
            button: BevyMouseButton::Left,
            state: ButtonState::Pressed,
            window: primary_window,
        });
    run_routed_input(&mut app);

    begin_frame_for_context(&mut app, primary_context, |ui| {
        assert!(
            !ui.is_mouse_down(imgui::MouseButton::Left),
            "a cached pointer target must not bypass updated exclusive priority"
        );
    });
    begin_frame_for_context(&mut app, secondary_context, |ui| {
        assert_eq!(ui.mouse_pos(), [100.0, 120.0]);
        assert!(
            ui.is_mouse_down(imgui::MouseButton::Left),
            "the newly selected route must receive a stationary pointer click"
        );
    });
}

#[cfg(feature = "render")]
#[test]
fn camera_viewport_input_routes_partition_one_window_in_physical_regions() {
    let _guard = imgui_context_guard();
    let (mut app, primary_context, primary_window, _) = routed_input_app();
    let (secondary_context, _) = add_routed_input_context(&mut app);
    let left_camera = app
        .world_mut()
        .spawn((
            Camera {
                viewport: Some(Viewport {
                    physical_position: UVec2::ZERO,
                    physical_size: UVec2::new(320, 480),
                    ..Default::default()
                }),
                ..Default::default()
            },
            RenderTarget::Window(WindowRef::Entity(primary_window)),
            CameraRenderGraph::new(Core2d),
        ))
        .id();
    let right_camera = app
        .world_mut()
        .spawn((
            Camera {
                viewport: Some(Viewport {
                    physical_position: UVec2::new(320, 0),
                    physical_size: UVec2::new(320, 480),
                    ..Default::default()
                }),
                ..Default::default()
            },
            RenderTarget::Window(WindowRef::Entity(primary_window)),
            CameraRenderGraph::new(Core2d),
        ))
        .id();
    app.world_mut()
        .spawn(ImguiRenderRoute::new(primary_context, left_camera));
    app.world_mut()
        .spawn(ImguiRenderRoute::new(secondary_context, right_camera));
    app.world_mut()
        .spawn(ImguiInputRoute::from_camera(primary_context, left_camera));
    app.world_mut().spawn(
        ImguiInputRoute::from_camera(secondary_context, right_camera)
            .with_policy(ImguiInputPolicy::exclusive(1)),
    );
    resolve_routed_input(&mut app);

    app.world_mut()
        .resource_mut::<Messages<CursorMoved>>()
        .write(CursorMoved {
            window: primary_window,
            position: Vec2::new(100.0, 140.0),
            delta: None,
        });
    app.world_mut()
        .resource_mut::<Messages<MouseButtonInput>>()
        .write(MouseButtonInput {
            button: BevyMouseButton::Left,
            state: ButtonState::Pressed,
            window: primary_window,
        });
    app.world_mut()
        .resource_mut::<Messages<KeyboardInput>>()
        .write(key_input(
            primary_window,
            KeyCode::KeyL,
            BevyKey::Character("l".into()),
            ButtonState::Pressed,
            Some("l"),
        ));
    run_routed_input(&mut app);
    begin_frame_for_context(&mut app, primary_context, |ui| {
        assert_eq!(ui.mouse_pos(), [100.0, 140.0]);
        assert!(ui.is_key_down(imgui::Key::L));
        assert!(ui.is_mouse_down(imgui::MouseButton::Left));
    });
    begin_frame_for_context(&mut app, secondary_context, |ui| {
        assert!(!ui.is_key_down(imgui::Key::L));
        assert!(!ui.is_mouse_down(imgui::MouseButton::Left));
    });

    app.world_mut()
        .resource_mut::<Messages<CursorMoved>>()
        .write(CursorMoved {
            window: primary_window,
            position: Vec2::new(420.0, 140.0),
            delta: None,
        });
    app.world_mut()
        .resource_mut::<Messages<MouseButtonInput>>()
        .write(MouseButtonInput {
            button: BevyMouseButton::Right,
            state: ButtonState::Pressed,
            window: primary_window,
        });
    app.world_mut()
        .resource_mut::<Messages<KeyboardInput>>()
        .write(key_input(
            primary_window,
            KeyCode::KeyR,
            BevyKey::Character("r".into()),
            ButtonState::Pressed,
            Some("r"),
        ));
    run_routed_input(&mut app);
    begin_frame_for_context(&mut app, primary_context, |ui| {
        assert!(!ui.is_key_down(imgui::Key::L));
        assert!(!ui.is_mouse_down(imgui::MouseButton::Left));
        assert!(!ui.is_key_down(imgui::Key::R));
    });
    begin_frame_for_context(&mut app, secondary_context, |ui| {
        assert_eq!(ui.mouse_pos(), [100.0, 140.0]);
        assert!(ui.is_key_down(imgui::Key::R));
        assert!(ui.is_mouse_down(imgui::MouseButton::Right));
    });
}

#[cfg(feature = "render")]
#[test]
fn logical_image_input_maps_only_the_declared_host_region_and_drives_frame_metrics() {
    let _guard = imgui_context_guard();
    let (mut app, primary_context, primary_window, _) = routed_input_app();
    let image = Image::new_fill(
        Extent3d {
            width: 200,
            height: 100,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD,
    );
    let image = app.world_mut().resource_mut::<Assets<Image>>().add(image);
    let camera = app
        .world_mut()
        .spawn((
            Camera {
                is_active: true,
                ..Default::default()
            },
            RenderTarget::Image(image.into()),
            CameraRenderGraph::new(Core2d),
        ))
        .id();
    app.world_mut()
        .spawn(ImguiRenderRoute::new(primary_context, camera));
    app.world_mut().spawn(ImguiInputRoute::logical(
        primary_context,
        primary_window,
        bevy_math::Rect::from_corners(Vec2::new(100.0, 200.0), Vec2::new(500.0, 400.0)),
    ));
    resolve_routed_input(&mut app);

    app.world_mut()
        .resource_mut::<Messages<CursorMoved>>()
        .write(CursorMoved {
            window: primary_window,
            position: Vec2::new(300.0, 300.0),
            delta: None,
        });
    run_routed_input(&mut app);
    configure_context(&mut app, primary_context, |context| {
        assert_eq!(context.io().display_size(), [200.0, 100.0]);
        assert_eq!(context.io().display_framebuffer_scale(), [1.0, 1.0]);
    });
    begin_frame_for_context(&mut app, primary_context, |ui| {
        assert_eq!(ui.mouse_pos(), [100.0, 50.0]);
    });

    app.world_mut()
        .resource_mut::<Messages<CursorMoved>>()
        .write(CursorMoved {
            window: primary_window,
            position: Vec2::new(550.0, 300.0),
            delta: None,
        });
    run_routed_input(&mut app);
    begin_frame_for_context(&mut app, primary_context, |ui| {
        assert!(
            ui.mouse_pos()[0] < -1.0e30 && ui.mouse_pos()[1] < -1.0e30,
            "offscreen Contexts must not infer a host-window coordinate outside their logical source"
        );
    });

    app.update();
    configure_context(&mut app, primary_context, |context| {
        assert_eq!(
            context.io().display_size(),
            [200.0, 100.0],
            "the frame driver must preserve routed image dimensions"
        );
    });
}

#[cfg(feature = "render")]
#[test]
fn scoped_capture_queries_update_before_update_and_clear_when_a_window_disappears() {
    let _guard = imgui_context_guard();
    let (mut app, primary_context, primary_window, secondary_window) = routed_input_app();
    let (secondary_context, _) = add_routed_input_context(&mut app);
    let primary_region = logical_window_region(&app, primary_window);
    let secondary_region = logical_window_region(&app, secondary_window);
    app.world_mut().spawn(ImguiInputRoute::logical(
        primary_context,
        primary_window,
        primary_region,
    ));
    app.world_mut().spawn(ImguiInputRoute::logical(
        secondary_context,
        secondary_window,
        secondary_region,
    ));
    resolve_routed_input(&mut app);
    app.init_resource::<ScopedCaptureRunCount>().add_systems(
        Update,
        count_scoped_capture.run_if(imgui_context_wants_keyboard_input(primary_context)),
    );

    configure_context(&mut app, primary_context, |_| unsafe {
        let io = imgui::sys::igGetIO_Nil();
        (*io).WantCaptureKeyboard = true;
    });
    configure_context(&mut app, secondary_context, |_| unsafe {
        let io = imgui::sys::igGetIO_Nil();
        (*io).WantCaptureMouse = true;
    });
    assert!(!run_condition_once(
        &mut app,
        imgui_context_wants_keyboard_input(primary_context)
    ));
    assert!(!run_condition_once(
        &mut app,
        imgui_window_wants_pointer_input(secondary_window)
    ));

    run_routed_input(&mut app);
    {
        let capture = app.world().resource::<ImguiInputCapture>();
        assert!(capture.wants_keyboard_input_for_context(primary_context));
        assert!(!capture.wants_pointer_input_for_context(primary_context));
        assert!(capture.wants_pointer_input_for_context(secondary_context));
        assert!(capture.wants_pointer_input_for_window(secondary_window));
        assert!(capture.wants_keyboard_input());
        assert!(capture.wants_pointer_input());
    }
    assert!(run_condition_once(
        &mut app,
        imgui_context_wants_keyboard_input(primary_context)
    ));
    assert!(run_condition_once(
        &mut app,
        imgui_context_wants_pointer_input(secondary_context)
    ));
    assert!(run_condition_once(
        &mut app,
        imgui_window_wants_pointer_input(secondary_window)
    ));
    app.update();
    assert_eq!(
        app.world().resource::<ScopedCaptureRunCount>().0,
        1,
        "scoped capture must be available before normal Update systems run"
    );

    app.world_mut().despawn(secondary_window);
    run_routed_input(&mut app);
    let capture = app.world().resource::<ImguiInputCapture>();
    assert!(capture.for_context(primary_context).is_some());
    assert!(capture.for_context(secondary_context).is_none());
    assert!(capture.for_window(secondary_window).is_none());
}

#[cfg(feature = "render")]
#[test]
fn capture_decision_remains_stable_for_the_input_batch_that_update_consumes() {
    let _guard = imgui_context_guard();
    let (mut app, primary_context, primary_window, _) = routed_input_app();
    let region = logical_window_region(&app, primary_window);
    app.world_mut().spawn(ImguiInputRoute::logical(
        primary_context,
        primary_window,
        region,
    ));
    resolve_routed_input(&mut app);

    let primary_pass = app.imgui_primary_pass();
    app.add_imgui_systems(
        &primary_pass,
        primary_pass.system(request_pointer_capture_next_frame),
    );
    app.init_resource::<ScopedCaptureRunCount>().add_systems(
        Update,
        count_scoped_capture.run_if(imgui_context_wants_pointer_input(primary_context)),
    );

    app.update();
    app.update();
    assert_eq!(
        app.world().resource::<ScopedCaptureRunCount>().0,
        0,
        "a capture override applied by NewFrame must not rewrite the decision for its input batch"
    );

    app.update();
    assert_eq!(
        app.world().resource::<ScopedCaptureRunCount>().0,
        1,
        "the next PreUpdate must sample the capture state produced by the preceding frame"
    );
}
