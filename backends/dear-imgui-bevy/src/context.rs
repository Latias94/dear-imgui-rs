//! Main-world Dear ImGui context lifecycle for Bevy.
//!
//! This module bridges Bevy's schedule model to Dear ImGui's immediate-mode frame model. User
//! systems should be added to [`crate::ImguiPrimaryContextPass`] and request [`ImguiContexts`]
//! instead of calling `Context::frame()` / `Context::render()` directly.

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use crate::ImguiViewportBridge;
use crate::{
    ImguiBackendStatus, ImguiContext, ImguiViewportWindow,
    input::{
        ImguiInputState, map_imgui_mouse_cursor, sanitized_window_display_size,
        sanitized_window_framebuffer_scale,
    },
};
use bevy_app::App;
use bevy_ecs::prelude::*;
use bevy_ecs::system::{NonSendMarker, SystemParam};
use bevy_math::Vec2;
use bevy_time::{Real, Time};
use bevy_window::{CursorIcon, CursorOptions, PrimaryWindow, Window, WindowPosition};
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use bevy_window::{Monitor, PrimaryMonitor};
use dear_imgui_rs as imgui;
use std::ptr::NonNull;
#[cfg(feature = "render")]
use std::sync::{Arc, Mutex, MutexGuard};

type PrimaryInputWindowQuery<'w, 's> =
    Query<'w, 's, (Entity, &'static Window), With<PrimaryWindow>>;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
type ViewportInputWindowQuery<'w, 's> =
    Query<'w, 's, (Entity, &'static Window, &'static ImguiViewportWindow), Without<PrimaryWindow>>;
type PrimaryFeedbackWindowQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static mut Window,
        &'static mut CursorOptions,
        Option<&'static mut CursorIcon>,
    ),
    With<PrimaryWindow>,
>;
type ViewportFeedbackWindowQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static mut Window,
        &'static mut CursorOptions,
        Option<&'static mut CursorIcon>,
        &'static ImguiViewportWindow,
    ),
    Without<PrimaryWindow>,
>;

#[derive(SystemParam)]
struct BeginFrameParams<'w, 's> {
    primary_window: PrimaryInputWindowQuery<'w, 's>,
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    viewport_windows: ViewportInputWindowQuery<'w, 's>,
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    monitors: Query<'w, 's, (&'static Monitor, Option<&'static PrimaryMonitor>)>,
    imgui_context: NonSendMut<'w, ImguiContext>,
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    viewport_bridge: Option<NonSendMut<'w, ImguiViewportBridge>>,
    frame_state: NonSendMut<'w, ImguiFrameState>,
    output: ResMut<'w, ImguiFrameOutput>,
    #[cfg(feature = "render")]
    snapshot_mailbox: Res<'w, ImguiFrameMailbox>,
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    backend_status: Res<'w, ImguiBackendStatus>,
    real_time: Option<Res<'w, Time<Real>>>,
}

/// Output produced by the last completed Dear ImGui frame.
#[derive(Resource, Debug, Default)]
pub struct ImguiFrameOutput {
    frame_index: u64,
    snapshot_epoch: Option<imgui::render::snapshot::SnapshotEpoch>,
    snapshot_error: Option<String>,
}

impl ImguiFrameOutput {
    /// Monotonic primary-context frame index for the latest completed frame.
    #[must_use]
    pub fn frame_index(&self) -> u64 {
        self.frame_index
    }

    /// Epoch of the latest snapshot handed to the render world.
    #[must_use]
    pub fn snapshot_epoch(&self) -> Option<imgui::render::snapshot::SnapshotEpoch> {
        self.snapshot_epoch
    }

    /// Snapshot error produced by the latest completed frame, if snapshotting failed.
    #[must_use]
    pub fn snapshot_error(&self) -> Option<&str> {
        self.snapshot_error.as_deref()
    }

