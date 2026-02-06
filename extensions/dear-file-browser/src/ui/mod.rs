use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use dear_imgui_rs::TreeNodeFlags;
use dear_imgui_rs::Ui;
use dear_imgui_rs::input::{Key, MouseButton};
use dear_imgui_rs::sys;

use crate::core::{ClickAction, DialogMode, FileDialogError, LayoutStyle, Selection, SortBy};
use crate::custom_pane::{CustomPane, CustomPaneCtx};
use crate::dialog_core::{ConfirmGate, DirEntry, Modifiers};
use crate::dialog_state::FileDialogState;
use crate::dialog_state::FileListViewMode;
use crate::dialog_state::{ValidationButtonsAlign, ValidationButtonsOrder};
use crate::file_style::EntryKind;
use crate::fs::{FileSystem, StdFileSystem};
use crate::places::{Place, PlaceOrigin, Places};
use crate::thumbnails::ThumbnailBackend;

/// Configuration for hosting the file browser in an ImGui window.
#[derive(Clone, Debug)]
pub struct WindowHostConfig {
    /// Window title
    pub title: String,
    /// Initial window size (used with `size_condition`)
    pub initial_size: [f32; 2],
    /// Condition used when setting the window size
    pub size_condition: dear_imgui_rs::Condition,
}

impl WindowHostConfig {
    /// Default window host configuration for the given dialog mode.
    pub fn for_mode(mode: DialogMode) -> Self {
        let title = match mode {
            DialogMode::OpenFile | DialogMode::OpenFiles => "Open",
            DialogMode::PickFolder => "Select Folder",
            DialogMode::SaveFile => "Save",
        };
        Self {
            title: title.to_string(),
            initial_size: [760.0, 520.0],
            size_condition: dear_imgui_rs::Condition::FirstUseEver,
        }
    }
}

/// Configuration for hosting the file browser in an ImGui modal popup.
///
/// `popup_label` must be stable across frames. For multiple concurrent dialogs,
/// ensure the label includes a unique ID suffix (ImGui `###` syntax is fine).
#[derive(Clone, Debug)]
pub struct ModalHostConfig {
    /// Modal popup label/title (supports `###` id suffix).
    pub popup_label: String,
    /// Initial modal size (used with `size_condition`).
    pub initial_size: [f32; 2],
    /// Condition used when setting the popup size.
    pub size_condition: dear_imgui_rs::Condition,
}

impl ModalHostConfig {
    /// Default modal host configuration for the given dialog mode.
    pub fn for_mode(mode: DialogMode) -> Self {
        let title = match mode {
            DialogMode::OpenFile | DialogMode::OpenFiles => "Open",
            DialogMode::PickFolder => "Select Folder",
            DialogMode::SaveFile => "Save",
        };
        Self {
            popup_label: format!("{title}###FileBrowserModal"),
            initial_size: [760.0, 520.0],
            size_condition: dear_imgui_rs::Condition::FirstUseEver,
        }
    }
}

/// UI handle for file browser
pub struct FileBrowser<'ui> {
    pub ui: &'ui Ui,
}

/// Extend Ui with a file browser entry point
pub trait FileDialogExt {
    /// Entry point for showing the file browser widget
    fn file_browser(&self) -> FileBrowser<'_>;
}

impl FileDialogExt for Ui {
    fn file_browser(&self) -> FileBrowser<'_> {
        FileBrowser { ui: self }
    }
}

impl<'ui> FileBrowser<'ui> {
    /// Draw only the contents of the file browser (no window/modal host).
    ///
    /// This is useful for embedding the browser into an existing window, popup,
    /// tab, or child region managed by the caller.
    ///
    /// Returns Some(result) once the user confirms/cancels; None otherwise.
    pub fn draw_contents(
        &self,
        state: &mut FileDialogState,
    ) -> Option<Result<Selection, FileDialogError>> {
        self.draw_contents_with_fs(state, &StdFileSystem)
    }

    /// Draw only the contents of the file browser (no window/modal host) using a custom filesystem.
    pub fn draw_contents_with_fs(
        &self,
        state: &mut FileDialogState,
        fs: &dyn FileSystem,
    ) -> Option<Result<Selection, FileDialogError>> {
        draw_contents_with_fs_and_hooks(self.ui, state, fs, None, None)
    }

    /// Draw only the contents of the file browser (no window/modal host) with a thumbnail backend.
    ///
    /// When `state.ui.thumbnails_enabled` is true, the UI will request thumbnails for visible
    /// entries and call `maintain()` each frame to decode/upload and destroy evicted textures.
    pub fn draw_contents_with_thumbnail_backend(
        &self,
        state: &mut FileDialogState,
        backend: &mut ThumbnailBackend<'_>,
    ) -> Option<Result<Selection, FileDialogError>> {
        self.draw_contents_with_fs_and_thumbnail_backend(state, &StdFileSystem, backend)
    }

    /// Draw only the contents of the file browser (no window/modal host) using a custom filesystem
    /// and a thumbnail backend.
    pub fn draw_contents_with_fs_and_thumbnail_backend(
        &self,
        state: &mut FileDialogState,
        fs: &dyn FileSystem,
        backend: &mut ThumbnailBackend<'_>,
    ) -> Option<Result<Selection, FileDialogError>> {
        draw_contents_with_fs_and_hooks(self.ui, state, fs, None, Some(backend))
    }

    /// Draw only the contents of the file browser (no window/modal host) with a custom pane.
    ///
    /// The pane can draw additional UI below the file list and can block confirmation.
    pub fn draw_contents_with_custom_pane(
        &self,
        state: &mut FileDialogState,
        custom_pane: &mut dyn CustomPane,
    ) -> Option<Result<Selection, FileDialogError>> {
        self.draw_contents_with_fs_and_custom_pane(state, &StdFileSystem, custom_pane)
    }

    /// Draw only the contents of the file browser (no window/modal host) using a custom filesystem
    /// and a custom pane.
    pub fn draw_contents_with_fs_and_custom_pane(
        &self,
        state: &mut FileDialogState,
        fs: &dyn FileSystem,
        custom_pane: &mut dyn CustomPane,
    ) -> Option<Result<Selection, FileDialogError>> {
        draw_contents_with_fs_and_hooks(self.ui, state, fs, Some(custom_pane), None)
    }

    /// Draw only the contents of the file browser (no window/modal host) using a custom filesystem,
    /// a custom pane and a thumbnail backend.
    pub fn draw_contents_with_fs_and_custom_pane_and_thumbnail_backend(
        &self,
        state: &mut FileDialogState,
        fs: &dyn FileSystem,
        custom_pane: &mut dyn CustomPane,
        backend: &mut ThumbnailBackend<'_>,
    ) -> Option<Result<Selection, FileDialogError>> {
        draw_contents_with_fs_and_hooks(self.ui, state, fs, Some(custom_pane), Some(backend))
    }

    /// Draw the file browser in a standard ImGui window with default host config.
    /// Returns Some(result) once the user confirms/cancels; None otherwise.
    pub fn show(&self, state: &mut FileDialogState) -> Option<Result<Selection, FileDialogError>> {
        let cfg = WindowHostConfig::for_mode(state.core.mode);
        self.show_windowed(state, &cfg)
    }

    /// Draw the file browser in a standard ImGui window using the given host configuration.
    /// Returns Some(result) once the user confirms/cancels; None otherwise.
    pub fn show_windowed(
        &self,
        state: &mut FileDialogState,
        cfg: &WindowHostConfig,
    ) -> Option<Result<Selection, FileDialogError>> {
        self.show_windowed_with_fs(state, cfg, &StdFileSystem)
    }

    /// Draw the file browser in a standard ImGui window using a custom filesystem.
    pub fn show_windowed_with_fs(
        &self,
        state: &mut FileDialogState,
        cfg: &WindowHostConfig,
        fs: &dyn FileSystem,
    ) -> Option<Result<Selection, FileDialogError>> {
        self.show_windowed_with_fs_and_hooks(state, cfg, fs, None, None)
    }

    /// Draw the file browser in a standard ImGui window using a custom filesystem, custom pane,
    /// and/or thumbnail backend.
    pub fn show_windowed_with_fs_and_hooks(
        &self,
        state: &mut FileDialogState,
        cfg: &WindowHostConfig,
        fs: &dyn FileSystem,
        mut custom_pane: Option<&mut dyn CustomPane>,
        mut thumbnails_backend: Option<&mut ThumbnailBackend<'_>>,
    ) -> Option<Result<Selection, FileDialogError>> {
        if !state.ui.visible {
            return None;
        }

        let mut out: Option<Result<Selection, FileDialogError>> = None;
        self.ui
            .window(&cfg.title)
            .size(cfg.initial_size, cfg.size_condition)
            .build(|| {
                out = draw_contents_with_fs_and_hooks(
                    self.ui,
                    state,
                    fs,
                    custom_pane.take(),
                    thumbnails_backend.take(),
                );
            });
        out
    }

    /// Draw the file browser in an ImGui modal popup with default host config.
    /// Returns Some(result) once the user confirms/cancels; None otherwise.
    pub fn show_modal(
        &self,
        state: &mut FileDialogState,
    ) -> Option<Result<Selection, FileDialogError>> {
        let cfg = ModalHostConfig::for_mode(state.core.mode);
        self.show_modal_with_fs(state, &cfg, &StdFileSystem)
    }

    /// Draw the file browser in an ImGui modal popup using the given host configuration.
    pub fn show_modal_with_fs(
        &self,
        state: &mut FileDialogState,
        cfg: &ModalHostConfig,
        fs: &dyn FileSystem,
    ) -> Option<Result<Selection, FileDialogError>> {
        self.show_modal_with_fs_and_hooks(state, cfg, fs, None, None)
    }

