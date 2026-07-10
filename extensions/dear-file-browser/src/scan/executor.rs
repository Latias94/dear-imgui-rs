use std::io;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use crate::dialog_core::ScanGeneration;
use crate::fs::{FileSystem, ScanVisit};

use super::session::{ScanJob, ScanSessionState};
use super::{RuntimeBatch, RuntimeBatchKind};

const BACKGROUND_WORKER_COUNT: usize = 4;
const BACKGROUND_SESSION_QUEUE_CAPACITY: usize = 64;

pub(super) struct BackgroundExecutor {
    sender: std::sync::mpsc::SyncSender<Arc<ScanSessionState>>,
    _workers: Vec<std::thread::JoinHandle<()>>,
}

impl BackgroundExecutor {
    fn new() -> io::Result<Self> {
        let (sender, receiver) = std::sync::mpsc::sync_channel(BACKGROUND_SESSION_QUEUE_CAPACITY);
        let receiver = Arc::new(Mutex::new(receiver));
        let mut workers = Vec::with_capacity(BACKGROUND_WORKER_COUNT);
        for index in 0..BACKGROUND_WORKER_COUNT {
            let receiver = Arc::clone(&receiver);
            match std::thread::Builder::new()
                .name(format!("dear-file-browser-scan-{index}"))
                .spawn(move || executor_worker_loop(receiver))
            {
                Ok(worker) => workers.push(worker),
                Err(error) => {
                    drop(sender);
                    for worker in workers {
                        let _ = worker.join();
                    }
                    return Err(error);
                }
            }
        }
        Ok(Self {
            sender,
            _workers: workers,
        })
    }

    pub(super) fn schedule(&self, session: Arc<ScanSessionState>) -> io::Result<()> {
        self.sender.try_send(session).map_err(|error| match error {
            std::sync::mpsc::TrySendError::Full(_) => io::Error::new(
                io::ErrorKind::WouldBlock,
                "background scan executor queue is full",
            ),
            std::sync::mpsc::TrySendError::Disconnected(_) => io::Error::new(
                io::ErrorKind::BrokenPipe,
                "background scan executor is unavailable",
            ),
        })
    }
}

pub(super) fn background_executor() -> io::Result<&'static BackgroundExecutor> {
    static EXECUTOR: OnceLock<BackgroundExecutor> = OnceLock::new();
    static INIT: Mutex<()> = Mutex::new(());

    if let Some(executor) = EXECUTOR.get() {
        return Ok(executor);
    }
    let _guard = INIT.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(executor) = EXECUTOR.get() {
        return Ok(executor);
    }
    let executor = BackgroundExecutor::new()?;
    let _ = EXECUTOR.set(executor);
    EXECUTOR.get().ok_or_else(|| {
        io::Error::other("background executor initialization did not publish an instance")
    })
}

fn executor_worker_loop(receiver: Arc<Mutex<std::sync::mpsc::Receiver<Arc<ScanSessionState>>>>) {
    loop {
        let session = {
            let receiver = receiver
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            match receiver.recv() {
                Ok(session) => session,
                Err(_) => return,
            }
        };

        while let Some((job, cancel)) = session.take_next() {
            let generation = job.generation;
            run_scan_job(job, session.batch_sender(), Arc::clone(&cancel));
            session.finish(generation, &cancel);
        }
    }
}

fn run_scan_job(
    job: ScanJob,
    sender: std::sync::mpsc::SyncSender<RuntimeBatch>,
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
    sender: std::sync::mpsc::SyncSender<RuntimeBatch>,
    cancel: Arc<std::sync::atomic::AtomicBool>,
) {
    if !send_cancellable(
        &sender,
        &cancel,
        RuntimeBatch {
            generation,
            kind: RuntimeBatchKind::Begin { cwd: cwd.clone() },
        },
    ) {
        return;
    }

    let mut pending = Vec::with_capacity(batch_entries);
    let mut loaded = 0usize;
    let result = filesystem.visit_dir(&cwd, &mut |entry| {
        if is_cancelled(&cancel) {
            return ScanVisit::Stop;
        }

        pending.push(entry);
        if pending.len() < batch_entries {
            return ScanVisit::Continue;
        }

        loaded += pending.len();
        if send_cancellable(
            &sender,
            &cancel,
            RuntimeBatch {
                generation,
                kind: RuntimeBatchKind::Entries {
                    cwd: cwd.clone(),
                    entries: std::mem::take(&mut pending),
                    loaded,
                },
            },
        ) {
            ScanVisit::Continue
        } else {
            ScanVisit::Stop
        }
    });

    if is_cancelled(&cancel) {
        return;
    }

    match result {
        Ok(()) => {
            if !pending.is_empty() {
                loaded += pending.len();
                if !send_cancellable(
                    &sender,
                    &cancel,
                    RuntimeBatch {
                        generation,
                        kind: RuntimeBatchKind::Entries {
                            cwd: cwd.clone(),
                            entries: pending,
                            loaded,
                        },
                    },
                ) {
                    return;
                }
            }
            let _ = send_cancellable(
                &sender,
                &cancel,
                RuntimeBatch {
                    generation,
                    kind: RuntimeBatchKind::Complete { loaded },
                },
            );
        }
        Err(error) => {
            let _ = send_cancellable(
                &sender,
                &cancel,
                RuntimeBatch {
                    generation,
                    kind: RuntimeBatchKind::Error {
                        cwd,
                        message: error.to_string(),
                    },
                },
            );
        }
    }
}

fn send_cancellable(
    sender: &std::sync::mpsc::SyncSender<RuntimeBatch>,
    cancel: &std::sync::atomic::AtomicBool,
    mut batch: RuntimeBatch,
) -> bool {
    loop {
        if is_cancelled(cancel) {
            return false;
        }
        match sender.try_send(batch) {
            Ok(()) => return true,
            Err(std::sync::mpsc::TrySendError::Disconnected(_)) => return false,
            Err(std::sync::mpsc::TrySendError::Full(returned)) => {
                batch = returned;
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        }
    }
}

fn is_cancelled(cancel: &std::sync::atomic::AtomicBool) -> bool {
    cancel.load(std::sync::atomic::Ordering::Acquire)
}
