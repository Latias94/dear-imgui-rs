use std::path::{Path, PathBuf};

use crate::browser_events::BrowserEvent;
use crate::browser_state::FileBrowserState;
use crate::core::{DialogMode, FileDialogError, FileFilter, Selection, SortBy};
use crate::fs::{FileSystem, StdFileSystem};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct EntryId(PathBuf);

impl EntryId {
    pub(crate) fn new(path: PathBuf) -> Self {
        Self(path)
    }

    pub(crate) fn as_path(&self) -> &Path {
        &self.0
    }
}

#[derive(Clone, Debug)]
pub(crate) struct BrowserEntry {
    pub(crate) id: EntryId,
    pub(crate) name: String,
    pub(crate) path: PathBuf,
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

fn select_single_by_name(state: &mut FileBrowserState, name: String) {
    state.selected.clear();
    state.selected.push(name.clone());
    state.focused_name = Some(name.clone());
    state.selection_anchor_name = Some(name);
}

fn select_range_by_name(view_names: &[String], anchor: &str, target: &str) -> Option<Vec<String>> {
    let ia = view_names.iter().position(|s| s == anchor)?;
    let it = view_names.iter().position(|s| s == target)?;
    let (lo, hi) = if ia <= it { (ia, it) } else { (it, ia) };
    Some(view_names[lo..=hi].to_vec())
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
    apply_event_with_fs(state, ev, &StdFileSystem);
}

pub(crate) fn apply_event_with_fs(
    state: &mut FileBrowserState,
    ev: BrowserEvent,
    fs: &dyn FileSystem,
) {
    match ev {
        BrowserEvent::NavigateUp => {
            let _ = state.cwd.pop();
            state.selected.clear();
        }
        BrowserEvent::NavigateTo(p) => {
            state.cwd = p;
            state.selected.clear();
            state.focused_name = None;
            state.selection_anchor_name = None;
        }
        BrowserEvent::StartPathEdit => {
            state.path_edit = true;
            state.path_edit_buffer = state.cwd.display().to_string();
            state.focus_path_edit_next = true;
        }
        BrowserEvent::SubmitPathEdit => {
            let input = state.path_edit_buffer.trim();
            let raw_p = std::path::PathBuf::from(input);
            let p = fs.canonicalize(&raw_p).unwrap_or(raw_p.clone());
            match fs.metadata(&p) {
                Ok(md) => {
                    if md.is_dir {
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
        BrowserEvent::ClickEntry {
            name,
            is_dir,
            modifiers,
        } => {
            if is_dir {
                match state.click_action {
                    crate::core::ClickAction::Select => {
                        select_single_by_name(state, name);
                    }
                    crate::core::ClickAction::Navigate => {
                        state.cwd.push(&name);
                        state.selected.clear();
                        state.focused_name = None;
                        state.selection_anchor_name = None;
                    }
                }
            } else {
                if modifiers.shift {
                    if let Some(anchor) = state.selection_anchor_name.clone() {
                        if let Some(range) = select_range_by_name(&state.view_names, &anchor, &name)
                        {
                            state.selected = range;
                            state.focused_name = Some(name);
                            return;
                        }
                    }
                    // Fallback if range selection isn't possible.
                    select_single_by_name(state, name);
                    return;
                }

                if !state.allow_multi || !modifiers.ctrl {
                    select_single_by_name(state, name);
                    return;
                }

                // Ctrl toggle selection
                toggle_select_name(&mut state.selected, &name);
                state.focused_name = Some(name.clone());
                state.selection_anchor_name = Some(name);
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
        BrowserEvent::MoveFocus { delta, modifiers } => {
            if state.view_names.is_empty() {
                return;
            }

            let len = state.view_names.len();
            let current_idx = state
                .focused_name
                .as_ref()
                .and_then(|n| state.view_names.iter().position(|s| s == n));

            let next_idx = match current_idx {
                Some(i) => {
                    let next = i as i32 + delta;
                    next.clamp(0, (len - 1) as i32) as usize
                }
                None => {
                    if delta >= 0 {
                        0
                    } else {
                        len - 1
                    }
                }
            };

            let target = state.view_names[next_idx].clone();
            if modifiers.shift {
                let anchor = state
                    .selection_anchor_name
                    .clone()
                    .or_else(|| state.focused_name.clone())
                    .unwrap_or_else(|| target.clone());
                if state.selection_anchor_name.is_none() {
                    state.selection_anchor_name = Some(anchor.clone());
                }

                if let Some(range) = select_range_by_name(&state.view_names, &anchor, &target) {
                    state.selected = range;
                    state.focused_name = Some(target);
                } else {
                    select_single_by_name(state, target);
                }
            } else {
                select_single_by_name(state, target);
            }
        }
        BrowserEvent::ActivateFocused => {
            if state.selected.is_empty() {
                if let Some(name) = state.focused_name.clone() {
                    state.selected.push(name);
                }
            }
            if !state.selected.is_empty() {
                apply_event_with_fs(state, BrowserEvent::Confirm, fs);
            }
        }
        BrowserEvent::SelectAll => {
            if state.allow_multi {
                state.selected = state.view_names.clone();
            }
        }
        BrowserEvent::Confirm => {
            // Special-case: if a single directory selected in file-open modes, navigate into it
            // instead of confirming.
            if matches!(state.mode, DialogMode::OpenFile | DialogMode::OpenFiles)
                && state.selected.len() == 1
            {
                let sel = state.selected[0].clone();
                let p = state.cwd.join(&sel);
                let is_dir = fs.metadata(&p).map(|m| m.is_dir).unwrap_or(false);
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
    read_entries_with_fs(&StdFileSystem, dir, show_hidden)
}

pub(crate) fn read_entries_with_fs(
    fs: &dyn FileSystem,
    dir: &Path,
    show_hidden: bool,
) -> Vec<BrowserEntry> {
    let mut out = Vec::new();
    let Ok(rd) = fs.read_dir(dir) else {
        return out;
    };
    for e in rd {
        if !show_hidden && e.name.starts_with('.') {
            continue;
        }
        let id = EntryId::new(e.path.clone());
        out.push(BrowserEntry {
            id,
            name: e.name,
            path: e.path,
            is_dir: e.is_dir,
            size: e.size,
            modified: e.modified,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browser_events::Modifiers;
    use crate::core::ClickAction;

    fn mods(ctrl: bool, shift: bool) -> Modifiers {
        Modifiers { ctrl, shift }
    }

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
                modifiers: mods(true, false),
            },
        );
        assert_eq!(state.selected, vec!["a.txt"]);
        apply_event(
            &mut state,
            BrowserEvent::ClickEntry {
                name: "a.txt".into(),
                is_dir: false,
                modifiers: mods(true, false),
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
                modifiers: mods(false, false),
            },
        );
        apply_event(
            &mut state,
            BrowserEvent::ClickEntry {
                name: "b.txt".into(),
                is_dir: false,
                modifiers: mods(false, false),
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
                modifiers: mods(false, false),
            },
        );
        assert!(state.selected.is_empty());
        assert!(state.cwd.ends_with("sub"));
    }

    #[test]
    fn shift_click_selects_a_range_in_view_order() {
        let mut state = FileBrowserState::new(DialogMode::OpenFiles);
        state.allow_multi = true;
        state.view_names = vec![
            "a.txt".into(),
            "b.txt".into(),
            "c.txt".into(),
            "d.txt".into(),
            "e.txt".into(),
        ];

        apply_event(
            &mut state,
            BrowserEvent::ClickEntry {
                name: "b.txt".into(),
                is_dir: false,
                modifiers: mods(false, false),
            },
        );
        assert_eq!(state.selected, vec!["b.txt"]);

        apply_event(
            &mut state,
            BrowserEvent::ClickEntry {
                name: "e.txt".into(),
                is_dir: false,
                modifiers: mods(false, true),
            },
        );
        assert_eq!(state.selected, vec!["b.txt", "c.txt", "d.txt", "e.txt"]);
    }

    #[test]
    fn ctrl_a_selects_all_when_multi_select_enabled() {
        let mut state = FileBrowserState::new(DialogMode::OpenFiles);
        state.allow_multi = true;
        state.view_names = vec!["a".into(), "b".into(), "c".into()];

        apply_event(&mut state, BrowserEvent::SelectAll);
        assert_eq!(state.selected, vec!["a", "b", "c"]);
    }

    #[test]
    fn move_focus_selects_first_when_unfocused() {
        let mut state = FileBrowserState::new(DialogMode::OpenFiles);
        state.allow_multi = true;
        state.view_names = vec!["a".into(), "b".into(), "c".into()];

        apply_event(
            &mut state,
            BrowserEvent::MoveFocus {
                delta: 1,
                modifiers: mods(false, false),
            },
        );
        assert_eq!(state.selected, vec!["a"]);
        assert_eq!(state.focused_name.as_deref(), Some("a"));
    }

    #[test]
    fn move_focus_with_shift_extends_range() {
        let mut state = FileBrowserState::new(DialogMode::OpenFiles);
        state.allow_multi = true;
        state.view_names = vec!["a".into(), "b".into(), "c".into(), "d".into()];

        apply_event(
            &mut state,
            BrowserEvent::ClickEntry {
                name: "b".into(),
                is_dir: false,
                modifiers: mods(false, false),
            },
        );

        apply_event(
            &mut state,
            BrowserEvent::MoveFocus {
                delta: 2,
                modifiers: mods(false, true),
            },
        );
        assert_eq!(state.selected, vec!["b", "c", "d"]);
        assert_eq!(state.focused_name.as_deref(), Some("d"));
    }

    #[test]
    fn activate_focused_confirms_selection() {
        let mut state = FileBrowserState::new(DialogMode::OpenFile);
        state.view_names = vec!["a.txt".into()];
        state.allow_multi = false;

        apply_event(
            &mut state,
            BrowserEvent::ClickEntry {
                name: "a.txt".into(),
                is_dir: false,
                modifiers: mods(false, false),
            },
        );
        apply_event(&mut state, BrowserEvent::ActivateFocused);

        assert!(!state.visible);
        let sel = state.result.unwrap().unwrap();
        assert_eq!(sel.paths.len(), 1);
        assert_eq!(
            sel.paths[0].file_name().and_then(|s| s.to_str()),
            Some("a.txt")
        );
    }
}