    /// Draw the file browser in an ImGui modal popup using a custom filesystem, custom pane,
    /// and/or thumbnail backend.
    pub fn show_modal_with_fs_and_hooks(
        &self,
        state: &mut FileDialogState,
        cfg: &ModalHostConfig,
        fs: &dyn FileSystem,
        mut custom_pane: Option<&mut dyn CustomPane>,
        mut thumbnails_backend: Option<&mut ThumbnailBackend<'_>>,
    ) -> Option<Result<Selection, FileDialogError>> {
        if !state.ui.visible {
            return None;
        }

        if !self.ui.is_popup_open(&cfg.popup_label) {
            self.ui.open_popup(&cfg.popup_label);
        }

        unsafe {
            let size_vec = sys::ImVec2 {
                x: cfg.initial_size[0],
                y: cfg.initial_size[1],
            };
            sys::igSetNextWindowSize(size_vec, cfg.size_condition as i32);
        }

        let Some(_popup) = self.ui.begin_modal_popup(&cfg.popup_label) else {
            return None;
        };

        let out = draw_contents_with_fs_and_hooks(
            self.ui,
            state,
            fs,
            custom_pane.take(),
            thumbnails_backend.take(),
        );
        if out.is_some() {
            self.ui.close_current_popup();
        }
        out
    }

    /// Draw the file browser in a standard ImGui window using a custom filesystem and custom pane.
    pub fn show_windowed_with_fs_and_custom_pane(
        &self,
        state: &mut FileDialogState,
        cfg: &WindowHostConfig,
        fs: &dyn FileSystem,
        custom_pane: Option<&mut dyn CustomPane>,
    ) -> Option<Result<Selection, FileDialogError>> {
        self.show_windowed_with_fs_and_hooks(state, cfg, fs, custom_pane, None)
    }

    /// Draw the file browser in a standard ImGui window using a thumbnail backend.
    pub fn show_windowed_with_thumbnail_backend(
        &self,
        state: &mut FileDialogState,
        cfg: &WindowHostConfig,
        backend: &mut ThumbnailBackend<'_>,
    ) -> Option<Result<Selection, FileDialogError>> {
        self.show_windowed_with_fs_and_thumbnail_backend(state, cfg, &StdFileSystem, backend)
    }

    /// Draw the file browser in a standard ImGui window using a custom filesystem and thumbnail backend.
    pub fn show_windowed_with_fs_and_thumbnail_backend(
        &self,
        state: &mut FileDialogState,
        cfg: &WindowHostConfig,
        fs: &dyn FileSystem,
        backend: &mut ThumbnailBackend<'_>,
    ) -> Option<Result<Selection, FileDialogError>> {
        self.show_windowed_with_fs_and_hooks(state, cfg, fs, None, Some(backend))
    }
}

struct TextColorToken {
    pushed: bool,
}

impl TextColorToken {
    fn push(color: [f32; 4]) -> Self {
        unsafe {
            sys::igPushStyleColor_Vec4(
                sys::ImGuiCol_Text as i32,
                sys::ImVec4 {
                    x: color[0],
                    y: color[1],
                    z: color[2],
                    w: color[3],
                },
            );
        }
        Self { pushed: true }
    }

    fn none() -> Self {
        Self { pushed: false }
    }
}

impl Drop for TextColorToken {
    fn drop(&mut self) {
        if self.pushed {
            unsafe { sys::igPopStyleColor(1) };
        }
    }
}

fn draw_contents(
    ui: &Ui,
    state: &mut FileDialogState,
) -> Option<Result<Selection, FileDialogError>> {
    draw_contents_with_fs_and_hooks(ui, state, &StdFileSystem, None, None)
}

fn draw_contents_with_fs_and_hooks(
    ui: &Ui,
    state: &mut FileDialogState,
    fs: &dyn FileSystem,
    mut custom_pane: Option<&mut dyn CustomPane>,
    mut thumbnails_backend: Option<&mut ThumbnailBackend<'_>>,
) -> Option<Result<Selection, FileDialogError>> {
    if !state.ui.visible {
        return None;
    }

    let mut request_confirm = false;
    let mut confirm_gate = ConfirmGate::default();

    // Top toolbar: Up, Refresh, Hidden toggle, Breadcrumbs, Filter, Search
    if ui.button("Up") {
        state.core.navigate_up();
    }
    ui.same_line();
    if ui.button("Refresh") {
        state.core.invalidate_dir_cache();
    }
    ui.same_line();
    if ui.button("New Folder") {
        state.ui.new_folder_open_next = true;
        state.ui.new_folder_name.clear();
        state.ui.new_folder_error = None;
        state.ui.new_folder_focus_next = true;
    }
    ui.same_line();
    ui.text("View:");
    ui.same_line();
    if ui.radio_button(
        "List",
        matches!(state.ui.file_list_view, FileListViewMode::List),
    ) {
        state.ui.file_list_view = FileListViewMode::List;
    }
    ui.same_line();
    if ui.radio_button(
        "Grid",
        matches!(state.ui.file_list_view, FileListViewMode::Grid),
    ) {
        state.ui.file_list_view = FileListViewMode::Grid;
        state.ui.thumbnails_enabled = true;
    }
    ui.same_line();
    let mut show_hidden = state.core.show_hidden;
    if ui.checkbox("Hidden", &mut show_hidden) {
        state.core.show_hidden = show_hidden;
    }
    ui.same_line();
    // Breadcrumbs or Path Edit
    if state.ui.path_edit {
        if state.ui.focus_path_edit_next {
            ui.set_keyboard_focus_here();
            state.ui.focus_path_edit_next = false;
        }
        ui.input_text("##path_edit", &mut state.ui.path_edit_buffer)
            .build();
        ui.same_line();
        if ui.button("Go") {
            submit_path_edit(state, fs);
        }
        ui.same_line();
        if ui.button("Cancel") {
            state.ui.path_edit = false;
        }
    } else {
        if let Some(p) = draw_breadcrumbs(ui, state, fs, state.ui.breadcrumbs_max_segments) {
            state.core.navigate_to(p);
        }
    }
    // Search box (aligned to the right)
    ui.same_line();
    if state.ui.focus_search_next {
        ui.set_keyboard_focus_here();
        state.ui.focus_search_next = false;
    }
    let search_changed = ui.input_text("Search", &mut state.core.search).build();
    if search_changed {
        // `rescan()` will apply search filtering.
    }

    ui.separator();

    // Content region
    let avail = ui.content_region_avail();
    match state.ui.layout {
        LayoutStyle::Standard => {
            let left_w = 180.0f32;
            let mut new_cwd: Option<PathBuf> = None;
            ui.child_window("quick_locations")
                .size([left_w, avail[1] - 80.0])
                .build(ui, || {
                    new_cwd = draw_quick_locations(ui, state);
                });
            if let Some(p) = new_cwd {
                state.core.navigate_to(p);
            }
            ui.same_line();
            ui.child_window("file_list")
                .size([avail[0] - left_w - 8.0, avail[1] - 80.0])
                .build(ui, || {
                    let inner = ui.content_region_avail();
                    let mut table_h = inner[1];
                    let show_pane =
                        state.ui.custom_pane_enabled && custom_pane.as_deref_mut().is_some();
                    let pane_h = if show_pane {
                        state.ui.custom_pane_height.clamp(0.0, inner[1].max(0.0))
                    } else {
                        0.0
                    };
                    if pane_h > 0.0 {
                        table_h = (table_h - pane_h - 8.0).max(0.0);
                    }

                    draw_file_table(
                        ui,
                        state,
                        [inner[0], table_h],
                        fs,
                        &mut request_confirm,
                        thumbnails_backend.as_deref_mut(),
                    );

                    if let Some(pane) = custom_pane.as_deref_mut() {
                        if state.ui.custom_pane_enabled && pane_h > 0.0 {
                            ui.separator();
                            ui.child_window("custom_pane")
                                .size([inner[0], pane_h])
                                .border(true)
                                .build(ui, || {
                                    let ctx = CustomPaneCtx {
                                        mode: state.core.mode,
                                        cwd: &state.core.cwd,
                                        selected_names: &state.core.selected,
                                        save_name: &state.core.save_name,
                                        active_filter: state
                                            .core
                                            .active_filter
                                            .and_then(|i| state.core.filters.get(i)),
                                    };
                                    confirm_gate = pane.draw(ui, ctx);
                                });
                        }
                    }
                });
        }
        LayoutStyle::Minimal => {
            ui.child_window("file_list_min")
                .size([avail[0], avail[1] - 80.0])
                .build(ui, || {
                    let inner = ui.content_region_avail();
                    let mut table_h = inner[1];
                    let show_pane =
                        state.ui.custom_pane_enabled && custom_pane.as_deref_mut().is_some();
                    let pane_h = if show_pane {
                        state.ui.custom_pane_height.clamp(0.0, inner[1].max(0.0))
                    } else {
                        0.0
                    };
                    if pane_h > 0.0 {
                        table_h = (table_h - pane_h - 8.0).max(0.0);
                    }

                    draw_file_table(
                        ui,
                        state,
                        [inner[0], table_h],
                        fs,
                        &mut request_confirm,
                        thumbnails_backend.as_deref_mut(),
                    );

                    if let Some(pane) = custom_pane.as_deref_mut() {
                        if state.ui.custom_pane_enabled && pane_h > 0.0 {
                            ui.separator();
                            ui.child_window("custom_pane")
                                .size([inner[0], pane_h])
                                .border(true)
                                .build(ui, || {
                                    let ctx = CustomPaneCtx {
                                        mode: state.core.mode,
                                        cwd: &state.core.cwd,
                                        selected_names: &state.core.selected,
                                        save_name: &state.core.save_name,
                                        active_filter: state
                                            .core
                                            .active_filter
                                            .and_then(|i| state.core.filters.get(i)),
                                    };
                                    confirm_gate = pane.draw(ui, ctx);
                                });
                        }
                    }
                });
        }
    }

    draw_places_io_modal(ui, state);
    draw_places_edit_modal(ui, state, fs);
    draw_new_folder_modal(ui, state, fs);
    draw_rename_modal(ui, state, fs);
    draw_delete_confirm_modal(ui, state, fs);

    ui.separator();
    // Footer: file name (Save) + buttons
    if matches!(state.core.mode, DialogMode::SaveFile) {
        ui.text("File name:");
        ui.same_line();
        ui.input_text("##save_name", &mut state.core.save_name)
            .build();
        ui.same_line();
    }
    // Filter selector (moved to footer like ImGuiFileDialog)
    if !state.core.filters.is_empty() && !matches!(state.core.mode, DialogMode::PickFolder) {
        ui.same_line();
        let preview = state
            .core
            .active_filter
            .and_then(|i| state.core.filters.get(i))
            .map(|f| f.name.as_str())
            .unwrap_or("All files");
        let mut next_active_filter = state.core.active_filter;
        if let Some(_c) = ui.begin_combo("Filter", preview) {
            if ui
                .selectable_config("All files")
                .selected(state.core.active_filter.is_none())
                .build()
            {
                next_active_filter = None;
            }
            for (i, f) in state.core.filters.iter().enumerate() {
                if ui
                    .selectable_config(&f.name)
                    .selected(state.core.active_filter == Some(i))
                    .build()
                {
                    next_active_filter = Some(i);
                }
            }
        }
        if next_active_filter != state.core.active_filter {
            state.core.active_filter = next_active_filter;
        }
    }

    // Click behavior toggle
    let mut nav_on_click = matches!(state.core.click_action, ClickAction::Navigate);
    if ui.checkbox("Navigate on click", &mut nav_on_click) {
        state.core.click_action = if nav_on_click {
            ClickAction::Navigate
        } else {
            ClickAction::Select
        };
    }
    ui.same_line();
    let mut dbl = state.core.double_click;
    if ui.checkbox("DblClick confirm", &mut dbl) {
        state.core.double_click = dbl;
    }

    if !confirm_gate.can_confirm {
        if let Some(msg) = confirm_gate.message.as_deref() {
            ui.same_line();
            ui.text_disabled(msg);
        }
    }

    ui.new_line();
    let (confirm, cancel) = draw_validation_buttons_row(ui, state, &confirm_gate);

    // Keyboard shortcuts (only when the host window is focused)
    if state.ui.visible && ui.is_window_focused() {
        let ctrl = ui.is_key_down(Key::LeftCtrl) || ui.is_key_down(Key::RightCtrl);
        if ctrl && ui.is_key_pressed(Key::L) {
            state.ui.path_edit = true;
            state.ui.path_edit_buffer = state.core.cwd.display().to_string();
            state.ui.focus_path_edit_next = true;
        }
        if ctrl && ui.is_key_pressed(Key::F) {
            state.ui.focus_search_next = true;
        }
        if !ui.io().want_capture_keyboard() && ui.is_key_pressed(Key::Backspace) {
            state.core.navigate_up();
        }
        if !state.ui.path_edit && !ui.io().want_text_input() && ui.is_key_pressed(Key::Enter) {
            request_confirm |= state.core.activate_focused();
        }
        if !ui.io().want_text_input() && ui.is_key_pressed(Key::F2) {
            if state.core.selected.len() == 1 {
                state.ui.rename_target = Some(state.core.selected[0].clone());
                state.ui.rename_to = state.core.selected[0].clone();
                state.ui.rename_error = None;
                state.ui.rename_open_next = true;
                state.ui.rename_focus_next = true;
            }
        }
        if !ui.io().want_text_input() && ui.is_key_pressed(Key::Delete) {
            if !state.core.selected.is_empty() {
                state.ui.delete_targets = state.core.selected.clone();
                state.ui.delete_error = None;
                state.ui.delete_open_next = true;
            }
        }
    }

    request_confirm |= confirm;
    if cancel {
        state.core.cancel();
    } else if request_confirm {
        state.ui.ui_error = None;
        if let Err(e) = state.core.confirm(fs, &confirm_gate) {
            state.ui.ui_error = Some(e.to_string());
        }
    }

    draw_confirm_overwrite_modal(ui, state);

    if let Some(err) = &state.ui.ui_error {
        ui.separator();
        ui.text_colored([1.0, 0.3, 0.3, 1.0], format!("Error: {err}"));
    }

    let out = state.core.take_result();
    if out.is_some() {
        state.ui.visible = false;
    }
    out
}

