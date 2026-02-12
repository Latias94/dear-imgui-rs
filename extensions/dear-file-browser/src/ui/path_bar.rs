use std::path::{Path, PathBuf};

use dear_imgui_rs::Ui;
use dear_imgui_rs::input::MouseButton;
use dear_imgui_rs::{
    HistoryDirection, InputTextCallback, InputTextCallbackHandler, TextCallbackData,
};

use crate::dialog_core::CoreEvent;
use crate::dialog_state::{FileDialogState, PathBarStyle};
use crate::fs::FileSystem;

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
                ca.eq_ignore_ascii_case(&cb)
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
                        nc.eq_ignore_ascii_case(&pc)
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

pub(super) fn draw_path_input_text(
    ui: &Ui,
    state: &mut FileDialogState,
    fs: &dyn FileSystem,
    recent_paths: &[PathBuf],
    path_w: f32,
    show_go_button: bool,
) {
    let prev_path_buffer = state.ui.path_edit_buffer.clone();
    ui.set_next_item_width(path_w);
    let select_all = state.ui.focus_path_edit_next;
    if select_all {
        ui.set_keyboard_focus_here();
        state.ui.focus_path_edit_next = false;
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

    if show_go_button {
        ui.same_line();
        if ui.button("Go") || (path_active && submitted) {
            submit_path_edit(state, fs);
        }
    } else if path_active && submitted {
        submit_path_edit(state, fs);
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

pub(super) fn estimate_breadcrumbs_total_width(
    ui: &Ui,
    cwd: &Path,
    max_segments: usize,
    quick_select: bool,
) -> f32 {
    let sep_label = if std::path::MAIN_SEPARATOR == '\\' {
        "\\"
    } else {
        "/"
    };

    let style = ui.clone_style();
    let spacing_x = style.item_spacing()[0];
    let pad_x = style.frame_padding()[0];
    let font = ui.current_font();
    let font_size = ui.current_font_size();

    let button_w = |label: &str| -> f32 {
        let tw = font.calc_text_size(font_size, f32::MAX, 0.0, label)[0];
        tw + pad_x * 2.0
    };
    let sep_w = |label: &str| -> f32 {
        if quick_select {
            (font.calc_text_size(font_size, f32::MAX, 0.0, label)[0] + pad_x * 2.0).max(1.0)
        } else {
            font.calc_text_size(font_size, f32::MAX, 0.0, label)[0]
        }
    };

    let mut crumbs: Vec<String> = Vec::new();
    let mut saw_prefix = false;
    for comp in cwd.components() {
        use std::path::Component;
        match comp {
            Component::Prefix(p) => {
                saw_prefix = true;
                crumbs.push(p.as_os_str().to_string_lossy().to_string());
            }
            Component::RootDir => {
                // Match IGFD: on Windows, the root separator after a drive/UNC prefix is not a
                // separate breadcrumb element (e.g. `F:` + `\` is rendered as `F:\`).
                if !saw_prefix {
                    crumbs.push(String::from(std::path::MAIN_SEPARATOR));
                }
            }
            Component::Normal(seg) => crumbs.push(seg.to_string_lossy().to_string()),
            _ => {}
        }
    }

    let n = crumbs.len();
    if n == 0 {
        return 0.0;
    }

    let compress = max_segments > 0 && n > max_segments && max_segments >= 3;
    let mut widths: Vec<f32> = Vec::new();

    if !compress {
        for (i, label) in crumbs.iter().enumerate() {
            widths.push(button_w(label));
            if i + 1 < n {
                // Avoid a duplicated root separator: "/" + "home" should render as "/home",
                // not "//home". Same for Windows root paths like "\\Windows".
                let is_root_crumb = i == 0 && label == sep_label;
                if !is_root_crumb {
                    widths.push(sep_w(sep_label));
                }
            }
        }
    } else {
        let tail = max_segments - 2;
        let start_tail = n.saturating_sub(tail);

        widths.push(button_w(&crumbs[0]));
        if crumbs[0] != sep_label {
            widths.push(sep_w(sep_label));
        }
        widths.push(button_w("..."));
        widths.push(sep_w(sep_label));

        for (i, label) in crumbs.iter().enumerate().skip(start_tail) {
            widths.push(button_w(label));
            if i + 1 < n {
                widths.push(sep_w(sep_label));
            }
        }
    }

    let gaps = widths.len().saturating_sub(1) as f32;
    widths.iter().sum::<f32>() + spacing_x * gaps
}

pub(super) fn draw_breadcrumbs(
    ui: &Ui,
    state: &mut FileDialogState,
    _fs: &dyn FileSystem,
    max_segments: usize,
    newline_at_end: bool,
    auto_scroll_end: bool,
) -> Option<PathBuf> {
    let sep_label = if std::path::MAIN_SEPARATOR == '\\' {
        "\\"
    } else {
        "/"
    };

    // Build crumbs first to avoid borrowing cwd while mutating it
    let mut crumbs: Vec<(String, PathBuf)> = Vec::new();
    let mut acc = PathBuf::new();
    let mut last_is_prefix = false;
    for comp in state.core.cwd.components() {
        use std::path::Component;
        match comp {
            Component::Prefix(p) => {
                acc.push(p.as_os_str());
                crumbs.push((p.as_os_str().to_string_lossy().to_string(), acc.clone()));
                last_is_prefix = true;
            }
            Component::RootDir => {
                acc.push(std::path::MAIN_SEPARATOR.to_string());
                // Match IGFD: on Windows, the root separator after a drive/UNC prefix is not a
                // separate breadcrumb element, but clicking the prefix crumb should navigate to
                // the root (e.g. `F:` navigates to `F:\`).
                if last_is_prefix {
                    if let Some((_, p)) = crumbs.last_mut() {
                        *p = acc.clone();
                    }
                } else {
                    crumbs.push((String::from(std::path::MAIN_SEPARATOR), acc.clone()));
                }
                last_is_prefix = false;
            }
            Component::Normal(seg) => {
                acc.push(seg);
                crumbs.push((seg.to_string_lossy().to_string(), acc.clone()));
                last_is_prefix = false;
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
            if ui.is_item_clicked_with_button(MouseButton::Right) {
                state.ui.path_edit = true;
                state.ui.path_edit_buffer = path.display().to_string();
                state.ui.focus_path_edit_next = true;
                if state.ui.path_bar_style == PathBarStyle::Breadcrumbs {
                    state.ui.path_input_mode = true;
                }
            }
            if ui.is_item_hovered() {
                ui.tooltip_text(path.display().to_string());
            }
            if auto_scroll_end && i + 1 == n && state.ui.breadcrumbs_scroll_to_end_next {
                ui.set_scroll_here_x(1.0);
                state.ui.breadcrumbs_scroll_to_end_next = false;
            }
            ui.same_line();
            if i + 1 < n {
                let is_root_crumb = i == 0 && label == sep_label;
                if is_root_crumb {
                    continue;
                }
                if state.ui.breadcrumbs_quick_select {
                    if ui.small_button(sep_label) {
                        state.ui.breadcrumb_quick_parent = Some(path.clone());
                        ui.open_popup("##igfd_path_popup");
                    }
                    if ui.is_item_clicked_with_button(MouseButton::Right) {
                        state.ui.path_edit = true;
                        state.ui.path_edit_buffer = path.display().to_string();
                        state.ui.focus_path_edit_next = true;
                        if state.ui.path_bar_style == PathBarStyle::Breadcrumbs {
                            state.ui.path_input_mode = true;
                        }
                    }
                } else {
                    ui.text(sep_label);
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
            if ui.is_item_clicked_with_button(MouseButton::Right) {
                state.ui.path_edit = true;
                state.ui.path_edit_buffer = path.display().to_string();
                state.ui.focus_path_edit_next = true;
                if state.ui.path_bar_style == PathBarStyle::Breadcrumbs {
                    state.ui.path_input_mode = true;
                }
            }
            if ui.is_item_hovered() {
                ui.tooltip_text(path.display().to_string());
            }
            ui.same_line();
            if label != sep_label {
                if state.ui.breadcrumbs_quick_select {
                    if ui.small_button(sep_label) {
                        state.ui.breadcrumb_quick_parent = Some(path.clone());
                        ui.open_popup("##igfd_path_popup");
                    }
                    if ui.is_item_clicked_with_button(MouseButton::Right) {
                        state.ui.path_edit = true;
                        state.ui.path_edit_buffer = path.display().to_string();
                        state.ui.focus_path_edit_next = true;
                        if state.ui.path_bar_style == PathBarStyle::Breadcrumbs {
                            state.ui.path_input_mode = true;
                        }
                    }
                } else {
                    ui.text(sep_label);
                }
            }
            ui.same_line();
        }

        // Ellipsis
        if ui.small_button("...") {
            ui.open_popup("##breadcrumb_ellipsis_popup");
        }
        if ui.is_item_hovered() {
            ui.tooltip_text("Jump to hidden segments");
        }
        if let Some(_popup) = ui.begin_popup("##breadcrumb_ellipsis_popup") {
            let tail = max_segments - 2;
            let start_tail = n.saturating_sub(tail);
            let hidden_end = start_tail.saturating_sub(1);
            ui.text_disabled("Path:");
            ui.separator();
            for (i, (label, path)) in crumbs.iter().enumerate().skip(1).take(hidden_end) {
                let _id = ui.push_id(i as i32);
                if ui.selectable(label) {
                    new_cwd = Some(path.clone());
                    ui.close_current_popup();
                    break;
                }
                if ui.is_item_hovered() {
                    ui.tooltip_text(path.display().to_string());
                }
            }
        }
        ui.same_line();

        let tail = max_segments - 2;
        let start_tail = n.saturating_sub(tail);
        if let Some((_, parent)) = crumbs.get(start_tail.saturating_sub(1)) {
            if state.ui.breadcrumbs_quick_select {
                if ui.small_button(sep_label) {
                    state.ui.breadcrumb_quick_parent = Some(parent.clone());
                    ui.open_popup("##igfd_path_popup");
                }
                if ui.is_item_clicked_with_button(MouseButton::Right) {
                    state.ui.path_edit = true;
                    state.ui.path_edit_buffer = parent.display().to_string();
                    state.ui.focus_path_edit_next = true;
                    if state.ui.path_bar_style == PathBarStyle::Breadcrumbs {
                        state.ui.path_input_mode = true;
                    }
                }
            } else {
                ui.text(sep_label);
            }
            ui.same_line();
        } else {
            ui.text(sep_label);
            ui.same_line();
        }

        // Tail segments
        let start_tail = n.saturating_sub(tail);
        for (i, (label, path)) in crumbs.iter().enumerate().skip(start_tail) {
            let _id = ui.push_id(i as i32);
            if ui.button(label) {
                new_cwd = Some(path.clone());
            }
            if ui.is_item_clicked_with_button(MouseButton::Right) {
                state.ui.path_edit = true;
                state.ui.path_edit_buffer = path.display().to_string();
                state.ui.focus_path_edit_next = true;
                if state.ui.path_bar_style == PathBarStyle::Breadcrumbs {
                    state.ui.path_input_mode = true;
                }
            }
            if ui.is_item_hovered() {
                ui.tooltip_text(path.display().to_string());
            }
            if auto_scroll_end && i + 1 == n && state.ui.breadcrumbs_scroll_to_end_next {
                ui.set_scroll_here_x(1.0);
                state.ui.breadcrumbs_scroll_to_end_next = false;
            }
            ui.same_line();
            if i + 1 < n {
                if state.ui.breadcrumbs_quick_select {
                    if ui.small_button(sep_label) {
                        state.ui.breadcrumb_quick_parent = Some(path.clone());
                        ui.open_popup("##igfd_path_popup");
                    }
                    if ui.is_item_clicked_with_button(MouseButton::Right) {
                        state.ui.path_edit = true;
                        state.ui.path_edit_buffer = path.display().to_string();
                        state.ui.focus_path_edit_next = true;
                        if state.ui.path_bar_style == PathBarStyle::Breadcrumbs {
                            state.ui.path_input_mode = true;
                        }
                    }
                } else {
                    ui.text(sep_label);
                }
                ui.same_line();
            }
        }
    }
    if newline_at_end {
        ui.new_line();
    }
    new_cwd
}
