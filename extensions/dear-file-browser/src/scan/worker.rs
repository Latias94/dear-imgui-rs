use std::io;
use std::path::PathBuf;
use std::sync::{Arc, mpsc};
use std::thread;

use crate::dialog_core::ScanGeneration;
use crate::fs::FileSystem;

use super::producer::produce_scan_batches;
use super::session::{ScanJob, ScanSessionState};
use super::{RuntimeBatch, RuntimeBatchKind};

/// One native worker owned by a scan session.
pub(super) struct ScanWorker {
    state: Arc<ScanSessionState>,
    wake_sender: Option<mpsc::SyncSender<()>>,
    handle: Option<thread::JoinHandle<()>>,
}

impl ScanWorker {
    pub(super) fn spawn(state: Arc<ScanSessionState>) -> io::Result<Self> {
        let (wake_sender, wake_receiver) = mpsc::sync_channel(1);
        let worker_state = Arc::clone(&state);
        let handle = thread::Builder::new()
            .name("dear-file-browser-scan".to_owned())
            .spawn(move || worker_loop(worker_state, wake_receiver))?;

        Ok(Self {
            state,
            wake_sender: Some(wake_sender),
            handle: Some(handle),
        })
    }

    pub(super) fn wake(&self) -> io::Result<()> {
        let Some(wake_sender) = self.wake_sender.as_ref() else {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "background scan worker is closed",
            ));
        };

        match wake_sender.try_send(()) {
            Ok(()) | Err(mpsc::TrySendError::Full(())) => Ok(()),
            Err(mpsc::TrySendError::Disconnected(())) => Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "background scan worker is unavailable",
            )),
        }
    }

    pub(super) fn close(&mut self) {
        self.state.close();
        self.wake_sender.take();
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for ScanWorker {
    fn drop(&mut self) {
        self.close();
    }
}

fn worker_loop(state: Arc<ScanSessionState>, wake_receiver: mpsc::Receiver<()>) {
    while wake_receiver.recv().is_ok() {
        while let Some((job, cancel)) = state.take_next() {
            let generation = job.generation;
            run_scan_job(job, state.batch_sender(), Arc::clone(&cancel));
            state.finish(generation, &cancel);
        }
    }
}

fn run_scan_job(
    job: ScanJob,
    sender: mpsc::SyncSender<RuntimeBatch>,
    cancel: Arc<std::sync::atomic::AtomicBool>,
) {
    let ScanJob {
        generation,
        cwd,
        batch_entries,
        filesystem,
    } = job;
    let panic_sender = sender.clone();
    let panic_cancel = Arc::clone(&cancel);
    let panic_cwd = cwd.clone();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        run_worker(generation, cwd, batch_entries, filesystem, sender, cancel);
    }));
    if result.is_err() {
        let _ = send_cancellable(
            &panic_sender,
            &panic_cancel,
            RuntimeBatch {
                generation,
                kind: RuntimeBatchKind::Error {
                    cwd: panic_cwd,
                    message: "filesystem panicked during background scan".to_owned(),
                },
            },
        );
    }
}

fn run_worker(
    generation: ScanGeneration,
    cwd: PathBuf,
    batch_entries: usize,
    filesystem: Arc<dyn FileSystem + Send + Sync>,
    sender: mpsc::SyncSender<RuntimeBatch>,
    cancel: Arc<std::sync::atomic::AtomicBool>,
) {
    produce_scan_batches(
        generation,
        cwd,
        batch_entries,
        filesystem.as_ref(),
        |batch| send_cancellable(&sender, &cancel, batch),
        || is_cancelled(&cancel),
    );
}

fn send_cancellable(
    sender: &mpsc::SyncSender<RuntimeBatch>,
    cancel: &std::sync::atomic::AtomicBool,
    mut batch: RuntimeBatch,
) -> bool {
    loop {
        if is_cancelled(cancel) {
            return false;
        }
        match sender.try_send(batch) {
            Ok(()) => return true,
            Err(mpsc::TrySendError::Disconnected(_)) => return false,
            Err(mpsc::TrySendError::Full(returned)) => {
                batch = returned;
                thread::sleep(std::time::Duration::from_millis(1));
            }
        }
    }
}

fn is_cancelled(cancel: &std::sync::atomic::AtomicBool) -> bool {
    cancel.load(std::sync::atomic::Ordering::Acquire)
}
