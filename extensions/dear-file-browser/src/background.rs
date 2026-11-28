use std::io;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};

#[derive(Debug)]
pub(crate) struct BackgroundTask<T> {
    cancelled: Arc<AtomicBool>,
    result: mpsc::Receiver<T>,
}

impl<T: Send + 'static> BackgroundTask<T> {
    pub(crate) fn spawn(
        name: &str,
        work: impl FnOnce(&AtomicBool) -> T + Send + 'static,
    ) -> io::Result<Self> {
        let (sender, result) = mpsc::sync_channel(1);
        let cancelled = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&cancelled);
        std::thread::Builder::new()
            .name(name.into())
            .spawn(move || {
                let value = work(&flag);
                let _ = sender.try_send(value);
            })?;
        Ok(Self { cancelled, result })
    }

    pub(crate) fn poll(&self) -> Option<io::Result<T>> {
        match self.result.try_recv() {
            Ok(value) => Some(Ok(value)),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => Some(Err(io::Error::other(
                "background task stopped without a result",
            ))),
        }
    }

    pub(crate) fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    pub(crate) fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }
}

impl<T> Drop for BackgroundTask<T> {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn dropping_a_task_signals_cancellation_without_joining_the_worker() {
        let (start, started) = mpsc::channel();
        let (release, gate) = mpsc::channel();
        let (observe, cancelled) = mpsc::channel();
        let task = BackgroundTask::spawn("test-cancellable-task", move |flag| {
            start.send(()).unwrap();
            gate.recv_timeout(Duration::from_secs(5)).unwrap();
            observe.send(flag.load(Ordering::Relaxed)).unwrap();
        })
        .unwrap();
        started.recv_timeout(Duration::from_secs(5)).unwrap();
        drop(task);
        release.send(()).unwrap();
        assert!(cancelled.recv_timeout(Duration::from_secs(5)).unwrap());
    }
}
