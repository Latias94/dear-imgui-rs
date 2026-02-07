use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use dear_imgui_rs::Direction;
use dear_imgui_rs::StyleVar;
use dear_imgui_rs::TreeNodeFlags;
use dear_imgui_rs::Ui;
use dear_imgui_rs::input::{Key, MouseButton, MouseCursor};
use dear_imgui_rs::sys;
use dear_imgui_rs::{
    HistoryDirection, InputTextCallback, InputTextCallbackHandler, TextCallbackData,
};

use crate::core::{
    ClickAction, DialogMode, ExtensionPolicy, FileDialogError, LayoutStyle, Selection, SortBy,
    SortMode,
};
use crate::custom_pane::{CustomPane, CustomPaneCtx};
use crate::dialog_core::{
    ConfirmGate, CoreEvent, CoreEventOutcome, DirEntry, EntryId, Modifiers, ScanStatus,
};
use crate::dialog_state::FileDialogState;
use crate::dialog_state::{
    ClipboardOp, CustomPaneDock, FileClipboard, FileListColumnsConfig, FileListDataColumn,
    FileListViewMode, PasteConflictAction, PasteConflictPrompt, PendingPasteJob, ToolbarDensity,
    ToolbarIconMode,
};
use crate::dialog_state::{ValidationButtonsAlign, ValidationButtonsOrder};
use crate::file_style::EntryKind;
use crate::fs::{FileSystem, StdFileSystem};
use crate::fs_ops::{
    ExistingTargetDecision, ExistingTargetPolicy, apply_existing_target_policy, copy_tree,
    move_tree,
};
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
    /// Optional minimum size constraint.
    pub min_size: Option<[f32; 2]>,
    /// Optional maximum size constraint.
    pub max_size: Option<[f32; 2]>,
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
            min_size: None,
            max_size: None,
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
    /// Optional minimum size constraint.
    pub min_size: Option<[f32; 2]>,
    /// Optional maximum size constraint.
    pub max_size: Option<[f32; 2]>,
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
            min_size: None,
            max_size: None,
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
        self.draw_contents_with(state, &StdFileSystem, None, None)
    }

    /// Draw only the contents of the file browser (no window/modal host) with explicit hooks.
    ///
    /// - `fs`: filesystem backend used by core operations.
    /// - `custom_pane`: optional custom pane that can render extra UI and block confirm.
    /// - `thumbnails_backend`: optional backend for thumbnail decode/upload lifecycle.
    pub fn draw_contents_with(
        &self,
        state: &mut FileDialogState,
        fs: &dyn FileSystem,
        mut custom_pane: Option<&mut dyn CustomPane>,
        mut thumbnails_backend: Option<&mut ThumbnailBackend<'_>>,
    ) -> Option<Result<Selection, FileDialogError>> {
        draw_contents_with_fs_and_hooks(
            self.ui,
            state,
            fs,
            custom_pane.take(),
            thumbnails_backend.take(),
        )
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
        self.show_windowed_with(state, cfg, &StdFileSystem, None, None)
    }

    /// Draw the file browser in a standard ImGui window with explicit hooks.
    pub fn show_windowed_with(
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
        let mut window = self
            .ui
            .window(&cfg.title)
            .size(cfg.initial_size, cfg.size_condition);
        if let Some((min_size, max_size)) =
            resolve_host_size_constraints(cfg.min_size, cfg.max_size)
        {
            window = window.size_constraints(min_size, max_size);
        }
        window.build(|| {
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
        self.show_modal_with(state, &cfg, &StdFileSystem, None, None)
    }

    /// Draw the file browser in an ImGui modal popup with explicit hooks.
    pub fn show_modal_with(
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

        if let Some((min_size, max_size)) =
            resolve_host_size_constraints(cfg.min_size, cfg.max_size)
        {
            unsafe {
                let min_vec = sys::ImVec2_c {
                    x: min_size[0],
                    y: min_size[1],
                };
                let max_vec = sys::ImVec2_c {
                    x: max_size[0],
                    y: max_size[1],
                };
                sys::igSetNextWindowSizeConstraints(min_vec, max_vec, None, std::ptr::null_mut());
            }
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
}

struct TextColorToken {
    pushed: bool,
}

struct StyleVisual {
    text_color: Option<[f32; 4]>,
    icon: Option<String>,
    tooltip: Option<String>,
    font_id: Option<dear_imgui_rs::FontId>,
}

fn resolve_host_size_constraints(
    min_size: Option<[f32; 2]>,
    max_size: Option<[f32; 2]>,
) -> Option<([f32; 2], [f32; 2])> {
    if min_size.is_none() && max_size.is_none() {
        return None;
    }

    let sanitize = |value: f32, fallback: f32| -> f32 {
        if value.is_finite() {
            value.max(0.0)
        } else {
            fallback
        }
    };

    let mut min = min_size.unwrap_or([0.0, 0.0]);
    min[0] = sanitize(min[0], 0.0);
    min[1] = sanitize(min[1], 0.0);

    let mut max = max_size.unwrap_or([f32::MAX, f32::MAX]);
    max[0] = sanitize(max[0], f32::MAX);
    max[1] = sanitize(max[1], f32::MAX);

    max[0] = max[0].max(min[0]);
    max[1] = max[1].max(min[1]);

    Some((min, max))
}

fn style_visual_for_entry(state: &mut FileDialogState, e: &DirEntry) -> StyleVisual {
    let kind = if e.is_symlink {
        EntryKind::Link
    } else if e.is_dir {
        EntryKind::Dir
    } else {
        EntryKind::File
    };
    let style = state.ui.file_styles.style_for_owned(&e.name, kind);
    let font_id = style
        .as_ref()
        .and_then(|s| s.font_token.as_deref())
        .and_then(|token| state.ui.file_style_fonts.get(token).copied());

    StyleVisual {
        text_color: style.as_ref().and_then(|s| s.text_color),
        icon: style.as_ref().and_then(|s| s.icon.clone()),
        tooltip: style.as_ref().and_then(|s| s.tooltip.clone()),
        font_id,
    }
}

fn toolbar_label(id: &str, text: &str, icon: Option<&str>, mode: ToolbarIconMode) -> String {
    let display = match mode {
        ToolbarIconMode::Text => text.to_string(),
        ToolbarIconMode::IconOnly => icon.unwrap_or(text).to_string(),
        ToolbarIconMode::IconAndText => {
            icon.map_or_else(|| text.to_string(), |icon| format!("{icon} {text}"))
        }
    };
    format!("{display}###{id}")
}

fn toolbar_button(
    ui: &Ui,
    id: &str,
    text: &str,
    icon: Option<&str>,
    mode: ToolbarIconMode,
    show_tooltips: bool,
    tooltip: &str,
) -> bool {
    let clicked = ui.button(toolbar_label(id, text, icon, mode));
    if show_tooltips && !tooltip.is_empty() && ui.is_item_hovered() {
        ui.tooltip_text(tooltip);
    }
    clicked
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

    let has_thumbnail_backend = thumbnails_backend.is_some();
    let mut request_confirm = false;
    let mut confirm_gate = ConfirmGate::default();

    // Chrome density scope (toolbar + address bar).
    {
        let show_tooltips = state.ui.toolbar.show_tooltips;
        let icon_mode = state.ui.toolbar.icons.mode;
        let chrome_style = ui.clone_style();
        let (scale, min) = match state.ui.toolbar.density {
            ToolbarDensity::Normal => (1.0, 0.0),
            ToolbarDensity::Compact => (0.82, 1.0),
            ToolbarDensity::Spacious => (1.18, 0.0),
        };
        let scale_vec2 =
            |v: [f32; 2]| -> [f32; 2] { [(v[0] * scale).max(min), (v[1] * scale).max(min)] };
        let _frame_padding = ui.push_style_var(StyleVar::FramePadding(scale_vec2(
            chrome_style.frame_padding(),
        )));
        let _item_spacing = ui.push_style_var(StyleVar::ItemSpacing(scale_vec2(
            chrome_style.item_spacing(),
        )));
        let _item_inner_spacing = ui.push_style_var(StyleVar::ItemInnerSpacing(scale_vec2(
            chrome_style.item_inner_spacing(),
        )));

        // Top toolbar: Back/Forward/Up/Refresh, view, sort, etc.
        let can_back = state.core.can_navigate_back();
        let can_forward = state.core.can_navigate_forward();
        {
            let _disabled = ui.begin_disabled_with_cond(!can_back);
            if ui.arrow_button("##nav_back", Direction::Left) {
                let _ = state.core.handle_event(CoreEvent::NavigateBack);
            }
        }
        if show_tooltips && ui.is_item_hovered() {
            ui.tooltip_text("Back (Alt+Left)");
        }
        ui.same_line();
        {
            let _disabled = ui.begin_disabled_with_cond(!can_forward);
            if ui.arrow_button("##nav_forward", Direction::Right) {
                let _ = state.core.handle_event(CoreEvent::NavigateForward);
            }
        }
        if show_tooltips && ui.is_item_hovered() {
            ui.tooltip_text("Forward (Alt+Right)");
        }
        ui.same_line();
        if ui.arrow_button("##nav_up", Direction::Up) {
            let _ = state.core.handle_event(CoreEvent::NavigateUp);
        }
        if show_tooltips && ui.is_item_hovered() {
            ui.tooltip_text("Up (Backspace)");
        }
        ui.same_line();
        if toolbar_button(
            ui,
            "toolbar_refresh",
            "Refresh",
            state.ui.toolbar.icons.refresh.as_deref(),
            icon_mode,
            show_tooltips,
            "Refresh (F5)",
        ) {
            let _ = state.core.handle_event(CoreEvent::Refresh);
        }
        ui.same_line();
        if state.ui.new_folder_enabled {
            if toolbar_button(
                ui,
                "toolbar_new_folder",
                "New Folder",
                state.ui.toolbar.icons.new_folder.as_deref(),
                icon_mode,
                show_tooltips,
                "New folder",
            ) {
                state.ui.new_folder_open_next = true;
                state.ui.new_folder_name.clear();
                state.ui.new_folder_error = None;
                state.ui.new_folder_focus_next = true;
            }
            ui.same_line();
        }
        ui.text("View:");
        ui.same_line();
        let view_preview = match state.ui.file_list_view {
            FileListViewMode::List => "List",
            FileListViewMode::ThumbnailsList => "Thumbs",
            FileListViewMode::Grid => "Grid",
        };
        if let Some(_c) = ui.begin_combo("##view_mode", view_preview) {
            if ui
                .selectable_config("List")
                .selected(matches!(state.ui.file_list_view, FileListViewMode::List))
                .build()
            {
                state.ui.file_list_view = FileListViewMode::List;
            }
            if ui
                .selectable_config("Thumbs")
                .selected(matches!(
                    state.ui.file_list_view,
                    FileListViewMode::ThumbnailsList
                ))
                .build()
            {
                state.ui.file_list_view = FileListViewMode::ThumbnailsList;
                state.ui.thumbnails_enabled = true;
                state.ui.file_list_columns.show_preview = true;
            }
            if ui
                .selectable_config("Grid")
                .selected(matches!(state.ui.file_list_view, FileListViewMode::Grid))
                .build()
            {
                state.ui.file_list_view = FileListViewMode::Grid;
                state.ui.thumbnails_enabled = true;
            }
        }

        if matches!(
            state.ui.file_list_view,
            FileListViewMode::ThumbnailsList | FileListViewMode::Grid
        ) {
            state.ui.thumbnails_enabled = true;
        }

        if matches!(state.ui.file_list_view, FileListViewMode::Grid) {
            ui.same_line();
            ui.text("Sort:");
            ui.same_line();
            let preview = format!(
                "{} {}",
                match state.core.sort_by {
                    SortBy::Name => "Name",
                    SortBy::Extension => "Ext",
                    SortBy::Size => "Size",
                    SortBy::Modified => "Modified",
                },
                if state.core.sort_ascending {
                    "↑"
                } else {
                    "↓"
                }
            );
            let mut next_by = state.core.sort_by;
            let mut next_asc = state.core.sort_ascending;
            if let Some(_c) = ui.begin_combo("##grid_sort", &preview) {
                let items = [
                    (SortBy::Name, "Name"),
                    (SortBy::Extension, "Ext"),
                    (SortBy::Size, "Size"),
                    (SortBy::Modified, "Modified"),
                ];
                for (by, label) in items {
                    if ui.selectable_config(label).selected(next_by == by).build() {
                        next_by = by;
                    }
                }
                ui.separator();
                if ui.selectable_config("Ascending").selected(next_asc).build() {
                    next_asc = true;
                }
                if ui
                    .selectable_config("Descending")
                    .selected(!next_asc)
                    .build()
                {
                    next_asc = false;
                }
            }
            if next_by != state.core.sort_by || next_asc != state.core.sort_ascending {
                state.core.sort_by = next_by;
                state.core.sort_ascending = next_asc;
            }
        }

        if state.ui.thumbnails_enabled {
            ui.same_line();
            ui.text("Thumb:");
            ui.same_line();
            if ui.small_button("S") {
                state.ui.thumbnail_size = [20.0, 20.0];
            }
            ui.same_line();
            if ui.small_button("M") {
                state.ui.thumbnail_size = [32.0, 32.0];
            }
            ui.same_line();
            if ui.small_button("L") {
                state.ui.thumbnail_size = [48.0, 48.0];
            }
            if !has_thumbnail_backend {
                ui.same_line();
                ui.text_disabled("No thumbnail backend");
            }
        }

        if matches!(
            state.ui.file_list_view,
            FileListViewMode::List | FileListViewMode::ThumbnailsList
        ) {
            ui.same_line();
            let columns_label = match (icon_mode, state.ui.toolbar.icons.columns.as_deref()) {
                (ToolbarIconMode::IconOnly, Some(icon)) => format!("{icon}###columns"),
                (ToolbarIconMode::IconAndText, Some(icon)) => format!("{icon} Columns###columns"),
                _ => "Columns###columns".to_string(),
            };
            if let Some(_popup) = ui.begin_combo(&columns_label, "Configure") {
                match state.ui.file_list_view {
                    FileListViewMode::List => {
                        let mut enabled = state.ui.thumbnails_enabled;
                        if ui.checkbox("Enable thumbnails", &mut enabled) {
                            state.ui.thumbnails_enabled = enabled;
                        }
                        if state.ui.thumbnails_enabled {
                            ui.checkbox("Preview", &mut state.ui.file_list_columns.show_preview);
                        } else {
                            ui.text_disabled("Preview (enable thumbnails)");
                        }
                    }
                    FileListViewMode::ThumbnailsList => {
                        ui.text_disabled("Preview (forced by Thumbs view)");
                    }
                    FileListViewMode::Grid => {}
                }
                ui.checkbox("Ext", &mut state.ui.file_list_columns.show_extension);
                ui.checkbox("Size", &mut state.ui.file_list_columns.show_size);
                ui.checkbox("Modified", &mut state.ui.file_list_columns.show_modified);

                ui.separator();
                if ui.small_button("Compact") {
                    if matches!(state.ui.file_list_view, FileListViewMode::ThumbnailsList) {
                        apply_compact_column_layout_keep_preview(&mut state.ui.file_list_columns);
                    } else {
                        apply_compact_column_layout(&mut state.ui.file_list_columns);
                    }
                }
                ui.same_line();
                if ui.small_button("Balanced") {
                    if matches!(state.ui.file_list_view, FileListViewMode::ThumbnailsList) {
                        apply_balanced_column_layout_keep_preview(&mut state.ui.file_list_columns);
                    } else {
                        apply_balanced_column_layout(&mut state.ui.file_list_columns);
                    }
                }

                ui.separator();
                ui.text("Order:");
                let mut order = state.ui.file_list_columns.normalized_order();
                let mut changed = false;
                for index in 0..order.len() {
                    let column = order[index];
                    let mut label = data_column_label(column).to_string();
                    if !is_data_column_visible(&state.ui.file_list_columns, column) {
                        label.push_str(" (hidden)");
                    }
                    ui.text(label);
                    ui.same_line();
                    if ui.small_button(format!("Up##col_order_up_{index}")) {
                        changed |= move_column_order_up(&mut order, index);
                    }
                    ui.same_line();
                    if ui.small_button(format!("Down##col_order_down_{index}")) {
                        changed |= move_column_order_down(&mut order, index);
                    }
                }
                if changed {
                    state.ui.file_list_columns.order = order;
                }

                if ui.small_button("Reset columns") {
                    state.ui.file_list_columns = FileListColumnsConfig::default();
                }

                ui.separator();
                let mut natural_sort = matches!(state.core.sort_mode, SortMode::Natural);
                if ui.checkbox("Natural sort", &mut natural_sort) {
                    state.core.sort_mode = if natural_sort {
                        SortMode::Natural
                    } else {
                        SortMode::Lexicographic
                    };
                }
            }
            if show_tooltips && ui.is_item_hovered() {
                ui.tooltip_text("Columns");
            }
        }
        ui.same_line();
        let mut show_hidden = state.core.show_hidden;
        if ui.checkbox("Hidden", &mut show_hidden) {
            state.core.show_hidden = show_hidden;
        }

        ui.same_line();
        if toolbar_button(
            ui,
            "toolbar_options",
            "Options",
            state.ui.toolbar.icons.options.as_deref(),
            icon_mode,
            show_tooltips,
            "Options",
        ) {
            ui.open_popup("##fb_options");
        }
        if let Some(_popup) = ui.begin_popup("##fb_options") {
            let mut nav_on_click = matches!(state.core.click_action, ClickAction::Navigate);
            if ui.checkbox("Navigate on click", &mut nav_on_click) {
                state.core.click_action = if nav_on_click {
                    ClickAction::Navigate
                } else {
                    ClickAction::Select
                };
            }
            let mut dbl = state.core.double_click;
            if ui.checkbox("DblClick confirm", &mut dbl) {
                state.core.double_click = dbl;
            }
            ui.separator();
            ui.text_disabled("Shortcuts:");
            ui.bullet_text("Ctrl+L: focus Path");
            ui.bullet_text("Ctrl+F: focus Search");
            ui.bullet_text("Alt+Left/Right: back/forward");
            ui.bullet_text("Backspace: up");
            ui.bullet_text("F5: refresh");
            ui.bullet_text("Tab: path completion");
            ui.bullet_text("Up/Down: path history");
        }
        ui.new_line();

        // Path bar (file-dialog style address input) + Search.
        let cwd_s = state.core.cwd.display().to_string();
        if state.ui.path_edit_last_cwd != cwd_s && !state.ui.path_edit {
            state.ui.path_edit_last_cwd = cwd_s.clone();
            state.ui.path_edit_buffer = cwd_s.clone();
        } else if state.ui.path_edit_last_cwd.is_empty() {
            state.ui.path_edit_last_cwd = cwd_s.clone();
            if state.ui.path_edit_buffer.trim().is_empty() {
                state.ui.path_edit_buffer = cwd_s.clone();
            }
        }

        ui.text("Path:");
        ui.same_line();
        let style = ui.clone_style();
        let font = ui.current_font();
        let font_size = ui.current_font_size();
        let spacing_x = style.item_spacing()[0];
        let frame_pad_x = style.frame_padding()[0];
        let history_button_w = ui.frame_height();

        const MIN_PATH_W: f32 = 120.0;
        const SEARCH_W: f32 = 220.0;

        let go_label_w = font.calc_text_size(font_size, f32::MAX, 0.0, "Go")[0];
        let go_w = go_label_w + frame_pad_x * 2.0;

        let search_label_w = font.calc_text_size(font_size, f32::MAX, 0.0, "Search:")[0];
        let search_total_w = search_label_w + spacing_x + SEARCH_W;

        let row_start_x = ui.cursor_pos_x();
        let row_w = ui.content_region_avail_width();
        let row_right_x = row_start_x + row_w;
        let min_total_w = history_button_w
            + spacing_x
            + MIN_PATH_W
            + spacing_x
            + go_w
            + spacing_x
            + search_total_w;

        let stacked = row_w < min_total_w;

        // Path input (+ Go). If we can't fit Search on the same line, Search moves to the next line.
        let path_w = if stacked {
            (row_w - history_button_w - spacing_x - go_w - spacing_x).max(40.0)
        } else {
            let search_start_x = row_right_x - search_total_w;
            (search_start_x - row_start_x - history_button_w - spacing_x - go_w - spacing_x * 2.0)
                .max(MIN_PATH_W)
        };

        let recent_paths = state.core.recent_paths().cloned().collect::<Vec<_>>();
        {
            let _disabled = ui.begin_disabled_with_cond(recent_paths.is_empty());
            if ui.arrow_button("##path_history_dropdown", Direction::Down) {
                ui.open_popup("##path_history_dropdown_popup");
            }
        }
        if ui.is_item_hovered() {
            ui.tooltip_text("Path history");
        }
        if let Some(_popup) = ui.begin_popup("##path_history_dropdown_popup") {
            ui.text_disabled("Recent:");
            ui.separator();
            for (i, p) in recent_paths.iter().enumerate() {
                let _id = ui.push_id(i as i32);
                let label = p.display().to_string();
                if ui.selectable(&label) {
                    let _ = state.core.handle_event(CoreEvent::NavigateTo(p.clone()));
                    state.ui.path_edit = false;
                    state.ui.path_edit_last_cwd = state.core.cwd.display().to_string();
                    state.ui.path_edit_buffer = state.ui.path_edit_last_cwd.clone();
                    state.ui.path_history_index = None;
                    state.ui.path_history_saved_buffer = None;
                    state.ui.ui_error = None;
                    ui.close_current_popup();
                }
            }
        }
        ui.same_line();

        let prev_path_buffer = state.ui.path_edit_buffer.clone();
        ui.set_next_item_width(path_w);
        let select_all = state.ui.focus_path_edit_next;
        if select_all {
            ui.set_keyboard_focus_here();
            state.ui.focus_path_edit_next = false;
        }

        struct PathBarCallback<'a> {
            cwd: PathBuf,
            fs: &'a dyn FileSystem,
            recent_paths: Vec<String>,
            history_index: *mut Option<usize>,
            history_saved_buffer: *mut Option<String>,
            programmatic_edit: *mut bool,
        }

        impl PathBarCallback<'_> {
            fn set_text(&mut self, mut data: TextCallbackData, text: &str) {
                let old = data.str();
                if old == text {
                    return;
                }
                data.remove_chars(0, old.len());
                data.insert_chars(0, text);
                data.set_cursor_pos(text.len());
                unsafe { *self.programmatic_edit = true };
            }

            fn common_prefix_len(a: &str, b: &str) -> usize {
                let mut n = 0usize;
                for (ca, cb) in a.chars().zip(b.chars()) {
                    let same = if ca.is_ascii() && cb.is_ascii() {
                        ca.to_ascii_lowercase() == cb.to_ascii_lowercase()
                    } else {
                        ca == cb
                    };
                    if !same {
                        break;
                    }
                    n += ca.len_utf8();
                }
                n
            }

            fn starts_with_case_insensitive(name: &str, prefix: &str) -> bool {
                let mut it_name = name.chars();
                let mut it_prefix = prefix.chars();
                loop {
                    match it_prefix.next() {
                        None => return true,
                        Some(pc) => {
                            let Some(nc) = it_name.next() else {
                                return false;
                            };
                            let same = if nc.is_ascii() && pc.is_ascii() {
                                nc.to_ascii_lowercase() == pc.to_ascii_lowercase()
                            } else {
                                nc == pc
                            };
                            if !same {
                                return false;
                            }
                        }
                    }
                }
            }

            fn last_sep_pos(s: &str) -> Option<(usize, char)> {
                s.char_indices()
                    .filter(|(_, c)| *c == '/' || *c == '\\')
                    .last()
            }

            fn try_complete_path(&mut self, data: TextCallbackData) {
                let input = data.str().trim();
                if input.is_empty() {
                    return;
                }

                let (dir_prefix, frag, sep) = match Self::last_sep_pos(input) {
                    Some((i, c)) => (&input[..=i], &input[i + 1..], c),
                    None => ("", input, std::path::MAIN_SEPARATOR),
                };

                if frag.is_empty() {
                    return;
                }

                let base_dir = if dir_prefix.is_empty() {
                    self.cwd.clone()
                } else {
                    let raw = PathBuf::from(dir_prefix);
                    if raw.is_absolute() {
                        raw
                    } else {
                        self.cwd.join(raw)
                    }
                };

                let Ok(entries) = self.fs.read_dir(&base_dir) else {
                    return;
                };

                let mut matches = entries
                    .into_iter()
                    .filter(|e| e.is_dir)
                    .filter(|e| Self::starts_with_case_insensitive(&e.name, frag))
                    .map(|e| e.name)
                    .collect::<Vec<_>>();
                if matches.is_empty() {
                    return;
                }
                matches.sort();

                let completed = if matches.len() == 1 {
                    let mut s = matches[0].clone();
                    s.push(sep);
                    s
                } else {
                    let first = matches[0].as_str();
                    let mut prefix_len = first.len();
                    for other in matches.iter().skip(1) {
                        prefix_len = prefix_len.min(Self::common_prefix_len(first, other));
                    }
                    first[..prefix_len].to_string()
                };

                let new_text = if dir_prefix.is_empty() {
                    completed
                } else {
                    format!("{dir_prefix}{completed}")
                };

                unsafe { *self.history_index = None };
                unsafe { *self.history_saved_buffer = None };
                self.set_text(data, &new_text);
            }

            fn apply_history(&mut self, direction: HistoryDirection, data: TextCallbackData) {
                if self.recent_paths.is_empty() {
                    return;
                }

                let idx = unsafe { &mut *self.history_index };
                let saved = unsafe { &mut *self.history_saved_buffer };

                match (direction, *idx) {
                    (HistoryDirection::Up, None) => {
                        *saved = Some(data.str().to_string());
                        *idx = Some(0);
                        let p = self.recent_paths[0].clone();
                        self.set_text(data, &p);
                        return;
                    }
                    (HistoryDirection::Down, None) => return,
                    (_, Some(_)) => {}
                }

                let Some(mut i) = *idx else { return };
                match direction {
                    HistoryDirection::Up => {
                        if i + 1 < self.recent_paths.len() {
                            i += 1;
                            *idx = Some(i);
                            let p = self.recent_paths[i].clone();
                            self.set_text(data, &p);
                        }
                    }
                    HistoryDirection::Down => {
                        if i == 0 {
                            let restore = saved.clone().unwrap_or_else(String::new);
                            *idx = None;
                            *saved = None;
                            self.set_text(data, &restore);
                        } else {
                            i -= 1;
                            *idx = Some(i);
                            let p = self.recent_paths[i].clone();
                            self.set_text(data, &p);
                        }
                    }
                }
            }
        }

        impl InputTextCallbackHandler for PathBarCallback<'_> {
            fn on_completion(&mut self, data: TextCallbackData) {
                self.try_complete_path(data);
            }

            fn on_history(&mut self, direction: HistoryDirection, data: TextCallbackData) {
                self.apply_history(direction, data);
            }
        }

        let callback_recent_paths = recent_paths
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>();
        let history_index_ptr: *mut Option<usize> = &mut state.ui.path_history_index;
        let history_saved_ptr: *mut Option<String> = &mut state.ui.path_history_saved_buffer;
        let programmatic_edit_ptr: *mut bool = &mut state.ui.path_bar_programmatic_edit;
        let callback = PathBarCallback {
            cwd: state.core.cwd.clone(),
            fs,
            recent_paths: callback_recent_paths,
            history_index: history_index_ptr,
            history_saved_buffer: history_saved_ptr,
            programmatic_edit: programmatic_edit_ptr,
        };
        let submitted = ui
            .input_text("##path_bar", &mut state.ui.path_edit_buffer)
            .callback(callback)
            .callback_flags(InputTextCallback::COMPLETION | InputTextCallback::HISTORY)
            .auto_select_all(select_all)
            .enter_returns_true(true)
            .build();
        let path_active = ui.is_item_active() || ui.is_item_focused();
        state.ui.path_edit = path_active;
        if path_active
            && !state.ui.path_bar_programmatic_edit
            && state.ui.path_edit_buffer != prev_path_buffer
        {
            state.ui.path_history_index = None;
            state.ui.path_history_saved_buffer = None;
        }
        state.ui.path_bar_programmatic_edit = false;

        ui.same_line();
        let go = ui.button("Go") || (path_active && submitted);
        if go {
            submit_path_edit(state, fs);
        }

        if stacked {
            ui.new_line();
        } else {
            let search_start_x = row_right_x - search_total_w;
            ui.same_line_with_pos(search_start_x);
        }

        ui.text("Search:");
        ui.same_line();
        if state.ui.focus_search_next {
            ui.set_keyboard_focus_here();
            state.ui.focus_search_next = false;
        }
        ui.set_next_item_width(SEARCH_W);
        let _search_changed = ui.input_text("##search", &mut state.core.search).build();

        if let Some(p) = draw_breadcrumbs(ui, state, fs, state.ui.breadcrumbs_max_segments) {
            let _ = state.core.handle_event(CoreEvent::NavigateTo(p));
        }

        ui.separator();
    }

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
                let _ = state.core.handle_event(CoreEvent::NavigateTo(p));
            }
            ui.same_line();
            ui.child_window("file_list")
                .size([avail[0] - left_w - 8.0, avail[1] - 80.0])
                .build(ui, || {
                    let inner = ui.content_region_avail();
                    let show_pane =
                        state.ui.custom_pane_enabled && custom_pane.as_deref_mut().is_some();
                    if !show_pane {
                        draw_file_table(
                            ui,
                            state,
                            [inner[0], inner[1]],
                            fs,
                            &mut request_confirm,
                            thumbnails_backend.as_deref_mut(),
                        );
                        return;
                    }

                    match state.ui.custom_pane_dock {
                        CustomPaneDock::Bottom => {
                            let pane_h = state.ui.custom_pane_height.clamp(0.0, inner[1].max(0.0));
                            let mut table_h = inner[1];
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
                                            let selected_entry_ids =
                                                state.core.selected_entry_ids();
                                            let selected_paths =
                                                selected_entry_paths_from_ids(state);
                                            let (selected_files_count, selected_dirs_count) =
                                                selected_entry_counts_from_ids(state);
                                            let ctx = CustomPaneCtx {
                                                mode: state.core.mode,
                                                cwd: &state.core.cwd,
                                                selected_entry_ids: &selected_entry_ids,
                                                selected_paths: &selected_paths,
                                                selected_files_count,
                                                selected_dirs_count,
                                                save_name: &state.core.save_name,
                                                active_filter: state.core.active_filter(),
                                            };
                                            confirm_gate = pane.draw(ui, ctx);
                                        });
                                }
                            }
                        }
                        CustomPaneDock::Right => {
                            const SPLITTER_W: f32 = 6.0;
                            const MIN_TABLE_W: f32 = 120.0;
                            const MIN_PANE_W: f32 = 120.0;

                            let max_pane_w = (inner[0] - MIN_TABLE_W - SPLITTER_W).max(0.0);
                            let mut pane_w = state.ui.custom_pane_width.clamp(0.0, max_pane_w);
                            if max_pane_w >= MIN_PANE_W {
                                pane_w = pane_w.clamp(MIN_PANE_W, max_pane_w);
                            }

                            let table_w = (inner[0] - pane_w - SPLITTER_W).max(0.0);

                            ui.child_window("file_table_rightdock")
                                .size([table_w, inner[1]])
                                .build(ui, || {
                                    draw_file_table(
                                        ui,
                                        state,
                                        [table_w, inner[1]],
                                        fs,
                                        &mut request_confirm,
                                        thumbnails_backend.as_deref_mut(),
                                    );
                                });

                            ui.same_line();
                            ui.invisible_button("custom_pane_splitter", [SPLITTER_W, inner[1]]);
                            if ui.is_item_hovered() || ui.is_item_active() {
                                ui.set_mouse_cursor(Some(MouseCursor::ResizeEW));
                            }
                            if ui.is_item_active() {
                                let dx = ui.io().mouse_delta()[0];
                                let new_w = (pane_w - dx).clamp(0.0, max_pane_w);
                                state.ui.custom_pane_width = if max_pane_w >= MIN_PANE_W {
                                    new_w.clamp(MIN_PANE_W, max_pane_w)
                                } else {
                                    new_w
                                };
                            }

                            ui.same_line();
                            ui.child_window("custom_pane_rightdock")
                                .size([pane_w, inner[1]])
                                .border(true)
                                .build(ui, || {
                                    if let Some(pane) = custom_pane.as_deref_mut() {
                                        let selected_entry_ids = state.core.selected_entry_ids();
                                        let selected_paths = selected_entry_paths_from_ids(state);
                                        let (selected_files_count, selected_dirs_count) =
                                            selected_entry_counts_from_ids(state);
                                        let ctx = CustomPaneCtx {
                                            mode: state.core.mode,
                                            cwd: &state.core.cwd,
                                            selected_entry_ids: &selected_entry_ids,
                                            selected_paths: &selected_paths,
                                            selected_files_count,
                                            selected_dirs_count,
                                            save_name: &state.core.save_name,
                                            active_filter: state.core.active_filter(),
                                        };
                                        confirm_gate = pane.draw(ui, ctx);
                                    }
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
                    let show_pane =
                        state.ui.custom_pane_enabled && custom_pane.as_deref_mut().is_some();
                    if !show_pane {
                        draw_file_table(
                            ui,
                            state,
                            [inner[0], inner[1]],
                            fs,
                            &mut request_confirm,
                            thumbnails_backend.as_deref_mut(),
                        );
                        return;
                    }

                    match state.ui.custom_pane_dock {
                        CustomPaneDock::Bottom => {
                            let pane_h = state.ui.custom_pane_height.clamp(0.0, inner[1].max(0.0));
                            let mut table_h = inner[1];
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
                                            let selected_entry_ids =
                                                state.core.selected_entry_ids();
                                            let selected_paths =
                                                selected_entry_paths_from_ids(state);
                                            let (selected_files_count, selected_dirs_count) =
                                                selected_entry_counts_from_ids(state);
                                            let ctx = CustomPaneCtx {
                                                mode: state.core.mode,
                                                cwd: &state.core.cwd,
                                                selected_entry_ids: &selected_entry_ids,
                                                selected_paths: &selected_paths,
                                                selected_files_count,
                                                selected_dirs_count,
                                                save_name: &state.core.save_name,
                                                active_filter: state.core.active_filter(),
                                            };
                                            confirm_gate = pane.draw(ui, ctx);
                                        });
                                }
                            }
                        }
                        CustomPaneDock::Right => {
                            const SPLITTER_W: f32 = 6.0;
                            const MIN_TABLE_W: f32 = 120.0;
                            const MIN_PANE_W: f32 = 120.0;

                            let max_pane_w = (inner[0] - MIN_TABLE_W - SPLITTER_W).max(0.0);
                            let mut pane_w = state.ui.custom_pane_width.clamp(0.0, max_pane_w);
                            if max_pane_w >= MIN_PANE_W {
                                pane_w = pane_w.clamp(MIN_PANE_W, max_pane_w);
                            }

                            let table_w = (inner[0] - pane_w - SPLITTER_W).max(0.0);

                            ui.child_window("file_table_rightdock")
                                .size([table_w, inner[1]])
                                .build(ui, || {
                                    draw_file_table(
                                        ui,
                                        state,
                                        [table_w, inner[1]],
                                        fs,
                                        &mut request_confirm,
                                        thumbnails_backend.as_deref_mut(),
                                    );
                                });

                            ui.same_line();
                            ui.invisible_button("custom_pane_splitter", [SPLITTER_W, inner[1]]);
                            if ui.is_item_hovered() || ui.is_item_active() {
                                ui.set_mouse_cursor(Some(MouseCursor::ResizeEW));
                            }
                            if ui.is_item_active() {
                                let dx = ui.io().mouse_delta()[0];
                                let new_w = (pane_w - dx).clamp(0.0, max_pane_w);
                                state.ui.custom_pane_width = if max_pane_w >= MIN_PANE_W {
                                    new_w.clamp(MIN_PANE_W, max_pane_w)
                                } else {
                                    new_w
                                };
                            }

                            ui.same_line();
                            ui.child_window("custom_pane_rightdock")
                                .size([pane_w, inner[1]])
                                .border(true)
                                .build(ui, || {
                                    if let Some(pane) = custom_pane.as_deref_mut() {
                                        let selected_entry_ids = state.core.selected_entry_ids();
                                        let selected_paths = selected_entry_paths_from_ids(state);
                                        let (selected_files_count, selected_dirs_count) =
                                            selected_entry_counts_from_ids(state);
                                        let ctx = CustomPaneCtx {
                                            mode: state.core.mode,
                                            cwd: &state.core.cwd,
                                            selected_entry_ids: &selected_entry_ids,
                                            selected_paths: &selected_paths,
                                            selected_files_count,
                                            selected_dirs_count,
                                            save_name: &state.core.save_name,
                                            active_filter: state.core.active_filter(),
                                        };
                                        confirm_gate = pane.draw(ui, ctx);
                                    }
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
    draw_paste_conflict_modal(ui, state, fs);

    ui.separator();
    // Footer: save name + filter + status + buttons
    if matches!(state.core.mode, DialogMode::SaveFile) {
        ui.text("File name:");
        ui.same_line();
        ui.input_text("##save_name", &mut state.core.save_name)
            .build();
        ui.same_line();
        ui.text_disabled(extension_policy_hint(
            state.core.save_policy.extension_policy,
        ));
        ui.same_line();
        let overwrite = if state.core.save_policy.confirm_overwrite {
            "Overwrite: ask"
        } else {
            "Overwrite: replace"
        };
        ui.text_disabled(overwrite);
        ui.new_line();
    }
    // Action row: filter + status + buttons
    if !state.core.filters().is_empty() && !matches!(state.core.mode, DialogMode::PickFolder) {
        ui.text("Filter:");
        ui.same_line();
        ui.set_next_item_width(220.0);
        let preview = state
            .core
            .active_filter()
            .map(|f| f.name.as_str())
            .unwrap_or("All files");
        let mut next_active_filter = state.core.active_filter_index();
        if let Some(_c) = ui.begin_combo("##filter", preview) {
            if ui
                .selectable_config("All files")
                .selected(state.core.active_filter_index().is_none())
                .build()
            {
                next_active_filter = None;
            }
            for (i, f) in state.core.filters().iter().enumerate() {
                if ui
                    .selectable_config(&f.name)
                    .selected(state.core.active_filter_index() == Some(i))
                    .build()
                {
                    next_active_filter = Some(i);
                }
            }
        }
        if next_active_filter != state.core.active_filter_index() {
            if let Some(i) = next_active_filter {
                let _ = state.core.set_active_filter_index(i);
            } else {
                state.core.set_active_filter_all();
            }
        }
        ui.same_line();
    }

    ui.text_disabled(footer_status_text(state, &confirm_gate));
    ui.same_line();
    let (confirm, cancel) = draw_validation_buttons_row(ui, state, &confirm_gate);

    // Keyboard shortcuts (only when the host window is focused)
    if state.ui.visible && ui.is_window_focused() {
        let ctrl = ui.is_key_down(Key::LeftCtrl) || ui.is_key_down(Key::RightCtrl);
        let alt = ui.is_key_down(Key::LeftAlt) || ui.is_key_down(Key::RightAlt);
        if ctrl && ui.is_key_pressed(Key::L) {
            state.ui.path_edit = true;
            state.ui.path_edit_buffer = state.core.cwd.display().to_string();
            state.ui.focus_path_edit_next = true;
        }
        if ctrl && ui.is_key_pressed(Key::F) {
            state.ui.focus_search_next = true;
        }
        if !ui.io().want_capture_keyboard() && ui.is_key_pressed(Key::Backspace) {
            let _ = state.core.handle_event(CoreEvent::NavigateUp);
        }
        if !ui.io().want_text_input() && alt && ui.is_key_pressed(Key::LeftArrow) {
            let _ = state.core.handle_event(CoreEvent::NavigateBack);
        }
        if !ui.io().want_text_input() && alt && ui.is_key_pressed(Key::RightArrow) {
            let _ = state.core.handle_event(CoreEvent::NavigateForward);
        }
        if !ui.io().want_text_input() && ui.is_key_pressed(Key::F5) {
            let _ = state.core.handle_event(CoreEvent::Refresh);
        }
        if !state.ui.path_edit && !ui.io().want_text_input() && ui.is_key_pressed(Key::Enter) {
            request_confirm |= matches!(
                state.core.handle_event(CoreEvent::ActivateFocused),
                CoreEventOutcome::RequestConfirm
            );
        }
        if !ui.io().want_text_input() && ui.is_key_pressed(Key::F2) {
            open_rename_modal_from_selection(state);
        }
        if !ui.io().want_text_input() && ui.is_key_pressed(Key::Delete) {
            if state.core.has_selection() {
                open_delete_modal_from_selection(state);
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
        state.close();
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

fn extension_policy_hint(p: ExtensionPolicy) -> &'static str {
    match p {
        ExtensionPolicy::KeepUser => "Ext: keep user",
        ExtensionPolicy::AddIfMissing => "Ext: add if missing",
        ExtensionPolicy::ReplaceByFilter => "Ext: enforce filter",
    }
}

fn footer_status_text(state: &FileDialogState, gate: &ConfirmGate) -> String {
    let visible = state.core.entries().len();
    let selected = state.core.selected_len();

    let scan = match state.core.scan_status() {
        ScanStatus::Idle => None,
        ScanStatus::Scanning { .. } => Some("Scanning".to_string()),
        ScanStatus::Partial { loaded, .. } => Some(format!("Loading {loaded}")),
        ScanStatus::Complete { .. } => None,
        ScanStatus::Failed { .. } => Some("Scan failed".to_string()),
    };

    let mut parts: Vec<String> = Vec::new();
    if let Some(s) = scan {
        parts.push(s);
    }
    parts.push(format!("{visible} items"));
    if selected > 0 {
        parts.push(format!("{selected} selected"));
    }

    if !state.core.filters().is_empty() && !matches!(state.core.mode, DialogMode::PickFolder) {
        let f = state
            .core
            .active_filter()
            .map(|f| f.name.as_str())
            .unwrap_or("All files");
        parts.push(format!("Filter: {f}"));
    }

    if !state.core.search.trim().is_empty() {
        parts.push("Search: on".to_string());
    }

    if !gate.can_confirm {
        if let Some(msg) = gate.message.as_deref() {
            parts.push(format!("Blocked: {msg}"));
        } else {
            parts.push("Blocked".to_string());
        }
    }

    parts.join(" | ")
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

    if !state.ui.new_folder_enabled {
        state.ui.new_folder_open_next = false;
        state.ui.new_folder_error = None;
        return;
    }

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
                        let id = EntryId::from_path(&path);
                        state.core.focus_and_select_by_id(id);
                        state.ui.reveal_id_next = Some(id);
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
        let Some(target_id) = state.ui.rename_target_id else {
            ui.text_disabled("No entry selected for rename.");
            if ui.button("Close") {
                ui.close_current_popup();
            }
            return;
        };

        let Some(from_path) = state
            .core
            .entry_path_by_id(target_id)
            .map(Path::to_path_buf)
        else {
            ui.text_disabled("Selected entry is no longer available.");
            if ui.button("Close") {
                state.ui.rename_target_id = None;
                ui.close_current_popup();
            }
            return;
        };

        let from_name = from_path
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| from_path.display().to_string());

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
            state.ui.rename_target_id = None;
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
                let to_path = from_path.with_file_name(&to_name);

                if fs.metadata(&to_path).is_ok() {
                    state.ui.rename_error = Some("Target already exists".into());
                } else {
                    match fs.rename(&from_path, &to_path) {
                        Ok(()) => {
                            let id = EntryId::from_path(&to_path);
                            state.core.focus_and_select_by_id(id);
                            state.ui.reveal_id_next = Some(id);
                            state.core.invalidate_dir_cache();
                            state.ui.rename_target_id = None;
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
        if state.ui.delete_target_ids.is_empty() {
            ui.text_disabled("No entries selected for deletion.");
            if ui.button("Close") {
                ui.close_current_popup();
            }
            return;
        }

        let delete_targets = state
            .ui
            .delete_target_ids
            .iter()
            .copied()
            .filter_map(|id| state.core.entry_path_by_id(id).map(Path::to_path_buf))
            .collect::<Vec<_>>();

        if delete_targets.len() != state.ui.delete_target_ids.len() {
            ui.text_disabled("Some selected entries are no longer available.");
            if ui.button("Close") {
                state.ui.delete_error = None;
                state.ui.delete_target_ids.clear();
                state.ui.delete_recursive = false;
                ui.close_current_popup();
            }
            return;
        }

        let delete_target_names = delete_targets
            .iter()
            .map(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .filter(|name| !name.is_empty())
                    .map(ToOwned::to_owned)
                    .unwrap_or_else(|| path.display().to_string())
            })
            .collect::<Vec<_>>();

        ui.text(format!(
            "Delete {} entr{} in:",
            delete_target_names.len(),
            if delete_target_names.len() == 1 {
                "y"
            } else {
                "ies"
            }
        ));
        ui.text_disabled(state.core.cwd.display().to_string());
        ui.separator();

        let preview_n = 6usize.min(delete_target_names.len());
        for name in delete_target_names.iter().take(preview_n) {
            ui.text(name);
        }
        if delete_target_names.len() > preview_n {
            ui.text_disabled(format!(
                "... and {} more",
                delete_target_names.len() - preview_n
            ));
        }

        ui.separator();

        let any_dir = delete_targets
            .iter()
            .any(|path| fs.metadata(path).map(|m| m.is_dir).unwrap_or(false));
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
            state.ui.delete_target_ids.clear();
            state.ui.delete_recursive = false;
            ui.close_current_popup();
        }

        if del {
            state.ui.delete_error = None;
            let recursive = state.ui.delete_recursive;
            for (path, name) in delete_targets.iter().zip(delete_target_names.iter()) {
                let is_dir = fs.metadata(path).map(|m| m.is_dir).unwrap_or(false);
                let result = if is_dir {
                    if recursive {
                        fs.remove_dir_all(path)
                    } else {
                        fs.remove_dir(path)
                    }
                } else {
                    fs.remove_file(path)
                };
                if let Err(e) = result {
                    state.ui.delete_error = Some(format!("Failed to delete '{name}': {e}"));
                    break;
                }
            }

            if state.ui.delete_error.is_none() {
                state.core.clear_selection();
                state.core.invalidate_dir_cache();
                state.ui.delete_target_ids.clear();
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

fn selected_entry_paths_from_ids(state: &FileDialogState) -> Vec<PathBuf> {
    state.core.selected_entry_paths()
}

fn selected_entry_counts_from_ids(state: &FileDialogState) -> (usize, usize) {
    state.core.selected_entry_counts()
}

fn open_rename_modal_from_selection(state: &mut FileDialogState) {
    if state.core.selected_len() != 1 {
        return;
    }
    let Some(rename_target_id) = state.core.selected_entry_ids().into_iter().next() else {
        return;
    };
    let Some(rename_to) = state
        .core
        .entry_path_by_id(rename_target_id)
        .and_then(|path| path.file_name())
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .map(ToOwned::to_owned)
    else {
        return;
    };
    state.ui.rename_target_id = Some(rename_target_id);
    state.ui.rename_to = rename_to;
    state.ui.rename_error = None;
    state.ui.rename_open_next = true;
    state.ui.rename_focus_next = true;
}

fn open_delete_modal_from_selection(state: &mut FileDialogState) {
    let delete_target_ids = state.core.selected_entry_ids();
    if delete_target_ids.is_empty() {
        return;
    }
    state.ui.delete_target_ids = delete_target_ids;
    state.ui.delete_error = None;
    state.ui.delete_open_next = true;
}
fn clipboard_set_from_selection(state: &mut FileDialogState, op: ClipboardOp) {
    if !state.core.has_selection() {
        return;
    }

    let sources = selected_entry_paths_from_ids(state);
    if sources.is_empty() {
        return;
    }
    state.ui.clipboard = Some(FileClipboard { op, sources });
}

fn start_paste_into_cwd(state: &mut FileDialogState) {
    let Some(clipboard) = state.ui.clipboard.clone() else {
        return;
    };
    if clipboard.sources.is_empty() {
        return;
    }

    state.ui.paste_job = Some(PendingPasteJob {
        clipboard,
        dest_dir: state.core.cwd.clone(),
        next_index: 0,
        created: Vec::new(),
        apply_all_conflicts: None,
        pending_conflict_action: None,
        conflict: None,
    });
}

fn try_complete_paste_job(state: &mut FileDialogState) {
    let Some(job) = state.ui.paste_job.take() else {
        return;
    };
    if job.created.is_empty() {
        return;
    }

    state.core.invalidate_dir_cache();

    let selected_ids = job
        .created
        .iter()
        .map(|name| EntryId::from_path(&state.core.cwd.join(name)))
        .collect::<Vec<_>>();
    let reveal_id = selected_ids.first().copied();
    state.core.replace_selection_by_ids(selected_ids);
    state.ui.reveal_id_next = reveal_id;

    if matches!(job.clipboard.op, ClipboardOp::Cut) {
        state.ui.clipboard = None;
    }
}

fn step_paste_job(state: &mut FileDialogState, fs: &dyn FileSystem) -> Result<bool, String> {
    let Some(job) = state.ui.paste_job.as_mut() else {
        return Ok(false);
    };

    if job.conflict.is_some() {
        return Ok(false);
    }

    while job.next_index < job.clipboard.sources.len() {
        let src = job.clipboard.sources[job.next_index].clone();
        let name = src
            .file_name()
            .ok_or_else(|| format!("Invalid source path: {}", src.display()))?
            .to_string_lossy()
            .to_string();

        let mut dest = job.dest_dir.join(&name);
        if dest == src {
            job.next_index += 1;
            continue;
        }
        if dest.starts_with(&src) {
            return Err(format!("Refusing to paste '{name}' into itself"));
        }

        let exists = fs.metadata(&dest).is_ok();
        if exists {
            if let Some(action) = job
                .pending_conflict_action
                .take()
                .or(job.apply_all_conflicts)
            {
                let policy = match action {
                    PasteConflictAction::Overwrite => ExistingTargetPolicy::Overwrite,
                    PasteConflictAction::Skip => ExistingTargetPolicy::Skip,
                    PasteConflictAction::KeepBoth => ExistingTargetPolicy::KeepBoth,
                };
                match apply_existing_target_policy(fs, &job.dest_dir, &name, policy)
                    .map_err(|e| format!("Failed to resolve target conflict for '{name}': {e}"))?
                {
                    ExistingTargetDecision::Skip => {
                        job.next_index += 1;
                        continue;
                    }
                    ExistingTargetDecision::Continue(p) => dest = p,
                }
            } else {
                job.conflict = Some(PasteConflictPrompt {
                    source: src,
                    dest,
                    apply_to_all: false,
                });
                state.ui.paste_conflict_open_next = true;
                return Ok(false);
            }
        }

        let r = match job.clipboard.op {
            ClipboardOp::Copy => copy_tree(fs, &src, &dest),
            ClipboardOp::Cut => move_tree(fs, &src, &dest),
        };
        if let Err(e) = r {
            return Err(format!("Failed to paste '{name}': {e}"));
        }

        let created_name = dest
            .file_name()
            .map(|v| v.to_string_lossy().to_string())
            .unwrap_or(name);
        job.created.push(created_name);
        job.next_index += 1;
    }

    Ok(true)
}

fn run_paste_job_until_wait_or_done(
    state: &mut FileDialogState,
    fs: &dyn FileSystem,
) -> Result<(), String> {
    loop {
        match step_paste_job(state, fs)? {
            true => {
                try_complete_paste_job(state);
                return Ok(());
            }
            false => {
                return Ok(());
            }
        }
    }
}

fn draw_paste_conflict_modal(ui: &Ui, state: &mut FileDialogState, fs: &dyn FileSystem) {
    const POPUP_ID: &str = "Paste Conflict";

    if state.ui.paste_conflict_open_next {
        state.ui.paste_conflict_open_next = false;
        if !ui.is_popup_open(POPUP_ID) {
            ui.open_popup(POPUP_ID);
        }
    }

    if let Some(_popup) = ui.begin_modal_popup(POPUP_ID) {
        let prompt = state
            .ui
            .paste_job
            .as_ref()
            .and_then(|j| j.conflict.as_ref())
            .cloned();

        let Some(prompt) = prompt else {
            ui.text_disabled("No pending paste conflict.");
            if ui.button("Close") {
                ui.close_current_popup();
            }
            return;
        };

        let src_name = prompt
            .source
            .file_name()
            .map(|v| v.to_string_lossy().to_string())
            .unwrap_or_else(|| prompt.source.display().to_string());

        ui.text(format!("Target already exists: {src_name}"));
        ui.text_disabled(format!("Source: {}", prompt.source.display()));
        ui.text_disabled(format!("Target: {}", prompt.dest.display()));
        ui.separator();

        let mut apply_to_all = prompt.apply_to_all;
        ui.checkbox("Apply to all conflicts", &mut apply_to_all);

        ui.separator();
        let overwrite = ui.button("Overwrite");
        ui.same_line();
        let keep_both = ui.button("Keep Both");
        ui.same_line();
        let skip = ui.button("Skip");
        ui.same_line();
        let cancel = ui.button("Cancel Paste");

        if cancel {
            state.ui.paste_job = None;
            ui.close_current_popup();
            return;
        }

        let selected = if overwrite {
            Some(PasteConflictAction::Overwrite)
        } else if keep_both {
            Some(PasteConflictAction::KeepBoth)
        } else if skip {
            Some(PasteConflictAction::Skip)
        } else {
            None
        };

        if let Some(action) = selected {
            if let Some(job) = state.ui.paste_job.as_mut() {
                if apply_to_all {
                    job.apply_all_conflicts = Some(action);
                }
                job.pending_conflict_action = Some(action);
                job.conflict = None;
            }
            ui.close_current_popup();
            state.ui.ui_error = None;
            if let Err(e) = run_paste_job_until_wait_or_done(state, fs) {
                state.ui.ui_error = Some(e);
                state.ui.paste_job = None;
            }
        } else if let Some(job) = state.ui.paste_job.as_mut() {
            if let Some(conflict) = job.conflict.as_mut() {
                conflict.apply_to_all = apply_to_all;
            }
        }
    }
}

fn submit_path_edit(state: &mut FileDialogState, fs: &dyn FileSystem) {
    let input = state.ui.path_edit_buffer.trim();
    if input.is_empty() {
        state.ui.ui_error = Some("Path is empty".into());
        return;
    }

    let raw_p = std::path::PathBuf::from(input);
    let raw_p = if raw_p.is_absolute() {
        raw_p
    } else {
        state.core.cwd.join(&raw_p)
    };
    let p = fs.canonicalize(&raw_p).unwrap_or(raw_p.clone());
    match fs.metadata(&p) {
        Ok(md) => {
            if md.is_dir {
                let _ = state.core.handle_event(CoreEvent::NavigateTo(p));
                state.ui.path_edit = false;
                state.ui.path_edit_last_cwd = state.core.cwd.display().to_string();
                state.ui.path_edit_buffer = state.ui.path_edit_last_cwd.clone();
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

    let mut groups = state.core.places.groups.clone();
    groups.sort_by(|a, b| {
        a.display_order
            .cmp(&b.display_order)
            .then_with(|| a.label.to_lowercase().cmp(&b.label.to_lowercase()))
    });
    let mut remove_place: Option<(String, PathBuf)> = None;
    let mut edit_req: Option<PlacesEditRequest> = None;
    for (gi, g) in groups.iter().enumerate() {
        let flags = if g.default_opened {
            TreeNodeFlags::DEFAULT_OPEN
        } else {
            TreeNodeFlags::NONE
        };
        let open = ui.collapsing_header(&g.label, flags);
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
            if let Some(thickness) = p.separator_thickness {
                if thickness > 1 {
                    ui.dummy([0.0, (thickness - 1) as f32]);
                }
                ui.separator();
                if thickness > 1 {
                    ui.dummy([0.0, (thickness - 1) as f32]);
                }
                continue;
            }
            if ui.selectable_config(&p.label).build() {
                out = Some(p.path.clone());
            }
            if let Some(_popup) = ui.begin_popup_context_item() {
                ui.text_disabled(&p.path.display().to_string());
                ui.separator();
                let editable = !p.is_separator()
                    && p.origin == PlaceOrigin::User
                    && g.label != Places::SYSTEM_GROUP;
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
                        state.core.places.merge_from(
                            p,
                            crate::places::PlacesMergeOptions {
                                overwrite_group_metadata: true,
                            },
                        );
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
        FileListViewMode::List => draw_file_table_view(
            ui,
            state,
            size,
            fs,
            request_confirm,
            thumbnails_backend,
            false,
        ),
        FileListViewMode::ThumbnailsList => {
            state.ui.thumbnails_enabled = true;
            draw_file_table_view(
                ui,
                state,
                size,
                fs,
                request_confirm,
                thumbnails_backend,
                true,
            )
        }
        FileListViewMode::Grid => {
            draw_file_grid_view(ui, state, size, fs, request_confirm, thumbnails_backend)
        }
    }
}

fn data_column_label(column: FileListDataColumn) -> &'static str {
    match column {
        FileListDataColumn::Name => "Name",
        FileListDataColumn::Extension => "Ext",
        FileListDataColumn::Size => "Size",
        FileListDataColumn::Modified => "Modified",
    }
}

fn is_data_column_visible(config: &FileListColumnsConfig, column: FileListDataColumn) -> bool {
    match column {
        FileListDataColumn::Name => true,
        FileListDataColumn::Extension => config.show_extension,
        FileListDataColumn::Size => config.show_size,
        FileListDataColumn::Modified => config.show_modified,
    }
}

fn apply_compact_column_layout(config: &mut FileListColumnsConfig) {
    config.show_preview = false;
    config.show_extension = true;
    config.show_size = true;
    config.show_modified = false;
    config.order = [
        FileListDataColumn::Name,
        FileListDataColumn::Extension,
        FileListDataColumn::Size,
        FileListDataColumn::Modified,
    ];
}

fn apply_compact_column_layout_keep_preview(config: &mut FileListColumnsConfig) {
    let preview = config.show_preview;
    apply_compact_column_layout(config);
    config.show_preview = preview;
}

fn apply_balanced_column_layout(config: &mut FileListColumnsConfig) {
    config.show_preview = true;
    config.show_extension = true;
    config.show_size = true;
    config.show_modified = true;
    config.order = [
        FileListDataColumn::Name,
        FileListDataColumn::Extension,
        FileListDataColumn::Size,
        FileListDataColumn::Modified,
    ];
}

fn apply_balanced_column_layout_keep_preview(config: &mut FileListColumnsConfig) {
    let preview = config.show_preview;
    apply_balanced_column_layout(config);
    config.show_preview = preview;
}

fn move_column_order_up(order: &mut [FileListDataColumn; 4], index: usize) -> bool {
    if index == 0 || index >= order.len() {
        return false;
    }
    order.swap(index, index - 1);
    true
}

fn move_column_order_down(order: &mut [FileListDataColumn; 4], index: usize) -> bool {
    if index + 1 >= order.len() {
        return false;
    }
    order.swap(index, index + 1);
    true
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ListColumnLayout {
    data_columns: Vec<FileListDataColumn>,
    name: i16,
    extension: Option<i16>,
    size: Option<i16>,
    modified: Option<i16>,
}

fn list_column_layout(show_preview: bool, config: &FileListColumnsConfig) -> ListColumnLayout {
    let mut data_columns = Vec::with_capacity(4);
    for column in config.normalized_order() {
        match column {
            FileListDataColumn::Name => data_columns.push(column),
            FileListDataColumn::Extension if config.show_extension => data_columns.push(column),
            FileListDataColumn::Size if config.show_size => data_columns.push(column),
            FileListDataColumn::Modified if config.show_modified => data_columns.push(column),
            _ => {}
        }
    }

    let mut index: i16 = if show_preview { 1 } else { 0 };
    let mut name = None;
    let mut extension = None;
    let mut size = None;
    let mut modified = None;

    for column in &data_columns {
        match column {
            FileListDataColumn::Name => name = Some(index),
            FileListDataColumn::Extension => extension = Some(index),
            FileListDataColumn::Size => size = Some(index),
            FileListDataColumn::Modified => modified = Some(index),
        }
        index += 1;
    }

    ListColumnLayout {
        data_columns,
        name: name.expect("name column should always be present"),
        extension,
        size,
        modified,
    }
}

fn validated_column_weight(override_weight: Option<f32>, default_weight: f32) -> f32 {
    match override_weight {
        Some(weight) if weight.is_finite() && weight > 0.0 => weight,
        _ => default_weight,
    }
}

fn default_preview_column_weight() -> f32 {
    0.12
}

fn default_data_column_weight(
    column: FileListDataColumn,
    show_preview: bool,
    show_size: bool,
    show_modified: bool,
) -> f32 {
    match column {
        FileListDataColumn::Name => {
            if show_size || show_modified {
                if show_preview { 0.52 } else { 0.56 }
            } else if show_preview {
                0.88
            } else {
                0.92
            }
        }
        FileListDataColumn::Extension => {
            if show_size || show_modified {
                0.12
            } else {
                0.08
            }
        }
        FileListDataColumn::Size => {
            if show_modified {
                0.16
            } else {
                0.2
            }
        }
        FileListDataColumn::Modified => {
            if show_size {
                0.2
            } else {
                0.24
            }
        }
    }
}

fn column_weight_override(
    config: &FileListColumnsConfig,
    column: FileListDataColumn,
) -> Option<f32> {
    match column {
        FileListDataColumn::Name => config.weight_overrides.name,
        FileListDataColumn::Extension => config.weight_overrides.extension,
        FileListDataColumn::Size => config.weight_overrides.size,
        FileListDataColumn::Modified => config.weight_overrides.modified,
    }
}

fn resolved_preview_column_weight(config: &FileListColumnsConfig) -> f32 {
    validated_column_weight(
        config.weight_overrides.preview,
        default_preview_column_weight(),
    )
}

fn resolved_data_column_weight(
    config: &FileListColumnsConfig,
    column: FileListDataColumn,
    show_preview: bool,
    show_size: bool,
    show_modified: bool,
) -> f32 {
    validated_column_weight(
        column_weight_override(config, column),
        default_data_column_weight(column, show_preview, show_size, show_modified),
    )
}

fn table_column_stretch_weight(table: *const sys::ImGuiTable, column_index: i16) -> Option<f32> {
    if table.is_null() || column_index < 0 {
        return None;
    }
    let columns_count = unsafe { (*table).ColumnsCount.max(0) as usize };
    let index = column_index as usize;
    if index >= columns_count {
        return None;
    }

    let columns_ptr = unsafe { (*table).Columns.Data };
    if columns_ptr.is_null() {
        return None;
    }

    let weight = unsafe { (*columns_ptr.add(index)).StretchWeight };
    if weight.is_finite() && weight > 0.0 {
        Some(weight)
    } else {
        None
    }
}

fn table_data_column_for_index(
    layout: &ListColumnLayout,
    column_index: i16,
) -> Option<FileListDataColumn> {
    if column_index == layout.name {
        return Some(FileListDataColumn::Name);
    }
    if layout.extension == Some(column_index) {
        return Some(FileListDataColumn::Extension);
    }
    if layout.size == Some(column_index) {
        return Some(FileListDataColumn::Size);
    }
    if layout.modified == Some(column_index) {
        return Some(FileListDataColumn::Modified);
    }
    None
}

fn merged_order_with_current(
    visible_order: &[FileListDataColumn],
    current_order: [FileListDataColumn; 4],
) -> [FileListDataColumn; 4] {
    let mut merged = Vec::with_capacity(4);
    for &column in visible_order {
        if !merged.contains(&column) {
            merged.push(column);
        }
    }
    for column in current_order {
        if !merged.contains(&column) {
            merged.push(column);
        }
    }
    for column in [
        FileListDataColumn::Name,
        FileListDataColumn::Extension,
        FileListDataColumn::Size,
        FileListDataColumn::Modified,
    ] {
        if !merged.contains(&column) {
            merged.push(column);
        }
    }
    [merged[0], merged[1], merged[2], merged[3]]
}

fn table_data_columns_by_display_order(
    table: *const sys::ImGuiTable,
    layout: &ListColumnLayout,
) -> Vec<FileListDataColumn> {
    if table.is_null() {
        return Vec::new();
    }
    let columns_count = unsafe { (*table).ColumnsCount.max(0) as usize };
    let columns_ptr = unsafe { (*table).Columns.Data };
    if columns_ptr.is_null() {
        return Vec::new();
    }

    let mut ordered = Vec::with_capacity(layout.data_columns.len());
    for index in 0..columns_count {
        let index_i16 = index as i16;
        let Some(column) = table_data_column_for_index(layout, index_i16) else {
            continue;
        };
        let display_order = unsafe { (*columns_ptr.add(index)).DisplayOrder };
        ordered.push((display_order, column));
    }
    ordered.sort_by_key(|(display_order, _)| *display_order);
    ordered.into_iter().map(|(_, column)| column).collect()
}

fn sync_runtime_column_order_from_table(
    layout: &ListColumnLayout,
    config: &mut FileListColumnsConfig,
) {
    let table = unsafe { sys::igGetCurrentTable() };
    if table.is_null() {
        return;
    }
    let visible_order = table_data_columns_by_display_order(table, layout);
    if visible_order.is_empty() {
        return;
    }
    config.order = merged_order_with_current(&visible_order, config.normalized_order());
}

fn sync_runtime_column_weights_from_table(
    show_preview: bool,
    layout: &ListColumnLayout,
    config: &mut FileListColumnsConfig,
) {
    let table = unsafe { sys::igGetCurrentTable() };
    if table.is_null() {
        return;
    }

    let resized_column = unsafe { (*table).ResizedColumn };
    if resized_column < 0 {
        return;
    }

    if show_preview {
        if let Some(weight) = table_column_stretch_weight(table, 0) {
            config.weight_overrides.preview = Some(weight);
        }
    }
    if let Some(weight) = table_column_stretch_weight(table, layout.name) {
        config.weight_overrides.name = Some(weight);
    }
    if let Some(index) = layout.extension {
        if let Some(weight) = table_column_stretch_weight(table, index) {
            config.weight_overrides.extension = Some(weight);
        }
    }
    if let Some(index) = layout.size {
        if let Some(weight) = table_column_stretch_weight(table, index) {
            config.weight_overrides.size = Some(weight);
        }
    }
    if let Some(index) = layout.modified {
        if let Some(weight) = table_column_stretch_weight(table, index) {
            config.weight_overrides.modified = Some(weight);
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
    force_preview: bool,
) {
    state.core.rescan_if_needed(fs);
    if state.ui.thumbnails_enabled {
        state.ui.thumbnails.advance_frame();
    }

    // Table
    use dear_imgui_rs::{SortDirection, TableColumnFlags, TableFlags};
    let flags = TableFlags::RESIZABLE
        | TableFlags::REORDERABLE
        | TableFlags::ROW_BG
        | TableFlags::BORDERS_V
        | TableFlags::BORDERS_OUTER
        | TableFlags::SCROLL_Y
        | TableFlags::SIZING_STRETCH_PROP
        | TableFlags::SORTABLE; // enable built-in header sorting
    let columns_config = &state.ui.file_list_columns;
    let show_preview =
        state.ui.thumbnails_enabled && (columns_config.show_preview || force_preview);
    let show_size = columns_config.show_size;
    let show_modified = columns_config.show_modified;
    let layout = list_column_layout(show_preview, columns_config);

    let mut table = ui.table("file_table").flags(flags).outer_size(size);
    if show_preview {
        table = table
            .column("Preview")
            .flags(
                TableColumnFlags::NO_SORT
                    | TableColumnFlags::NO_RESIZE
                    | TableColumnFlags::NO_REORDER,
            )
            .weight(resolved_preview_column_weight(columns_config))
            .done();
    }

    for column in &layout.data_columns {
        table = match column {
            FileListDataColumn::Name => table
                .column("Name")
                .flags(TableColumnFlags::PREFER_SORT_ASCENDING)
                .user_id(0)
                .weight(resolved_data_column_weight(
                    columns_config,
                    *column,
                    show_preview,
                    show_size,
                    show_modified,
                ))
                .done(),
            FileListDataColumn::Extension => table
                .column("Ext")
                .flags(TableColumnFlags::PREFER_SORT_ASCENDING)
                .user_id(1)
                .weight(resolved_data_column_weight(
                    columns_config,
                    *column,
                    show_preview,
                    show_size,
                    show_modified,
                ))
                .done(),
            FileListDataColumn::Size => table
                .column("Size")
                .flags(TableColumnFlags::PREFER_SORT_DESCENDING)
                .user_id(2)
                .weight(resolved_data_column_weight(
                    columns_config,
                    *column,
                    show_preview,
                    show_size,
                    show_modified,
                ))
                .done(),
            FileListDataColumn::Modified => table
                .column("Modified")
                .flags(TableColumnFlags::PREFER_SORT_DESCENDING)
                .user_id(3)
                .weight(resolved_data_column_weight(
                    columns_config,
                    *column,
                    show_preview,
                    show_size,
                    show_modified,
                ))
                .done(),
        };
    }

    table = table.headers(true);

    table.build(|ui| {
        // Apply ImGui sort specs (single primary sort)
        if let Some(mut specs) = ui.table_get_sort_specs() {
            if specs.is_dirty() {
                if let Some(s) = specs.iter().next() {
                    let (by, asc) = match (s.column_index, s.sort_direction) {
                        (i, SortDirection::Ascending) if i == layout.name => (SortBy::Name, true),
                        (i, SortDirection::Descending) if i == layout.name => (SortBy::Name, false),
                        (i, SortDirection::Ascending) if layout.extension == Some(i) => {
                            (SortBy::Extension, true)
                        }
                        (i, SortDirection::Descending) if layout.extension == Some(i) => {
                            (SortBy::Extension, false)
                        }
                        (i, SortDirection::Ascending) if layout.size == Some(i) => {
                            (SortBy::Size, true)
                        }
                        (i, SortDirection::Descending) if layout.size == Some(i) => {
                            (SortBy::Size, false)
                        }
                        (i, SortDirection::Ascending) if layout.modified == Some(i) => {
                            (SortBy::Modified, true)
                        }
                        (i, SortDirection::Descending) if layout.modified == Some(i) => {
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
                let _ = state.core.handle_event(CoreEvent::SelectAll);
            }
            if modifiers.ctrl && ui.is_key_pressed(Key::C) && !modifiers.shift {
                clipboard_set_from_selection(state, ClipboardOp::Copy);
            }
            if modifiers.ctrl && ui.is_key_pressed(Key::X) && !modifiers.shift {
                clipboard_set_from_selection(state, ClipboardOp::Cut);
            }
            if modifiers.ctrl && ui.is_key_pressed(Key::V) && !modifiers.shift {
                state.ui.ui_error = None;
                start_paste_into_cwd(state);
                if let Err(e) = run_paste_job_until_wait_or_done(state, fs) {
                    state.ui.ui_error = Some(e);
                    state.ui.paste_job = None;
                }
            }
            if ui.is_key_pressed_with_repeat(Key::UpArrow, true) {
                let _ = state.core.handle_event(CoreEvent::MoveFocus {
                    delta: -1,
                    modifiers,
                });
            }
            if ui.is_key_pressed_with_repeat(Key::DownArrow, true) {
                let _ = state.core.handle_event(CoreEvent::MoveFocus {
                    delta: 1,
                    modifiers,
                });
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
                if show_preview {
                    ui.table_next_column();
                    ui.text("");
                }
                ui.table_next_column();
                let msg = if let Some(custom) = &state.ui.empty_hint_static_message {
                    custom.clone()
                } else {
                    let filter_label = state
                        .core
                        .active_filter()
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

            let selected = state.core.is_selected_id(e.id);
            let visual = style_visual_for_entry(state, e);

            let mut label = e.display_name();
            if let Some(icon) = visual.icon.as_deref() {
                label = format!("{icon} {label}");
            }

            for column in &layout.data_columns {
                ui.table_next_column();
                match column {
                    FileListDataColumn::Name => {
                        let _font = visual.font_id.map(|id| ui.push_font(id));
                        let _color = visual
                            .text_color
                            .map(TextColorToken::push)
                            .unwrap_or_else(TextColorToken::none);
                        {
                            if ui
                                .selectable_config(label.as_str())
                                .selected(selected)
                                .span_all_columns(false)
                                .build()
                            {
                                let modifiers = Modifiers {
                                    ctrl: ui.is_key_down(Key::LeftCtrl)
                                        || ui.is_key_down(Key::RightCtrl),
                                    shift: ui.is_key_down(Key::LeftShift)
                                        || ui.is_key_down(Key::RightShift),
                                };
                                let _ = state.core.handle_event(CoreEvent::ClickEntry {
                                    id: e.id,
                                    modifiers,
                                });
                                if matches!(state.core.mode, DialogMode::SaveFile) && !e.is_dir {
                                    state.core.save_name = e.name.clone();
                                }
                            }
                        }

                        if ui.is_item_hovered() {
                            if let Some(t) = visual.tooltip.as_deref() {
                                ui.tooltip_text(t);
                            }
                        }

                        if let Some(_popup) = ui.begin_popup_context_item() {
                            if !selected {
                                let _ =
                                    state.core.handle_event(CoreEvent::FocusAndSelectById(e.id));
                            }
                            let has_selection = state.core.has_selection();
                            let can_paste = state
                                .ui
                                .clipboard
                                .as_ref()
                                .map(|c| !c.sources.is_empty())
                                .unwrap_or(false);

                            if ui.menu_item_enabled_selected(
                                "Copy",
                                Some("Ctrl+C"),
                                false,
                                has_selection,
                            ) {
                                clipboard_set_from_selection(state, ClipboardOp::Copy);
                                ui.close_current_popup();
                            }
                            if ui.menu_item_enabled_selected(
                                "Cut",
                                Some("Ctrl+X"),
                                false,
                                has_selection,
                            ) {
                                clipboard_set_from_selection(state, ClipboardOp::Cut);
                                ui.close_current_popup();
                            }
                            if ui.menu_item_enabled_selected(
                                "Paste",
                                Some("Ctrl+V"),
                                false,
                                can_paste,
                            ) {
                                state.ui.ui_error = None;
                                start_paste_into_cwd(state);
                                if let Err(e) = run_paste_job_until_wait_or_done(state, fs) {
                                    state.ui.ui_error = Some(e);
                                    state.ui.paste_job = None;
                                }
                                ui.close_current_popup();
                            }

                            ui.separator();
                            let can_rename = state.core.selected_len() == 1;
                            if ui.menu_item_enabled_selected(
                                "Rename",
                                Some("F2"),
                                false,
                                can_rename,
                            ) {
                                open_rename_modal_from_selection(state);
                                ui.close_current_popup();
                            }
                            if ui.menu_item_enabled_selected("Delete", Some("Del"), false, true) {
                                open_delete_modal_from_selection(state);
                                ui.close_current_popup();
                            }
                        }

                        if ui.is_item_hovered() && ui.is_mouse_double_clicked(MouseButton::Left) {
                            state.ui.ui_error = None;
                            *request_confirm |= matches!(
                                state
                                    .core
                                    .handle_event(CoreEvent::DoubleClickEntry { id: e.id }),
                                CoreEventOutcome::RequestConfirm
                            );
                        }
                    }
                    FileListDataColumn::Extension => {
                        if e.is_dir {
                            ui.text("");
                        } else if let Some(i) = e.name.find('.') {
                            ui.text(&e.name[i..]);
                        } else {
                            ui.text("");
                        }
                    }
                    FileListDataColumn::Size => {
                        ui.text(match e.size {
                            Some(s) => format_size(s),
                            None => String::new(),
                        });
                    }
                    FileListDataColumn::Modified => {
                        let modified_str = format_modified_ago(e.modified);
                        ui.text(&modified_str);
                        if ui.is_item_hovered() {
                            if let Some(m) = e.modified {
                                use chrono::{DateTime, Local};
                                let dt: DateTime<Local> = DateTime::<Local>::from(m);
                                ui.tooltip_text(dt.format("%Y-%m-%d %H:%M:%S").to_string());
                            }
                        }
                    }
                }
            }

            if state.ui.reveal_id_next == Some(e.id) {
                ui.set_scroll_here_y(0.5);
                state.ui.reveal_id_next = None;
            }
        }

        if let Some(_popup) = ui.begin_popup_context_window() {
            let can_paste = state
                .ui
                .clipboard
                .as_ref()
                .map(|c| !c.sources.is_empty())
                .unwrap_or(false);
            if ui.menu_item_enabled_selected("Paste", Some("Ctrl+V"), false, can_paste) {
                state.ui.ui_error = None;
                start_paste_into_cwd(state);
                if let Err(e) = run_paste_job_until_wait_or_done(state, fs) {
                    state.ui.ui_error = Some(e);
                    state.ui.paste_job = None;
                }
                ui.close_current_popup();
            }
        }

        sync_runtime_column_order_from_table(&layout, &mut state.ui.file_list_columns);

        sync_runtime_column_weights_from_table(
            show_preview,
            &layout,
            &mut state.ui.file_list_columns,
        );
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
                    let _ = state.core.handle_event(CoreEvent::SelectAll);
                }
                if modifiers.ctrl && ui.is_key_pressed(Key::C) && !modifiers.shift {
                    clipboard_set_from_selection(state, ClipboardOp::Copy);
                }
                if modifiers.ctrl && ui.is_key_pressed(Key::X) && !modifiers.shift {
                    clipboard_set_from_selection(state, ClipboardOp::Cut);
                }
                if modifiers.ctrl && ui.is_key_pressed(Key::V) && !modifiers.shift {
                    state.ui.ui_error = None;
                    start_paste_into_cwd(state);
                    if let Err(e) = run_paste_job_until_wait_or_done(state, fs) {
                        state.ui.ui_error = Some(e);
                        state.ui.paste_job = None;
                    }
                }
                if ui.is_key_pressed_with_repeat(Key::LeftArrow, true) {
                    let _ = state.core.handle_event(CoreEvent::MoveFocus {
                        delta: -1,
                        modifiers,
                    });
                }
                if ui.is_key_pressed_with_repeat(Key::RightArrow, true) {
                    let _ = state.core.handle_event(CoreEvent::MoveFocus {
                        delta: 1,
                        modifiers,
                    });
                }
                if ui.is_key_pressed_with_repeat(Key::UpArrow, true) {
                    let _ = state.core.handle_event(CoreEvent::MoveFocus {
                        delta: -(cols as i32),
                        modifiers,
                    });
                }
                if ui.is_key_pressed_with_repeat(Key::DownArrow, true) {
                    let _ = state.core.handle_event(CoreEvent::MoveFocus {
                        delta: cols as i32,
                        modifiers,
                    });
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

                    let selected = state.core.is_selected_id(e.id);
                    let visual = style_visual_for_entry(state, e);

                    let mut label = e.display_name();
                    if let Some(icon) = visual.icon.as_deref() {
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

                    if state.ui.reveal_id_next == Some(e.id) {
                        ui.set_scroll_here_y(0.5);
                        state.ui.reveal_id_next = None;
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
                    let col = visual
                        .text_color
                        .map(|c| dear_imgui_rs::Color::from_array(c))
                        .unwrap_or_else(|| dear_imgui_rs::Color::rgb(1.0, 1.0, 1.0));
                    let _font = visual.font_id.map(|id| ui.push_font(id));
                    {
                        dl.with_clip_rect(item_min, item_max, || {
                            dl.add_text(text_pos, col, &label);
                        });
                    }

                    if clicked {
                        let modifiers = Modifiers {
                            ctrl: ui.is_key_down(Key::LeftCtrl) || ui.is_key_down(Key::RightCtrl),
                            shift: ui.is_key_down(Key::LeftShift)
                                || ui.is_key_down(Key::RightShift),
                        };
                        let _ = state.core.handle_event(CoreEvent::ClickEntry {
                            id: e.id,
                            modifiers,
                        });
                        if matches!(state.core.mode, DialogMode::SaveFile) && !e.is_dir {
                            state.core.save_name = e.name.clone();
                        }
                    }

                    if ui.is_item_hovered() {
                        if let Some(t) = visual.tooltip.as_deref() {
                            ui.tooltip_text(t);
                        }
                    }

                    if let Some(_popup) = ui.begin_popup_context_item() {
                        if !selected {
                            let _ = state.core.handle_event(CoreEvent::FocusAndSelectById(e.id));
                        }
                        let has_selection = state.core.has_selection();
                        let can_paste = state
                            .ui
                            .clipboard
                            .as_ref()
                            .map(|c| !c.sources.is_empty())
                            .unwrap_or(false);

                        if ui.menu_item_enabled_selected(
                            "Copy",
                            Some("Ctrl+C"),
                            false,
                            has_selection,
                        ) {
                            clipboard_set_from_selection(state, ClipboardOp::Copy);
                            ui.close_current_popup();
                        }
                        if ui.menu_item_enabled_selected(
                            "Cut",
                            Some("Ctrl+X"),
                            false,
                            has_selection,
                        ) {
                            clipboard_set_from_selection(state, ClipboardOp::Cut);
                            ui.close_current_popup();
                        }
                        if ui.menu_item_enabled_selected("Paste", Some("Ctrl+V"), false, can_paste)
                        {
                            state.ui.ui_error = None;
                            start_paste_into_cwd(state);
                            if let Err(e) = run_paste_job_until_wait_or_done(state, fs) {
                                state.ui.ui_error = Some(e);
                                state.ui.paste_job = None;
                            }
                            ui.close_current_popup();
                        }

                        ui.separator();
                        let can_rename = state.core.selected_len() == 1;
                        if ui.menu_item_enabled_selected("Rename", Some("F2"), false, can_rename) {
                            open_rename_modal_from_selection(state);
                            ui.close_current_popup();
                        }
                        if ui.menu_item_enabled_selected("Delete", Some("Del"), false, true) {
                            open_delete_modal_from_selection(state);
                            ui.close_current_popup();
                        }
                    }

                    if ui.is_item_hovered() && ui.is_mouse_double_clicked(MouseButton::Left) {
                        state.ui.ui_error = None;
                        *request_confirm |= matches!(
                            state
                                .core
                                .handle_event(CoreEvent::DoubleClickEntry { id: e.id }),
                            CoreEventOutcome::RequestConfirm
                        );
                    }
                }
            }
            if let Some(_popup) = ui.begin_popup_context_window() {
                let can_paste = state
                    .ui
                    .clipboard
                    .as_ref()
                    .map(|c| !c.sources.is_empty())
                    .unwrap_or(false);
                if ui.menu_item_enabled_selected("Paste", Some("Ctrl+V"), false, can_paste) {
                    state.ui.ui_error = None;
                    start_paste_into_cwd(state);
                    if let Err(e) = run_paste_job_until_wait_or_done(state, fs) {
                        state.ui.ui_error = Some(e);
                        state.ui.paste_job = None;
                    }
                    ui.close_current_popup();
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
    let _ = state.core.handle_event(CoreEvent::SelectByPrefix(
        state.ui.type_select_buffer.clone(),
    ));
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

#[cfg(test)]
mod tests {
    use super::{
        ListColumnLayout, list_column_layout, open_delete_modal_from_selection,
        open_rename_modal_from_selection, resolve_host_size_constraints,
    };
    use crate::core::DialogMode;
    use crate::dialog_core::EntryId;
    use crate::dialog_state::{
        FileDialogState, FileListColumnWeightOverrides, FileListColumnsConfig, FileListDataColumn,
    };
    use crate::fs::{FileSystem, FsEntry, FsMetadata};
    use std::path::{Path, PathBuf};

    fn columns_config(
        show_size: bool,
        show_modified: bool,
        order: [FileListDataColumn; 4],
    ) -> FileListColumnsConfig {
        let mut cfg = FileListColumnsConfig::default();
        cfg.show_size = show_size;
        cfg.show_modified = show_modified;
        cfg.order = order;
        cfg
    }

    #[test]
    fn resolve_host_size_constraints_returns_none_when_unset() {
        assert!(resolve_host_size_constraints(None, None).is_none());
    }

    #[test]
    fn resolve_host_size_constraints_supports_one_sided_values() {
        let (min, max) = resolve_host_size_constraints(Some([200.0, 150.0]), None).unwrap();
        assert_eq!(min, [200.0, 150.0]);
        assert_eq!(max, [f32::MAX, f32::MAX]);

        let (min, max) = resolve_host_size_constraints(None, Some([900.0, 700.0])).unwrap();
        assert_eq!(min, [0.0, 0.0]);
        assert_eq!(max, [900.0, 700.0]);
    }

    #[test]
    fn resolve_host_size_constraints_normalizes_invalid_values() {
        let (min, max) =
            resolve_host_size_constraints(Some([300.0, f32::NAN]), Some([100.0, f32::INFINITY]))
                .unwrap();
        assert_eq!(min, [300.0, 0.0]);
        assert_eq!(max, [300.0, f32::MAX]);
    }

    #[derive(Clone, Default)]
    struct UiTestFs {
        entries: Vec<FsEntry>,
    }

    impl FileSystem for UiTestFs {
        fn read_dir(&self, _dir: &Path) -> std::io::Result<Vec<FsEntry>> {
            Ok(self.entries.clone())
        }

        fn canonicalize(&self, path: &Path) -> std::io::Result<PathBuf> {
            Ok(path.to_path_buf())
        }

        fn metadata(&self, path: &Path) -> std::io::Result<FsMetadata> {
            self.entries
                .iter()
                .find(|entry| entry.path == path)
                .map(|entry| FsMetadata {
                    is_dir: entry.is_dir,
                    is_symlink: entry.is_symlink,
                })
                .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "not found"))
        }

        fn create_dir(&self, _path: &Path) -> std::io::Result<()> {
            Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "create_dir not supported in UiTestFs",
            ))
        }

        fn rename(&self, _from: &Path, _to: &Path) -> std::io::Result<()> {
            Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "rename not supported in UiTestFs",
            ))
        }

        fn remove_file(&self, _path: &Path) -> std::io::Result<()> {
            Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "remove_file not supported in UiTestFs",
            ))
        }

        fn remove_dir(&self, _path: &Path) -> std::io::Result<()> {
            Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "remove_dir not supported in UiTestFs",
            ))
        }

        fn remove_dir_all(&self, _path: &Path) -> std::io::Result<()> {
            Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "remove_dir_all not supported in UiTestFs",
            ))
        }

        fn copy_file(&self, _from: &Path, _to: &Path) -> std::io::Result<u64> {
            Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "copy_file not supported in UiTestFs",
            ))
        }
    }

    fn file_entry(path: &str) -> FsEntry {
        let path = PathBuf::from(path);
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(path.as_os_str().to_string_lossy().as_ref())
            .to_string();
        FsEntry {
            name,
            path,
            is_dir: false,
            is_symlink: false,
            size: None,
            modified: None,
        }
    }
    #[test]
    fn list_column_layout_all_columns_visible_without_preview() {
        let cfg = columns_config(
            true,
            true,
            [
                FileListDataColumn::Name,
                FileListDataColumn::Extension,
                FileListDataColumn::Size,
                FileListDataColumn::Modified,
            ],
        );
        assert_eq!(
            list_column_layout(false, &cfg),
            ListColumnLayout {
                data_columns: vec![
                    FileListDataColumn::Name,
                    FileListDataColumn::Extension,
                    FileListDataColumn::Size,
                    FileListDataColumn::Modified,
                ],
                name: 0,
                extension: Some(1),
                size: Some(2),
                modified: Some(3),
            }
        );
    }

    #[test]
    fn list_column_layout_hides_extension_column() {
        let mut cfg = columns_config(
            true,
            true,
            [
                FileListDataColumn::Name,
                FileListDataColumn::Extension,
                FileListDataColumn::Size,
                FileListDataColumn::Modified,
            ],
        );
        cfg.show_extension = false;

        assert_eq!(
            list_column_layout(false, &cfg),
            ListColumnLayout {
                data_columns: vec![
                    FileListDataColumn::Name,
                    FileListDataColumn::Size,
                    FileListDataColumn::Modified,
                ],
                name: 0,
                extension: None,
                size: Some(1),
                modified: Some(2),
            }
        );
    }

    #[test]
    fn list_column_layout_all_columns_visible_with_preview() {
        let cfg = columns_config(
            true,
            true,
            [
                FileListDataColumn::Name,
                FileListDataColumn::Extension,
                FileListDataColumn::Size,
                FileListDataColumn::Modified,
            ],
        );
        assert_eq!(
            list_column_layout(true, &cfg),
            ListColumnLayout {
                data_columns: vec![
                    FileListDataColumn::Name,
                    FileListDataColumn::Extension,
                    FileListDataColumn::Size,
                    FileListDataColumn::Modified,
                ],
                name: 1,
                extension: Some(2),
                size: Some(3),
                modified: Some(4),
            }
        );
    }

    #[test]
    fn list_column_layout_hides_size_column() {
        let cfg = columns_config(
            false,
            true,
            [
                FileListDataColumn::Name,
                FileListDataColumn::Extension,
                FileListDataColumn::Size,
                FileListDataColumn::Modified,
            ],
        );
        assert_eq!(
            list_column_layout(false, &cfg),
            ListColumnLayout {
                data_columns: vec![
                    FileListDataColumn::Name,
                    FileListDataColumn::Extension,
                    FileListDataColumn::Modified,
                ],
                name: 0,
                extension: Some(1),
                size: None,
                modified: Some(2),
            }
        );
    }

    #[test]
    fn list_column_layout_hides_modified_column() {
        let cfg = columns_config(
            true,
            false,
            [
                FileListDataColumn::Name,
                FileListDataColumn::Extension,
                FileListDataColumn::Size,
                FileListDataColumn::Modified,
            ],
        );
        assert_eq!(
            list_column_layout(false, &cfg),
            ListColumnLayout {
                data_columns: vec![
                    FileListDataColumn::Name,
                    FileListDataColumn::Extension,
                    FileListDataColumn::Size,
                ],
                name: 0,
                extension: Some(1),
                size: Some(2),
                modified: None,
            }
        );
    }

    #[test]
    fn list_column_layout_hides_size_and_modified_columns() {
        let cfg = columns_config(
            false,
            false,
            [
                FileListDataColumn::Name,
                FileListDataColumn::Extension,
                FileListDataColumn::Size,
                FileListDataColumn::Modified,
            ],
        );
        assert_eq!(
            list_column_layout(false, &cfg),
            ListColumnLayout {
                data_columns: vec![FileListDataColumn::Name, FileListDataColumn::Extension],
                name: 0,
                extension: Some(1),
                size: None,
                modified: None,
            }
        );
    }

    #[test]
    fn list_column_layout_respects_custom_order() {
        let cfg = columns_config(
            true,
            true,
            [
                FileListDataColumn::Name,
                FileListDataColumn::Size,
                FileListDataColumn::Modified,
                FileListDataColumn::Extension,
            ],
        );
        assert_eq!(
            list_column_layout(false, &cfg),
            ListColumnLayout {
                data_columns: vec![
                    FileListDataColumn::Name,
                    FileListDataColumn::Size,
                    FileListDataColumn::Modified,
                    FileListDataColumn::Extension,
                ],
                name: 0,
                extension: Some(3),
                size: Some(1),
                modified: Some(2),
            }
        );
    }

    #[test]
    fn merged_order_with_current_keeps_hidden_columns() {
        let merged = super::merged_order_with_current(
            &[FileListDataColumn::Name, FileListDataColumn::Modified],
            [
                FileListDataColumn::Name,
                FileListDataColumn::Size,
                FileListDataColumn::Modified,
                FileListDataColumn::Extension,
            ],
        );
        assert_eq!(
            merged,
            [
                FileListDataColumn::Name,
                FileListDataColumn::Modified,
                FileListDataColumn::Size,
                FileListDataColumn::Extension,
            ]
        );
    }

    #[test]
    fn move_column_order_up_swaps_adjacent_items() {
        let mut order = [
            FileListDataColumn::Name,
            FileListDataColumn::Extension,
            FileListDataColumn::Size,
            FileListDataColumn::Modified,
        ];
        assert!(super::move_column_order_up(&mut order, 2));
        assert_eq!(
            order,
            [
                FileListDataColumn::Name,
                FileListDataColumn::Size,
                FileListDataColumn::Extension,
                FileListDataColumn::Modified,
            ]
        );
    }

    #[test]
    fn move_column_order_down_swaps_adjacent_items() {
        let mut order = [
            FileListDataColumn::Name,
            FileListDataColumn::Extension,
            FileListDataColumn::Size,
            FileListDataColumn::Modified,
        ];
        assert!(super::move_column_order_down(&mut order, 1));
        assert_eq!(
            order,
            [
                FileListDataColumn::Name,
                FileListDataColumn::Size,
                FileListDataColumn::Extension,
                FileListDataColumn::Modified,
            ]
        );
    }

    #[test]
    fn move_column_order_up_rejects_first_item() {
        let mut order = [
            FileListDataColumn::Name,
            FileListDataColumn::Extension,
            FileListDataColumn::Size,
            FileListDataColumn::Modified,
        ];
        assert!(!super::move_column_order_up(&mut order, 0));
        assert_eq!(
            order,
            [
                FileListDataColumn::Name,
                FileListDataColumn::Extension,
                FileListDataColumn::Size,
                FileListDataColumn::Modified,
            ]
        );
    }

    #[test]
    fn apply_compact_column_layout_updates_visibility_and_order_only() {
        let expected_weights = FileListColumnWeightOverrides {
            preview: Some(0.11),
            name: Some(0.57),
            extension: Some(0.14),
            size: Some(0.18),
            modified: Some(0.22),
        };

        let mut cfg = FileListColumnsConfig {
            show_preview: true,
            show_extension: true,
            show_size: false,
            show_modified: true,
            order: [
                FileListDataColumn::Modified,
                FileListDataColumn::Size,
                FileListDataColumn::Extension,
                FileListDataColumn::Name,
            ],
            weight_overrides: expected_weights.clone(),
        };

        super::apply_compact_column_layout(&mut cfg);

        assert!(!cfg.show_preview);
        assert!(cfg.show_size);
        assert!(!cfg.show_modified);
        assert_eq!(
            cfg.order,
            [
                FileListDataColumn::Name,
                FileListDataColumn::Extension,
                FileListDataColumn::Size,
                FileListDataColumn::Modified,
            ]
        );
        assert_eq!(cfg.weight_overrides, expected_weights);
    }

    #[test]
    fn apply_balanced_column_layout_updates_visibility_and_order_only() {
        let expected_weights = FileListColumnWeightOverrides {
            preview: Some(0.13),
            name: Some(0.54),
            extension: Some(0.16),
            size: Some(0.17),
            modified: Some(0.21),
        };

        let mut cfg = FileListColumnsConfig {
            show_preview: false,
            show_extension: true,
            show_size: false,
            show_modified: false,
            order: [
                FileListDataColumn::Size,
                FileListDataColumn::Name,
                FileListDataColumn::Modified,
                FileListDataColumn::Extension,
            ],
            weight_overrides: expected_weights.clone(),
        };

        super::apply_balanced_column_layout(&mut cfg);

        assert!(cfg.show_preview);
        assert!(cfg.show_size);
        assert!(cfg.show_modified);
        assert_eq!(
            cfg.order,
            [
                FileListDataColumn::Name,
                FileListDataColumn::Extension,
                FileListDataColumn::Size,
                FileListDataColumn::Modified,
            ]
        );
        assert_eq!(cfg.weight_overrides, expected_weights);
    }

    #[test]
    fn open_rename_modal_from_selection_prefills_name_from_id() {
        let mut state = FileDialogState::new(DialogMode::OpenFiles);
        state.core.set_cwd(PathBuf::from("/tmp"));

        let fs = UiTestFs {
            entries: vec![file_entry("/tmp/a.txt")],
        };
        state.core.rescan_if_needed(&fs);

        let id = state
            .core
            .entries()
            .iter()
            .find(|entry| entry.path == Path::new("/tmp/a.txt"))
            .map(|entry| entry.id)
            .expect("missing /tmp/a.txt entry id");
        state.core.focus_and_select_by_id(id);

        open_rename_modal_from_selection(&mut state);

        assert_eq!(state.ui.rename_target_id, Some(id));
        assert_eq!(state.ui.rename_to, "a.txt");
        assert!(state.ui.rename_open_next);
        assert!(state.ui.rename_focus_next);
    }

    #[test]
    fn open_rename_modal_from_selection_ignores_unresolved_id() {
        let mut state = FileDialogState::new(DialogMode::OpenFiles);
        let id = EntryId::from_path(Path::new("/tmp/missing.txt"));
        state.core.focus_and_select_by_id(id);

        open_rename_modal_from_selection(&mut state);

        assert_eq!(state.ui.rename_target_id, None);
        assert!(state.ui.rename_to.is_empty());
        assert!(!state.ui.rename_open_next);
    }

    #[test]
    fn open_delete_modal_from_selection_stores_selected_ids() {
        let mut state = FileDialogState::new(DialogMode::OpenFiles);
        state.core.set_cwd(PathBuf::from("/tmp"));

        let fs = UiTestFs {
            entries: vec![file_entry("/tmp/a.txt"), file_entry("/tmp/b.txt")],
        };
        state.core.rescan_if_needed(&fs);

        let a = state
            .core
            .entries()
            .iter()
            .find(|entry| entry.path == Path::new("/tmp/a.txt"))
            .map(|entry| entry.id)
            .expect("missing /tmp/a.txt entry id");
        let b = state
            .core
            .entries()
            .iter()
            .find(|entry| entry.path == Path::new("/tmp/b.txt"))
            .map(|entry| entry.id)
            .expect("missing /tmp/b.txt entry id");
        state.core.replace_selection_by_ids([b, a]);

        open_delete_modal_from_selection(&mut state);

        assert_eq!(state.ui.delete_target_ids, vec![b, a]);
        assert!(state.ui.delete_open_next);
    }
}
