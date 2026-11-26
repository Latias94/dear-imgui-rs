use std::path::{Path, PathBuf};

use dear_imgui_rs::Ui;
use dear_imgui_rs::input::MouseButton;
use dear_imgui_rs::{
    HistoryDirection, InputTextCallback, InputTextCallbackHandler, TextCallbackData,
};

use crate::dialog_core::{CoreEvent, ProjectedEntries};
use crate::dialog_state::{FileDialogState, PathBarStyle};
use crate::fs::display_path;

struct PathBarCallback<'a> {
    cwd: &'a Path,
    entries: ProjectedEntries<'a>,
    recent_paths: &'a [PathBuf],
    history_index: &'a mut Option<usize>,
    history_saved_buffer: &'a mut Option<(String, Option<PathBuf>)>,
    buffer_path: &'a mut Option<PathBuf>,
    programmatic_edit: &'a mut bool,
}

impl PathBarCallback<'_> {
    fn set_text(&mut self, mut data: TextCallbackData, text: &str) {
        let Ok(old) = data.str() else {
            return;
        };
        *self.programmatic_edit = true;
        if old == text {
            return;
        }
        data.remove_chars(0, old.len());
        data.insert_chars(0, text);
        data.set_cursor_pos(text.len());
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
        s.char_indices().rfind(|(_, c)| *c == '/' || *c == '\\')
    }

    fn try_complete_path(&mut self, data: TextCallbackData) {
        let Ok(input) = data.str() else {
            return;
        };
        let input = input.trim();
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
            self.cwd.to_owned()
        } else {
            let raw = PathBuf::from(dir_prefix);
            if raw.is_absolute() {
                raw
            } else {
                self.cwd.join(raw)
            }
        };

        if base_dir != self.cwd && display_path(&base_dir) != display_path(self.cwd) {
            return;
        }

        let mut matches = self
            .entries
            .iter()
            .filter(|entry| entry.is_dir && Self::starts_with_case_insensitive(&entry.name, frag))
            .collect::<Vec<_>>();
        if matches.is_empty() {
            return;
        }
        matches.sort_unstable_by(|left, right| left.name.cmp(&right.name));

        let completed = if matches.len() == 1 {
            let mut s = matches[0].name.clone();
            s.push(sep);
            s
        } else {
            let first = matches[0].name.as_str();
            let mut prefix_len = first.len();
            for other in matches.iter().skip(1) {
                prefix_len = prefix_len.min(Self::common_prefix_len(first, &other.name));
            }
            first[..prefix_len].to_string()
        };

        let new_text = if dir_prefix.is_empty() {
            completed
        } else {
            format!("{dir_prefix}{completed}")
        };

        *self.history_index = None;
        *self.history_saved_buffer = None;
        *self.buffer_path = (matches.len() == 1).then(|| matches[0].path.clone());
        self.set_text(data, &new_text);
    }

    fn apply_history(&mut self, direction: HistoryDirection, data: TextCallbackData) {
        if self.recent_paths.is_empty() {
            return;
        }
        let Ok(input) = data.str() else {
            return;
        };

        let index = match (direction, *self.history_index) {
            (HistoryDirection::Up, None) => {
                *self.history_saved_buffer = Some((input.to_owned(), self.buffer_path.clone()));
                0
            }
            (HistoryDirection::Up, Some(index)) if index + 1 < self.recent_paths.len() => index + 1,
            (HistoryDirection::Down, Some(0)) => {
                let (text, path) = self.history_saved_buffer.take().unwrap_or_default();
                *self.history_index = None;
                *self.buffer_path = path;
                self.set_text(data, &text);
                return;
            }
            (HistoryDirection::Down, Some(index)) => index - 1,
            _ => return,
        };
        let path = &self.recent_paths[index];
        *self.history_index = Some(index);
        *self.buffer_path = Some(path.clone());
        self.set_text(data, &display_path(path));
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
    recent_paths: &[PathBuf],
    path_w: f32,
    show_go_button: bool,
) {
    let prev_path_buffer = state.ui.runtime.path.buffer.clone();
    ui.set_next_item_width(path_w);
    let select_all = state.ui.runtime.path.focus_next;
    if select_all {
        ui.set_keyboard_focus_here();
        state.ui.runtime.path.focus_next = false;
    }

    let path = &mut state.ui.runtime.path;
    let callback = PathBarCallback {
        cwd: &state.core.cwd,
        entries: state.core.entries(),
        recent_paths,
        history_index: &mut path.history_index,
        history_saved_buffer: &mut path.history_saved_buffer,
        buffer_path: &mut path.buffer_path,
        programmatic_edit: &mut path.programmatic_edit,
    };
    let submitted = ui
        .input_text("##path_bar", &mut path.buffer)
        .callback(callback)
        .callback_flags(InputTextCallback::COMPLETION | InputTextCallback::HISTORY)
        .auto_select_all(select_all)
        .enter_returns_true(true)
        .build();

    let path_active = ui.is_item_active() || ui.is_item_focused();
    state.ui.runtime.path.edit = path_active;
    if path_active
        && !state.ui.runtime.path.programmatic_edit
        && state.ui.runtime.path.buffer != prev_path_buffer
    {
        state.ui.runtime.path.history_index = None;
        state.ui.runtime.path.history_saved_buffer = None;
        state.ui.runtime.path.buffer_path = None;
    }
    state.ui.runtime.path.programmatic_edit = false;

    if show_go_button {
        ui.same_line();
        if ui.button("Go") || (path_active && submitted) {
            submit_path_edit(state);
        }
    } else if path_active && submitted {
        submit_path_edit(state);
    }
}