fn draw_validation_buttons_row(
    ui: &Ui,
    state: &mut FileDialogState,
    gate: &ConfirmGate,
) -> (bool, bool) {
    let default_confirm = match state.core.mode {
        DialogMode::OpenFile | DialogMode::OpenFiles => "Open",
        DialogMode::PickFolder => "Select",
        DialogMode::SaveFile => "Save",
    };
    let cfg = &state.ui.validation_buttons;
    let confirm_label = cfg.confirm_label.as_deref().unwrap_or(default_confirm);
    let cancel_label = cfg.cancel_label.as_deref().unwrap_or("Cancel");

    let style = ui.clone_style();
    let spacing_x = style.item_spacing()[0];
    let pad_x = style.frame_padding()[0];
    let font = ui.current_font();
    let font_size = ui.current_font_size();

    let calc_button_width = |label: &str| -> f32 {
        let text_w = font.calc_text_size(font_size, f32::MAX, 0.0, label)[0];
        text_w + pad_x * 2.0
    };

    let base_w = cfg.button_width;
    let confirm_w = cfg
        .confirm_width
        .or(base_w)
        .unwrap_or_else(|| calc_button_width(confirm_label));
    let cancel_w = cfg
        .cancel_width
        .or(base_w)
        .unwrap_or_else(|| calc_button_width(cancel_label));

    let group_w = confirm_w + cancel_w + spacing_x;
    if cfg.align == ValidationButtonsAlign::Right {
        let start_x = ui.cursor_pos_x();
        let avail_w = ui.content_region_avail_width();
        let x = (start_x + avail_w - group_w).max(start_x);
        ui.set_cursor_pos_x(x);
    }

    match cfg.order {
        ValidationButtonsOrder::ConfirmCancel => {
            let _disabled = ui.begin_disabled_with_cond(!gate.can_confirm);
            let confirm_clicked = ui.button_with_size(confirm_label, [confirm_w, 0.0]);
            drop(_disabled);
            ui.same_line();
            let cancel_clicked = ui.button_with_size(cancel_label, [cancel_w, 0.0]);
            (confirm_clicked, cancel_clicked)
        }
        ValidationButtonsOrder::CancelConfirm => {
            let cancel_clicked = ui.button_with_size(cancel_label, [cancel_w, 0.0]);
            ui.same_line();
            let _disabled = ui.begin_disabled_with_cond(!gate.can_confirm);
            let confirm_clicked = ui.button_with_size(confirm_label, [confirm_w, 0.0]);
            drop(_disabled);
            (confirm_clicked, cancel_clicked)
        }
    }
}

fn draw_confirm_overwrite_modal(ui: &Ui, state: &mut FileDialogState) {
    const POPUP_ID: &str = "Confirm overwrite";

    let Some(path_text) = state
        .core
        .pending_overwrite()
        .and_then(|s| s.paths.get(0))
        .map(|p| p.display().to_string())
    else {
        return;
    };

    if !ui.is_popup_open(POPUP_ID) {
        ui.open_popup(POPUP_ID);
    }

    ui.modal_popup(POPUP_ID, || {
        ui.text("The file already exists:");
        ui.separator();
        ui.text(&path_text);
        ui.separator();
        if ui.button("Overwrite") {
            state.core.accept_overwrite();
            ui.close_current_popup();
        }
        ui.same_line();
        if ui.button("Cancel") {
            state.core.cancel_overwrite();
            ui.close_current_popup();
        }
    });
}

fn draw_new_folder_modal(ui: &Ui, state: &mut FileDialogState, fs: &dyn FileSystem) {
    const POPUP_ID: &str = "New Folder";

    if state.ui.new_folder_open_next {
        state.ui.new_folder_open_next = false;
        if !ui.is_popup_open(POPUP_ID) {
            ui.open_popup(POPUP_ID);
        }
    }

    if let Some(_popup) = ui.begin_modal_popup(POPUP_ID) {
        ui.text("Create a new folder in:");
        ui.text_disabled(state.core.cwd.display().to_string());
        ui.separator();

        if state.ui.new_folder_focus_next {
            ui.set_keyboard_focus_here();
            state.ui.new_folder_focus_next = false;
        }
        ui.input_text("Name", &mut state.ui.new_folder_name).build();

        let create = ui.button("Create");
        ui.same_line();
        let cancel = ui.button("Cancel");
        if cancel {
            state.ui.new_folder_error = None;
            ui.close_current_popup();
        }

        if create {
            state.ui.new_folder_error = None;
            let name = state.ui.new_folder_name.trim();
            let invalid = name.is_empty()
                || name == "."
                || name == ".."
                || name.contains('/')
                || name.contains('\\')
                || name.contains('\0');
            if invalid {
                state.ui.new_folder_error = Some("Invalid folder name".into());
            } else {
                let name = name.to_string();
                let path = state.core.cwd.join(&name);
                match fs.create_dir(&path) {
                    Ok(()) => {
                        state.ui.new_folder_name.clear();
                        state.core.focus_and_select_by_name(name.clone());
                        state.ui.reveal_name_next = Some(name);
                        state.core.invalidate_dir_cache();
                        ui.close_current_popup();
                    }
                    Err(e) => {
                        state.ui.new_folder_error =
                            Some(format!("Failed to create '{}': {}", name, e));
                    }
                }
            }
        }

        if let Some(err) = &state.ui.new_folder_error {
            ui.separator();
            ui.text_colored([1.0, 0.3, 0.3, 1.0], err);
        }
    }
}

