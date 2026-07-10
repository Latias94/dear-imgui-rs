use std::collections::VecDeque;
use std::io;
use std::path::PathBuf;

#[cfg(not(target_arch = "wasm32"))]
use std::sync::{Arc, Mutex, OnceLock};

use crate::dialog_core::ScanGeneration;
use crate::fs::{FileSystem, FsEntry, ScanVisit};

/// State-owned filesystem capability.
///
/// A blocking filesystem never crosses a thread boundary. Native background
/// scans require the stronger `Arc + Send + Sync` capability explicitly.
pub(crate) enum FileSystemCapability {
    Blocking(Box<dyn FileSystem>),
    #[cfg(not(target_arch = "wasm32"))]
    Background(Arc<dyn FileSystem + Send + Sync>),
}

impl std::fmt::Debug for FileSystemCapability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Blocking(_) => f.write_str("FileSystemCapability::Blocking(..)"),
            #[cfg(not(target_arch = "wasm32"))]
            Self::Background(_) => f.write_str("FileSystemCapability::Background(..)"),
        }
    }
}

impl FileSystemCapability {
    pub(crate) fn blocking(filesystem: Box<dyn FileSystem>) -> Self {
        Self::Blocking(filesystem)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn background(filesystem: Arc<dyn FileSystem + Send + Sync>) -> Self {
        Self::Background(filesystem)
    }

    pub(crate) fn as_file_system(&self) -> &dyn FileSystem {
        match self {
            Self::Blocking(filesystem) => filesystem.as_ref(),
            #[cfg(not(target_arch = "wasm32"))]
            Self::Background(filesystem) => filesystem.as_ref(),
        }
    }

    pub(crate) fn supports_background(&self) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        {
            matches!(self, Self::Background(_))
        }

        #[cfg(target_arch = "wasm32")]
        {
            false
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn background_file_system(&self) -> Option<Arc<dyn FileSystem + Send + Sync>> {
        match self {
            Self::Background(filesystem) => Some(Arc::clone(filesystem)),
            Self::Blocking(_) => None,
        }
    }
}

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
/// Native scans run on a fixed process-wide executor. Each runtime has at most
/// one running and one coalesced pending request, so a filesystem that ignores
/// cancellation cannot cause unbounded thread or request growth.
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

        let mut pending = Vec::with_capacity(batch_entries.max(1));
        let mut loaded = 0usize;
        let result = filesystem.visit_dir(&cwd, &mut |entry| {
            pending.push(entry);
            if pending.len() >= batch_entries.max(1) {
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
        let queue_capacity = max_batches_per_tick.saturating_mul(2).clamp(2, 64);
        let session = self
            .background_session
            .get_or_insert_with(|| BackgroundSession::new(queue_capacity));
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
            let session = self.background_session.as_mut()?;
            match session.receiver.try_recv() {
                Ok(batch) => Some(batch),
                Err(std::sync::mpsc::TryRecvError::Empty) => None,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => None,
            }
        }

        #[cfg(target_arch = "wasm32")]
        {
            None
        }
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
        self.background_session.as_ref().is_none_or(|session| {
            let inner = session
                .state
                .inner
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            !inner.scheduled && inner.current.is_none() && inner.pending.is_none()
        })
    }
}

impl Drop for ScanRuntime {
    fn drop(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(session) = self.background_session.as_ref() {
            session.close();
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
struct BackgroundSession {
    state: Arc<ScanSessionState>,
    receiver: std::sync::mpsc::Receiver<RuntimeBatch>,
}

#[cfg(not(target_arch = "wasm32"))]
impl std::fmt::Debug for BackgroundSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BackgroundSession")
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl BackgroundSession {
    fn new(queue_capacity: usize) -> Self {
        let (sender, receiver) = std::sync::mpsc::sync_channel(queue_capacity);
        Self {
            state: Arc::new(ScanSessionState {
                sender,
                inner: Mutex::new(ScanSessionInner::default()),
            }),
            receiver,
        }
    }

    fn submit(&self, job: ScanJob) -> io::Result<()> {
        if !self.state.coalesce(job) {
            return Ok(());
        }

        if let Err(error) = background_executor()?.schedule(Arc::clone(&self.state)) {
            self.state.rollback_schedule();
            return Err(error);
        }
        Ok(())
    }

    fn cancel_current(&self) {
        self.state.cancel_current();
    }

    fn close(&self) {
        self.state.close();
    }
}

#[cfg(not(target_arch = "wasm32"))]
struct ScanJob {
    generation: ScanGeneration,
    cwd: PathBuf,
    batch_entries: usize,
    filesystem: Arc<dyn FileSystem + Send + Sync>,
}

#[cfg(not(target_arch = "wasm32"))]
impl std::fmt::Debug for ScanJob {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScanJob")
            .field("generation", &self.generation)
            .field("cwd", &self.cwd)
            .field("batch_entries", &self.batch_entries)
            .finish_non_exhaustive()
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug)]
struct CurrentScan {
    generation: ScanGeneration,
    cancel: Arc<std::sync::atomic::AtomicBool>,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Default)]
struct ScanSessionInner {
    closed: bool,
    scheduled: bool,
    current: Option<CurrentScan>,
    pending: Option<ScanJob>,
}

#[cfg(not(target_arch = "wasm32"))]
struct ScanSessionState {
    sender: std::sync::mpsc::SyncSender<RuntimeBatch>,
    inner: Mutex<ScanSessionInner>,
}

#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(not(target_arch = "wasm32"))]
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

    fn take_next(&self) -> Option<(ScanJob, Arc<std::sync::atomic::AtomicBool>)> {
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

    fn finish(&self, generation: ScanGeneration, cancel: &Arc<std::sync::atomic::AtomicBool>) {
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
}

#[cfg(not(target_arch = "wasm32"))]
const BACKGROUND_WORKER_COUNT: usize = 4;
#[cfg(not(target_arch = "wasm32"))]
const BACKGROUND_SESSION_QUEUE_CAPACITY: usize = 64;

#[cfg(not(target_arch = "wasm32"))]
struct BackgroundExecutor {
    sender: std::sync::mpsc::SyncSender<Arc<ScanSessionState>>,
    _workers: Vec<std::thread::JoinHandle<()>>,
}

#[cfg(not(target_arch = "wasm32"))]
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

    fn schedule(&self, session: Arc<ScanSessionState>) -> io::Result<()> {
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

#[cfg(not(target_arch = "wasm32"))]
fn background_executor() -> io::Result<&'static BackgroundExecutor> {
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
    Ok(EXECUTOR
        .get()
        .expect("background executor was just initialized"))
}

#[cfg(not(target_arch = "wasm32"))]
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
            run_scan_job(job, session.sender.clone(), Arc::clone(&cancel));
            session.finish(generation, &cancel);
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(not(target_arch = "wasm32"))]
fn is_cancelled(cancel: &std::sync::atomic::AtomicBool) -> bool {
    cancel.load(std::sync::atomic::Ordering::Acquire)
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Condvar, Mutex, mpsc};
    use std::thread::ThreadId;
    use std::time::{Duration, Instant};

    use super::*;
    use crate::fs::{FsMetadata, ScanVisit};

    struct ControlledFileSystem {
        entries: Vec<FsEntry>,
        started: mpsc::Sender<ThreadId>,
        release: (Mutex<bool>, Condvar),
        fail_after_release: bool,
        panic_after_release: bool,
        finished: AtomicBool,
        stop_observed: AtomicBool,
    }

    impl ControlledFileSystem {
        fn new(entries: Vec<FsEntry>, started: mpsc::Sender<ThreadId>, released: bool) -> Self {
            Self {
                entries,
                started,
                release: (Mutex::new(released), Condvar::new()),
                fail_after_release: false,
                panic_after_release: false,
                finished: AtomicBool::new(false),
                stop_observed: AtomicBool::new(false),
            }
        }

        fn failing_after_release(entries: Vec<FsEntry>, started: mpsc::Sender<ThreadId>) -> Self {
            Self {
                fail_after_release: true,
                ..Self::new(entries, started, false)
            }
        }

        fn panicking(entries: Vec<FsEntry>, started: mpsc::Sender<ThreadId>) -> Self {
            Self {
                panic_after_release: true,
                ..Self::new(entries, started, true)
            }
        }

        fn release(&self) {
            let (released, wake) = &self.release;
            *released.lock().expect("release mutex poisoned") = true;
            wake.notify_all();
        }
    }

    impl FileSystem for ControlledFileSystem {
        fn visit_dir(
            &self,
            _dir: &Path,
            visit: &mut dyn FnMut(FsEntry) -> ScanVisit,
        ) -> io::Result<()> {
            let _ = self.started.send(std::thread::current().id());
            let (released, wake) = &self.release;
            let mut released = released.lock().expect("release mutex poisoned");
            while !*released {
                released = wake.wait(released).expect("release mutex poisoned");
            }
            drop(released);

            assert!(!self.panic_after_release, "controlled filesystem panic");
            if self.fail_after_release {
                self.finished.store(true, Ordering::Release);
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "controlled scan failed",
                ));
            }

            for entry in self.entries.iter().cloned() {
                if matches!(visit(entry), ScanVisit::Stop) {
                    self.stop_observed.store(true, Ordering::Release);
                    break;
                }
            }
            self.finished.store(true, Ordering::Release);
            Ok(())
        }

        fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
            Ok(path.to_path_buf())
        }