    #[cfg(feature = "render")]
    fn set_snapshot(
        &mut self,
        mailbox: &ImguiFrameMailbox,
        frame_index: u64,
        snapshot: Result<
            imgui::render::snapshot::FrameSnapshot,
            imgui::render::snapshot::SnapshotError,
        >,
    ) {
        self.frame_index = frame_index;
        match snapshot {
            Ok(snapshot) => {
                self.snapshot_epoch = Some(snapshot.epoch());
                mailbox.publish(frame_index, snapshot);
                self.snapshot_error = None;
            }
            Err(err) => {
                self.snapshot_epoch = None;
                mailbox.clear();
                self.snapshot_error = Some(err.to_string());
            }
        }
    }

    fn clear_snapshot(&mut self, frame_index: u64) {
        self.frame_index = frame_index;
        self.snapshot_epoch = None;
        self.snapshot_error = None;
    }
}

#[cfg(feature = "render")]
#[derive(Resource, Clone, Debug, Default)]
pub(crate) struct ImguiFrameMailbox {
    pending: Arc<Mutex<Option<(u64, imgui::render::snapshot::FrameSnapshot)>>>,
}

#[cfg(feature = "render")]
impl ImguiFrameMailbox {
    fn pending(&self) -> MutexGuard<'_, Option<(u64, imgui::render::snapshot::FrameSnapshot)>> {
        self.pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn publish(&self, frame_index: u64, snapshot: imgui::render::snapshot::FrameSnapshot) {
        let previous = self.pending().replace((frame_index, snapshot));
        drop(previous);
    }

    pub(crate) fn take(&self) -> Option<(u64, imgui::render::snapshot::FrameSnapshot)> {
        self.pending().take()
    }

    fn clear(&self) {
        let previous = self.pending().take();
        drop(previous);
    }
}

/// Non-send state for the currently open Bevy-managed Dear ImGui frame.
///
/// The raw pointer stored here is valid only between `ImguiBeginFrame` and `ImguiEndFrame`. It
/// points to the `Ui` owned by the non-send [`ImguiContext`] resource and is never sent to another
/// thread.
#[derive(Default)]
pub struct ImguiFrameState {
    frame_index: u64,
    ui: Option<NonNull<imgui::Ui>>,
}

impl ImguiFrameState {
    /// Current or most recently opened frame index.
    #[must_use]
    pub fn frame_index(&self) -> u64 {
        self.frame_index
    }

    /// Whether the Bevy-managed Dear ImGui frame is currently open.
    #[must_use]
    pub fn is_frame_open(&self) -> bool {
        self.ui.is_some()
    }

    /// Borrow the currently open `Ui`, if called inside `ImguiPrimaryContextPass`.
    #[must_use]
    pub fn ui(&self) -> Option<&imgui::Ui> {
        let ui = self.ui?;
        // SAFETY: `ui` is set from the live `ImguiContext` in `begin_primary_frame_system` and
        // cleared by `end_primary_frame_system` before the context renders/closes the frame.
        Some(unsafe { ui.as_ref() })
    }

    fn begin(&mut self, ui: &imgui::Ui) {
        self.frame_index = self.frame_index.saturating_add(1);
        self.ui = Some(NonNull::from(ui));
    }

    fn end(&mut self) -> u64 {
        self.ui = None;
        self.frame_index
    }
}

/// Bevy system param used by user UI systems in [`crate::ImguiPrimaryContextPass`].
#[derive(SystemParam)]
pub struct ImguiContexts<'w> {
    frame_state: NonSend<'w, ImguiFrameState>,
    _main_thread: NonSendMarker,
}

impl<'w> ImguiContexts<'w> {
    /// Borrow the primary Dear ImGui `Ui` for the current frame.
    #[must_use]
    pub fn primary_ui_mut(&mut self) -> Option<&imgui::Ui> {
        self.frame_state.ui()
    }

    /// Current primary-context frame index, if a frame is open.
    #[must_use]
    pub fn frame_index(&self) -> Option<u64> {
        self.frame_state
            .is_frame_open()
            .then_some(self.frame_state.frame_index())
    }
}