fn draw_rename_modal(ui: &Ui, state: &mut FileDialogState, fs: &dyn FileSystem) {
    const POPUP_ID: &str = "Rename";

    if state.ui.rename_open_next {
        state.ui.rename_open_next = false;
        if !ui.is_popup_open(POPUP_ID) {
            ui.open_popup(POPUP_ID);
        }
    }

    if let Some(_popup) = ui.begin_modal_popup(POPUP_ID) {
        let Some(from_name) = state.ui.rename_target.clone() else {
            ui.text_disabled("No entry selected for rename.");
            if ui.button("Close") {
                ui.close_current_popup();
            }
            return;
        };

        ui.text("Rename in:");
        ui.text_disabled(state.core.cwd.display().to_string());
        ui.separator();
        ui.text(format!("From: {from_name}"));

        if state.ui.rename_focus_next {
            ui.set_keyboard_focus_here();
            state.ui.rename_focus_next = false;
        }
        ui.input_text("To", &mut state.ui.rename_to).build();

        let rename = ui.button("Rename");
        ui.same_line();
        let cancel = ui.button("Cancel");
        if cancel {
            state.ui.rename_error = None;
            state.ui.rename_target = None;
            ui.close_current_popup();
        }

        if rename {
            state.ui.rename_error = None;
            let to_name = state.ui.rename_to.trim();
            let invalid = to_name.is_empty()
                || to_name == "."
                || to_name == ".."
                || to_name.contains('/')
                || to_name.contains('\\')
                || to_name.contains('\0');
            if invalid {
                state.ui.rename_error = Some("Invalid target name".into());
            } else if to_name == from_name.as_str() {
                state.ui.rename_error = Some("Target name is unchanged".into());
            } else {
                let to_name = to_name.to_string();
                let from_path = state.core.cwd.join(&from_name);
                let to_path = state.core.cwd.join(&to_name);

                if fs.metadata(&to_path).is_ok() {
                    state.ui.rename_error = Some("Target already exists".into());
                } else {
                    match fs.rename(&from_path, &to_path) {
                        Ok(()) => {
                            state.core.focus_and_select_by_name(to_name.clone());
                            state.ui.reveal_name_next = Some(to_name);
                            state.core.invalidate_dir_cache();
                            state.ui.rename_target = None;
                            state.ui.rename_to.clear();
                            ui.close_current_popup();
                        }
                        Err(e) => {
                            state.ui.rename_error =
                                Some(format!("Failed to rename '{from_name}': {e}"));
                        }
                    }
                }
            }
        }

        if let Some(err) = &state.ui.rename_error {
            ui.separator();
            ui.text_colored([1.0, 0.3, 0.3, 1.0], err);
        }
    }
}

fn draw_delete_confirm_modal(ui: &Ui, state: &mut FileDialogState, fs: &dyn FileSystem) {
    const POPUP_ID: &str = "Delete";

    if state.ui.delete_open_next {
        state.ui.delete_open_next = false;
        state.ui.delete_recursive = false;
        if !ui.is_popup_open(POPUP_ID) {
            ui.open_popup(POPUP_ID);
        }
    }

    if let Some(_popup) = ui.begin_modal_popup(POPUP_ID) {
        if state.ui.delete_targets.is_empty() {
            ui.text_disabled("No entries selected for deletion.");
            if ui.button("Close") {
                ui.close_current_popup();
            }
            return;
        }

        ui.text(format!(
            "Delete {} entr{} in:",
            state.ui.delete_targets.len(),
            if state.ui.delete_targets.len() == 1 {
                "y"
            } else {
                "ies"
            }
        ));
        ui.text_disabled(state.core.cwd.display().to_string());
        ui.separator();

        let preview_n = 6usize.min(state.ui.delete_targets.len());
        for n in state.ui.delete_targets.iter().take(preview_n) {
            ui.text(n);
        }
        if state.ui.delete_targets.len() > preview_n {
            ui.text_disabled(format!(
                "... and {} more",
                state.ui.delete_targets.len() - preview_n
            ));
        }

        ui.separator();

        let any_dir = state.ui.delete_targets.iter().any(|name| {
            let p = state.core.cwd.join(name);
            fs.metadata(&p).map(|m| m.is_dir).unwrap_or(false)
        });
        if any_dir {
            ui.checkbox("Recursive", &mut state.ui.delete_recursive);
            ui.same_line();
            ui.text_disabled("Delete directories with contents");
        } else {
            state.ui.delete_recursive = false;
        }

        ui.separator();
        let del = ui.button("Delete");
        ui.same_line();
        let cancel = ui.button("Cancel");
        if cancel {
            state.ui.delete_error = None;
            state.ui.delete_targets.clear();
            state.ui.delete_recursive = false;
            ui.close_current_popup();
        }

        if del {
            state.ui.delete_error = None;
            let recursive = state.ui.delete_recursive;
            for name in &state.ui.delete_targets {
                let p = state.core.cwd.join(name);
                let is_dir = fs.metadata(&p).map(|m| m.is_dir).unwrap_or(false);
                let r = if is_dir {
                    if recursive {
                        fs.remove_dir_all(&p)
                    } else {
                        fs.remove_dir(&p)
                    }
                } else {
                    fs.remove_file(&p)
                };
                if let Err(e) = r {
                    state.ui.delete_error = Some(format!("Failed to delete '{name}': {e}"));
                    break;
                }
            }

            if state.ui.delete_error.is_none() {
                state.core.selected.clear();
                state.core.invalidate_dir_cache();
                state.ui.delete_targets.clear();
                state.ui.delete_recursive = false;
                ui.close_current_popup();
            }
        }

        if let Some(err) = &state.ui.delete_error {
            ui.separator();
            ui.text_colored([1.0, 0.3, 0.3, 1.0], err);
        }
    }
}

fn submit_path_edit(state: &mut FileDialogState, fs: &dyn FileSystem) {
    let input = state.ui.path_edit_buffer.trim();
    let raw_p = std::path::PathBuf::from(input);
    let p = fs.canonicalize(&raw_p).unwrap_or(raw_p.clone());
    match fs.metadata(&p) {
        Ok(md) => {
            if md.is_dir {
                state.core.set_cwd(p);
                state.ui.path_edit = false;
                state.ui.ui_error = None;
            } else {
                state.ui.ui_error = Some("Path exists but is not a directory".into());
            }
        }
        Err(e) => {
            use std::io::ErrorKind::*;
            let msg = match e.kind() {
                NotFound => format!("No such directory: {}", input),
                PermissionDenied => format!("Permission denied: {}", input),
                _ => format!("Invalid directory '{}': {}", input, e),
            };
            state.ui.ui_error = Some(msg);
        }
    }
}

fn draw_breadcrumbs(
    ui: &Ui,
    state: &mut FileDialogState,
    fs: &dyn FileSystem,
    max_segments: usize,
) -> Option<PathBuf> {
    // Build crumbs first to avoid borrowing cwd while mutating it
    let mut crumbs: Vec<(String, PathBuf)> = Vec::new();
    let mut acc = PathBuf::new();
    for comp in state.core.cwd.components() {
        use std::path::Component;
        match comp {
            Component::Prefix(p) => {
                acc.push(p.as_os_str());
                crumbs.push((p.as_os_str().to_string_lossy().to_string(), acc.clone()));
            }
            Component::RootDir => {
                acc.push(std::path::MAIN_SEPARATOR.to_string());
                crumbs.push((String::from(std::path::MAIN_SEPARATOR), acc.clone()));
            }
            Component::Normal(seg) => {
                acc.push(seg);
                crumbs.push((seg.to_string_lossy().to_string(), acc.clone()));
            }
            _ => {}
        }
    }
    let mut new_cwd: Option<PathBuf> = None;
    let n = crumbs.len();
    let compress = max_segments > 0 && n > max_segments && max_segments >= 3;
    if !compress {
        for (i, (label, path)) in crumbs.iter().enumerate() {
            let _id = ui.push_id(i as i32);
            if ui.button(label) {
                new_cwd = Some(path.clone());
            }
            if let Some(_popup) = ui.begin_popup_context_item() {
                ui.text_disabled(path.display().to_string());
                ui.separator();
                if ui.menu_item("Edit path...") {
                    state.ui.path_edit = true;
                    state.ui.path_edit_buffer = path.display().to_string();
                    state.ui.focus_path_edit_next = true;
                    ui.close_current_popup();
                }
            }
            ui.same_line();
            if i + 1 < n {
                if ui.small_button(">") {
                    ui.open_popup("##breadcrumb_sep_popup");
                }
                if let Some(_popup) = ui.begin_popup("##breadcrumb_sep_popup") {
                    draw_breadcrumb_sep_popup(ui, fs, path, &mut new_cwd);
                }
                ui.same_line();
            }
        }
    } else {
        // First segment
        if let Some((label, path)) = crumbs.first() {
            let _id = ui.push_id(0i32);
            if ui.button(label) {
                new_cwd = Some(path.clone());
            }
            if let Some(_popup) = ui.begin_popup_context_item() {
                ui.text_disabled(path.display().to_string());
                ui.separator();
                if ui.menu_item("Edit path...") {
                    state.ui.path_edit = true;
                    state.ui.path_edit_buffer = path.display().to_string();
                    state.ui.focus_path_edit_next = true;
                    ui.close_current_popup();
                }
            }
            ui.same_line();
            if ui.small_button(">") {
                ui.open_popup("##breadcrumb_sep_popup");
            }
            if let Some(_popup) = ui.begin_popup("##breadcrumb_sep_popup") {
                draw_breadcrumb_sep_popup(ui, fs, path, &mut new_cwd);
            }
            ui.same_line();
        }
        // Ellipsis
        ui.text("...");
        ui.same_line();
        ui.text(">");
        ui.same_line();
        // Tail segments
        let tail = max_segments - 2;
        let start_tail = n.saturating_sub(tail);
        for (i, (label, path)) in crumbs.iter().enumerate().skip(start_tail) {
            let _id = ui.push_id(i as i32);
            if ui.button(label) {
                new_cwd = Some(path.clone());
            }
            if let Some(_popup) = ui.begin_popup_context_item() {
                ui.text_disabled(path.display().to_string());
                ui.separator();
                if ui.menu_item("Edit path...") {
                    state.ui.path_edit = true;
                    state.ui.path_edit_buffer = path.display().to_string();
                    state.ui.focus_path_edit_next = true;
                    ui.close_current_popup();
                }
            }
            ui.same_line();
            if i + 1 < n {
                if ui.small_button(">") {
                    ui.open_popup("##breadcrumb_sep_popup");
                }
                if let Some(_popup) = ui.begin_popup("##breadcrumb_sep_popup") {
                    draw_breadcrumb_sep_popup(ui, fs, path, &mut new_cwd);
                }
                ui.same_line();
            }
        }
    }
    ui.new_line();
    new_cwd
}

