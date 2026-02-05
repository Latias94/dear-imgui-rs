use std::path::Path;

use crate::browser_events::BrowserEvent;
use crate::browser_state::FileBrowserState;
use crate::core::{DialogMode, FileDialogError, FileFilter, Selection, SortBy};

#[derive(Clone, Debug)]
pub(crate) struct BrowserEntry {
    pub(crate) name: String,
    pub(crate) is_dir: bool,
    pub(crate) size: Option<u64>,
    pub(crate) modified: Option<std::time::SystemTime>,
}

impl BrowserEntry {
    pub(crate) fn display_name(&self) -> String {
        if self.is_dir {
            format!("[{}]", self.name)
        } else {
            self.name.clone()
        }
    }
}

pub(crate) fn effective_filters(
    filters: &[FileFilter],
    active_filter: Option<usize>,
) -> Vec<FileFilter> {
    match active_filter {
        Some(i) => filters.get(i).cloned().into_iter().collect(),
        None => Vec::new(),
    }
}

pub(crate) fn matches_filters(name: &str, filters: &[FileFilter]) -> bool {
    if filters.is_empty() {
        return true;
    }
    let ext = Path::new(name)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase());
    match ext {
        Some(e) => filters.iter().any(|f| f.extensions.iter().any(|x| x == &e)),
        None => false,
    }
}

pub(crate) fn toggle_select_name(list: &mut Vec<String>, name: &str) {
    if let Some(i) = list.iter().position(|s| s == name) {
        list.remove(i);
    } else {
        list.push(name.to_string());
    }
}

pub(crate) fn finalize_selection(
    mode: DialogMode,
    cwd: &Path,
    selected_names: Vec<String>,
    save_name: &str,
    filters: &[FileFilter],
    active_filter: Option<usize>,
) -> Result<Selection, FileDialogError> {
    let mut sel = Selection { paths: Vec::new() };
    let eff_filters = effective_filters(filters, active_filter);
    match mode {
        DialogMode::PickFolder => {
            sel.paths.push(cwd.to_path_buf());
        }
        DialogMode::OpenFile | DialogMode::OpenFiles => {
            if selected_names.is_empty() {
                return Err(FileDialogError::InvalidPath("no selection".into()));
            }
            for n in selected_names {
                if !matches_filters(&n, &eff_filters) {
                    continue;
                }
                sel.paths.push(cwd.join(n));
            }
            if sel.paths.is_empty() {
                return Err(FileDialogError::InvalidPath(
                    "no file matched filters".into(),
                ));
            }
        }
        DialogMode::SaveFile => {
            let name = save_name.trim();
            if name.is_empty() {
                return Err(FileDialogError::InvalidPath("empty file name".into()));
            }
            sel.paths.push(cwd.join(name));
        }
    }
    Ok(sel)
}

pub(crate) fn apply_event(state: &mut FileBrowserState, ev: BrowserEvent) {
    match ev {
        BrowserEvent::NavigateUp => {
            let _ = state.cwd.pop();
            state.selected.clear();
        }
        BrowserEvent::NavigateTo(p) => {
            state.cwd = p;
        }
        BrowserEvent::StartPathEdit => {
            state.path_edit = true;
            state.path_edit_buffer = state.cwd.display().to_string();
            state.focus_path_edit_next = true;
        }
        BrowserEvent::SubmitPathEdit => {
            let input = state.path_edit_buffer.trim();
            let raw_p = std::path::PathBuf::from(input);
            let p = std::fs::canonicalize(&raw_p).unwrap_or(raw_p.clone());
            match std::fs::metadata(&p) {
                Ok(md) => {
                    if md.is_dir() {
                        state.cwd = p;
                        state.selected.clear();
                        state.path_edit = false;
                        state.ui_error = None;
                    } else {
                        state.ui_error = Some("Path exists but is not a directory".into());
                    }
                }
                Err(e) => {
                    use std::io::ErrorKind::*;
                    let msg = match e.kind() {
                        NotFound => format!("No such directory: {}", input),
                        PermissionDenied => format!("Permission denied: {}", input),
                        _ => format!("Invalid directory '{}': {}", input, e),
                    };
                    state.ui_error = Some(msg);
                }
            }
        }
        BrowserEvent::CancelPathEdit => {
            state.path_edit = false;
        }
        BrowserEvent::RequestSearchFocus => {
            state.focus_search_next = true;
        }
        BrowserEvent::SetShowHidden(v) => {
            state.show_hidden = v;
        }
        BrowserEvent::SetActiveFilter(v) => {
            state.active_filter = v;
        }
        BrowserEvent::SetSearch(v) => {
            state.search = v;
        }
        BrowserEvent::SetSort { by, ascending } => {
            state.sort_by = by;
            state.sort_ascending = ascending;
        }
        BrowserEvent::SetClickAction(v) => {
            state.click_action = v;
        }
        BrowserEvent::SetDoubleClick(v) => {
            state.double_click = v;
        }
        BrowserEvent::ClickEntry { name, is_dir } => {
            if is_dir {
                match state.click_action {
                    crate::core::ClickAction::Select => {
                        state.selected.clear();
                        state.selected.push(name);
                    }
                    crate::core::ClickAction::Navigate => {
                        state.cwd.push(&name);
                        state.selected.clear();
                    }
                }
            } else {
                if !state.allow_multi {
                    state.selected.clear();
                }
                toggle_select_name(&mut state.selected, &name);
            }
        }
        BrowserEvent::DoubleClickEntry { name, is_dir } => {
            if is_dir {
                state.cwd.push(&name);
                state.selected.clear();
            } else if matches!(state.mode, DialogMode::OpenFile | DialogMode::OpenFiles) {
                state.selected.clear();
                state.selected.push(name);
                match finalize_selection_with_take(state) {
                    Ok(sel) => {
                        state.result = Some(Ok(sel));
                        state.visible = false;
                    }
                    Err(err) => {
                        state.ui_error = Some(err.to_string());
                    }
                }
            }
        }
        BrowserEvent::Confirm => {
            // Special-case: if a single directory selected in file-open modes, navigate into it
            // instead of confirming.
            if matches!(state.mode, DialogMode::OpenFile | DialogMode::OpenFiles)
                && state.selected.len() == 1
            {
                let sel = state.selected[0].clone();
                let is_dir = state.cwd.join(&sel).is_dir();
                if is_dir {
                    state.cwd.push(sel);
                    state.selected.clear();
                    return;
                }
            }
            match finalize_selection_with_take(state) {
                Ok(sel) => {
                    state.result = Some(Ok(sel));
                    state.visible = false;
                }
                Err(e) => state.ui_error = Some(e.to_string()),
            }
        }
        BrowserEvent::Cancel => {
            state.result = Some(Err(FileDialogError::Cancelled));
            state.visible = false;
        }
    }
}

