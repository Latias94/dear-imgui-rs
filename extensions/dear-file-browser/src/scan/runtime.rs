use std::collections::VecDeque;
use std::io;
use std::path::PathBuf;

use crate::dialog_core::ScanGeneration;
use crate::fs::{FileSystem, FsEntry, ScanVisit};

use super::FileSystemCapability;
#[cfg(not(target_arch = "wasm32"))]
use super::session::{BackgroundSession, ScanJob};

#[derive(Debug)]
pub(crate) struct RuntimeBatch {
    pub(crate) generation: ScanGeneration,
    pub(crate) kind: RuntimeBatchKind,
}

#[derive(Debug)]
pub(crate) enum RuntimeBatchKind {
    Begin {
        cwd: PathBuf,
    },
    Entries {
        cwd: PathBuf,
        entries: Vec<FsEntry>,
        loaded: usize,
    },
    Complete {
        loaded: usize,
    },
    Error {
        cwd: PathBuf,
        message: String,
    },
}

/// A scan coordinator with a synchronous fallback and a native worker path.
///
/// Native scans run on a worker owned by this runtime. Each runtime has at most
/// one running and one coalesced pending request, so a filesystem that ignores
/// cancellation cannot cause unbounded request growth or starve another dialog.
#[derive(Debug, Default)]
pub(crate) struct ScanRuntime {
    blocking_batches: VecDeque<RuntimeBatch>,
    #[cfg(not(target_arch = "wasm32"))]
    background_session: Option<BackgroundSession>,
}

impl ScanRuntime {
    pub(crate) fn submit_blocking(
        &mut self,
        generation: ScanGeneration,
        cwd: PathBuf,
        batch_entries: usize,
        filesystem: &dyn FileSystem,
    ) {
        self.cancel_current();

        let mut batches = VecDeque::new();
        batches.push_back(RuntimeBatch {
            generation,
            kind: RuntimeBatchKind::Begin { cwd: cwd.clone() },
        });

        let batch_entries = batch_entries.max(1);
        let mut pending = Vec::with_capacity(batch_entries);
        let mut loaded = 0usize;
        let result = filesystem.visit_dir(&cwd, &mut |entry| {
            pending.push(entry);
            if pending.len() >= batch_entries {
                loaded += pending.len();
                batches.push_back(RuntimeBatch {
                    generation,
                    kind: RuntimeBatchKind::Entries {
                        cwd: cwd.clone(),
                        entries: std::mem::take(&mut pending),
                        loaded,
                    },
                });
            }
            ScanVisit::Continue
        });

        match result {
            Ok(()) => {
                if !pending.is_empty() {
                    loaded += pending.len();
                    batches.push_back(RuntimeBatch {
                        generation,
                        kind: RuntimeBatchKind::Entries {
                            cwd: cwd.clone(),
                            entries: pending,
                            loaded,
                        },
                    });
                }
                batches.push_back(RuntimeBatch {
                    generation,
                    kind: RuntimeBatchKind::Complete { loaded },
                });
            }
            Err(error) => batches.push_back(RuntimeBatch {
                generation,
                kind: RuntimeBatchKind::Error {
                    cwd,
                    message: error.to_string(),
                },
            }),
        }

        self.blocking_batches = batches;
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn submit_background(
        &mut self,
        generation: ScanGeneration,
        cwd: PathBuf,
        batch_entries: usize,
        max_batches_per_tick: usize,
        filesystem: &FileSystemCapability,
    ) -> io::Result<()> {
        self.cancel_current();
        self.blocking_batches.clear();
        let Some(filesystem) = filesystem.background_file_system() else {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "background scanning requires a shared thread-safe filesystem",
            ));
        };
        if self.background_session.is_none() {
            // Keep one channel for the runtime lifetime so policy changes never
            // join an in-flight worker merely to resize its bounded queue.
            let queue_capacity = max_batches_per_tick.saturating_mul(2).clamp(2, 64);
            self.background_session = Some(BackgroundSession::new(queue_capacity)?);
        }
        let session = self
            .background_session
            .as_ref()
            .expect("background session was just created");
        session.submit(ScanJob {
            generation,
            cwd,
            batch_entries: batch_entries.max(1),
            filesystem,
        })
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn submit_background(
        &mut self,
        _generation: ScanGeneration,
        _cwd: PathBuf,
        _batch_entries: usize,
        _max_batches_per_tick: usize,
        _filesystem: &FileSystemCapability,
    ) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "background scanning is unsupported on wasm",
        ))
    }

    pub(crate) fn poll_batch(&mut self) -> Option<RuntimeBatch> {
        if let Some(batch) = self.blocking_batches.pop_front() {
            return Some(batch);
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            self.background_session.as_ref()?.poll_batch()
        }

        #[cfg(target_arch = "wasm32")]
        {
            None
        }
    }

    #[cfg(test)]
    pub(crate) fn push_test_batch(&mut self, batch: RuntimeBatch) {
        self.blocking_batches.push_back(batch);
    }

    pub(crate) fn cancel_current(&mut self) {
        self.blocking_batches.clear();
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(session) = self.background_session.as_ref() {
            session.cancel_current();
        }
    }

    #[cfg(all(test, not(target_arch = "wasm32")))]
    pub(crate) fn background_is_idle(&self) -> bool {
        self.background_session
            .as_ref()
            .is_none_or(BackgroundSession::is_idle)
    }

    #[cfg(all(test, not(target_arch = "wasm32")))]
    pub(crate) fn background_queue_capacity(&self) -> Option<usize> {
        self.background_session
            .as_ref()
            .map(BackgroundSession::queue_capacity)
    }

    #[cfg(all(test, not(target_arch = "wasm32")))]
    pub(super) fn background_generations(
        &self,
    ) -> Option<(Option<ScanGeneration>, Option<ScanGeneration>)> {
        self.background_session
            .as_ref()
            .map(BackgroundSession::generations)
    }

    #[cfg(all(test, not(target_arch = "wasm32")))]
    pub(super) fn background_current_cancel(
        &self,
    ) -> Option<std::sync::Arc<std::sync::atomic::AtomicBool>> {
        self.background_session
            .as_ref()
            .and_then(BackgroundSession::current_cancel)
    }
}

impl Drop for ScanRuntime {
    fn drop(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(mut session) = self.background_session.take() {
            session.close();
        }
    }
}