fn draw_breadcrumb_sep_popup(
    ui: &Ui,
    fs: &dyn FileSystem,
    parent: &Path,
    out: &mut Option<PathBuf>,
) {
    let Ok(rd) = fs.read_dir(parent) else {
        ui.text_disabled("Failed to read directory");
        return;
    };
    let mut dirs: Vec<_> = rd.into_iter().filter(|e| e.is_dir).collect();
    dirs.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    if dirs.is_empty() {
        ui.text_disabled("No subdirectories");
        return;
    }

    for e in dirs {
        if ui.selectable_config(&e.name).build() {
            *out = Some(e.path);
            ui.close_current_popup();
            break;
        }
    }
}

fn draw_quick_locations(ui: &Ui, state: &mut FileDialogState) -> Option<PathBuf> {
    let mut out: Option<PathBuf> = None;

    if ui.button("+ Bookmark") {
        state.core.places.add_bookmark_path(state.core.cwd.clone());
    }
    ui.same_line();
    if ui.button("+ Group") {
        state.ui.places_edit_mode = crate::dialog_state::PlacesEditMode::AddGroup;
        state.ui.places_edit_group.clear();
        state.ui.places_edit_group_from = None;
        state.ui.places_edit_error = None;
        state.ui.places_edit_open_next = true;
        state.ui.places_edit_focus_next = true;
    }
    ui.same_line();
    if ui.button("Refresh") {
        state.core.places.refresh_system_places();
    }
    ui.same_line();
    if ui.button("Export") {
        state.ui.places_io_mode = crate::dialog_state::PlacesIoMode::Export;
        state.ui.places_io_buffer =
            state
                .core
                .places
                .serialize_compact(crate::PlacesSerializeOptions {
                    include_code_places: state.ui.places_io_include_code,
                });
        state.ui.places_io_error = None;
        state.ui.places_io_open_next = true;
    }
    ui.same_line();
    if ui.button("Import") {
        state.ui.places_io_mode = crate::dialog_state::PlacesIoMode::Import;
        state.ui.places_io_buffer.clear();
        state.ui.places_io_error = None;
        state.ui.places_io_open_next = true;
    }

    ui.separator();

    let groups = state.core.places.groups.clone();
    let mut remove_place: Option<(String, PathBuf)> = None;
    let mut edit_req: Option<PlacesEditRequest> = None;
    for (gi, g) in groups.iter().enumerate() {
        let open = ui.collapsing_header(&g.label, TreeNodeFlags::DEFAULT_OPEN);
        if let Some(_popup) = ui.begin_popup_context_item() {
            let is_system = g.label == Places::SYSTEM_GROUP;
            let is_reserved = is_system || g.label == Places::BOOKMARKS_GROUP;

            if ui.menu_item_enabled_selected("Add place...", None::<&str>, false, !is_system) {
                edit_req = Some(PlacesEditRequest::add_place(&g.label, &state.core.cwd));
                ui.close_current_popup();
            }
            if ui.menu_item_enabled_selected("Rename group...", None::<&str>, false, !is_reserved) {
                edit_req = Some(PlacesEditRequest::rename_group(&g.label));
                ui.close_current_popup();
            }
            if ui.menu_item_enabled_selected("Remove group...", None::<&str>, false, !is_reserved) {
                edit_req = Some(PlacesEditRequest::remove_group_confirm(&g.label));
                ui.close_current_popup();
            }
        }
        if !open {
            continue;
        }

        if g.places.is_empty() {
            ui.text_disabled("Empty");
            continue;
        }

        for (pi, p) in g.places.iter().enumerate() {
            let _id = ui.push_id((gi * 10_000 + pi) as i32);
            if ui.selectable_config(&p.label).build() {
                out = Some(p.path.clone());
            }
            if let Some(_popup) = ui.begin_popup_context_item() {
                ui.text_disabled(&p.path.display().to_string());
                ui.separator();
                let editable = p.origin == PlaceOrigin::User && g.label != Places::SYSTEM_GROUP;
                if ui.menu_item_enabled_selected("Edit...", None::<&str>, false, editable) {
                    edit_req = Some(PlacesEditRequest::edit_place(&g.label, p));
                    ui.close_current_popup();
                }
                if ui.menu_item_enabled_selected("Remove", None::<&str>, false, editable) {
                    remove_place = Some((g.label.clone(), p.path.clone()));
                }
            }
        }
    }
    if let Some((g, p)) = remove_place {
        state.core.places.remove_place_path(&g, &p);
    }
    if let Some(req) = edit_req {
        req.apply_to_state(&mut state.ui);
    }
    out
}

#[derive(Clone, Debug)]
struct PlacesEditRequest {
    mode: crate::dialog_state::PlacesEditMode,
    group: String,
    group_from: Option<String>,
    place_from_path: Option<PathBuf>,
    place_label: String,
    place_path: String,
    focus: bool,
}

