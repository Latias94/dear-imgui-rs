use std::path::{Path, PathBuf};

use crate::dialog_core::ScanGeneration;
use crate::fs::{FileSystem, FsEntry, ScanVisit};

use super::{RuntimeBatch, RuntimeBatchKind};

pub(super) fn produce_scan_batches(
    generation: ScanGeneration,
    cwd: PathBuf,
    batch_entries: usize,
    filesystem: &dyn FileSystem,
    mut emit: impl FnMut(RuntimeBatch) -> bool,
    is_cancelled: impl Fn() -> bool,
) {
    if !emit(RuntimeBatch {
        generation,
        kind: RuntimeBatchKind::Begin { cwd: cwd.clone() },
    }) {
        return;
    }

    let batch_entries = batch_entries.max(1);
    let mut pending = Vec::with_capacity(batch_entries);
    let mut loaded = 0usize;
    let result = filesystem.visit_dir(&cwd, &mut |entry| {
        if is_cancelled() {
            return ScanVisit::Stop;
        }

        pending.push(entry);
        if pending.len() < batch_entries {
            return ScanVisit::Continue;
        }

        loaded += pending.len();
        if emit(entries_batch(
            generation,
            &cwd,
            std::mem::take(&mut pending),
            loaded,
        )) {
            ScanVisit::Continue
        } else {
            ScanVisit::Stop
        }
    });

    if is_cancelled() {
        return;
    }

    match result {
        Ok(()) => {
            if !pending.is_empty() {
                loaded += pending.len();
                if !emit(entries_batch(generation, &cwd, pending, loaded)) {
                    return;
                }
            }
            let _ = emit(RuntimeBatch {
                generation,
                kind: RuntimeBatchKind::Complete { loaded },
            });
        }
        Err(error) => {
            let _ = emit(RuntimeBatch {
                generation,
                kind: RuntimeBatchKind::Error {
                    cwd,
                    message: error.to_string(),
                },
            });
        }
    }
}

fn entries_batch(
    generation: ScanGeneration,
    cwd: &Path,
    entries: Vec<FsEntry>,
    loaded: usize,
) -> RuntimeBatch {
    RuntimeBatch {
        generation,
        kind: RuntimeBatchKind::Entries {
            cwd: cwd.to_path_buf(),
            entries,
            loaded,
        },
    }
}
