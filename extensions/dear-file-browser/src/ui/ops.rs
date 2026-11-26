use std::path::PathBuf;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::atomic::Ordering;

#[cfg(not(target_arch = "wasm32"))]
use crate::background::BackgroundTask;
use crate::dialog_core::EntryId;
use crate::dialog_state::{
    ClipboardOp, FileClipboard, FileDialogState, PasteConflictAction, PasteConflictPrompt,
    PendingPasteJob,
};
use crate::fs::FileSystem;
use crate::fs_ops::{PasteError, delete_entry, entry_exists, keep_both_target, paste_cancellable};

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug)]
pub(crate) enum FileOperation {
    Delete(DeleteOutcome),
    Paste(PasteOutcome),
}

#[derive(Debug)]
pub(crate) struct DeleteOutcome {
    cwd: PathBuf,
    ids: Vec<EntryId>,
    completed: usize,
    error: Option<String>,
}

#[derive(Debug)]
pub(crate) struct PasteOutcome {
    job: PendingPasteJob,
    previous_count: usize,
    result: Result<bool, String>,
}

pub(super) fn file_operation_in_progress(_state: &FileDialogState) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        _state.ui.operations.file_task.is_some()
    }
    #[cfg(target_arch = "wasm32")]
    {
        false
    }
}

pub(crate) fn cancel_file_operations(state: &mut FileDialogState, replace_filesystem: bool) {
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(task) = &state.ui.operations.file_task {
        task.cancel();
        if replace_filesystem {
            state.ui.operations.discard_file_result = true;
        } else {
            return;
        }
    }
    cancel_paste_job(state);
    if replace_filesystem {
        state.ui.operations.paste.clipboard = None;
        state.ui.operations.delete.target_ids.clear();
    }
}

pub(super) fn poll_file_operation(_state: &mut FileDialogState) {
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(task) = _state.ui.operations.file_task.as_ref()
        && let Some(outcome) = task.poll()
    {
        let cancelled = task.is_cancelled();
        _state.ui.operations.file_task = None;
        if std::mem::take(&mut _state.ui.operations.discard_file_result) {
            return;
        }
        match outcome {
            Ok(FileOperation::Delete(outcome)) => {
                finish_delete(_state, outcome, true, cancelled);
            }
            Ok(FileOperation::Paste(outcome)) => {
                if let Err(error) = finish_paste(_state, outcome, cancelled) {
                    _state.ui.runtime.error = Some(error);
                }
            }
            Err(error) => {
                if _state
                    .ui
                    .operations
                    .paste
                    .job
                    .take()
                    .is_some_and(|job| job.clipboard.op == ClipboardOp::Cut)
                {
                    _state.ui.operations.paste.clipboard = None;
                }
                _state.core.invalidate_dir_cache();
                _state.ui.runtime.error = Some(error.to_string());
            }
        }
    }
}

pub(super) fn open_rename_modal_from_selection(state: &mut FileDialogState) {
    if file_operation_in_progress(state) || state.core.selected_len() != 1 {
        return;
    }
    let Some(rename_target_id) = state.core.selected_entry_ids().into_iter().next() else {
        return;
    };
    let Some(rename_to) = state
        .core
        .entry_path_by_id(rename_target_id)
        .and_then(|path| path.file_name())
        .filter(|name| !name.is_empty())
        .map(|name| name.to_string_lossy().into_owned())
    else {
        return;
    };
    state.ui.operations.rename.target_id = Some(rename_target_id);
    state.ui.operations.rename.to = rename_to;
    state.ui.operations.rename.error = None;
    state.ui.operations.rename.open_next = true;
    state.ui.operations.rename.focus_next = true;
}

pub(super) fn open_delete_modal_from_selection(state: &mut FileDialogState) {
    if file_operation_in_progress(state) {
        return;
    }
    let delete_target_ids = state.core.selected_entry_ids();
    if delete_target_ids.is_empty() {
        return;
    }
    state.ui.operations.delete.target_ids = delete_target_ids;
    state.ui.operations.delete.error = None;
    state.ui.operations.delete.open_next = true;
}

