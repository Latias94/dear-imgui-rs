use std::io;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::thread;

use super::{DecodedRgbaImage, MAX_PENDING_REQUESTS, ThumbnailProvider, ThumbnailRequest};

const WORKERS: usize = 2;
type DecodeResult = Result<DecodedRgbaImage, String>;

pub(super) enum DecodeOutcome {
    Finished(DecodeResult),
    Abandoned,
}

struct DecodeJob {
    request: ThumbnailRequest,
    reply: mpsc::SyncSender<DecodeResult>,
}

#[derive(Debug)]
pub(super) struct PendingDecode {
    pub(super) request: ThumbnailRequest,
    reply: mpsc::Receiver<DecodeResult>,
    cancelled: Arc<AtomicBool>,
}

impl PendingDecode {
    pub(super) fn poll(&self, decoder: &ThumbnailDecoder) -> Option<DecodeOutcome> {
        if !Arc::ptr_eq(&self.cancelled, &decoder.cancelled) {
            return Some(DecodeOutcome::Abandoned);
        }
        match self.reply.try_recv() {
            Ok(result) => Some(DecodeOutcome::Finished(result)),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => Some(DecodeOutcome::Abandoned),
        }
    }
}

pub(super) enum SubmitError {
    Full(ThumbnailRequest),
    Closed(ThumbnailRequest),
}

/// Bounded native thumbnail decode pool. It never receives a renderer or texture ID.
#[derive(Debug)]
pub struct ThumbnailDecoder {
    requests: mpsc::SyncSender<DecodeJob>,
    cancelled: Arc<AtomicBool>,
}

impl ThumbnailDecoder {
    /// Starts two workers with independent providers and a shared queue of up to 16 requests.
    ///
    /// The factory runs on the caller's thread, once per worker. Dropping the decoder
    /// discards queued work and does not wait for a provider already inside `decode`.
    pub fn spawn<P>(mut make_provider: impl FnMut() -> P) -> io::Result<Self>
    where
        P: ThumbnailProvider + Send + 'static,
    {
        let (requests, receiver) = mpsc::sync_channel::<DecodeJob>(MAX_PENDING_REQUESTS);
        let receiver = Arc::new(Mutex::new(receiver));
        let pool = Self {
            requests,
            cancelled: Arc::new(AtomicBool::new(false)),
        };
        for _ in 0..WORKERS {
            let mut provider = make_provider();
            let receiver = Arc::clone(&receiver);
            let cancelled = Arc::clone(&pool.cancelled);
            thread::Builder::new()
                .name("dear-file-browser-thumbnail".into())
                .spawn(move || {
                    while let Ok(job) = receive_job(&receiver) {
                        if cancelled.load(Ordering::Relaxed) {
                            break;
                        }
                        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            provider.decode(&job.request)
                        }))
                        .unwrap_or_else(|_| Err("thumbnail provider panicked".into()));
                        let _ = job.reply.try_send(result);
                    }
                })?;
        }
        Ok(pool)
    }

    pub(super) fn submit(&self, request: ThumbnailRequest) -> Result<PendingDecode, SubmitError> {
        let (reply, receiver) = mpsc::sync_channel(1);
        match self.requests.try_send(DecodeJob {
            request: request.clone(),
            reply,
        }) {
            Ok(()) => Ok(PendingDecode {
                request,
                reply: receiver,
                cancelled: Arc::clone(&self.cancelled),
            }),
            Err(mpsc::TrySendError::Full(_)) => Err(SubmitError::Full(request)),
            Err(mpsc::TrySendError::Disconnected(_)) => Err(SubmitError::Closed(request)),
        }
    }
}

impl Drop for ThumbnailDecoder {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

fn receive_job(queue: &Mutex<mpsc::Receiver<DecodeJob>>) -> Result<DecodeJob, mpsc::RecvError> {
    queue
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .recv()
}