pub(crate) fn install_context_lifecycle(app: &mut App) {
    app.init_non_send::<ImguiFrameState>()
        .init_resource::<ImguiFrameOutput>()
        .add_systems(crate::ImguiBeginFrame, begin_primary_frame_system)
        .add_systems(crate::ImguiEndFrame, end_primary_frame_system);
    #[cfg(feature = "render")]
    app.init_resource::<ImguiFrameMailbox>();
}

#[cfg_attr(
    not(all(feature = "multi-viewport", not(target_arch = "wasm32"))),
    allow(unused_variables)
)]
fn begin_primary_frame_system(mut params: BeginFrameParams) {
    if params.frame_state.is_frame_open() {
        return;
    }

    let Ok((primary_window_entity, window)) = params.primary_window.single() else {
        let _ = params
            .imgui_context
            .context_mut()
            .poll_snapshot_completions();
        #[cfg(feature = "render")]
        params.snapshot_mailbox.clear();
        params
            .output
            .clear_snapshot(params.frame_state.frame_index());
        return;
    };

    let context = params.imgui_context.context_mut();

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    if let Some(viewport_bridge) = params.viewport_bridge.as_deref_mut() {
        let viewport_feedback = params
            .viewport_windows
            .iter()
            .map(|(entity, window, viewport_window)| {
                (
                    entity,
                    viewport_window.viewport_id,
                    crate::viewport::viewport_feedback_from_window(
                        entity,
                        window,
                        viewport_bridge.viewport_feedback(viewport_window.viewport_id),
                    ),
                )
            })
            .collect::<Vec<_>>();
        let monitors = crate::viewport::platform_monitors_from_bevy_monitors(
            params
                .monitors
                .iter()
                .map(|(monitor, primary)| (monitor.clone(), primary.is_some())),
        );
        crate::viewport::prepare_platform_viewports_for_frame(
            context,
            viewport_bridge,
            primary_window_entity,
            window,
            &monitors,
            viewport_feedback.into_iter(),
            params.backend_status.multi_viewport_supported,
        );
    }

    context.prepare_frame(
        imgui::FramePrepareOptions::new(
            sanitized_window_display_size(window),
            imgui_delta_time(context, params.real_time.as_deref()),
        )
        .framebuffer_scale(sanitized_window_framebuffer_scale(window)),
    );

    let ui = context.frame();
    params.frame_state.begin(ui);
}