pub(super) fn delete_targets(state: &mut FileDialogState, paths: &[PathBuf]) -> bool {
    if file_operation_in_progress(state) {
        state.ui.operations.delete.error = Some("Another file operation is in progress".into());
        return false;
    }
    let recursive = state.ui.operations.delete.recursive;
    let cwd = state.core.cwd.clone();
    let ids = state.ui.operations.delete.target_ids.clone();
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(fs) = state.filesystem.background_file_system() {
        let paths = paths.to_vec();
        return match BackgroundTask::spawn("dear-file-browser-delete", move |cancelled| {
            FileOperation::Delete(run_delete(
                fs.as_ref(),
                cwd,
                ids,
                &paths,
                recursive,
                &|| cancelled.load(Ordering::Relaxed),
            ))
        }) {
            Ok(task) => {
                state.ui.operations.file_task = Some(task);
                true
            }
            Err(error) => {
                state.ui.operations.delete.error = Some(error.to_string());
                false
            }
        };
    }
    let outcome = run_delete(
        state.filesystem.as_file_system(),
        cwd,
        ids,
        paths,
        recursive,
        &|| false,
    );
    finish_delete(state, outcome, false, false)
}

fn run_delete(
    fs: &dyn FileSystem,
    cwd: PathBuf,
    ids: Vec<EntryId>,
    paths: &[PathBuf],
    recursive: bool,
    cancelled: &dyn Fn() -> bool,
) -> DeleteOutcome {
    let mut outcome = DeleteOutcome {
        cwd,
        ids,
        completed: 0,
        error: None,
    };
    for path in paths {
        if cancelled() {
            break;
        }
        if let Err(cause) = delete_entry(fs, path, recursive) {
            outcome.error = Some(format!("Failed to delete {}: {cause}", path.display()));
            break;
        }
        outcome.completed += 1;
    }
    outcome
}

fn finish_delete(
    state: &mut FileDialogState,
    mut outcome: DeleteOutcome,
    reopen_on_error: bool,
    cancelled: bool,
) -> bool {
    state.core.invalidate_dir_cache();
    let same_directory = state.core.cwd == outcome.cwd;
    let selection_unchanged = same_directory && state.core.selected_entry_ids() == outcome.ids;
    outcome.ids.drain(..outcome.completed);
    let delete = &mut state.ui.operations.delete;
    if cancelled || !same_directory {
        delete.target_ids.clear();
        delete.recursive = false;
        if !cancelled {
            state.ui.runtime.error = outcome.error;
        }
        return true;
    }
    if selection_unchanged {
        state
            .core
            .replace_selection_by_ids(outcome.ids.iter().copied());
    }
    delete.target_ids = outcome.ids;
    delete.error = outcome.error;
    if delete.error.is_none() {
        delete.recursive = false;
        true
    } else {
        delete.open_next = reopen_on_error;
        false
    }
}

pub(super) fn clipboard_set_from_selection(state: &mut FileDialogState, op: ClipboardOp) {
    if file_operation_in_progress(state)
        || state.ui.operations.paste.job.is_some()
        || !state.core.has_selection()
    {
        return;
    }

    let sources = state.core.selected_entry_paths();
    if sources.is_empty() {
        return;
    }
    state.ui.operations.paste.clipboard = Some(FileClipboard { op, sources });
}

pub(super) fn start_paste_into_cwd(state: &mut FileDialogState) -> Result<(), String> {
    if file_operation_in_progress(state) || state.ui.operations.paste.job.is_some() {
        return Err("Another file operation is in progress".into());
    }
    let Some(clipboard) = state.ui.operations.paste.clipboard.clone() else {
        return Ok(());
    };
    if clipboard.sources.is_empty() {
        return Ok(());
    }

    state.ui.operations.paste.job = Some(PendingPasteJob {
        clipboard,
        dest_dir: state.core.cwd.clone(),
        next_index: 0,
        created: Vec::new(),
        apply_all_conflicts: None,
        pending_conflict_action: None,
        conflict: None,
    });
    run_paste_job_until_wait_or_done(state)
}

pub(super) fn cancel_paste_job(state: &mut FileDialogState) {
    let paste = &mut state.ui.operations.paste;
    paste.conflict_open_next = false;
    if let Some(mut job) = paste.job.take()
        && job.clipboard.op == ClipboardOp::Cut
        && job.next_index > 0
    {
        // A completed cut may already have a new file at its former name.
        job.clipboard.sources.drain(..job.next_index);
        paste.clipboard = (!job.clipboard.sources.is_empty()).then_some(job.clipboard);
    }
}