impl PlacesEditRequest {
    fn add_place(group: &str, cwd: &Path) -> Self {
        let label = cwd
            .file_name()
            .and_then(|s| s.to_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .unwrap_or_else(|| cwd.display().to_string());
        Self {
            mode: crate::dialog_state::PlacesEditMode::AddPlace,
            group: group.to_string(),
            group_from: None,
            place_from_path: None,
            place_label: label,
            place_path: cwd.display().to_string(),
            focus: true,
        }
    }

    fn edit_place(group: &str, p: &Place) -> Self {
        Self {
            mode: crate::dialog_state::PlacesEditMode::EditPlace,
            group: group.to_string(),
            group_from: None,
            place_from_path: Some(p.path.clone()),
            place_label: p.label.clone(),
            place_path: p.path.display().to_string(),
            focus: true,
        }
    }

    fn rename_group(group: &str) -> Self {
        Self {
            mode: crate::dialog_state::PlacesEditMode::RenameGroup,
            group: group.to_string(),
            group_from: Some(group.to_string()),
            place_from_path: None,
            place_label: String::new(),
            place_path: String::new(),
            focus: true,
        }
    }

    fn remove_group_confirm(group: &str) -> Self {
        Self {
            mode: crate::dialog_state::PlacesEditMode::RemoveGroupConfirm,
            group: group.to_string(),
            group_from: Some(group.to_string()),
            place_from_path: None,
            place_label: String::new(),
            place_path: String::new(),
            focus: false,
        }
    }

    fn apply_to_state(self, ui: &mut crate::FileDialogUiState) {
        ui.places_edit_mode = self.mode;
        ui.places_edit_group = self.group;
        ui.places_edit_group_from = self.group_from;
        ui.places_edit_place_from_path = self.place_from_path;
        ui.places_edit_place_label = self.place_label;
        ui.places_edit_place_path = self.place_path;
        ui.places_edit_error = None;
        ui.places_edit_open_next = true;
        ui.places_edit_focus_next = self.focus;
    }
}

fn draw_places_io_modal(ui: &Ui, state: &mut FileDialogState) {
    if state.ui.places_io_open_next {
        ui.open_popup("Places");
        state.ui.places_io_open_next = false;
    }

    if let Some(_popup) = ui.begin_modal_popup("Places") {
        let is_export = state.ui.places_io_mode == crate::dialog_state::PlacesIoMode::Export;

        ui.text("Places persistence (compact format)");
        ui.separator();

        if ui.button("Export") {
            state.ui.places_io_mode = crate::dialog_state::PlacesIoMode::Export;
            state.ui.places_io_buffer =
                state
                    .core
                    .places
                    .serialize_compact(crate::PlacesSerializeOptions {
                        include_code_places: state.ui.places_io_include_code,
                    });
            state.ui.places_io_error = None;
        }
        ui.same_line();
        if ui.button("Import") {
            state.ui.places_io_mode = crate::dialog_state::PlacesIoMode::Import;
            state.ui.places_io_error = None;
        }
        ui.same_line();
        if ui.button("Close") {
            ui.close_current_popup();
            state.ui.places_io_error = None;
        }

        ui.separator();

        if is_export {
            let mut include_code = state.ui.places_io_include_code;
            if ui.checkbox("Include code places", &mut include_code) {
                state.ui.places_io_include_code = include_code;
                state.ui.places_io_buffer =
                    state
                        .core
                        .places
                        .serialize_compact(crate::PlacesSerializeOptions {
                            include_code_places: state.ui.places_io_include_code,
                        });
            }
        }

        let avail = ui.content_region_avail();
        let size = [avail[0].max(200.0), (avail[1] - 95.0).max(120.0)];
        if is_export {
            ui.input_text_multiline("##places_export", &mut state.ui.places_io_buffer, size)
                .read_only(true)
                .build();
        } else {
            ui.input_text_multiline("##places_import", &mut state.ui.places_io_buffer, size)
                .build();

            if ui.button("Replace") {
                match Places::deserialize_compact(&state.ui.places_io_buffer) {
                    Ok(p) => {
                        state.core.places = p;
                        state.ui.places_io_error = None;
                    }
                    Err(e) => {
                        state.ui.places_io_error = Some(e.to_string());
                    }
                }
            }
            ui.same_line();
            if ui.button("Merge") {
                match Places::deserialize_compact(&state.ui.places_io_buffer) {
                    Ok(p) => {
                        for g in p.groups {
                            for place in g.places {
                                state.core.places.add_place(g.label.clone(), place);
                            }
                        }
                        state.ui.places_io_error = None;
                    }
                    Err(e) => {
                        state.ui.places_io_error = Some(e.to_string());
                    }
                }
            }
        }

        if let Some(err) = &state.ui.places_io_error {
            ui.separator();
            ui.text_colored([1.0, 0.3, 0.3, 1.0], err);
        }
    }
}

fn draw_file_table(
    ui: &Ui,
    state: &mut FileDialogState,
    size: [f32; 2],
    fs: &dyn FileSystem,
    request_confirm: &mut bool,
    thumbnails_backend: Option<&mut ThumbnailBackend<'_>>,
) {
    match state.ui.file_list_view {
        FileListViewMode::List => {
            draw_file_table_view(ui, state, size, fs, request_confirm, thumbnails_backend)
        }
        FileListViewMode::Grid => {
            draw_file_grid_view(ui, state, size, fs, request_confirm, thumbnails_backend)
        }
    }
}

fn draw_file_table_view(
    ui: &Ui,
    state: &mut FileDialogState,
    size: [f32; 2],
    fs: &dyn FileSystem,
    request_confirm: &mut bool,
    thumbnails_backend: Option<&mut ThumbnailBackend<'_>>,
) {
    state.core.rescan_if_needed(fs);
    if state.ui.thumbnails_enabled {
        state.ui.thumbnails.advance_frame();
    }

    // Table
    use dear_imgui_rs::{SortDirection, TableColumnFlags, TableFlags};
    let flags = TableFlags::RESIZABLE
        | TableFlags::ROW_BG
        | TableFlags::BORDERS_V
        | TableFlags::BORDERS_OUTER
        | TableFlags::SCROLL_Y
        | TableFlags::SIZING_STRETCH_PROP
        | TableFlags::SORTABLE; // enable built-in header sorting
    let show_preview = state.ui.thumbnails_enabled;
    let mut table = ui.table("file_table").flags(flags).outer_size(size);
    if show_preview {
        table = table
            .column("Preview")
            .flags(TableColumnFlags::NO_SORT | TableColumnFlags::NO_RESIZE)
            .weight(0.12)
            .done();
    }
    table = table
        .column("Name")
        .flags(TableColumnFlags::PREFER_SORT_ASCENDING)
        .user_id(0)
        .weight(if show_preview { 0.52 } else { 0.56 })
        .done()
        .column("Ext")
        .flags(TableColumnFlags::PREFER_SORT_ASCENDING)
        .user_id(1)
        .weight(0.12)
        .done()
        .column("Size")
        .flags(TableColumnFlags::PREFER_SORT_DESCENDING)
        .user_id(2)
        .weight(0.16)
        .done()
        .column("Modified")
        .flags(TableColumnFlags::PREFER_SORT_DESCENDING)
        .user_id(3)
        .weight(0.2)
        .done()
        .headers(true);

    table.build(|ui| {
        // Apply ImGui sort specs (single primary sort)
        if let Some(mut specs) = ui.table_get_sort_specs() {
            if specs.is_dirty() {
                if let Some(s) = specs.iter().next() {
                    let name_col = if show_preview { 1 } else { 0 };
                    let ext_col = name_col + 1;
                    let size_col = name_col + 2;
                    let modified_col = name_col + 3;
                    let (by, asc) = match (s.column_index, s.sort_direction) {
                        (i, SortDirection::Ascending) if i == name_col => (SortBy::Name, true),
                        (i, SortDirection::Descending) if i == name_col => (SortBy::Name, false),
                        (i, SortDirection::Ascending) if i == ext_col => (SortBy::Extension, true),
                        (i, SortDirection::Descending) if i == ext_col => {
                            (SortBy::Extension, false)
                        }
                        (i, SortDirection::Ascending) if i == size_col => (SortBy::Size, true),
                        (i, SortDirection::Descending) if i == size_col => (SortBy::Size, false),
                        (i, SortDirection::Ascending) if i == modified_col => {
                            (SortBy::Modified, true)
                        }
                        (i, SortDirection::Descending) if i == modified_col => {
                            (SortBy::Modified, false)
                        }
                        _ => (state.core.sort_by, state.core.sort_ascending),
                    };
                    state.core.sort_by = by;
                    state.core.sort_ascending = asc;
                    state.core.rescan_if_needed(fs);
                }
                specs.clear_dirty();
            }
        }

        if ui.is_window_focused() && !ui.io().want_text_input() {
            let modifiers = Modifiers {
                ctrl: ui.is_key_down(Key::LeftCtrl) || ui.is_key_down(Key::RightCtrl),
                shift: ui.is_key_down(Key::LeftShift) || ui.is_key_down(Key::RightShift),
            };

            if modifiers.ctrl && ui.is_key_pressed(Key::A) && !modifiers.shift {
                state.core.select_all();
            }
            if ui.is_key_pressed_with_repeat(Key::UpArrow, true) {
                state.core.move_focus(-1, modifiers);
            }
            if ui.is_key_pressed_with_repeat(Key::DownArrow, true) {
                state.core.move_focus(1, modifiers);
            }
            if state.ui.type_select_enabled && !modifiers.ctrl && !modifiers.shift {
                handle_type_select(ui, state);
            }
        }

        // Clone the entry list so we can mutate `state.core` while iterating (selection, navigation).
        let entries: Vec<DirEntry> = state.core.entries().to_vec();
        if entries.is_empty() {
            if state.ui.empty_hint_enabled {
                ui.table_next_row();
                ui.table_next_column();
                let msg = if let Some(custom) = &state.ui.empty_hint_static_message {
                    custom.clone()
                } else {
                    let filter_label = state
                        .core
                        .active_filter
                        .and_then(|i| state.core.filters.get(i))
                        .map(|f| f.name.as_str())
                        .unwrap_or("All files");
                    let hidden_label = if state.core.show_hidden { "on" } else { "off" };
                    if state.core.search.is_empty() {
                        format!(
                            "No matching entries. Filter: {}, Hidden: {}",
                            filter_label, hidden_label
                        )
                    } else {
                        format!(
                            "No matching entries. Filter: {}, Search: '{}', Hidden: {}",
                            filter_label, state.core.search, hidden_label
                        )
                    }
                };
                ui.text_colored(state.ui.empty_hint_color, msg);
            }
            return;
        }

        for e in &entries {
            ui.table_next_row();
            if show_preview {
                ui.table_next_column();
                draw_thumbnail_cell(ui, state, e);
            }
            ui.table_next_column();

            let selected = state.core.selected.iter().any(|s| s == &e.name);
            let kind = if e.is_dir {
                EntryKind::Dir
            } else {
                EntryKind::File
            };
            let (text_color, icon, tooltip) = state
                .ui
                .file_styles
                .style_for(&e.name, kind)
                .map(|s| (s.text_color, s.icon.clone(), s.tooltip.clone()))
                .unwrap_or((None, None, None));

            let mut label = e.display_name();
            if let Some(icon) = icon.as_deref() {
                label = format!("{icon} {label}");
            }
            let _color = text_color
                .map(TextColorToken::push)
                .unwrap_or_else(TextColorToken::none);
            if ui
                .selectable_config(label)
                .selected(selected)
                .span_all_columns(false)
                .build()
            {
                let modifiers = Modifiers {
                    ctrl: ui.is_key_down(Key::LeftCtrl) || ui.is_key_down(Key::RightCtrl),
                    shift: ui.is_key_down(Key::LeftShift) || ui.is_key_down(Key::RightShift),
                };
                state.core.click_entry(e.name.clone(), e.is_dir, modifiers);
                if matches!(state.core.mode, DialogMode::SaveFile) && !e.is_dir {
                    state.core.save_name = e.name.clone();
                }
            }

            if ui.is_item_hovered() {
                if let Some(t) = tooltip.as_deref() {
                    ui.tooltip_text(t);
                }
            }

            if let Some(_popup) = ui.begin_popup_context_item() {
                if !selected {
                    state.core.focus_and_select_by_name(e.name.clone());
                }
                let can_rename = state.core.selected.len() == 1;
                if ui.menu_item_enabled_selected("Rename", Some("F2"), false, can_rename) {
                    state.ui.rename_target = Some(state.core.selected[0].clone());
                    state.ui.rename_to = state.core.selected[0].clone();
                    state.ui.rename_error = None;
                    state.ui.rename_open_next = true;
                    state.ui.rename_focus_next = true;
                    ui.close_current_popup();
                }
                if ui.menu_item_enabled_selected("Delete", Some("Del"), false, true) {
                    state.ui.delete_targets = state.core.selected.clone();
                    state.ui.delete_error = None;
                    state.ui.delete_open_next = true;
                    ui.close_current_popup();
                }
            }

            if ui.is_item_hovered() && ui.is_mouse_double_clicked(MouseButton::Left) {
                state.ui.ui_error = None;
                if state.core.double_click_entry(e.name.clone(), e.is_dir) {
                    *request_confirm = true;
                }
            }

            ui.table_next_column();
            if e.is_dir {
                ui.text("");
            } else if let Some(i) = e.name.find('.') {
                ui.text(&e.name[i..]);
            } else {
                ui.text("");
            }

            ui.table_next_column();
            ui.text(match e.size {
                Some(s) => format_size(s),
                None => String::new(),
            });

            ui.table_next_column();
            let modified_str = format_modified_ago(e.modified);
            ui.text(&modified_str);
            if ui.is_item_hovered() {
                if let Some(m) = e.modified {
                    use chrono::{DateTime, Local};
                    let dt: DateTime<Local> = DateTime::<Local>::from(m);
                    ui.tooltip_text(dt.format("%Y-%m-%d %H:%M:%S").to_string());
                }
            }

            if state.ui.reveal_name_next.as_deref() == Some(e.name.as_str()) {
                ui.set_scroll_here_y(0.5);
                state.ui.reveal_name_next = None;
            }
        }
    });

    let mut thumbnails_backend = thumbnails_backend;
    if state.ui.thumbnails_enabled {
        if let Some(backend) = thumbnails_backend.as_deref_mut() {
            state.ui.thumbnails.maintain(backend);
        }
    }
}

fn draw_file_grid_view(
    ui: &Ui,
    state: &mut FileDialogState,
    size: [f32; 2],
    fs: &dyn FileSystem,
    request_confirm: &mut bool,
    thumbnails_backend: Option<&mut ThumbnailBackend<'_>>,
) {
    state.core.rescan_if_needed(fs);
    if state.ui.thumbnails_enabled {
        state.ui.thumbnails.advance_frame();
    }

    use dear_imgui_rs::{SelectableFlags, TableColumnFlags, TableColumnSetup, TableFlags};

    let entries: Vec<DirEntry> = state.core.entries().to_vec();
    if entries.is_empty() {
        if state.ui.empty_hint_enabled {
            let msg = state
                .ui
                .empty_hint_static_message
                .clone()
                .unwrap_or_else(|| "No matching entries.".to_string());
            ui.text_colored(state.ui.empty_hint_color, msg);
        }
        return;
    }

    let thumb = state.ui.thumbnail_size;
    let pad = 6.0f32;
    let text_h = ui.text_line_height_with_spacing();
    let cell_w = (thumb[0] + pad * 2.0).max(64.0);
    let cell_h = thumb[1] + text_h + pad * 3.0;
    let cols = ((size[0].max(1.0)) / cell_w).floor() as usize;
    let cols = cols.clamp(1, 16);

    let flags = TableFlags::SCROLL_Y
        | TableFlags::SIZING_FIXED_FIT
        | TableFlags::NO_PAD_OUTER_X
        | TableFlags::NO_PAD_INNER_X;
    let mut col_setups = Vec::with_capacity(cols);
    for i in 0..cols {
        col_setups.push(
            TableColumnSetup::new(format!("##grid_col_{i}"))
                .flags(TableColumnFlags::NO_SORT | TableColumnFlags::NO_RESIZE)
                .init_width_or_weight(cell_w),
        );
    }

    ui.table("file_grid")
        .flags(flags)
        .outer_size(size)
        .columns(col_setups)
        .headers(false)
        .build(|ui| {
            let dl = ui.get_window_draw_list();

            if ui.is_window_focused() && !ui.io().want_text_input() {
                let modifiers = Modifiers {
                    ctrl: ui.is_key_down(Key::LeftCtrl) || ui.is_key_down(Key::RightCtrl),
                    shift: ui.is_key_down(Key::LeftShift) || ui.is_key_down(Key::RightShift),
                };

                if modifiers.ctrl && ui.is_key_pressed(Key::A) && !modifiers.shift {
                    state.core.select_all();
                }
                if ui.is_key_pressed_with_repeat(Key::LeftArrow, true) {
                    state.core.move_focus(-1, modifiers);
                }
                if ui.is_key_pressed_with_repeat(Key::RightArrow, true) {
                    state.core.move_focus(1, modifiers);
                }
                if ui.is_key_pressed_with_repeat(Key::UpArrow, true) {
                    state.core.move_focus(-(cols as i32), modifiers);
                }
                if ui.is_key_pressed_with_repeat(Key::DownArrow, true) {
                    state.core.move_focus(cols as i32, modifiers);
                }
                if state.ui.type_select_enabled && !modifiers.ctrl && !modifiers.shift {
                    handle_type_select(ui, state);
                }
            }

            let mut idx = 0usize;
            while idx < entries.len() {
                ui.table_next_row();
                for _ in 0..cols {
                    ui.table_next_column();
                    if idx >= entries.len() {
                        break;
                    }
                    let item_idx = idx;
                    let e = &entries[item_idx];
                    idx += 1;

                    let selected = state.core.selected.iter().any(|s| s == &e.name);
                    let kind = if e.is_dir {
                        EntryKind::Dir
                    } else {
                        EntryKind::File
                    };
                    let (text_color, icon, tooltip) = state
                        .ui
                        .file_styles
                        .style_for(&e.name, kind)
                        .map(|s| (s.text_color, s.icon.clone(), s.tooltip.clone()))
                        .unwrap_or((None, None, None));

                    let mut label = e.display_name();
                    if let Some(icon) = icon.as_deref() {
                        label = format!("{icon} {label}");
                    }

                    let _id = ui.push_id(item_idx as i32);
                    let clicked = ui
                        .selectable_config("##grid_item")
                        .selected(selected)
                        .flags(SelectableFlags::ALLOW_OVERLAP)
                        .size([cell_w, cell_h])
                        .build();

                    let item_min = ui.item_rect_min();
                    let item_max = ui.item_rect_max();
                    let img_min = [item_min[0] + pad, item_min[1] + pad];
                    let img_max = [img_min[0] + thumb[0], img_min[1] + thumb[1]];

                    if state.ui.reveal_name_next.as_deref() == Some(e.name.as_str()) {
                        ui.set_scroll_here_y(0.5);
                        state.ui.reveal_name_next = None;
                    }

                    if state.ui.thumbnails_enabled && !e.is_dir {
                        let max_size_u32 = [thumb[0].max(1.0) as u32, thumb[1].max(1.0) as u32];
                        if let Some(tex) = state.ui.thumbnails.texture_id(&e.path) {
                            dl.add_image(
                                tex,
                                img_min,
                                img_max,
                                [0.0, 0.0],
                                [1.0, 1.0],
                                dear_imgui_rs::Color::rgb(1.0, 1.0, 1.0),
                            );
                        } else {
                            dl.add_rect(
                                img_min,
                                img_max,
                                dear_imgui_rs::Color::new(0.2, 0.2, 0.2, 1.0),
                            )
                            .filled(true)
                            .build();
                            if ui.is_item_visible() {
                                state.ui.thumbnails.request_visible(&e.path, max_size_u32);
                            }
                        }
                    } else {
                        dl.add_rect(
                            img_min,
                            img_max,
                            dear_imgui_rs::Color::new(0.2, 0.2, 0.2, 1.0),
                        )
                        .filled(true)
                        .build();
                    }

                    let text_pos = [item_min[0] + pad, img_max[1] + pad];
                    let col = text_color
                        .map(|c| dear_imgui_rs::Color::from_array(c))
                        .unwrap_or_else(|| dear_imgui_rs::Color::rgb(1.0, 1.0, 1.0));
                    dl.with_clip_rect(item_min, item_max, || {
                        dl.add_text(text_pos, col, &label);
                    });

                    if clicked {
                        let modifiers = Modifiers {
                            ctrl: ui.is_key_down(Key::LeftCtrl) || ui.is_key_down(Key::RightCtrl),
                            shift: ui.is_key_down(Key::LeftShift)
                                || ui.is_key_down(Key::RightShift),
                        };
                        state.core.click_entry(e.name.clone(), e.is_dir, modifiers);
                        if matches!(state.core.mode, DialogMode::SaveFile) && !e.is_dir {
                            state.core.save_name = e.name.clone();
                        }
                    }

                    if ui.is_item_hovered() {
                        if let Some(t) = tooltip.as_deref() {
                            ui.tooltip_text(t);
                        }
                    }

                    if let Some(_popup) = ui.begin_popup_context_item() {
                        if !selected {
                            state.core.focus_and_select_by_name(e.name.clone());
                        }
                        let can_rename = state.core.selected.len() == 1;
                        if ui.menu_item_enabled_selected("Rename", Some("F2"), false, can_rename) {
                            state.ui.rename_target = Some(state.core.selected[0].clone());
                            state.ui.rename_to = state.core.selected[0].clone();
                            state.ui.rename_error = None;
                            state.ui.rename_open_next = true;
                            state.ui.rename_focus_next = true;
                            ui.close_current_popup();
                        }
                        if ui.menu_item_enabled_selected("Delete", Some("Del"), false, true) {
                            state.ui.delete_targets = state.core.selected.clone();
                            state.ui.delete_error = None;
                            state.ui.delete_open_next = true;
                            ui.close_current_popup();
                        }
                    }

                    if ui.is_item_hovered() && ui.is_mouse_double_clicked(MouseButton::Left) {
                        state.ui.ui_error = None;
                        if state.core.double_click_entry(e.name.clone(), e.is_dir) {
                            *request_confirm = true;
                        }
                    }
                }
            }
        });

    let mut thumbnails_backend = thumbnails_backend;
    if state.ui.thumbnails_enabled {
        if let Some(backend) = thumbnails_backend.as_deref_mut() {
            state.ui.thumbnails.maintain(backend);
        }
    }
}

fn draw_thumbnail_cell(ui: &Ui, state: &mut FileDialogState, e: &DirEntry) {
    if e.is_dir {
        ui.text("");
        return;
    }

    let max_size_u32 = [
        state.ui.thumbnail_size[0].max(1.0) as u32,
        state.ui.thumbnail_size[1].max(1.0) as u32,
    ];
    let size = state.ui.thumbnail_size;

    if let Some(tex) = state.ui.thumbnails.texture_id(&e.path) {
        ui.image(tex, size);
        return;
    }

    ui.text_disabled("...");
    if ui.is_item_visible() {
        state.ui.thumbnails.request_visible(&e.path, max_size_u32);
    }
}

fn handle_type_select(ui: &Ui, state: &mut FileDialogState) {
    if !state.ui.type_select_enabled {
        return;
    }
    let now = Instant::now();
    let timeout = Duration::from_millis(state.ui.type_select_timeout_ms);
    if let Some(last) = state.ui.type_select_last_input {
        if now.duration_since(last) > timeout {
            state.ui.type_select_buffer.clear();
        }
    }

    let Some(ch) = collect_type_select_char(ui) else {
        return;
    };
    if ch.is_whitespace() {
        return;
    }
    state.ui.type_select_buffer.push(ch.to_ascii_lowercase());
    state.ui.type_select_last_input = Some(now);
    state.core.select_by_prefix(&state.ui.type_select_buffer);
}

fn collect_type_select_char(ui: &Ui) -> Option<char> {
    let alpha = [
        (Key::A, 'a'),
        (Key::B, 'b'),
        (Key::C, 'c'),
        (Key::D, 'd'),
        (Key::E, 'e'),
        (Key::F, 'f'),
        (Key::G, 'g'),
        (Key::H, 'h'),
        (Key::I, 'i'),
        (Key::J, 'j'),
        (Key::K, 'k'),
        (Key::L, 'l'),
        (Key::M, 'm'),
        (Key::N, 'n'),
        (Key::O, 'o'),
        (Key::P, 'p'),
        (Key::Q, 'q'),
        (Key::R, 'r'),
        (Key::S, 's'),
        (Key::T, 't'),
        (Key::U, 'u'),
        (Key::V, 'v'),
        (Key::W, 'w'),
        (Key::X, 'x'),
        (Key::Y, 'y'),
        (Key::Z, 'z'),
    ];
    for (k, c) in alpha {
        if ui.is_key_pressed(k) {
            return Some(c);
        }
    }

    let digits = [
        (Key::Key0, '0'),
        (Key::Key1, '1'),
        (Key::Key2, '2'),
        (Key::Key3, '3'),
        (Key::Key4, '4'),
        (Key::Key5, '5'),
        (Key::Key6, '6'),
        (Key::Key7, '7'),
        (Key::Key8, '8'),
        (Key::Key9, '9'),
    ];
    for (k, c) in digits {
        if ui.is_key_pressed(k) {
            return Some(c);
        }
    }

    let punct = [(Key::Minus, '-'), (Key::Period, '.'), (Key::Slash, '/')];
    for (k, c) in punct {
        if ui.is_key_pressed(k) {
            return Some(c);
        }
    }

    None
}

fn format_size(size: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let s = size as f64;
    if s >= GB {
        format!("{:.2} GB", s / GB)
    } else if s >= MB {
        format!("{:.2} MB", s / MB)
    } else if s >= KB {
        format!("{:.0} KB", s / KB)
    } else {
        format!("{} B", size)
    }
}

fn format_modified_ago(modified: Option<std::time::SystemTime>) -> String {
    use std::time::SystemTime;
    let m = match modified {
        Some(t) => t,
        None => return String::new(),
    };
    let now = SystemTime::now();
    let delta = match now.duration_since(m) {
        Ok(d) => d,
        Err(e) => e.duration(),
    };
    // For older than a week, show short absolute date inline; full datetime remains in tooltip
    const DAY: u64 = 24 * 60 * 60;
    const WEEK: u64 = 7 * DAY;
    if delta.as_secs() >= WEEK {
        use chrono::{DateTime, Local};
        let dt: DateTime<Local> = DateTime::<Local>::from(m);
        return dt.format("%Y-%m-%d").to_string();
    }
    humanize_duration(delta)
}

fn humanize_duration(d: std::time::Duration) -> String {
    let secs = d.as_secs();
    const MIN: u64 = 60;
    const HOUR: u64 = 60 * MIN;
    const DAY: u64 = 24 * HOUR;
    const WEEK: u64 = 7 * DAY;
    if secs < 10 {
        return "just now".into();
    }
    if secs < MIN {
        return format!("{}s ago", secs);
    }
    if secs < HOUR {
        return format!("{}m ago", secs / MIN);
    }
    if secs < DAY {
        return format!("{}h ago", secs / HOUR);
    }
    if secs < WEEK {
        return format!("{}d ago", secs / DAY);
    }
    let days = secs / DAY;
    format!("{}d ago", days)
}

fn draw_places_edit_modal(ui: &Ui, state: &mut FileDialogState, fs: &dyn FileSystem) {
    const POPUP_ID: &str = "Edit Places";
    if state.ui.places_edit_open_next {
        ui.open_popup(POPUP_ID);
        state.ui.places_edit_open_next = false;
    }

    let Some(_popup) = ui.begin_modal_popup(POPUP_ID) else {
        return;
    };

    use crate::dialog_state::PlacesEditMode;
    let mode = state.ui.places_edit_mode;
    match mode {
        PlacesEditMode::AddGroup => {
            ui.text("Create a new places group:");
            ui.separator();
            if state.ui.places_edit_focus_next {
                ui.set_keyboard_focus_here();
                state.ui.places_edit_focus_next = false;
            }
            ui.input_text("Group", &mut state.ui.places_edit_group)
                .build();

            let create = ui.button("Create");
            ui.same_line();
            let cancel = ui.button("Cancel");
            if cancel {
                state.ui.places_edit_error = None;
                ui.close_current_popup();
                return;
            }
            if create {
                state.ui.places_edit_error = None;
                let label = state.ui.places_edit_group.trim();
                if label.is_empty() {
                    state.ui.places_edit_error = Some("Group name is empty".into());
                } else if label == Places::SYSTEM_GROUP || label == Places::BOOKMARKS_GROUP {
                    state.ui.places_edit_error = Some("Group name is reserved".into());
                } else if state.core.places.groups.iter().any(|g| g.label == label) {
                    state.ui.places_edit_error = Some("Group already exists".into());
                } else {
                    state.core.places.add_group(label.to_string());
                    ui.close_current_popup();
                }
            }
        }
        PlacesEditMode::RenameGroup => {
            let Some(from) = state.ui.places_edit_group_from.clone() else {
                ui.text_disabled("Missing source group.");
                if ui.button("Close") {
                    ui.close_current_popup();
                }
                return;
            };
            ui.text("Rename group:");
            ui.text_disabled(&from);
            ui.separator();
            if state.ui.places_edit_focus_next {
                ui.set_keyboard_focus_here();
                state.ui.places_edit_focus_next = false;
            }
            ui.input_text("To", &mut state.ui.places_edit_group).build();

            let rename = ui.button("Rename");
            ui.same_line();
            let cancel = ui.button("Cancel");
            if cancel {
                state.ui.places_edit_error = None;
                ui.close_current_popup();
                return;
            }
            if rename {
                state.ui.places_edit_error = None;
                let to = state.ui.places_edit_group.trim();
                if to.is_empty() {
                    state.ui.places_edit_error = Some("Target group name is empty".into());
                } else if to == Places::SYSTEM_GROUP || to == Places::BOOKMARKS_GROUP {
                    state.ui.places_edit_error = Some("Target group name is reserved".into());
                } else if to == from.as_str() {
                    state.ui.places_edit_error = Some("Target group name is unchanged".into());
                } else if state.core.places.groups.iter().any(|g| g.label == to) {
                    state.ui.places_edit_error = Some("Target group already exists".into());
                } else if !state.core.places.rename_group(&from, to.to_string()) {
                    state.ui.places_edit_error = Some("Group not found".into());
                } else {
                    ui.close_current_popup();
                }
            }
        }
        PlacesEditMode::RemoveGroupConfirm => {
            let Some(group) = state.ui.places_edit_group_from.clone() else {
                ui.text_disabled("Missing group.");
                if ui.button("Close") {
                    ui.close_current_popup();
                }
                return;
            };

            let places_count = state
                .core
                .places
                .groups
                .iter()
                .find(|g| g.label == group)
                .map(|g| g.places.len())
                .unwrap_or(0);

            ui.text("Remove group?");
            ui.separator();
            ui.text(format!("Group: {group}"));
            ui.text_disabled(format!("Places: {places_count}"));
            ui.separator();
            let remove = ui.button("Remove");
            ui.same_line();
            let cancel = ui.button("Cancel");
            if cancel {
                state.ui.places_edit_error = None;
                ui.close_current_popup();
                return;
            }
            if remove {
                state.ui.places_edit_error = None;
                if group == Places::SYSTEM_GROUP || group == Places::BOOKMARKS_GROUP {
                    state.ui.places_edit_error = Some("Cannot remove reserved group".into());
                } else if !state.core.places.remove_group(&group) {
                    state.ui.places_edit_error = Some("Group not found".into());
                } else {
                    ui.close_current_popup();
                }
            }
        }
        PlacesEditMode::AddPlace | PlacesEditMode::EditPlace => {
            let is_add = mode == PlacesEditMode::AddPlace;
            let group = state.ui.places_edit_group.clone();
            ui.text(if is_add { "Add place:" } else { "Edit place:" });
            ui.text_disabled(&group);
            ui.separator();

            if state.ui.places_edit_focus_next {
                ui.set_keyboard_focus_here();
                state.ui.places_edit_focus_next = false;
            }
            ui.input_text("Label", &mut state.ui.places_edit_place_label)
                .build();
            ui.input_text("Path", &mut state.ui.places_edit_place_path)
                .build();

            let ok_label = if is_add { "Add" } else { "Save" };
            let ok = ui.button(ok_label);
            ui.same_line();
            let cancel = ui.button("Cancel");
            if cancel {
                state.ui.places_edit_error = None;
                ui.close_current_popup();
                return;
            }

            if ok {
                state.ui.places_edit_error = None;
                let path_s = state.ui.places_edit_place_path.trim();
                if path_s.is_empty() {
                    state.ui.places_edit_error = Some("Path is empty".into());
                } else {
                    let raw = PathBuf::from(path_s);
                    let p = fs.canonicalize(&raw).unwrap_or(raw);
                    let is_dir = fs.metadata(&p).map(|m| m.is_dir).unwrap_or(false);
                    if !is_dir {
                        state.ui.places_edit_error =
                            Some("Path does not exist or is not a directory".into());
                    } else {
                        let mut label = state.ui.places_edit_place_label.trim().to_string();
                        if label.is_empty() {
                            label = p
                                .file_name()
                                .and_then(|s| s.to_str())
                                .filter(|s| !s.is_empty())
                                .map(|s| s.to_string())
                                .unwrap_or_else(|| p.display().to_string());
                        }

                        let group_places = state
                            .core
                            .places
                            .groups
                            .iter()
                            .find(|g| g.label == group)
                            .map(|g| g.places.clone())
                            .unwrap_or_default();

                        let from_path = state.ui.places_edit_place_from_path.clone();
                        let is_duplicate = group_places.iter().any(|x| {
                            if let Some(from) = &from_path {
                                if x.path == *from {
                                    return false;
                                }
                            }
                            x.path == p
                        });
                        if is_duplicate {
                            state.ui.places_edit_error =
                                Some("Place already exists in group".into());
                        } else if is_add {
                            state
                                .core
                                .places
                                .add_place(group, Place::new(label, p, PlaceOrigin::User));
                            ui.close_current_popup();
                        } else {
                            let Some(from_path) = from_path else {
                                state.ui.places_edit_error = Some("Missing source place".into());
                                return;
                            };
                            if !state
                                .core
                                .places
                                .edit_place_by_path(&group, &from_path, label, p)
                            {
                                state.ui.places_edit_error = Some("Place not found".into());
                            } else {
                                ui.close_current_popup();
                            }
                        }
                    }
                }
            }
        }
    }

    if let Some(err) = &state.ui.places_edit_error {
        ui.separator();
        ui.text_colored([1.0, 0.3, 0.3, 1.0], err);
    }
}

// Places helpers live in `places.rs`.
