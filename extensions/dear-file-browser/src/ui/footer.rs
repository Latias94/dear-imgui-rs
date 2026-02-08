use crate::core::DialogMode;
use crate::dialog_core::{ConfirmGate, CoreEvent, CoreEventOutcome, ScanStatus};
use crate::dialog_state::{
    FileDialogState, PathBarStyle, ValidationButtonsAlign, ValidationButtonsOrder,
};
use crate::fs::FileSystem;
use dear_imgui_rs::Ui;
use dear_imgui_rs::input::Key;

pub(super) fn draw_footer(
    ui: &Ui,
    state: &mut FileDialogState,
    fs: &dyn FileSystem,
    confirm_gate: &ConfirmGate,
    request_confirm: &mut bool,
) {
    // Footer (IGFD-style): file field + filters + buttons, then a compact status line.
    let footer_start_y = ui.cursor_pos_y();
    ui.separator();

    let style = ui.clone_style();
    let spacing_x = style.item_spacing()[0];

    let show_filter =
        !state.core.filters().is_empty() && !matches!(state.core.mode, DialogMode::PickFolder);
    let filter_preview = state
        .core
        .active_filter()
        .map(|f| f.name.clone())
        .unwrap_or_else(|| "All files".to_string());
    let filter_combo_w = if show_filter {
        calc_filter_combo_width(ui, filter_preview.as_str())
    } else {
        0.0
    };

    // Compute an expected buttons width so we can size the file input without hard-coded constants.
    let (_confirm_label, _cancel_label, _confirm_w, _cancel_w, buttons_group_w) =
        validation_buttons_layout(ui, state);

    ui.align_text_to_frame_padding();
    let file_label = match state.core.mode {
        DialogMode::PickFolder => "Folder:",
        _ => "File:",
    };
    ui.text(file_label);
    ui.same_line();

    let footer_display = footer_file_name_display(state);
    if matches!(
        state.core.mode,
        DialogMode::OpenFile | DialogMode::OpenFiles | DialogMode::PickFolder
    ) {
        if state.ui.footer_file_name_buffer.trim().is_empty()
            || state.ui.footer_file_name_buffer == state.ui.footer_file_name_last_display
        {
            state.ui.footer_file_name_buffer = footer_display.clone();
        }
        state.ui.footer_file_name_last_display = footer_display.clone();
    }

    let reserved_w = buttons_group_w
        + if show_filter {
            spacing_x + filter_combo_w
        } else {
            0.0
        };
    let input_w = (ui.content_region_avail_width() - reserved_w - spacing_x).max(60.0);
    ui.set_next_item_width(input_w);

    let file_entered = match state.core.mode {
        DialogMode::SaveFile => ui
            .input_text("##footer_file_name", &mut state.core.save_name)
            .enter_returns_true(true)
            .build(),
        DialogMode::OpenFile | DialogMode::OpenFiles => ui
            .input_text("##footer_file_name", &mut state.ui.footer_file_name_buffer)
            .enter_returns_true(true)
            .build(),
        DialogMode::PickFolder => ui
            .input_text("##footer_file_name", &mut state.ui.footer_file_name_buffer)
            .read_only(true)
            .enter_returns_true(true)
            .build(),
    };

    if file_entered {
        *request_confirm = match state.core.mode {
            DialogMode::SaveFile => !state.core.save_name.trim().is_empty(),
            DialogMode::OpenFile | DialogMode::OpenFiles => {
                !state.ui.footer_file_name_buffer.trim().is_empty()
            }
            DialogMode::PickFolder => false,
        };
    }

    // Filter combobox (auto width like IGFD).
    if show_filter && !matches!(state.core.mode, DialogMode::PickFolder) {
        ui.same_line();
        ui.set_next_item_width(filter_combo_w);
        let mut next_active_filter = state.core.active_filter_index();
        if let Some(_c) = ui.begin_combo("##filter", filter_preview.as_str()) {
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
    }

    // Buttons (right-aligned).
    ui.same_line();
    let (confirm, cancel) = draw_validation_buttons_row(ui, state, &confirm_gate);

    // Compact status line (non-interactive).
    ui.text_disabled(footer_status_text(state, &confirm_gate));

    // Keyboard shortcuts (only when the host window is focused)
    if state.ui.visible && ui.is_window_focused() {
        let ctrl = ui.is_key_down(Key::LeftCtrl) || ui.is_key_down(Key::RightCtrl);
        let alt = ui.is_key_down(Key::LeftAlt) || ui.is_key_down(Key::RightAlt);
        if ctrl && ui.is_key_pressed(Key::L) {
            if state.ui.path_bar_style == PathBarStyle::Breadcrumbs {
                state.ui.path_input_mode = true;
            }
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
            *request_confirm |= matches!(
                state.core.handle_event(CoreEvent::ActivateFocused),
                CoreEventOutcome::RequestConfirm
            );
        }
        if !ui.io().want_text_input() && ui.is_key_pressed(Key::F2) {
            super::ops::open_rename_modal_from_selection(state);
        }
        if !ui.io().want_text_input() && ui.is_key_pressed(Key::Delete) {
            if state.core.has_selection() {
                super::ops::open_delete_modal_from_selection(state);
            }
        }
    }

    *request_confirm |= confirm;
    if cancel {
        state.core.cancel();
    } else if *request_confirm {
        state.ui.ui_error = None;
        let typed_footer_name = match state.core.mode {
            DialogMode::OpenFile | DialogMode::OpenFiles => {
                Some(state.ui.footer_file_name_buffer.as_str())
            }
            _ => None,
        };
        let can_confirm = confirm_gate.can_confirm && core_can_confirm(state);
        if can_confirm {
            if let Err(e) = state.core.confirm(fs, &confirm_gate, typed_footer_name) {
                state.ui.ui_error = Some(e.to_string());
            }
        }
    }

    draw_confirm_overwrite_modal(ui, state);

    if let Some(err) = &state.ui.ui_error {
        ui.separator();
        ui.text_colored([1.0, 0.3, 0.3, 1.0], format!("Error: {err}"));
    }

    // Update measured footer height for the next frame's content sizing.
    state.ui.footer_height_last = (ui.cursor_pos_y() - footer_start_y).max(0.0);
}

pub(super) fn estimate_footer_height(ui: &Ui, _state: &FileDialogState) -> f32 {
    // We keep this derived from current style metrics to avoid hard-coded pixel constants.
    // The actual value is measured each frame and stored in `state.ui.footer_height_last`.
    let style = ui.clone_style();
    let row_h = ui
        .frame_height_with_spacing()
        .max(ui.text_line_height_with_spacing());
    let sep_h = style.item_spacing()[1] * 2.0 + 1.0;
    // Footer row + compact status line.
    sep_h + row_h + ui.text_line_height_with_spacing()
}

fn calc_filter_combo_width(ui: &Ui, preview: &str) -> f32 {
    const MIN_W: f32 = 150.0;
    let style = ui.clone_style();
    let font = ui.current_font();
    let font_size = ui.current_font_size();
    let text_w = font.calc_text_size(font_size, f32::MAX, 0.0, preview)[0];
    // Match IGFD: text width + arrow area (frame height) + inner spacing.
    (text_w + ui.frame_height() + style.item_inner_spacing()[0]).max(MIN_W)
}

fn validation_buttons_layout(ui: &Ui, state: &FileDialogState) -> (String, String, f32, f32, f32) {
    let default_confirm = match state.core.mode {
        DialogMode::OpenFile | DialogMode::OpenFiles => "Open",
        DialogMode::PickFolder => "Select",
        DialogMode::SaveFile => "Save",
    };
    let cfg = &state.ui.validation_buttons;
    let confirm_label = cfg
        .confirm_label
        .as_deref()
        .unwrap_or(default_confirm)
        .to_string();
    let cancel_label = cfg.cancel_label.as_deref().unwrap_or("Cancel").to_string();

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
        .unwrap_or_else(|| calc_button_width(&confirm_label));
    let cancel_w = cfg
        .cancel_width
        .or(base_w)
        .unwrap_or_else(|| calc_button_width(&cancel_label));
    let group_w = confirm_w + cancel_w + spacing_x;

    (confirm_label, cancel_label, confirm_w, cancel_w, group_w)
}

fn footer_file_name_display(state: &FileDialogState) -> String {
    let selected_len = state.core.selected_len();
    if selected_len == 0 {
        return String::new();
    }

    let (files, dirs) = state.core.selected_entry_counts();
    match state.core.mode {
        DialogMode::OpenFile => {
            if files == 1 && dirs == 0 {
                state
                    .core
                    .selected_entry_paths()
                    .into_iter()
                    .next()
                    .and_then(|p| p.file_name().map(|s| s.to_string_lossy().to_string()))
                    .unwrap_or_default()
            } else if selected_len > 1 {
                format!("{selected_len} files selected")
            } else {
                String::new()
            }
        }
        DialogMode::OpenFiles => {
            if selected_len == 1 && files == 1 && dirs == 0 {
                state
                    .core
                    .selected_entry_paths()
                    .into_iter()
                    .next()
                    .and_then(|p| p.file_name().map(|s| s.to_string_lossy().to_string()))
                    .unwrap_or_default()
            } else {
                format!("{selected_len} files selected")
            }
        }
        DialogMode::PickFolder => {
            if selected_len == 0 {
                ".".to_string()
            } else if selected_len == 1 && dirs == 1 && files == 0 {
                state
                    .core
                    .selected_entry_paths()
                    .into_iter()
                    .next()
                    .and_then(|p| p.file_name().map(|s| s.to_string_lossy().to_string()))
                    .unwrap_or_default()
            } else if selected_len > 1 {
                format!("{selected_len} items selected")
            } else {
                String::new()
            }
        }
        DialogMode::SaveFile => String::new(),
    }
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

    let can_confirm = gate.can_confirm && core_can_confirm(state);

    match cfg.order {
        ValidationButtonsOrder::ConfirmCancel => {
            let _disabled = ui.begin_disabled_with_cond(!can_confirm);
            let confirm_clicked = ui.button_with_size(confirm_label, [confirm_w, 0.0]);
            drop(_disabled);
            if !can_confirm && ui.is_item_hovered() {
                ui.tooltip_text(confirm_disabled_reason(state, gate));
            }
            ui.same_line();
            let cancel_clicked = ui.button_with_size(cancel_label, [cancel_w, 0.0]);
            (confirm_clicked, cancel_clicked)
        }
        ValidationButtonsOrder::CancelConfirm => {
            let cancel_clicked = ui.button_with_size(cancel_label, [cancel_w, 0.0]);
            ui.same_line();
            let _disabled = ui.begin_disabled_with_cond(!can_confirm);
            let confirm_clicked = ui.button_with_size(confirm_label, [confirm_w, 0.0]);
            drop(_disabled);
            if !can_confirm && ui.is_item_hovered() {
                ui.tooltip_text(confirm_disabled_reason(state, gate));
            }
            (confirm_clicked, cancel_clicked)
        }
    }
}

fn core_can_confirm(state: &FileDialogState) -> bool {
    match state.core.mode {
        DialogMode::SaveFile => !state.core.save_name.trim().is_empty(),
        DialogMode::OpenFile | DialogMode::OpenFiles => {
            state.core.has_selection() || !state.ui.footer_file_name_buffer.trim().is_empty()
        }
        DialogMode::PickFolder => true,
    }
}

fn confirm_disabled_reason(state: &FileDialogState, gate: &ConfirmGate) -> String {
    if !gate.can_confirm {
        if let Some(msg) = gate.message.as_deref() {
            return msg.to_string();
        }
        return "Blocked".to_string();
    }
    match state.core.mode {
        DialogMode::SaveFile => "Type a file name to save.".to_string(),
        DialogMode::OpenFile | DialogMode::OpenFiles => {
            "Select a file, or type a file name/path in the footer field.".to_string()
        }
        DialogMode::PickFolder => "Select a folder, or confirm the current directory.".to_string(),
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
