//! Main-world Dear ImGui context lifecycle for Bevy.
//!
//! This module bridges Bevy's schedule model to Dear ImGui's immediate-mode frame model. User
//! systems should be added to [`crate::ImguiPrimaryContextPass`] and request [`ImguiContexts`]
//! instead of calling `Context::frame()` / `Context::render()` directly.

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use crate::ImguiViewportBridge;
use crate::{
    ImguiBackendStatus, ImguiContext, ImguiTextureFeedbackQueue, ImguiViewportWindow,
    input::map_imgui_mouse_cursor,
};
use bevy_app::App;
use bevy_ecs::prelude::*;
use bevy_ecs::system::{NonSendMarker, SystemParam};
use bevy_math::Vec2;
use bevy_window::{CursorIcon, CursorOptions, Monitor, PrimaryMonitor, PrimaryWindow, Window};
use dear_imgui_rs as imgui;
use std::ptr::NonNull;

/// Output produced by the last completed Dear ImGui frame.
#[derive(Resource, Debug, Default)]
pub struct ImguiFrameOutput {
    frame_index: u64,
    snapshot: Option<imgui::render::snapshot::FrameSnapshot>,
    snapshot_error: Option<String>,
}

impl ImguiFrameOutput {
    /// Monotonic primary-context frame index for the latest completed frame.
    #[must_use]
    pub fn frame_index(&self) -> u64 {
        self.frame_index
    }

    /// Thread-safe snapshot produced by the latest completed frame.
    #[must_use]
    pub fn snapshot(&self) -> Option<&imgui::render::snapshot::FrameSnapshot> {
        self.snapshot.as_ref()
    }

    /// Snapshot error produced by the latest completed frame, if snapshotting failed.
    #[must_use]
    pub fn snapshot_error(&self) -> Option<&str> {
        self.snapshot_error.as_deref()
    }