        fn metadata(&self, _path: &Path) -> io::Result<FsMetadata> {
            Err(io::Error::new(io::ErrorKind::Unsupported, "test-only"))
        }

        fn create_dir(&self, _path: &Path) -> io::Result<()> {
            Err(io::Error::new(io::ErrorKind::Unsupported, "test-only"))
        }

        fn rename(&self, _from: &Path, _to: &Path) -> io::Result<()> {
            Err(io::Error::new(io::ErrorKind::Unsupported, "test-only"))
        }

        fn remove_file(&self, _path: &Path) -> io::Result<()> {
            Err(io::Error::new(io::ErrorKind::Unsupported, "test-only"))
        }

        fn remove_dir(&self, _path: &Path) -> io::Result<()> {
            Err(io::Error::new(io::ErrorKind::Unsupported, "test-only"))
        }

        fn remove_dir_all(&self, _path: &Path) -> io::Result<()> {
            Err(io::Error::new(io::ErrorKind::Unsupported, "test-only"))
        }

        fn copy_file(&self, _from: &Path, _to: &Path) -> io::Result<u64> {
            Err(io::Error::new(io::ErrorKind::Unsupported, "test-only"))
        }
    }

    fn entries(count: usize) -> Vec<FsEntry> {
        (0..count)
            .map(|index| FsEntry {
                name: format!("file-{index}.txt"),
                path: PathBuf::from(format!("/tmp/file-{index}.txt")),
                is_dir: false,
                is_symlink: false,
                size: Some(index as u64),
                modified: None,
            })
            .collect()
    }

    fn collect_until_complete(runtime: &mut ScanRuntime) -> (usize, usize) {
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut entry_batches = 0usize;
        let mut entries = 0usize;
        loop {
            if let Some(batch) = runtime.poll_batch() {
                match batch.kind {
                    RuntimeBatchKind::Entries { entries: batch, .. } => {
                        entry_batches += 1;
                        entries += batch.len();
                    }
                    RuntimeBatchKind::Complete { loaded } => {
                        assert_eq!(loaded, entries);
                        return (entry_batches, entries);
                    }
                    RuntimeBatchKind::Error { message, .. } => {
                        panic!("unexpected worker failure: {message}")
                    }
                    RuntimeBatchKind::Begin { .. } => {}
                }
            }
            assert!(Instant::now() < deadline, "worker did not complete");
            std::thread::yield_now();
        }
    }

    #[test]
    fn background_submit_returns_before_filesystem_unblocks_and_streams_batches() {
        let (started_tx, started_rx) = mpsc::channel();
        let filesystem = Arc::new(ControlledFileSystem::new(entries(5), started_tx, false));
        let capability = FileSystemCapability::background(filesystem.clone());
        let mut runtime = ScanRuntime::default();

        let (returned_tx, returned_rx) = mpsc::channel();
        let (timely_tx, timely_rx) = mpsc::channel();
        let watchdog_filesystem = filesystem.clone();
        let watchdog = std::thread::spawn(move || {
            let returned = returned_rx.recv_timeout(Duration::from_secs(1)).is_ok();
            watchdog_filesystem.release();
            timely_tx.send(returned).expect("test receiver disappeared");
        });

        runtime
            .submit_background(
                ScanGeneration::new(1),
                PathBuf::from("/tmp"),
                2,
                1,
                &capability,
            )
            .expect("worker should start");
        returned_tx.send(()).expect("watchdog disappeared");

        assert!(timely_rx.recv().expect("watchdog result missing"));
        let worker_thread = started_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("filesystem was not visited");
        assert_ne!(worker_thread, std::thread::current().id());

        let (batch_count, loaded) = collect_until_complete(&mut runtime);
        assert_eq!(batch_count, 3);
        assert_eq!(loaded, 5);
        assert!(filesystem.finished.load(Ordering::Acquire));
        watchdog.join().expect("watchdog panicked");
    }

    #[test]
    fn superseding_scan_cancels_old_worker_without_waiting_for_it() {
        let (old_started_tx, old_started_rx) = mpsc::channel();
        let old_filesystem = Arc::new(ControlledFileSystem::failing_after_release(
            entries(2),
            old_started_tx,
        ));
        let old_capability = FileSystemCapability::background(old_filesystem.clone());

        let (new_started_tx, _new_started_rx) = mpsc::channel();
        let new_filesystem = Arc::new(ControlledFileSystem::new(entries(1), new_started_tx, true));
        let new_capability = FileSystemCapability::background(new_filesystem);
        let mut runtime = ScanRuntime::default();

        runtime
            .submit_background(
                ScanGeneration::new(1),
                PathBuf::from("/old"),
                1,
                1,
                &old_capability,
            )
            .expect("old worker should start");
        old_started_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("old scan did not start");

        let (returned_tx, returned_rx) = mpsc::channel();
        let (timely_tx, timely_rx) = mpsc::channel();
        let release_old = old_filesystem.clone();
        let watchdog = std::thread::spawn(move || {
            let returned = returned_rx.recv_timeout(Duration::from_secs(1)).is_ok();
            release_old.release();
            timely_tx.send(returned).expect("test receiver disappeared");
        });

        runtime
            .submit_background(
                ScanGeneration::new(2),
                PathBuf::from("/new"),
                1,
                1,
                &new_capability,
            )
            .expect("replacement worker should start");
        returned_tx.send(()).expect("watchdog disappeared");
        assert!(timely_rx.recv().expect("watchdog result missing"));
        let (_, loaded) = collect_until_complete(&mut runtime);
        assert_eq!(loaded, 1);

        drop(runtime);
        assert!(old_filesystem.finished.load(Ordering::Acquire));
        watchdog.join().expect("watchdog panicked");
    }

    #[test]
    fn repeated_supersession_coalesces_to_latest_request_without_spawning_more_workers() {
        let (started_tx, started_rx) = mpsc::channel();
        let filesystem = Arc::new(ControlledFileSystem::new(entries(1), started_tx, false));
        let capability = FileSystemCapability::background(filesystem.clone());
        let mut runtime = ScanRuntime::default();

        runtime
            .submit_background(
                ScanGeneration::new(1),
                PathBuf::from("/one"),
                1,
                1,
                &capability,
            )
            .expect("first worker should start");
        let first_thread = started_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("first scan did not start");

        for generation in 2..=4 {
            runtime
                .submit_background(
                    ScanGeneration::new(generation),
                    PathBuf::from(format!("/{generation}")),
                    1,
                    1,
                    &capability,
                )
                .expect("replacement request should be coalesced");
        }

        let session = runtime
            .background_session
            .as_ref()
            .expect("background session disappeared");
        {
            let inner = session.state.inner.lock().expect("session mutex poisoned");
            assert_eq!(
                inner.current.as_ref().map(|scan| scan.generation),
                Some(ScanGeneration::new(1))
            );
            assert_eq!(
                inner.pending.as_ref().map(|job| job.generation),
                Some(ScanGeneration::new(4))
            );
        }
        assert!(matches!(
            started_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));

        filesystem.release();
        let second_thread = started_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("latest scan did not start");
        assert_eq!(second_thread, first_thread);

        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if let Some(RuntimeBatch {
                generation,
                kind: RuntimeBatchKind::Complete { loaded },
            }) = runtime.poll_batch()
            {
                assert_eq!(generation, ScanGeneration::new(4));
                assert_eq!(loaded, 1);
                break;
            }
            assert!(Instant::now() < deadline, "latest scan did not complete");
            std::thread::yield_now();
        }
        assert!(matches!(
            started_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
    }

    #[test]
    fn runtime_drop_is_non_blocking_while_filesystem_is_blocked() {
        let (started_tx, started_rx) = mpsc::channel();
        let filesystem = Arc::new(ControlledFileSystem::new(entries(8), started_tx, false));
        let capability = FileSystemCapability::background(filesystem.clone());
        let mut runtime = ScanRuntime::default();
        runtime
            .submit_background(
                ScanGeneration::new(1),
                PathBuf::from("/tmp"),
                1,
                1,
                &capability,
            )
            .expect("worker should start");
        started_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("scan did not start");

        let cancel = {
            let session = runtime
                .background_session
                .as_ref()
                .expect("background session disappeared");
            let inner = session.state.inner.lock().expect("session mutex poisoned");
            Arc::clone(
                &inner
                    .current
                    .as_ref()
                    .expect("running scan disappeared")
                    .cancel,
            )
        };
        let (dropped_tx, dropped_rx) = mpsc::channel();
        let drop_thread = std::thread::spawn(move || {
            drop(runtime);
            dropped_tx.send(()).expect("drop observer disappeared");
        });
        if dropped_rx.recv_timeout(Duration::from_secs(1)).is_err() {
            filesystem.release();
            let _ = drop_thread.join();
            panic!("runtime drop waited for a blocked filesystem");
        }
        assert!(cancel.load(Ordering::Acquire));

        filesystem.release();
        drop_thread.join().expect("runtime drop panicked");

        let deadline = Instant::now() + Duration::from_secs(1);
        while !filesystem.finished.load(Ordering::Acquire) {
            assert!(
                Instant::now() < deadline,
                "cancelled filesystem visit did not finish"
            );
            std::thread::yield_now();
        }
        assert!(filesystem.finished.load(Ordering::Acquire));
        assert!(filesystem.stop_observed.load(Ordering::Acquire));
    }

    #[test]
    fn worker_panic_becomes_a_terminal_error_batch() {
        let (started_tx, _started_rx) = mpsc::channel();
        let filesystem = Arc::new(ControlledFileSystem::panicking(entries(1), started_tx));
        let capability = FileSystemCapability::background(filesystem);
        let mut runtime = ScanRuntime::default();
        runtime
            .submit_background(
                ScanGeneration::new(1),
                PathBuf::from("/tmp"),
                1,
                1,
                &capability,
            )
            .expect("worker should start");

        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if let Some(RuntimeBatch {
                kind: RuntimeBatchKind::Error { message, .. },
                ..
            }) = runtime.poll_batch()
            {
                assert!(message.contains("panicked"));
                break;
            }
            assert!(
                Instant::now() < deadline,
                "panic error batch was not emitted"
            );
            std::thread::yield_now();
        }
    }
}