fn finalize_selection_with_take(
    state: &mut FileBrowserState,
) -> Result<Selection, FileDialogError> {
    let selected_names = if matches!(state.mode, DialogMode::OpenFile | DialogMode::OpenFiles) {
        std::mem::take(&mut state.selected)
    } else {
        Vec::new()
    };
    finalize_selection(
        state.mode,
        &state.cwd,
        selected_names,
        &state.save_name,
        &state.filters,
        state.active_filter,
    )
}

pub(crate) fn filter_entries_in_place(
    entries: &mut Vec<BrowserEntry>,
    mode: DialogMode,
    filters: &[FileFilter],
    active_filter: Option<usize>,
    search: &str,
) {
    let display_filters = effective_filters(filters, active_filter);
    let search_lower = if search.is_empty() {
        None
    } else {
        Some(search.to_lowercase())
    };
    entries.retain(|e| {
        let pass_kind = if matches!(mode, DialogMode::PickFolder) {
            e.is_dir
        } else {
            e.is_dir || matches_filters(&e.name, &display_filters)
        };
        let pass_search = match &search_lower {
            None => true,
            Some(q) => e.name.to_lowercase().contains(q),
        };
        pass_kind && pass_search
    });
}

pub(crate) fn sort_entries_in_place(
    entries: &mut Vec<BrowserEntry>,
    sort_by: SortBy,
    sort_ascending: bool,
    dirs_first: bool,
) {
    entries.sort_by(|a, b| {
        if dirs_first && a.is_dir != b.is_dir {
            return b.is_dir.cmp(&a.is_dir);
        }
        let ord = match sort_by {
            SortBy::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            SortBy::Size => a.size.unwrap_or(0).cmp(&b.size.unwrap_or(0)),
            SortBy::Modified => a.modified.cmp(&b.modified),
        };
        if sort_ascending { ord } else { ord.reverse() }
    });
}

pub(crate) fn read_entries(dir: &Path, show_hidden: bool) -> Vec<BrowserEntry> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(dir) else {
        return out;
    };
    for e in rd.flatten() {
        let Ok(ft) = e.file_type() else {
            continue;
        };
        let name = e.file_name().to_string_lossy().to_string();
        if !show_hidden && name.starts_with('.') {
            continue;
        }
        let meta = e.metadata().ok();
        let modified = meta.as_ref().and_then(|m| m.modified().ok());
        let size = if ft.is_file() {
            meta.as_ref().map(|m| m.len())
        } else {
            None
        };
        out.push(BrowserEntry {
            name,
            is_dir: ft.is_dir(),
            size,
            modified,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ClickAction;

    #[test]
    fn cancel_sets_result_and_hides() {
        let mut state = FileBrowserState::new(DialogMode::OpenFile);
        apply_event(&mut state, BrowserEvent::Cancel);
        assert!(!state.visible);
        assert!(matches!(
            state.result,
            Some(Err(crate::FileDialogError::Cancelled))
        ));
    }

    #[test]
    fn click_file_toggles_in_multi_select() {
        let mut state = FileBrowserState::new(DialogMode::OpenFiles);
        state.allow_multi = true;
        apply_event(
            &mut state,
            BrowserEvent::ClickEntry {
                name: "a.txt".into(),
                is_dir: false,
            },
        );
        assert_eq!(state.selected, vec!["a.txt"]);
        apply_event(
            &mut state,
            BrowserEvent::ClickEntry {
                name: "a.txt".into(),
                is_dir: false,
            },
        );
        assert!(state.selected.is_empty());
    }

    #[test]
    fn click_file_replaces_in_single_select() {
        let mut state = FileBrowserState::new(DialogMode::OpenFile);
        state.allow_multi = false;
        apply_event(
            &mut state,
            BrowserEvent::ClickEntry {
                name: "a.txt".into(),
                is_dir: false,
            },
        );
        apply_event(
            &mut state,
            BrowserEvent::ClickEntry {
                name: "b.txt".into(),
                is_dir: false,
            },
        );
        assert_eq!(state.selected, vec!["b.txt"]);
    }

    #[test]
    fn click_dir_navigates_when_configured() {
        let mut state = FileBrowserState::new(DialogMode::OpenFile);
        state.click_action = ClickAction::Navigate;
        state.cwd = std::path::PathBuf::from("root");
        apply_event(
            &mut state,
            BrowserEvent::ClickEntry {
                name: "sub".into(),
                is_dir: true,
            },
        );
        assert!(state.selected.is_empty());
        assert!(state.cwd.ends_with("sub"));
    }
}
