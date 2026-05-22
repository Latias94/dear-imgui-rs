//! Main-world Dear ImGui context lifecycle for Bevy.
//!
//! This module bridges Bevy's schedule model to Dear ImGui's immediate-mode frame model. User
//! systems should be added to [`crate::ImguiPrimaryContextPass`] and request [`ImguiContexts`]
//! instead of calling `Context::frame()` / `Context::render()` directly.

use crate::ImguiContext;
use bevy_app::App;
use bevy_ecs::prelude::*;
use bevy_ecs::system::{NonSendMarker, SystemParam};
use bevy_window::{PrimaryWindow, Window};
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
        .add_systems(crate::ImguiBeginFrame, begin_primary_frame_system)
        .add_systems(crate::ImguiEndFrame, end_primary_frame_system);
}

fn begin_primary_frame_system(
    primary_window: Query<&Window, With<PrimaryWindow>>,
    mut imgui_context: NonSendMut<ImguiContext>,
    mut frame_state: NonSendMut<ImguiFrameState>,
) {
    if frame_state.is_frame_open() {
        return;
    }

    let Ok(window) = primary_window.single() else {
        return;
    };

    let context = imgui_context.context_mut();
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
    mut imgui_context: NonSendMut<ImguiContext>,
    mut frame_state: NonSendMut<ImguiFrameState>,
    mut output: ResMut<ImguiFrameOutput>,
) {
    if !frame_state.is_frame_open() {
        return;
    }

    let frame_index = frame_state.end();
    let draw_data = imgui_context.context_mut().render();
    let snapshot = imgui::render::snapshot::FrameSnapshot::from_draw_data(
        draw_data,
        imgui::render::snapshot::SnapshotOptions::default(),
    );
    output.set_snapshot(frame_index, snapshot);
}
