use std::path::PathBuf;

use crate::dialog_core::EntryId;
use crate::dialog_state::{
    ClipboardOp, FileClipboard, FileDialogState, PasteConflictAction, PasteConflictPrompt,
    PendingPasteJob,
};
use crate::fs::FileSystem;
use crate::fs_ops::{
    ExistingTargetDecision, ExistingTargetPolicy, apply_existing_target_policy, copy_tree,
    move_tree,
};

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

pub(super) fn open_delete_modal_from_selection(state: &mut FileDialogState) {
    let delete_target_ids = state.core.selected_entry_ids();
    if delete_target_ids.is_empty() {
        return;
    }
    state.ui.delete_target_ids = delete_target_ids;
    state.ui.delete_error = None;
    state.ui.delete_open_next = true;
}
pub(super) fn clipboard_set_from_selection(state: &mut FileDialogState, op: ClipboardOp) {
    if !state.core.has_selection() {
        return;
    }

    let sources = selected_entry_paths_from_ids(state);
    if sources.is_empty() {
        return;
    }
    state.ui.clipboard = Some(FileClipboard { op, sources });
}

pub(super) fn start_paste_into_cwd(state: &mut FileDialogState) {
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

pub(super) fn run_paste_job_until_wait_or_done(
    state: &mut FileDialogState,
    fs: &dyn FileSystem,
) -> Result<(), String> {
    if step_paste_job(state, fs)? {
        try_complete_paste_job(state);
    }

    Ok(())
}