fn step_paste_job(
    fs: &dyn FileSystem,
    job: &mut PendingPasteJob,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, String> {
    while job.next_index < job.clipboard.sources.len() {
        if cancelled() {
            return Err("File operation cancelled".into());
        }
        let src = &job.clipboard.sources[job.next_index];
        let name = src
            .file_name()
            .ok_or_else(|| format!("Invalid source path: {}", src.display()))?;

        let mut dest = job.dest_dir.join(name);
        // A one-shot decision belongs only to this item, even if the conflict disappeared.
        let decision = job.pending_conflict_action.take();
        if (&dest == src && job.clipboard.op == ClipboardOp::Cut)
            || matches!(decision, Some(PasteConflictAction::Skip))
        {
            job.next_index += 1;
            continue;
        }

        let action = if &dest == src {
            Some(PasteConflictAction::KeepBoth)
        } else if entry_exists(fs, &dest)
            .map_err(|e| format!("Could not inspect {}: {e}", dest.display()))?
        {
            let Some(action) = decision.or(job.apply_all_conflicts) else {
                job.conflict = Some(PasteConflictPrompt {
                    source: src.clone(),
                    dest,
                    apply_to_all: false,
                });
                return Ok(false);
            };
            Some(action)
        } else {
            None
        };
        if action == Some(PasteConflictAction::Skip) {
            job.next_index += 1;
            continue;
        }
        if action == Some(PasteConflictAction::KeepBoth) {
            dest = keep_both_target(fs, &dest)
                .map_err(|e| format!("Could not choose a name for {}: {e}", dest.display()))?;
        }

        let result = paste_cancellable(
            fs,
            job.clipboard.op,
            src,
            &dest,
            action == Some(PasteConflictAction::Overwrite),
            cancelled,
        );
        if let Err(PasteError::Transfer(e)) = &result {
            return Err(format!("Failed to paste {}: {e}", src.display()));
        }
        job.created.push(EntryId::from_path(&dest));
        job.next_index += 1;
        if let Err(PasteError::AfterInstall(e)) = result {
            return Err(format!("Pasted to {}. {e}", dest.display()));
        }
    }

    Ok(true)
}

pub(super) fn run_paste_job_until_wait_or_done(state: &mut FileDialogState) -> Result<(), String> {
    let Some(job) = state.ui.operations.paste.job.as_ref() else {
        return Ok(());
    };
    if job.conflict.is_some() || file_operation_in_progress(state) {
        return Ok(());
    }

    #[cfg(not(target_arch = "wasm32"))]
    if let Some(fs) = state.filesystem.background_file_system() {
        let job = job.clone();
        return match BackgroundTask::spawn("dear-file-browser-paste", move |cancelled| {
            FileOperation::Paste(run_paste(fs.as_ref(), job, &|| {
                cancelled.load(Ordering::Relaxed)
            }))
        }) {
            Ok(task) => {
                state.ui.operations.file_task = Some(task);
                Ok(())
            }
            Err(error) => {
                cancel_paste_job(state);
                Err(error.to_string())
            }
        };
    }
    let Some(job) = state.ui.operations.paste.job.take() else {
        return Ok(());
    };
    let outcome = run_paste(state.filesystem.as_file_system(), job, &|| false);
    finish_paste(state, outcome, false)
}

fn run_paste(
    fs: &dyn FileSystem,
    mut job: PendingPasteJob,
    cancelled: &dyn Fn() -> bool,
) -> PasteOutcome {
    let previous_count = job.created.len();
    let result = step_paste_job(fs, &mut job, cancelled);
    PasteOutcome {
        job,
        previous_count,
        result,
    }
}

fn finish_paste(
    state: &mut FileDialogState,
    outcome: PasteOutcome,
    cancelled: bool,
) -> Result<(), String> {
    let PasteOutcome {
        job,
        previous_count,
        result: done,
    } = outcome;
    let created = job.created.len() != previous_count;
    if created && !cancelled && state.core.cwd == job.dest_dir {
        state.ui.operations.reveal_id_next = job.created.first().copied();
        state
            .core
            .replace_selection_by_ids(job.created.iter().copied());
    }
    if created || done.is_err() {
        state.core.invalidate_dir_cache();
    }
    if cancelled {
        state.ui.operations.paste.job = Some(job);
        cancel_paste_job(state);
        return Ok(());
    }
    match done {
        Ok(true) => {
            if job.clipboard.op == ClipboardOp::Cut && !job.created.is_empty() {
                state.ui.operations.paste.clipboard = None;
            }
            state.ui.operations.paste.job = None;
        }
        Ok(false) => {
            state.ui.operations.paste.job = Some(job);
            state.ui.operations.paste.conflict_open_next = true;
        }
        Err(e) => {
            state.ui.operations.paste.job = Some(job);
            cancel_paste_job(state);
            return Err(e);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DialogMode;
    use crate::fs::StdFileSystem;
    use crate::fs_ops::tests::{MoveFs, TempDir};
    use std::{fs, path::Path};

    fn start(op: ClipboardOp, sources: Vec<PathBuf>, dir: &Path) -> FileDialogState {
        let mut state = FileDialogState::with_blocking_filesystem(
            DialogMode::OpenFiles,
            Box::new(StdFileSystem),
        );
        state.core.set_cwd(dir.to_owned());
        state.ui.operations.paste.clipboard = Some(FileClipboard { op, sources });
        start_paste_into_cwd(&mut state).unwrap();
        state
    }

    fn resolve(state: &mut FileDialogState, action: PasteConflictAction) {
        let job = state.ui.operations.paste.job.as_mut().unwrap();
        assert!(job.conflict.take().is_some());
        job.pending_conflict_action = Some(action);
        run_paste_job_until_wait_or_done(state).unwrap();
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn background_paste_waits_for_conflict_input_and_respects_later_navigation() {
        use std::time::{Duration, Instant};

        fn poll_until_idle(state: &mut FileDialogState) {
            let deadline = Instant::now() + Duration::from_secs(5);
            while file_operation_in_progress(state) {
                assert!(Instant::now() < deadline, "file operation did not return");
                poll_file_operation(state);
                std::thread::yield_now();
            }
        }

        let temp = TempDir::new();
        let source = temp.path().join("entry");
        let dest_dir = temp.path().join("target");
        let target = dest_dir.join("entry");
        fs::create_dir(&dest_dir).unwrap();
        fs::write(&source, b"source").unwrap();
        fs::write(&target, b"existing").unwrap();
        let mut state = FileDialogState::new(DialogMode::OpenFiles);
        state.core.set_cwd(dest_dir.clone());
        state.ui.operations.paste.clipboard = Some(FileClipboard {
            op: ClipboardOp::Copy,
            sources: vec![source.clone()],
        });
        start_paste_into_cwd(&mut state).unwrap();
        assert!(file_operation_in_progress(&state));
        poll_until_idle(&mut state);
        assert_eq!(fs::read(&target).unwrap(), b"existing");

        state.core.set_cwd(temp.path().to_owned());
        let selected = EntryId::from_path(&source);
        state.core.focus_and_select_by_id(selected);
        resolve(&mut state, PasteConflictAction::Overwrite);
        poll_until_idle(&mut state);
        assert_eq!(fs::read(&target).unwrap(), b"source");
        assert_eq!(state.core.selected_entry_ids(), [selected]);
        assert_eq!(state.ui.operations.reveal_id_next, None);
    }

    #[test]
    fn cancel_after_a_partial_cut_retries_only_the_remaining_sources() {
        let temp = TempDir::new();
        let first = temp.path().join("first");
        let second = temp.path().join("second");
        let dst = temp.path().join("target");
        let retry = temp.path().join("retry");
        fs::create_dir(&dst).unwrap();
        fs::create_dir(&retry).unwrap();
        fs::write(&first, b"first").unwrap();
        fs::write(&second, b"second").unwrap();
        fs::write(dst.join("second"), b"conflict").unwrap();
        let mut state = start(ClipboardOp::Cut, vec![first.clone(), second], &dst);
        assert!(
            state
                .ui
                .operations
                .paste
                .job
                .as_ref()
                .unwrap()
                .conflict
                .is_some()
        );
        cancel_paste_job(&mut state);

        fs::write(&first, b"reused name").unwrap();
        state.core.set_cwd(retry.clone());
        start_paste_into_cwd(&mut state).unwrap();
        assert_eq!(fs::read(first).unwrap(), b"reused name");
        assert_eq!(fs::read(dst.join("first")).unwrap(), b"first");
        assert_eq!(fs::read(dst.join("second")).unwrap(), b"conflict");
        assert_eq!(fs::read(retry.join("second")).unwrap(), b"second");
        assert!(!retry.join("first").exists());
    }

    #[test]
    fn copying_within_the_same_directory_creates_and_selects_a_sibling() {
        let temp = TempDir::new();
        let src = temp.path().join("a.txt");
        let copy = temp.path().join("a (copy).txt");
        fs::write(&src, b"original").unwrap();
        let state = start(ClipboardOp::Copy, vec![src.clone()], temp.path());
        assert_eq!(fs::read(src).unwrap(), b"original");
        assert_eq!(fs::read(&copy).unwrap(), b"original");
        assert_eq!(state.core.selected_entry_ids(), [EntryId::from_path(&copy)]);
    }

    #[test]
    fn partial_delete_retries_only_the_entries_that_remain() {
        let temp = TempDir::new();
        let first = temp.path().join("first");
        let dir = temp.path().join("nonempty");
        fs::write(&first, b"old").unwrap();
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("child"), b"child").unwrap();
        let mut state = FileDialogState::with_blocking_filesystem(
            DialogMode::OpenFiles,
            Box::new(StdFileSystem),
        );
        state.core.set_cwd(temp.path().to_owned());
        state.core.rescan_if_needed(&StdFileSystem);
        state
            .core
            .replace_selection_by_ids([EntryId::from_path(&first), EntryId::from_path(&dir)]);
        open_delete_modal_from_selection(&mut state);
        assert!(!delete_targets(&mut state, &[first.clone(), dir.clone()]));
        assert!(state.ui.operations.delete.error.is_some());
        assert_eq!(
            state.ui.operations.delete.target_ids,
            [EntryId::from_path(&dir)]
        );
        assert_eq!(state.core.selected_entry_ids(), [EntryId::from_path(&dir)]);

        fs::write(&first, b"new").unwrap();
        state.ui.operations.delete.recursive = true;
        let remaining = state.core.selected_entry_paths();
        assert!(delete_targets(&mut state, &remaining));
        assert!(!dir.exists());
        assert_eq!(fs::read(first).unwrap(), b"new");
    }

    #[test]
    fn skipped_and_kept_cut_items_use_the_chosen_destinations() {
        let temp = TempDir::new();
        let src = temp.path().join("source");
        let dst = temp.path().join("target");
        fs::create_dir(&src).unwrap();
        fs::create_dir(&dst).unwrap();
        for name in ["one", "two.txt"] {
            fs::write(src.join(name), b"incoming").unwrap();
            fs::write(dst.join(name), b"previous").unwrap();
        }
        let mut state = start(
            ClipboardOp::Cut,
            vec![src.join("one"), src.join("two.txt")],
            &dst,
        );
        resolve(&mut state, PasteConflictAction::Skip);
        resolve(&mut state, PasteConflictAction::KeepBoth);

        let kept = dst.join("two (copy).txt");
        assert_eq!(fs::read(src.join("one")).unwrap(), b"incoming");
        assert_eq!(fs::read(dst.join("one")).unwrap(), b"previous");
        assert_eq!(fs::read(dst.join("two.txt")).unwrap(), b"previous");
        assert_eq!(fs::read(&kept).unwrap(), b"incoming");
        assert!(!src.join("two.txt").exists());
        assert!(state.ui.operations.paste.clipboard.is_none());
        assert_eq!(state.core.selected_entry_ids(), [EntryId::from_path(&kept)]);
    }

    #[test]
    fn a_disappearing_conflict_does_not_authorize_the_next_overwrite() {
        let temp = TempDir::new();
        let dst = temp.path().join("target");
        fs::create_dir(&dst).unwrap();
        let sources = ["one", "two"].map(|name| {
            let path = temp.path().join(name);
            fs::write(&path, b"incoming").unwrap();
            fs::write(dst.join(name), b"previous").unwrap();
            path
        });
        let mut state = start(ClipboardOp::Copy, sources.to_vec(), &dst);
        fs::remove_file(dst.join("one")).unwrap();
        resolve(&mut state, PasteConflictAction::Overwrite);
        let job = state.ui.operations.paste.job.as_ref().unwrap();
        assert_eq!(job.conflict.as_ref().unwrap().dest, dst.join("two"));
        assert_eq!(fs::read(dst.join("one")).unwrap(), b"incoming");
        assert_eq!(fs::read(dst.join("two")).unwrap(), b"previous");
    }

    #[test]
    fn skipping_a_disappearing_conflict_preserves_the_cut_source() {
        let temp = TempDir::new();
        let src = temp.path().join("note");
        let dir = temp.path().join("target");
        let dst = dir.join("note");
        fs::create_dir(&dir).unwrap();
        fs::write(&src, b"source").unwrap();
        fs::write(&dst, b"previous").unwrap();
        let mut state = start(ClipboardOp::Cut, vec![src.clone()], &dir);
        fs::rename(&dst, dir.join("saved")).unwrap();
        resolve(&mut state, PasteConflictAction::Skip);

        assert_eq!(fs::read(src).unwrap(), b"source");
        assert!(!dst.exists());
        assert_eq!(fs::read(dir.join("saved")).unwrap(), b"previous");
    }

    #[test]
    fn partial_cut_cleanup_keeps_new_files_and_retries_only_unprocessed_items() {
        let temp = TempDir::new();
        let source = temp.path().join("source");
        let pending = temp.path().join("pending");
        let dst = temp.path().join("target");
        fs::create_dir(&source).unwrap();
        fs::create_dir(&dst).unwrap();
        fs::write(source.join("original"), b"original").unwrap();
        fs::write(&pending, b"pending").unwrap();
        let mut fs = MoveFs::cross_device(&source);
        fs.after_listing = |dir| fs::write(dir.join("later"), b"new contents");
        let mut state =
            FileDialogState::with_blocking_filesystem(DialogMode::OpenFiles, Box::new(fs));
        state.core.set_cwd(dst.clone());
        state.ui.operations.paste.clipboard = Some(FileClipboard {
            op: ClipboardOp::Cut,
            sources: vec![source.clone(), pending.clone()],
        });
        assert!(
            start_paste_into_cwd(&mut state)
                .unwrap_err()
                .contains("source cleanup stopped")
        );
        assert_eq!(
            state
                .ui
                .operations
                .paste
                .clipboard
                .as_ref()
                .unwrap()
                .sources,
            [pending]
        );

        start_paste_into_cwd(&mut state).unwrap();
        assert_eq!(fs::read(source.join("later")).unwrap(), b"new contents");
        assert_eq!(fs::read(dst.join("source/original")).unwrap(), b"original");
        assert!(!dst.join("source/later").exists());
        assert_eq!(fs::read(dst.join("pending")).unwrap(), b"pending");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn keep_both_copies_and_selects_native_names_losslessly() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;

        let temp = TempDir::new();
        let name = OsStr::from_bytes(b"a\xff.tar.gz");
        let src = temp.path().join(name);
        let dst = temp.path().join("target");
        fs::create_dir(&src).unwrap();
        fs::create_dir(&dst).unwrap();
        fs::write(dst.join(name), b"previous").unwrap();
        let children = [OsStr::from_bytes(b"x\xff"), OsStr::from_bytes(b"x\xfe")];
        for child in children {
            fs::write(src.join(child), child.as_bytes()).unwrap();
        }

        let mut state = start(ClipboardOp::Copy, vec![src], &dst);
        resolve(&mut state, PasteConflictAction::KeepBoth);
        let kept = dst.join(OsStr::from_bytes(b"a\xff (copy).tar.gz"));
        for child in children {
            assert_eq!(fs::read(kept.join(child)).unwrap(), child.as_bytes());
        }
        assert_eq!(fs::read(dst.join(name)).unwrap(), b"previous");
        assert_eq!(state.core.selected_entry_ids(), [EntryId::from_path(&kept)]);
    }
}
