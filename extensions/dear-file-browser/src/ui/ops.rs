use std::path::PathBuf;

use crate::dialog_core::EntryId;
use crate::dialog_state::{
    ClipboardOp, FileClipboard, FileDialogState, PasteConflictAction, PasteConflictPrompt,
    PendingPasteJob,
};
use crate::fs::FileSystem;
use crate::fs_ops::{PasteError, delete_entry, entry_exists, keep_both_target, paste};

pub(super) fn selected_entry_paths_from_ids(state: &FileDialogState) -> Vec<PathBuf> {
    state.core.selected_entry_paths()
}

pub(super) fn selected_entry_counts_from_ids(state: &FileDialogState) -> (usize, usize) {
    state.core.selected_entry_counts()
}

pub(super) fn open_rename_modal_from_selection(state: &mut FileDialogState) {
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
    let delete_target_ids = state.core.selected_entry_ids();
    if delete_target_ids.is_empty() {
        return;
    }
    state.ui.operations.delete.target_ids = delete_target_ids;
    state.ui.operations.delete.error = None;
    state.ui.operations.delete.open_next = true;
}

pub(super) fn delete_targets(state: &mut FileDialogState, paths: &[PathBuf]) -> bool {
    let delete = &mut state.ui.operations.delete;
    let fs = state.filesystem.as_file_system();
    state.core.invalidate_dir_cache();
    for (index, path) in paths.iter().enumerate() {
        if let Err(cause) = delete_entry(fs, path, delete.recursive) {
            delete.target_ids.drain(..index);
            state
                .core
                .replace_selection_by_ids(delete.target_ids.iter().copied());
            delete.error = Some(format!("Failed to delete {}: {cause}", path.display()));
            return false;
        }
    }
    state.core.clear_selection();
    delete.target_ids.clear();
    delete.error = None;
    delete.recursive = false;
    true
}

pub(super) fn clipboard_set_from_selection(state: &mut FileDialogState, op: ClipboardOp) {
    if !state.core.has_selection() {
        return;
    }

    let sources = selected_entry_paths_from_ids(state);
    if sources.is_empty() {
        return;
    }
    state.ui.operations.paste.clipboard = Some(FileClipboard { op, sources });
}

pub(super) fn start_paste_into_cwd(state: &mut FileDialogState) -> Result<(), String> {
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

fn step_paste_job(fs: &dyn FileSystem, job: &mut PendingPasteJob) -> Result<bool, String> {
    while job.next_index < job.clipboard.sources.len() {
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

        let result = paste(
            fs,
            job.clipboard.op,
            src,
            &dest,
            action == Some(PasteConflictAction::Overwrite),
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
    let Some(job) = state.ui.operations.paste.job.as_mut() else {
        return Ok(());
    };
    if job.conflict.is_some() {
        return Ok(());
    }

    let previous_count = job.created.len();
    let done = step_paste_job(state.filesystem.as_file_system(), job);
    let created = job.created.len() != previous_count;
    if created {
        state.ui.operations.reveal_id_next = job.created.first().copied();
        state
            .core
            .replace_selection_by_ids(job.created.iter().copied());
    }
    if created || done.is_err() {
        state.core.invalidate_dir_cache();
    }
    match done {
        Ok(true) => {
            if job.clipboard.op == ClipboardOp::Cut && !job.created.is_empty() {
                state.ui.operations.paste.clipboard = None;
            }
            state.ui.operations.paste.job = None;
        }
        Ok(false) => state.ui.operations.paste.conflict_open_next = true,
        Err(e) => {
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