    fn set_snapshot(
        &mut self,
        frame_index: u64,
        snapshot: Result<
            imgui::render::snapshot::FrameSnapshot,
            imgui::render::snapshot::SnapshotError,
        >,
    ) {
        self.frame_index = frame_index;
        match snapshot {
            Ok(snapshot) => {
                self.snapshot = Some(snapshot);
                self.snapshot_error = None;
            }
            Err(err) => {
                self.snapshot = None;
                self.snapshot_error = Some(err.to_string());
            }
        }
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
        .init_resource::<ImguiTextureFeedbackQueue>()
        .add_systems(crate::ImguiBeginFrame, begin_primary_frame_system)
        .add_systems(crate::ImguiEndFrame, end_primary_frame_system);
}

#[cfg_attr(
    not(all(feature = "multi-viewport", not(target_arch = "wasm32"))),
    allow(unused_variables)
)]
fn begin_primary_frame_system(
    primary_window: Query<(Entity, &Window), With<PrimaryWindow>>,
    viewport_windows: Query<(Entity, &Window, &ImguiViewportWindow), Without<PrimaryWindow>>,
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))] monitors: Query<(
        &Monitor,
        Option<&PrimaryMonitor>,
    )>,
    mut imgui_context: NonSendMut<ImguiContext>,
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    mut viewport_bridge: Option<NonSendMut<ImguiViewportBridge>>,
    mut frame_state: NonSendMut<ImguiFrameState>,
    mut texture_feedback: ResMut<ImguiTextureFeedbackQueue>,
    backend_status: Res<ImguiBackendStatus>,
) {
    if frame_state.is_frame_open() {
        return;
    }

    let Ok((primary_window_entity, window)) = primary_window.single() else {
        return;
    };

    let context = imgui_context.context_mut();
    let feedback = texture_feedback.drain();
    if !feedback.is_empty() {
        let applied = context.platform_io_mut().apply_texture_feedback(&feedback);
        texture_feedback.set_last_applied(applied);
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    if let Some(viewport_bridge) = viewport_bridge.as_deref_mut() {
        let viewport_feedback = viewport_windows
            .iter()
            .map(|(entity, window, viewport_window)| {
                (
                    entity,
                    viewport_window.viewport_id,
                    crate::viewport::viewport_feedback_from_window(
                        window,
                        viewport_bridge.viewport_feedback(viewport_window.viewport_id),
                    ),
                )
            })
            .collect::<Vec<_>>();
        let monitors = crate::viewport::platform_monitors_from_bevy_monitors(
            monitors
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
            backend_status.multi_viewport_supported,
        );
    }

    context.prepare_frame(
        imgui::FramePrepareOptions::new(
            [window.width(), window.height()],
            context.io().delta_time().max(f32::EPSILON),
        )
        .framebuffer_scale([window.scale_factor(), window.scale_factor()]),
    );

    let frame = context.begin_frame();
    frame_state.begin(frame.ui());
}

pub(crate) fn end_primary_frame_system(
    mut commands: Commands,
    mut imgui_context: NonSendMut<ImguiContext>,
    mut frame_state: NonSendMut<ImguiFrameState>,
    mut primary_window: Query<
        (
            Entity,
            &mut Window,
            &mut CursorOptions,
            Option<&mut CursorIcon>,
        ),
        With<PrimaryWindow>,
    >,
    mut viewport_windows: Query<
        (
            Entity,
            &mut Window,
            &mut CursorOptions,
            Option<&mut CursorIcon>,
            &ImguiViewportWindow,
        ),
        Without<PrimaryWindow>,
    >,
    mut output: ResMut<ImguiFrameOutput>,
) {
    if !frame_state.is_frame_open() {
        return;
    }

    if let Some(ui) = frame_state.ui() {
        sync_primary_window_platform_feedback(
            ui,
            &imgui_context,
            &mut commands,
            &mut primary_window,
            &mut viewport_windows,
        );
    }

    let frame_index = frame_state.end();
    let snapshot = render_frame_snapshot(imgui_context.context_mut());
    output.set_snapshot(frame_index, snapshot);
}

fn render_frame_snapshot(
    context: &mut imgui::Context,
) -> Result<imgui::render::snapshot::FrameSnapshot, imgui::render::snapshot::SnapshotError> {
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    {
        let _ = context.render();
        context.update_platform_windows();
        context.platform_viewport_snapshot(imgui::render::snapshot::SnapshotOptions::default())
    }

    #[cfg(not(all(feature = "multi-viewport", not(target_arch = "wasm32"))))]
    {
        let draw_data = context.render();
        imgui::render::snapshot::FrameSnapshot::from_draw_data(
            draw_data,
            imgui::render::snapshot::SnapshotOptions::default(),
        )
    }
}

fn sync_primary_window_platform_feedback(
    ui: &imgui::Ui,
    imgui_context: &ImguiContext,
    commands: &mut Commands,
    primary_window: &mut Query<
        (
            Entity,
            &mut Window,
            &mut CursorOptions,
            Option<&mut CursorIcon>,
        ),
        With<PrimaryWindow>,
    >,
    viewport_windows: &mut Query<
        (
            Entity,
            &mut Window,
            &mut CursorOptions,
            Option<&mut CursorIcon>,
            &ImguiViewportWindow,
        ),
        Without<PrimaryWindow>,
    >,
) {
    let Ok((primary_entity, mut primary_window, mut primary_cursor_options, primary_cursor_icon)) =
        primary_window.single_mut()
    else {
        return;
    };

    let raw_context = imgui_context.context().as_raw();
    // SAFETY: `ImguiContext` owns this live Dear ImGui context for the lifetime of the Bevy
    // resource, and the frame is still open while platform feedback is synchronized.
    let ime_data = unsafe { &(*raw_context).PlatformImeData };
    if ime_data.ViewportId == 0 {
        apply_window_cursor_feedback(
            ui,
            commands,
            primary_entity,
            &mut primary_cursor_options,
            primary_cursor_icon,
        );
        primary_window.ime_enabled = ime_data.WantTextInput;
        primary_window.ime_position = Vec2::new(ime_data.InputPos.x, ime_data.InputPos.y);
        return;
    }

    for (window_entity, mut window, mut cursor_options, cursor_icon, viewport_window) in
        viewport_windows.iter_mut()
    {
        if viewport_window.viewport_id.raw() == ime_data.ViewportId {
            apply_window_cursor_feedback(
                ui,
                commands,
                window_entity,
                &mut cursor_options,
                cursor_icon,
            );
            window.ime_enabled = ime_data.WantTextInput;
            window.ime_position = Vec2::new(ime_data.InputPos.x, ime_data.InputPos.y);
            primary_window.ime_enabled = false;
            return;
        }
    }

    apply_window_cursor_feedback(
        ui,
        commands,
        primary_entity,
        &mut primary_cursor_options,
        primary_cursor_icon,
    );
    primary_window.ime_enabled = ime_data.WantTextInput;
    primary_window.ime_position = Vec2::new(ime_data.InputPos.x, ime_data.InputPos.y);
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
    } else if let Some(mouse_cursor) = mouse_cursor {
        if let Some(cursor_icon_value) = map_imgui_mouse_cursor(mouse_cursor) {
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
}
