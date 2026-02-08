use std::path::PathBuf;

use crate::core::{DialogMode, FileDialogError, LayoutStyle, Selection};
use crate::custom_pane::{CustomPane, CustomPaneCtx};
use crate::dialog_core::{ConfirmGate, CoreEvent};
use crate::dialog_state::CustomPaneDock;
use crate::dialog_state::FileDialogState;
use crate::fs::{FileSystem, StdFileSystem};
use crate::thumbnails::ThumbnailBackend;
use dear_imgui_rs::Ui;
use dear_imgui_rs::input::MouseCursor;
use dear_imgui_rs::sys;

mod file_table;
mod footer;
mod header;
mod igfd_path_popup;
mod ops;
mod path_bar;
mod places;
mod popups;

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

    // Make all widget IDs inside this browser instance unique, even when embedding
    // multiple dialogs in the same host window. This avoids ImGui "conflicting ID"
    // warnings for internal child windows/popups/tooltips.
    let _dialog_id_scope = ui.push_id(state as *mut FileDialogState);

    let has_thumbnail_backend = thumbnails_backend.is_some();
    let mut request_confirm = false;
    let mut confirm_gate = ConfirmGate::default();

    header::draw_chrome(ui, state, fs, has_thumbnail_backend);

    // Content region
    let avail = ui.content_region_avail();
    let footer_h = state
        .ui
        .footer_height_last
        .max(footer::estimate_footer_height(ui, state));
    let content_h = (avail[1] - footer_h).max(0.0);
    match state.ui.layout {
        LayoutStyle::Standard => {
            if state.ui.places_pane_shown {
                const MIN_PLACES_W: f32 = 120.0;
                const MIN_FILE_LIST_W: f32 = 180.0;

                let splitter_w = splitter_width(ui);
                let spacing_x = ui.clone_style().item_spacing()[0];
                let max_places_w =
                    (avail[0] - MIN_FILE_LIST_W - splitter_w - spacing_x * 2.0).max(0.0);
                let mut places_w = state.ui.places_pane_width.clamp(0.0, max_places_w);
                if max_places_w >= MIN_PLACES_W {
                    places_w = places_w.clamp(MIN_PLACES_W, max_places_w);
                }
                let file_w = (avail[0] - places_w - splitter_w - spacing_x * 2.0).max(0.0);

                let mut new_cwd: Option<PathBuf> = None;
                ui.child_window("places_pane")
                    .size([places_w, content_h])
                    .border(true)
                    .build(ui, || {
                        new_cwd = places::draw_places_pane(ui, state);
                    });
                if let Some(p) = new_cwd {
                    let _ = state.core.handle_event(CoreEvent::NavigateTo(p));
                }

                ui.same_line();
                ui.invisible_button("places_pane_splitter", [splitter_w, content_h]);
                if ui.is_item_hovered() || ui.is_item_active() {
                    ui.set_mouse_cursor(Some(MouseCursor::ResizeEW));
                }
                if ui.is_item_active() {
                    let dx = ui.io().mouse_delta()[0];
                    let new_w = (places_w + dx).clamp(0.0, max_places_w);
                    state.ui.places_pane_width = if max_places_w >= MIN_PLACES_W {
                        new_w.clamp(MIN_PLACES_W, max_places_w)
                    } else {
                        new_w
                    };
                }

                ui.same_line();
                ui.child_window("file_list")
                    .size([file_w, content_h])
                    .build(ui, || {
                        let inner = ui.content_region_avail();
                        let show_pane =
                            state.ui.custom_pane_enabled && custom_pane.as_deref_mut().is_some();
                        if !show_pane {
                            file_table::draw_file_table(
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
                                let style = ui.clone_style();
                                let sep_h = style.item_spacing()[1] * 2.0 + 1.0;
                                let pane_h =
                                    state.ui.custom_pane_height.clamp(0.0, inner[1].max(0.0));
                                let mut table_h = inner[1];
                                if pane_h > 0.0 {
                                    table_h = (table_h - pane_h - sep_h).max(0.0);
                                }

                                file_table::draw_file_table(
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
                                                    ops::selected_entry_paths_from_ids(state);
                                                let (selected_files_count, selected_dirs_count) =
                                                    ops::selected_entry_counts_from_ids(state);
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
                                const MIN_TABLE_W: f32 = 120.0;
                                const MIN_PANE_W: f32 = 120.0;

                                let splitter_w = splitter_width(ui);
                                let max_pane_w = (inner[0] - MIN_TABLE_W - splitter_w).max(0.0);
                                let mut pane_w = state.ui.custom_pane_width.clamp(0.0, max_pane_w);
                                if max_pane_w >= MIN_PANE_W {
                                    pane_w = pane_w.clamp(MIN_PANE_W, max_pane_w);
                                }

                                let table_w = (inner[0] - pane_w - splitter_w).max(0.0);

                                ui.child_window("file_table_rightdock")
                                    .size([table_w, inner[1]])
                                    .build(ui, || {
                                        file_table::draw_file_table(
                                            ui,
                                            state,
                                            [table_w, inner[1]],
                                            fs,
                                            &mut request_confirm,
                                            thumbnails_backend.as_deref_mut(),
                                        );
                                    });

                                ui.same_line();
                                ui.invisible_button("custom_pane_splitter", [splitter_w, inner[1]]);
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
                                            let selected_entry_ids =
                                                state.core.selected_entry_ids();
                                            let selected_paths =
                                                ops::selected_entry_paths_from_ids(state);
                                            let (selected_files_count, selected_dirs_count) =
                                                ops::selected_entry_counts_from_ids(state);
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
            } else {
                ui.child_window("file_list")
                    .size([avail[0], content_h])
                    .build(ui, || {
                        let inner = ui.content_region_avail();
                        let show_pane =
                            state.ui.custom_pane_enabled && custom_pane.as_deref_mut().is_some();
                        if !show_pane {
                            file_table::draw_file_table(
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
                                let style = ui.clone_style();
                                let sep_h = style.item_spacing()[1] * 2.0 + 1.0;
                                let pane_h =
                                    state.ui.custom_pane_height.clamp(0.0, inner[1].max(0.0));
                                let mut table_h = inner[1];
                                if pane_h > 0.0 {
                                    table_h = (table_h - pane_h - sep_h).max(0.0);
                                }

                                file_table::draw_file_table(
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
                                                    ops::selected_entry_paths_from_ids(state);
                                                let (selected_files_count, selected_dirs_count) =
                                                    ops::selected_entry_counts_from_ids(state);
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
                                const MIN_TABLE_W: f32 = 120.0;
                                const MIN_PANE_W: f32 = 120.0;

                                let splitter_w = splitter_width(ui);
                                let max_pane_w = (inner[0] - MIN_TABLE_W - splitter_w).max(0.0);
                                let mut pane_w = state.ui.custom_pane_width.clamp(0.0, max_pane_w);
                                if max_pane_w >= MIN_PANE_W {
                                    pane_w = pane_w.clamp(MIN_PANE_W, max_pane_w);
                                }

                                let table_w = (inner[0] - pane_w - splitter_w).max(0.0);

                                ui.child_window("file_table_rightdock")
                                    .size([table_w, inner[1]])
                                    .build(ui, || {
                                        file_table::draw_file_table(
                                            ui,
                                            state,
                                            [table_w, inner[1]],
                                            fs,
                                            &mut request_confirm,
                                            thumbnails_backend.as_deref_mut(),
                                        );
                                    });

                                ui.same_line();
                                ui.invisible_button("custom_pane_splitter", [splitter_w, inner[1]]);
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
                                            let selected_entry_ids =
                                                state.core.selected_entry_ids();
                                            let selected_paths =
                                                ops::selected_entry_paths_from_ids(state);
                                            let (selected_files_count, selected_dirs_count) =
                                                ops::selected_entry_counts_from_ids(state);
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
        LayoutStyle::Minimal => {
            ui.child_window("file_list_min")
                .size([avail[0], content_h])
                .build(ui, || {
                    let inner = ui.content_region_avail();
                    let show_pane =
                        state.ui.custom_pane_enabled && custom_pane.as_deref_mut().is_some();
                    if !show_pane {
                        file_table::draw_file_table(
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
                            let style = ui.clone_style();
                            let sep_h = style.item_spacing()[1] * 2.0 + 1.0;
                            let pane_h = state.ui.custom_pane_height.clamp(0.0, inner[1].max(0.0));
                            let mut table_h = inner[1];
                            if pane_h > 0.0 {
                                table_h = (table_h - pane_h - sep_h).max(0.0);
                            }

                            file_table::draw_file_table(
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
                                                ops::selected_entry_paths_from_ids(state);
                                            let (selected_files_count, selected_dirs_count) =
                                                ops::selected_entry_counts_from_ids(state);
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
                            const MIN_TABLE_W: f32 = 120.0;
                            const MIN_PANE_W: f32 = 120.0;

                            let splitter_w = splitter_width(ui);
                            let max_pane_w = (inner[0] - MIN_TABLE_W - splitter_w).max(0.0);
                            let mut pane_w = state.ui.custom_pane_width.clamp(0.0, max_pane_w);
                            if max_pane_w >= MIN_PANE_W {
                                pane_w = pane_w.clamp(MIN_PANE_W, max_pane_w);
                            }

                            let table_w = (inner[0] - pane_w - splitter_w).max(0.0);

                            ui.child_window("file_table_rightdock")
                                .size([table_w, inner[1]])
                                .build(ui, || {
                                    file_table::draw_file_table(
                                        ui,
                                        state,
                                        [table_w, inner[1]],
                                        fs,
                                        &mut request_confirm,
                                        thumbnails_backend.as_deref_mut(),
                                    );
                                });

                            ui.same_line();
                            ui.invisible_button("custom_pane_splitter", [splitter_w, inner[1]]);
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
                                        let selected_paths =
                                            ops::selected_entry_paths_from_ids(state);
                                        let (selected_files_count, selected_dirs_count) =
                                            ops::selected_entry_counts_from_ids(state);
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

    // IGFD-style quick path selection popup (opened from breadcrumb separators).
    if let Some(p) = igfd_path_popup::draw_igfd_path_popup(ui, state, fs, [avail[0], content_h]) {
        let _ = state.core.handle_event(CoreEvent::NavigateTo(p));
    }

    places::draw_minimal_places_popup(ui, state);
    popups::draw_columns_popup(ui, state);
    popups::draw_options_popup(ui, state, has_thumbnail_backend);

    places::draw_places_io_modal(ui, state);
    places::draw_places_edit_modal(ui, state, fs);
    popups::draw_new_folder_modal(ui, state, fs);
    popups::draw_rename_modal(ui, state, fs);
    popups::draw_delete_confirm_modal(ui, state, fs);
    popups::draw_paste_conflict_modal(ui, state, fs);

    footer::draw_footer(ui, state, fs, &confirm_gate, &mut request_confirm);

    let out = state.core.take_result();
    if out.is_some() {
        state.close();
    }
    out
}

fn splitter_width(ui: &Ui) -> f32 {
    // Match IGFD's typical splitter thickness (~4px) but keep it relative to current style.
    let w = ui.frame_height() * 0.25;
    w.clamp(4.0, 10.0)
}

#[cfg(test)]
mod tests {
    use super::file_table::{ListColumnLayout, list_column_layout, merged_order_with_current};
    use super::ops::{open_delete_modal_from_selection, open_rename_modal_from_selection};
    use super::resolve_host_size_constraints;
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
        let merged = merged_order_with_current(
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
        assert!(super::file_table::move_column_order_up(&mut order, 2));
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
        assert!(super::file_table::move_column_order_down(&mut order, 1));
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
        assert!(!super::file_table::move_column_order_up(&mut order, 0));
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

        super::file_table::apply_compact_column_layout(&mut cfg);

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

        super::file_table::apply_balanced_column_layout(&mut cfg);

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
