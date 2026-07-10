use std::io;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::dialog_core::ScanGeneration;
use crate::fs::FileSystem;

use super::RuntimeBatch;
use super::executor::background_executor;

pub(super) struct BackgroundSession {
    queue_capacity: usize,
    state: Arc<ScanSessionState>,
    receiver: std::sync::mpsc::Receiver<RuntimeBatch>,
}

impl std::fmt::Debug for BackgroundSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BackgroundSession")
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

impl BackgroundSession {
    pub(super) fn new(queue_capacity: usize) -> Self {
        let (sender, receiver) = std::sync::mpsc::sync_channel(queue_capacity);
        Self {
            queue_capacity,
            state: Arc::new(ScanSessionState {
                sender,
                inner: Mutex::new(ScanSessionInner::default()),
            }),
            receiver,
        }
    }

    pub(super) fn submit(&self, job: ScanJob) -> io::Result<()> {
        if !self.state.coalesce(job) {
            return Ok(());
        }

        if let Err(error) = background_executor()?.schedule(Arc::clone(&self.state)) {
            self.state.rollback_schedule();
            return Err(error);
        }
        Ok(())
    }

    pub(super) fn cancel_current(&self) {
        self.state.cancel_current();
    }

    pub(super) fn queue_capacity(&self) -> usize {
        self.queue_capacity
    }

    pub(super) fn close(&self) {
        self.state.close();
    }

    pub(super) fn poll_batch(&self) -> Option<RuntimeBatch> {
        match self.receiver.try_recv() {
            Ok(batch) => Some(batch),
            Err(std::sync::mpsc::TryRecvError::Empty) => None,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => None,
        }
    }

    #[cfg(test)]
    pub(super) fn is_idle(&self) -> bool {
        let inner = self
            .state
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        !inner.scheduled && inner.current.is_none() && inner.pending.is_none()
    }

    #[cfg(test)]
    pub(super) fn generations(&self) -> (Option<ScanGeneration>, Option<ScanGeneration>) {
        let inner = self
            .state
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        (
            inner.current.as_ref().map(|scan| scan.generation),
            inner.pending.as_ref().map(|job| job.generation),
        )
    }

    #[cfg(test)]
    pub(super) fn current_cancel(&self) -> Option<Arc<std::sync::atomic::AtomicBool>> {
        let inner = self
            .state
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        inner.current.as_ref().map(|scan| Arc::clone(&scan.cancel))
    }
}

pub(super) struct ScanJob {
    pub(super) generation: ScanGeneration,
    pub(super) cwd: PathBuf,
    pub(super) batch_entries: usize,
    pub(super) filesystem: Arc<dyn FileSystem + Send + Sync>,
}

impl std::fmt::Debug for ScanJob {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScanJob")
            .field("generation", &self.generation)
            .field("cwd", &self.cwd)
            .field("batch_entries", &self.batch_entries)
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
struct CurrentScan {
    generation: ScanGeneration,
    cancel: Arc<std::sync::atomic::AtomicBool>,
}

#[derive(Debug, Default)]
struct ScanSessionInner {
    closed: bool,
    scheduled: bool,
    current: Option<CurrentScan>,
    pending: Option<ScanJob>,
}

pub(super) struct ScanSessionState {
    sender: std::sync::mpsc::SyncSender<RuntimeBatch>,
    inner: Mutex<ScanSessionInner>,
}

impl std::fmt::Debug for ScanSessionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let inner = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        f.debug_struct("ScanSessionState")
            .field("inner", &*inner)
            .finish_non_exhaustive()
    }
}

impl ScanSessionState {
    /// Replaces any pending request. Returns true when the session must be
    /// inserted into the executor queue.
    fn coalesce(&self, job: ScanJob) -> bool {
        let mut inner = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        debug_assert!(!inner.closed, "cannot submit to a closed scan session");
        if let Some(current) = inner.current.as_ref() {
            current
                .cancel
                .store(true, std::sync::atomic::Ordering::Release);
        }
        inner.pending = Some(job);
        if inner.scheduled {
            false
        } else {
            inner.scheduled = true;
            true
        }
    }

    fn rollback_schedule(&self) {
        let mut inner = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        inner.pending = None;
        inner.scheduled = false;
    }

    fn cancel_current(&self) {
        let mut inner = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(current) = inner.current.as_ref() {
            current
                .cancel
                .store(true, std::sync::atomic::Ordering::Release);
        }
        inner.pending = None;
    }

    fn close(&self) {
        let mut inner = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        inner.closed = true;
        if let Some(current) = inner.current.as_ref() {
            current
                .cancel
                .store(true, std::sync::atomic::Ordering::Release);
        }
        inner.pending = None;
    }

    pub(super) fn take_next(&self) -> Option<(ScanJob, Arc<std::sync::atomic::AtomicBool>)> {
        let mut inner = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if inner.closed {
            inner.pending = None;
            inner.current = None;
            inner.scheduled = false;
            return None;
        }
        let Some(job) = inner.pending.take() else {
            inner.current = None;
            inner.scheduled = false;
            return None;
        };
        let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
        inner.current = Some(CurrentScan {
            generation: job.generation,
            cancel: Arc::clone(&cancel),
        });
        Some((job, cancel))
    }

    pub(super) fn finish(
        &self,
        generation: ScanGeneration,
        cancel: &Arc<std::sync::atomic::AtomicBool>,
    ) {
        let mut inner = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if inner.current.as_ref().is_some_and(|current| {
            current.generation == generation && Arc::ptr_eq(&current.cancel, cancel)
        }) {
            inner.current = None;
        }
    }

    pub(super) fn batch_sender(&self) -> std::sync::mpsc::SyncSender<RuntimeBatch> {
        self.sender.clone()
    }
}