fn imgui_delta_time(context: &imgui::Context, real_time: Option<&Time<Real>>) -> f32 {
    real_time
        .map(Time::delta_secs)
        .unwrap_or_else(|| context.io().delta_time())
        .max(f32::EPSILON)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn end_primary_frame_system(
    mut commands: Commands,
    mut imgui_context: NonSendMut<ImguiContext>,
    mut frame_state: NonSendMut<ImguiFrameState>,
    _backend_status: Res<ImguiBackendStatus>,
    input_state: Res<ImguiInputState>,
    mut primary_window: PrimaryFeedbackWindowQuery,
    mut viewport_windows: ViewportFeedbackWindowQuery,
    mut output: ResMut<ImguiFrameOutput>,
    #[cfg(feature = "render")] snapshot_mailbox: Res<ImguiFrameMailbox>,
) {
    if !frame_state.is_frame_open() {
        return;
    }

    if let Some(ui) = frame_state.ui() {
        sync_primary_window_platform_feedback(
            ui,
            &imgui_context,
            &input_state,
            &mut commands,
            &mut primary_window,
            &mut viewport_windows,
        );
    }

    let frame_index = frame_state.end();
    #[cfg(feature = "render")]
    {
        let snapshot =
            imgui_context.render_frame_snapshot(_backend_status.multi_viewport_supported);
        output.set_snapshot(&snapshot_mailbox, frame_index, snapshot);
    }
    #[cfg(not(feature = "render"))]
    {
        let _ = imgui_context.context_mut().render();
        output.clear_snapshot(frame_index);
    }
}

fn sync_primary_window_platform_feedback(
    ui: &imgui::Ui,
    imgui_context: &ImguiContext,
    input_state: &ImguiInputState,
    commands: &mut Commands,
    primary_window: &mut PrimaryFeedbackWindowQuery,
    viewport_windows: &mut ViewportFeedbackWindowQuery,
) {
    let Ok((
        primary_entity,
        mut primary_window,
        mut primary_cursor_options,
        mut primary_cursor_icon,
    )) = primary_window.single_mut()
    else {
        return;
    };

    let raw_context = imgui_context.context().as_raw();
    // SAFETY: `ImguiContext` owns this live Dear ImGui context for the lifetime of the Bevy
    // resource, and the frame is still open while platform feedback is synchronized.
    let ime_data = unsafe { &(*raw_context).PlatformImeData };
    let ime_target_viewport = (ime_data.ViewportId != 0).then_some(ime_data.ViewportId);
    let ime_position = [ime_data.InputPos.x, ime_data.InputPos.y];
    let hovered_window = input_state.mouse_hovered_window();
    let cursor_target = hovered_window.unwrap_or(primary_entity);

    let mut cursor_applied = false;
    if cursor_target == primary_entity {
        apply_window_cursor_feedback(
            ui,
            commands,
            primary_entity,
            &mut primary_cursor_options,
            primary_cursor_icon.take(),
        );
        cursor_applied = true;
    } else {
        clear_window_cursor_feedback(
            commands,
            primary_entity,
            &mut primary_cursor_options,
            primary_cursor_icon.take(),
        );
    }

    let mut ime_applied = false;
    if ime_target_viewport.is_none() {
        apply_window_ime_feedback(
            primary_entity,
            &mut primary_window,
            ime_data.WantTextInput,
            ime_position,
            true,
        );
        ime_applied = true;
    } else {
        primary_window.ime_enabled = false;
    }

    for (window_entity, mut window, mut cursor_options, cursor_icon, viewport_window) in
        viewport_windows.iter_mut()
    {
        if cursor_target == window_entity {
            apply_window_cursor_feedback(
                ui,
                commands,
                window_entity,
                &mut cursor_options,
                cursor_icon,
            );
            cursor_applied = true;
        } else {
            clear_window_cursor_feedback(commands, window_entity, &mut cursor_options, cursor_icon);
        }

        if ime_target_viewport == Some(viewport_window.viewport_id.raw()) {
            apply_window_ime_feedback(
                window_entity,
                &mut window,
                ime_data.WantTextInput,
                ime_position,
                false,
            );
            ime_applied = true;
        } else {
            window.ime_enabled = false;
        }
    }

    if !cursor_applied {
        apply_window_cursor_feedback(
            ui,
            commands,
            primary_entity,
            &mut primary_cursor_options,
            primary_cursor_icon.take(),
        );
    }

    if !ime_applied {
        apply_window_ime_feedback(
            primary_entity,
            &mut primary_window,
            ime_data.WantTextInput,
            ime_position,
            true,
        );
    }
}

fn apply_window_ime_feedback(
    entity: Entity,
    window: &mut Window,
    want_text_input: bool,
    ime_position: [f32; 2],
    is_primary: bool,
) {
    window.ime_enabled = want_text_input;
    window.ime_position = ime_position_for_window(entity, window, ime_position, is_primary);
}

fn ime_position_for_window(
    _entity: Entity,
    window: &Window,
    ime_position: [f32; 2],
    is_primary: bool,
) -> Vec2 {
    let mut position = Vec2::new(ime_position[0], ime_position[1]);
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    {
        if let Some(origin) = crate::viewport::window_client_origin_logical(
            _entity,
            &window.position,
            window.scale_factor(),
        ) {
            position.x -= origin[0];
            position.y -= origin[1];
            return position;
        }
    }

    if !is_primary && let WindowPosition::At(window_position) = window.position {
        let scale_factor = sanitized_window_framebuffer_scale(window)[0];
        position.x -= window_position.x as f32 / scale_factor;
        position.y -= window_position.y as f32 / scale_factor;
    }
    position
}

fn apply_window_cursor_feedback(
    ui: &imgui::Ui,
    commands: &mut Commands,
    window_entity: Entity,
    cursor_options: &mut CursorOptions,
    cursor_icon: Option<Mut<CursorIcon>>,
) {
    let mouse_cursor = ui.mouse_cursor();
    let draw_cursor = ui.io().mouse_draw_cursor();
    let hide_os_cursor = draw_cursor || mouse_cursor.is_none();
    cursor_options.visible = !hide_os_cursor;

    let has_cursor_icon = cursor_icon.is_some();
    if hide_os_cursor {
        if has_cursor_icon {
            commands.entity(window_entity).remove::<CursorIcon>();
        }
    } else if let Some(mouse_cursor) = mouse_cursor
        && let Some(cursor_icon_value) = map_imgui_mouse_cursor(mouse_cursor)
    {
        match cursor_icon {
            Some(mut current_cursor_icon) => {
                *current_cursor_icon = cursor_icon_value;
            }
            None => {
                commands.entity(window_entity).insert(cursor_icon_value);
            }
        }
    }
}

fn clear_window_cursor_feedback(
    commands: &mut Commands,
    window_entity: Entity,
    cursor_options: &mut CursorOptions,
    cursor_icon: Option<Mut<CursorIcon>>,
) {
    cursor_options.visible = true;
    if cursor_icon.is_some() {
        commands.entity(window_entity).remove::<CursorIcon>();
    }
}

#[cfg(all(test, feature = "render"))]
mod tests {
    use super::*;

    fn context_and_consumer() -> (imgui::Context, imgui::render::RendererConsumer) {
        let mut context = imgui::Context::create();
        context.io_mut().set_config_input_trickle_event_queue(false);
        context.prepare_frame(
            imgui::FramePrepareOptions::new([64.0, 64.0], 1.0 / 60.0).renderer_has_textures(),
        );
        let _ = context.font_atlas().build();
        let _ = context.set_ini_filename::<std::path::PathBuf>(None);
        let consumer = context.create_renderer_consumer().unwrap();
        (context, consumer)
    }

    fn snapshot(
        context: &mut imgui::Context,
        consumer: &imgui::render::RendererConsumer,
    ) -> imgui::render::FrameSnapshot {
        context.prepare_frame(
            imgui::FramePrepareOptions::new([64.0, 64.0], 1.0 / 60.0).renderer_has_textures(),
        );
        context.begin_frame().render_snapshot(consumer).unwrap()
    }

    fn texture_snapshot(
        context: &mut imgui::Context,
        consumer: &imgui::render::RendererConsumer,
        texture: imgui::ManagedTextureId,
    ) -> imgui::render::FrameSnapshot {
        context.prepare_frame(
            imgui::FramePrepareOptions::new([64.0, 64.0], 1.0 / 60.0).renderer_has_textures(),
        );
        let frame = context.begin_frame();
        frame.ui().image(texture, [16.0, 16.0]);
        frame.render_snapshot(consumer).unwrap()
    }

    #[test]
    fn mailbox_abandon_finishes_draining_before_next_consumer_generation() {
        let (mut context, consumer) = context_and_consumer();
        let first_generation = consumer.generation();
        let mailbox = ImguiFrameMailbox::default();
        mailbox.publish(1, snapshot(&mut context, &consumer));

        drop(consumer);
        context.poll_snapshot_completions().unwrap();
        assert_eq!(
            context.create_renderer_consumer().unwrap_err(),
            imgui::render::RendererConsumerError::ConsumerDraining
        );

        mailbox.clear();
        let progress = context.poll_snapshot_completions().unwrap();
        assert_eq!(progress.abandoned(), 1);
        let replacement = context.create_renderer_consumer().unwrap();
        assert!(replacement.generation() > first_generation);
    }

    #[test]
    fn mailbox_snapshot_can_outlive_its_context_without_panicking() {
        let (mut context, consumer) = context_and_consumer();
        let mailbox = ImguiFrameMailbox::default();
        mailbox.publish(1, snapshot(&mut context, &consumer));

        drop(context);
        mailbox.clear();
        drop(consumer);
    }

    #[test]
    fn into_inner_rejects_an_outstanding_render_world_snapshot() {
        let (mut context, consumer) = context_and_consumer();
        let mailbox = ImguiFrameMailbox::default();
        mailbox.publish(1, snapshot(&mut context, &consumer));
        let mut imgui = ImguiContext::new(context);
        imgui.renderer_consumer = Some(consumer);

        let error = match imgui.into_inner() {
            Ok(_) => panic!("an outstanding render-world epoch must prevent Context extraction"),
            Err(error) => error,
        };
        assert_eq!(
            error,
            imgui::render::RendererConsumerError::OutstandingEpochs { count: 1 }
        );

        mailbox.clear();
    }

    #[test]
    fn into_inner_resets_idle_renderer_texture_bindings() {
        let (mut context, consumer) = context_and_consumer();
        let mut texture_data = imgui::texture::OwnedTextureData::new();
        texture_data.create(imgui::TextureFormat::RGBA32, 1, 1);
        texture_data.set_data(&[255, 0, 255, 255]);
        let texture = context.register_texture(texture_data);
        let snapshot = texture_snapshot(&mut context, &consumer, texture);
        let request = snapshot
            .texture_requests()
            .iter()
            .find(|request| request.texture() == imgui::render::SnapshotTextureId::User(texture))
            .expect("managed texture should request renderer creation");
        let feedback = request.uploaded(imgui::TextureId::new(0xBEEF)).unwrap();
        snapshot.commit([feedback]).unwrap();
        context.poll_snapshot_completions().unwrap();

        let mut imgui = ImguiContext::new(context);
        imgui.renderer_consumer = Some(consumer);
        let context = imgui
            .into_inner()
            .expect("an idle renderer consumer should reset and detach");
        context
            .with_texture(texture, |texture| {
                assert_eq!(texture.status(), imgui::TextureStatus::WantCreate);
                assert!(texture.texture_id().is_null());
            })
            .unwrap();
    }

    #[test]
    fn first_bevy_snapshot_rebinds_textures_from_a_previous_renderer() {
        let (mut context, consumer) = context_and_consumer();
        let previous_generation = consumer.generation();
        let mut texture_data = imgui::texture::OwnedTextureData::new();
        texture_data.create(imgui::TextureFormat::RGBA32, 1, 1);
        texture_data.set_data(&[255, 0, 255, 255]);
        let texture = context.register_texture(texture_data);
        let snapshot = texture_snapshot(&mut context, &consumer, texture);
        let request = snapshot
            .texture_requests()
            .iter()
            .find(|request| request.texture() == imgui::render::SnapshotTextureId::User(texture))
            .expect("previous renderer should receive the original create request");
        let feedback = request.uploaded(imgui::TextureId::new(0xCAFE)).unwrap();
        snapshot.commit([feedback]).unwrap();
        context.poll_snapshot_completions().unwrap();
        drop(consumer);
        context.poll_snapshot_completions().unwrap();

        let mut imgui = ImguiContext::new(context);
        imgui.context_mut().prepare_frame(
            imgui::FramePrepareOptions::new([64.0, 64.0], 1.0 / 60.0).renderer_has_textures(),
        );
        imgui.context_mut().frame().image(texture, [16.0, 16.0]);
        let snapshot = imgui.render_frame_snapshot(false).unwrap();
        assert!(
            imgui
                .renderer_consumer
                .as_ref()
                .is_some_and(|consumer| consumer.generation() > previous_generation)
        );
        assert!(snapshot.texture_requests().iter().any(|request| {
            request.texture() == imgui::render::SnapshotTextureId::User(texture)
                && matches!(request.operation(), imgui::render::TextureOp::Create { .. })
        }));

        snapshot.commit(std::iter::empty()).unwrap();
        imgui.context_mut().poll_snapshot_completions().unwrap();
    }
}