fn submit_path_edit(state: &mut FileDialogState) {
    let input = state.ui.runtime.path.buffer.trim();
    if input.is_empty() && state.ui.runtime.path.buffer_path.is_none() {
        state.ui.runtime.error = Some("Path is empty".into());
        return;
    }

    let raw_p = state
        .ui
        .runtime
        .path
        .buffer_path
        .clone()
        .unwrap_or_else(|| {
            let typed = Path::new(input);
            if typed.is_absolute() {
                typed.to_owned()
            } else {
                state.core.cwd.join(typed)
            }
        });
    let (p, metadata) = {
        let fs = state.filesystem.as_file_system();
        let p = fs.canonicalize(&raw_p).unwrap_or(raw_p.clone());
        let metadata = fs.metadata(&p);
        (p, metadata)
    };
    match metadata {
        Ok(md) => {
            if md.is_dir {
                let _ = state.core.handle_event(CoreEvent::NavigateTo(p));
                state.ui.runtime.path.edit = false;
                state.ui.runtime.path.last_cwd.clone_from(&state.core.cwd);
                state.ui.runtime.path.set_buffer_path(&state.core.cwd);
                state.ui.runtime.error = None;
            } else {
                state.ui.runtime.error = Some("Path exists but is not a directory".into());
            }
        }
        Err(e) => {
            use std::io::ErrorKind::*;
            let msg = match e.kind() {
                NotFound => format!("No such directory: {}", input),
                PermissionDenied => format!("Permission denied: {}", input),
                _ => format!("Invalid directory '{}': {}", input, e),
            };
            state.ui.runtime.error = Some(msg);
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

    let button_w = |label: &str| -> f32 {
        let tw = ui.calc_text_size(label)[0];
        tw + pad_x * 2.0
    };
    let sep_w = |label: &str| -> f32 {
        if quick_select {
            (ui.calc_text_size(label)[0] + pad_x * 2.0).max(1.0)
        } else {
            ui.calc_text_size(label)[0]
        }
    };

    let mut crumbs: Vec<String> = Vec::new();
    let mut saw_prefix = false;
    for comp in cwd.components() {
        use std::path::Component;
        match comp {
            Component::Prefix(p) => {
                saw_prefix = true;
                crumbs.push(display_path(Path::new(p.as_os_str())).into_owned());
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
                crumbs.push((
                    display_path(Path::new(p.as_os_str())).into_owned(),
                    acc.clone(),
                ));
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
                state.ui.runtime.path.edit = true;
                state.ui.runtime.path.set_buffer_path(path);
                state.ui.runtime.path.focus_next = true;
                if state.ui.config.path_bar_style == PathBarStyle::Breadcrumbs {
                    state.ui.runtime.path.input_mode = true;
                }
            }
            if ui.is_item_hovered() {
                ui.tooltip_text(display_path(path).as_ref());
            }
            if auto_scroll_end && i + 1 == n && state.ui.runtime.breadcrumb.scroll_to_end_next {
                ui.set_scroll_here_x(1.0);
                state.ui.runtime.breadcrumb.scroll_to_end_next = false;
            }
            ui.same_line();
            if i + 1 < n {
                let is_root_crumb = i == 0 && label == sep_label;
                if is_root_crumb {
                    continue;
                }
                if state.ui.config.breadcrumbs_quick_select {
                    if ui.small_button(sep_label) {
                        state.ui.runtime.breadcrumb.quick_parent = Some(path.clone());
                        ui.open_popup("##igfd_path_popup");
                    }
                    if ui.is_item_clicked_with_button(MouseButton::Right) {
                        state.ui.runtime.path.edit = true;
                        state.ui.runtime.path.set_buffer_path(path);
                        state.ui.runtime.path.focus_next = true;
                        if state.ui.config.path_bar_style == PathBarStyle::Breadcrumbs {
                            state.ui.runtime.path.input_mode = true;
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
                state.ui.runtime.path.edit = true;
                state.ui.runtime.path.set_buffer_path(path);
                state.ui.runtime.path.focus_next = true;
                if state.ui.config.path_bar_style == PathBarStyle::Breadcrumbs {
                    state.ui.runtime.path.input_mode = true;
                }
            }
            if ui.is_item_hovered() {
                ui.tooltip_text(display_path(path).as_ref());
            }
            ui.same_line();
            if label != sep_label {
                if state.ui.config.breadcrumbs_quick_select {
                    if ui.small_button(sep_label) {
                        state.ui.runtime.breadcrumb.quick_parent = Some(path.clone());
                        ui.open_popup("##igfd_path_popup");
                    }
                    if ui.is_item_clicked_with_button(MouseButton::Right) {
                        state.ui.runtime.path.edit = true;
                        state.ui.runtime.path.set_buffer_path(path);
                        state.ui.runtime.path.focus_next = true;
                        if state.ui.config.path_bar_style == PathBarStyle::Breadcrumbs {
                            state.ui.runtime.path.input_mode = true;
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
                    ui.tooltip_text(display_path(path).as_ref());
                }
            }
        }
        ui.same_line();

        let tail = max_segments - 2;
        let start_tail = n.saturating_sub(tail);
        if let Some((_, parent)) = crumbs.get(start_tail.saturating_sub(1)) {
            if state.ui.config.breadcrumbs_quick_select {
                if ui.small_button(sep_label) {
                    state.ui.runtime.breadcrumb.quick_parent = Some(parent.clone());
                    ui.open_popup("##igfd_path_popup");
                }
                if ui.is_item_clicked_with_button(MouseButton::Right) {
                    state.ui.runtime.path.edit = true;
                    state.ui.runtime.path.set_buffer_path(parent);
                    state.ui.runtime.path.focus_next = true;
                    if state.ui.config.path_bar_style == PathBarStyle::Breadcrumbs {
                        state.ui.runtime.path.input_mode = true;
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
                state.ui.runtime.path.edit = true;
                state.ui.runtime.path.set_buffer_path(path);
                state.ui.runtime.path.focus_next = true;
                if state.ui.config.path_bar_style == PathBarStyle::Breadcrumbs {
                    state.ui.runtime.path.input_mode = true;
                }
            }
            if ui.is_item_hovered() {
                ui.tooltip_text(display_path(path).as_ref());
            }
            if auto_scroll_end && i + 1 == n && state.ui.runtime.breadcrumb.scroll_to_end_next {
                ui.set_scroll_here_x(1.0);
                state.ui.runtime.breadcrumb.scroll_to_end_next = false;
            }
            ui.same_line();
            if i + 1 < n {
                if state.ui.config.breadcrumbs_quick_select {
                    if ui.small_button(sep_label) {
                        state.ui.runtime.breadcrumb.quick_parent = Some(path.clone());
                        ui.open_popup("##igfd_path_popup");
                    }
                    if ui.is_item_clicked_with_button(MouseButton::Right) {
                        state.ui.runtime.path.edit = true;
                        state.ui.runtime.path.set_buffer_path(path);
                        state.ui.runtime.path.focus_next = true;
                        if state.ui.config.path_bar_style == PathBarStyle::Breadcrumbs {
                            state.ui.runtime.path.input_mode = true;
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
