use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::thread::ThreadId;
use std::time::{Duration, Instant};

use super::*;
use crate::dialog_core::ScanGeneration;
use crate::fs::{FileSystem, FsEntry, FsMetadata, ScanVisit};

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

    assert_eq!(
        runtime.background_generations(),
        Some((Some(ScanGeneration::new(1)), Some(ScanGeneration::new(4))))
    );
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

    let cancel = runtime
        .background_current_cancel()
        .expect("running scan disappeared");
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
fn background_queue_capacity_tracks_policy_changes() {
    let (started_tx, started_rx) = mpsc::channel();
    let filesystem = Arc::new(ControlledFileSystem::new(entries(8), started_tx, false));
    let capability = FileSystemCapability::background(filesystem.clone());
    let mut runtime = ScanRuntime::default();

    runtime
        .submit_background(
            ScanGeneration::new(1),
            PathBuf::from("/tmp"),
            1,
            32,
            &capability,
        )
        .expect("first worker should start");
    started_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("first scan did not start");
    assert_eq!(runtime.background_queue_capacity(), Some(64));

    runtime
        .submit_background(
            ScanGeneration::new(2),
            PathBuf::from("/tmp"),
            1,
            1,
            &capability,
        )
        .expect("replacement worker should start");
    assert_eq!(runtime.background_queue_capacity(), Some(2));

    filesystem.release();
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
