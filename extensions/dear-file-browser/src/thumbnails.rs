use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};

use dear_imgui_rs::texture::TextureId;
use lru::LruCache;

#[cfg(not(target_arch = "wasm32"))]
mod worker;
#[cfg(not(target_arch = "wasm32"))]
pub use worker::ThumbnailDecoder;

#[cfg(target_arch = "wasm32")]
/// A native background decoder, unavailable on this target.
#[derive(Debug)]
pub enum ThumbnailDecoder {}

const MAX_PENDING_REQUESTS: usize = 16;

/// Decoded thumbnail image in RGBA8 format.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodedRgbaImage {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// RGBA8 pixel data (`width * height * 4` bytes).
    pub rgba: Vec<u8>,
}

/// Thumbnail decoder/provider.
///
/// Implementations are expected to:
/// - decode files (often images) to RGBA8,
/// - optionally downscale to `req.max_size`,
/// - return errors for unsupported formats.
pub trait ThumbnailProvider {
    /// Decode a thumbnail request into an RGBA8 image.
    fn decode(&mut self, req: &ThumbnailRequest) -> Result<DecodedRgbaImage, String>;
}

/// Thumbnail renderer interface (upload/destroy).
///
/// Implementations own the GPU lifecycle of `TextureId`. Register uploaded textures
/// with the application's ImGui renderer. Removal may be requested while the current
/// frame still references an ID. Queue retirement and retain both the ID mapping and
/// texture until the renderer has consumed that frame and recorded every draw using it;
/// only then unregister the ID. Release GPU resources after the relevant submitted
/// draws complete, or use an API that retains their storage until completion (such as
/// deleting an OpenGL texture after issuing its draws). Retaining only the GPU allocation
/// while unregistering the ID early is insufficient. This cache does not participate in
/// ImGui's managed font-atlas protocol.
pub trait ThumbnailRenderer {
    /// Upload an RGBA8 thumbnail image to the GPU and return a `TextureId`.
    fn upload_rgba8(&mut self, image: &DecodedRgbaImage) -> Result<TextureId, String>;
    /// Request retirement of a previously created `TextureId`.
    ///
    /// Queue the ID; do not unregister it before the current frame is consumed/recorded.
    /// GPU resource release must also respect draw completion as described on this trait.
    fn destroy(&mut self, texture_id: TextureId);
}

/// Convenience wrapper passed to [`ThumbnailCache::maintain`].
pub struct ThumbnailBackend<'a> {
    /// Persistent native decoder. Create it once and reuse it across frames.
    pub decoder: &'a ThumbnailDecoder,
    /// Renderer (upload/destroy).
    pub renderer: &'a mut dyn ThumbnailRenderer,
}

/// Configuration for [`ThumbnailCache`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThumbnailCacheConfig {
    /// Maximum number of cached thumbnails.
    pub max_entries: usize,
    /// Maximum number of new requests issued per frame.
    pub max_new_requests_per_frame: usize,
}

impl Default for ThumbnailCacheConfig {
    fn default() -> Self {
        Self {
            max_entries: 256,
            max_new_requests_per_frame: 24,
        }
    }
}

/// A thumbnail request produced by [`ThumbnailCache`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThumbnailRequest {
    /// Full filesystem path to the file.
    pub path: PathBuf,
    /// Maximum thumbnail size in pixels (width, height).
    pub max_size: [u32; 2],
    id: u64,
}

impl ThumbnailRequest {
    /// Creates a standalone decode request, not associated with a cache.
    pub fn new(path: PathBuf, max_size: [u32; 2]) -> Self {
        Self {
            path,
            max_size,
            id: 0,
        }
    }
}

#[derive(Debug)]
enum ThumbnailState {
    Ready(TextureId),
    Failed,
}

/// Monotonic per-frame token used by [`ThumbnailCache`] bookkeeping.
///
/// This is a semantic frame identity, not a duration or filesystem entry count.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ThumbnailFrameIndex(u64);

impl ThumbnailFrameIndex {
    /// Creates a thumbnail frame token from a raw counter value.
    #[cfg(test)]
    #[inline]
    const fn new(value: u64) -> Self {
        Self(value)
    }

    #[inline]
    const fn zero() -> Self {
        Self(0)
    }

    #[inline]
    fn next_wrapping(self) -> Self {
        Self(self.0.wrapping_add(1))
    }
}

/// An in-memory thumbnail request queue + LRU cache.
///
/// This type is renderer-agnostic: the application is expected to:
/// 1) call [`advance_frame`](Self::advance_frame) once per UI frame,
/// 2) drive visibility by calling [`request_visible`](Self::request_visible) for entries that are
///    currently visible, including cache hits,
/// 3) drain requests via [`take_requests`](Self::take_requests), decode/upload thumbnails in user
///    code, then call [`fulfill_request`](Self::fulfill_request), or call
///    [`maintain`](Self::maintain) to use the native decoder,
/// 4) queue evicted IDs from [`take_pending_destroys`](Self::take_pending_destroys) for
///    retirement according to the [`ThumbnailRenderer`] lifetime contract.
///
/// Completed entries, including failures, occupy one bounded LRU. The queue and in-flight
/// requests share a separate limit of 16 per cache. A failure is retried only once evicted
/// or explicitly invalidated.
#[derive(Debug)]
pub struct ThumbnailCache {
    config: ThumbnailCacheConfig,
    frame_index: ThumbnailFrameIndex,
    issued_this_frame: usize,
    next_request_id: u64,
    entries: LruCache<PathBuf, ThumbnailState>,
    pending: HashMap<PathBuf, u64>,
    requests: VecDeque<ThumbnailRequest>,
    pending_destroys: Vec<TextureId>,
    #[cfg(not(target_arch = "wasm32"))]
    decoding: Vec<worker::PendingDecode>,
}

/// Snapshot of thumbnail cache state, useful for UI indicators (e.g. "generation progress").
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ThumbnailStats {
    /// Total number of cached entries and pending requests.
    pub total: usize,
    /// Number of entries queued (requested but not yet decoded/uploaded).
    pub queued: usize,
    /// Number of entries currently marked as in-flight.
    pub in_flight: usize,
    /// Number of ready-to-display thumbnails.
    pub ready: usize,
    /// Number of failed thumbnails (decode/upload failure).
    pub failed: usize,
    /// Number of requests waiting in the decode/upload queue.
    pub pending_requests: usize,
    /// New requests issued in the current frame (budgeted by `max_new_requests_per_frame`).
    pub issued_this_frame: usize,
    /// Per-frame request budget.
    pub max_new_requests_per_frame: usize,
}

impl Default for ThumbnailCache {
    fn default() -> Self {
        Self::new(ThumbnailCacheConfig::default())
    }
}

impl ThumbnailCache {
    /// Create a new cache with the given config.
    pub fn new(config: ThumbnailCacheConfig) -> Self {
        Self {
            config,
            frame_index: ThumbnailFrameIndex::zero(),
            issued_this_frame: 0,
            next_request_id: 1,
            entries: LruCache::unbounded(),
            pending: HashMap::new(),
            requests: VecDeque::new(),
            pending_destroys: Vec::new(),
            #[cfg(not(target_arch = "wasm32"))]
            decoding: Vec::new(),
        }
    }

    /// Returns the current configuration.
    pub fn config(&self) -> ThumbnailCacheConfig {
        self.config
    }

    /// Updates the configuration, queuing excess textures for retirement immediately.
    pub fn set_config(&mut self, config: ThumbnailCacheConfig) {
        self.config = config;
        while self.entries.len() > config.max_entries {
            self.evict_lru();
        }
    }

    /// Advance per-frame bookkeeping.
    ///
    /// Call this once per UI frame before issuing visibility requests.
    pub fn advance_frame(&mut self) {
        self.frame_index = self.frame_index.next_wrapping();
        self.issued_this_frame = 0;
    }

    /// Returns the internal frame counter.
    pub fn frame_index(&self) -> ThumbnailFrameIndex {
        self.frame_index
    }

    /// Request a thumbnail for a visible file.
    ///
    /// If the thumbnail is not already cached, a request may be queued depending on the per-frame
    /// request budget.
    pub fn request_visible(&mut self, path: &Path, max_size: [u32; 2]) {
        if self.entries.get(path).is_some() || self.pending.contains_key(path) {
            return;
        }

        if self.config.max_entries == 0
            || self.pending.len() >= MAX_PENDING_REQUESTS
            || self.issued_this_frame >= self.config.max_new_requests_per_frame
        {
            return;
        }
        self.issued_this_frame += 1;

        let id = self.next_request_id;
        self.next_request_id = id.wrapping_add(1).max(1);
        self.pending.insert(path.to_owned(), id);
        self.requests.push_back(ThumbnailRequest {
            path: path.to_owned(),
            max_size,
            id,
        });
    }

    /// Returns the cached texture id for a path, if available.
    pub fn texture_id(&self, path: &Path) -> Option<TextureId> {
        match self.entries.peek(path) {
            Some(ThumbnailState::Ready(texture_id)) => Some(*texture_id),
            _ => None,
        }
    }

    /// Drain queued thumbnail requests.
    ///
    /// Drained requests remain in flight until [`fulfill_request`](Self::fulfill_request) is
    /// called, including on failure. Their slots are not reused until completion.
    pub fn take_requests(&mut self) -> Vec<ThumbnailRequest> {
        self.requests.drain(..).collect()
    }

    /// Completes a request issued by this cache with a texture ID or a failure.
    ///
    /// Queues evicted or unused textures for [`Self::take_pending_destroys`].
    pub fn fulfill_request(&mut self, req: &ThumbnailRequest, result: Result<TextureId, String>) {
        if self.pending.get(req.path.as_path()) != Some(&req.id) {
            if let Ok(id) = result
                && self.texture_id(&req.path) != Some(id)
            {
                self.pending_destroys.push(id);
            }
            return;
        }
        self.pending.remove(req.path.as_path());
        let state = match result {
            Ok(texture_id) => ThumbnailState::Ready(texture_id),
            Err(_message) => ThumbnailState::Failed,
        };
        if self.config.max_entries == 0 {
            if let ThumbnailState::Ready(id) = state {
                self.pending_destroys.push(id);
            }
            return;
        }
        if self.entries.len() == self.config.max_entries {
            self.evict_lru();
        }
        self.entries.put(req.path.clone(), state);
    }

    /// Removes a completed entry, allowing that path to be requested again.
    ///
    /// An already in-flight decode is not cancelled. Retire removed textures using
    /// [`Self::take_pending_destroys`].
    pub fn invalidate(&mut self, path: &Path) {
        if let Some(ThumbnailState::Ready(id)) = self.entries.pop(path) {
            self.pending_destroys.push(id);
        }
    }

    /// Process queued requests and request retirement of evicted textures.
    ///
    /// This is a convenience helper for applications that want `dear-file-browser` to drive the
    /// request lifecycle:
    /// - Submits queued requests to the bounded native decoder without waiting,
    /// - Uploads completed images and fulfills the cache on the caller's thread, and
    /// - Requests retirement of evicted/replaced textures via [`ThumbnailRenderer::destroy`].
    ///
    /// Requests abandoned by a stopped or replaced decoder can be retried.
    /// Decoder shutdown does not cache a thumbnail failure.
    ///
    /// The renderer must queue these requests until the current frame is consumed/recorded
    /// before unregistering IDs, and retain GPU storage for all relevant draws.
    ///
    /// If you prefer to manage decoding/upload externally, you can instead use
    /// [`take_requests`](Self::take_requests), [`fulfill_request`](Self::fulfill_request), and
    /// [`take_pending_destroys`](Self::take_pending_destroys).
    pub fn maintain(&mut self, backend: &mut ThumbnailBackend<'_>) {
        #[cfg(not(target_arch = "wasm32"))]
        self.poll_decoder(backend);

        for tex in self.take_pending_destroys() {
            backend.renderer.destroy(tex);
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn poll_decoder(&mut self, backend: &mut ThumbnailBackend<'_>) {
        let mut index = 0;
        while index < self.decoding.len() {
            let Some(outcome) = self.decoding[index].poll(backend.decoder) else {
                index += 1;
                continue;
            };
            let decode = self.decoding.swap_remove(index);
            match outcome {
                worker::DecodeOutcome::Finished(decoded) => {
                    let uploaded = decoded.and_then(|image| backend.renderer.upload_rgba8(&image));
                    self.fulfill_request(&decode.request, uploaded);
                }
                worker::DecodeOutcome::Abandoned => self.requests.push_back(decode.request),
            }
        }

        while let Some(request) = self.requests.pop_front() {
            match backend.decoder.submit(request) {
                Ok(pending) => self.decoding.push(pending),
                Err(worker::SubmitError::Full(request)) => {
                    self.requests.push_front(request);
                    break;
                }
                Err(worker::SubmitError::Closed(request)) => {
                    self.pending.remove(&request.path);
                }
            }
        }
    }

    /// Drain texture IDs queued for retirement after eviction or replacement.
    ///
    /// Keep renderer ID mappings until the current frame is consumed/recorded; then
    /// unregister IDs and release GPU storage according to the [`ThumbnailRenderer`]
    /// lifetime contract. Draining this queue does not itself make immediate release safe.
    pub fn take_pending_destroys(&mut self) -> Vec<TextureId> {
        std::mem::take(&mut self.pending_destroys)
    }

    /// Returns a snapshot of the cache state for UI display.
    pub fn stats(&self) -> ThumbnailStats {
        let mut stats = ThumbnailStats {
            total: self.entries.len() + self.pending.len(),
            queued: self.requests.len(),
            in_flight: self.pending.len() - self.requests.len(),
            pending_requests: self.requests.len(),
            issued_this_frame: self.issued_this_frame,
            max_new_requests_per_frame: self.config.max_new_requests_per_frame,
            ..ThumbnailStats::default()
        };

        for (_, entry) in self.entries.iter() {
            match entry {
                ThumbnailState::Ready(_) => stats.ready += 1,
                ThumbnailState::Failed => stats.failed += 1,
            }
        }

        stats
    }

    fn evict_lru(&mut self) {
        if let Some((_, ThumbnailState::Ready(id))) = self.entries.pop_lru() {
            self.pending_destroys.push(id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(target_arch = "wasm32"))]
    #[derive(Default)]
    struct DummyRenderer {
        next: u64,
    }

    #[cfg(not(target_arch = "wasm32"))]
    impl ThumbnailRenderer for DummyRenderer {
        fn upload_rgba8(&mut self, _image: &DecodedRgbaImage) -> Result<TextureId, String> {
            self.next += 1;
            Ok(TextureId::new(self.next))
        }

        fn destroy(&mut self, _texture_id: TextureId) {}
    }

    #[test]
    fn respects_request_budget_per_frame() {
        let mut c = ThumbnailCache::new(ThumbnailCacheConfig {
            max_entries: 16,
            max_new_requests_per_frame: 2,
        });
        c.advance_frame();
        c.request_visible(Path::new("/a.png"), [64, 64]);
        c.request_visible(Path::new("/b.png"), [64, 64]);
        c.request_visible(Path::new("/c.png"), [64, 64]);
        let reqs = c.take_requests();
        assert_eq!(reqs.len(), 2);
    }

    #[test]
    fn thumbnail_frame_index_is_typed_and_advances() {
        let mut c = ThumbnailCache::new(ThumbnailCacheConfig::default());

        assert_eq!(c.frame_index(), ThumbnailFrameIndex::new(0));

        c.advance_frame();

        assert_eq!(c.frame_index(), ThumbnailFrameIndex::new(1));
    }

    #[test]
    fn evicts_lru_and_collects_pending_destroys() {
        let mut cache = ThumbnailCache::new(ThumbnailCacheConfig {
            max_entries: 2,
            max_new_requests_per_frame: 10,
        });
        let first = Path::new("/first.png");
        let second = Path::new("/second.png");
        let third = Path::new("/third.png");
        for (path, id) in [(first, 1), (second, 2)] {
            cache.request_visible(path, [64, 64]);
            let request = cache.take_requests().pop().unwrap();
            cache.fulfill_request(&request, Ok(TextureId::new(id)));
        }
        for _ in 0..1_000 {
            cache.request_visible(first, [64, 64]);
        }
        cache.request_visible(third, [64, 64]);
        let request = cache.take_requests().pop().unwrap();
        cache.fulfill_request(&request, Ok(TextureId::new(3)));

        assert_eq!(cache.take_pending_destroys(), [TextureId::new(2)]);
        assert_eq!(cache.texture_id(first), Some(TextureId::new(1)));
        assert_eq!(cache.texture_id(second), None);
        assert_eq!(cache.stats().ready, 2);
    }

    #[test]
    fn negative_cache_and_in_flight_requests_are_bounded() {
        let mut cache = ThumbnailCache::default();
        let unsupported = Path::new("/unsupported.txt");
        cache.request_visible(unsupported, [64, 64]);
        let request = cache.take_requests().pop().unwrap();
        cache.fulfill_request(&request, Err("unsupported".into()));

        for frame in 0..4 {
            cache.advance_frame();
            cache.request_visible(unsupported, [64, 64]);
            for entry in 0..100 {
                cache.request_visible(Path::new(&format!("/{frame}-{entry}.png")), [64, 64]);
            }
            let queued = cache.take_requests();
            assert_eq!(
                queued.len(),
                if frame == 0 { MAX_PENDING_REQUESTS } else { 0 }
            );
            assert!(queued.iter().all(|item| item.path != unsupported));
            assert_eq!(cache.stats().in_flight, MAX_PENDING_REQUESTS);
            assert_eq!(cache.stats().failed, 1);
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn maintain_only_uploads_on_the_calling_thread_after_decode_finishes() {
        use std::sync::{Arc, Mutex, mpsc};
        use std::thread;
        use std::time::{Duration, Instant};

        struct GatedProvider {
            started: mpsc::Sender<thread::ThreadId>,
            release: Arc<Mutex<mpsc::Receiver<()>>>,
        }

        impl ThumbnailProvider for GatedProvider {
            fn decode(&mut self, _req: &ThumbnailRequest) -> Result<DecodedRgbaImage, String> {
                self.started.send(thread::current().id()).unwrap();
                self.release
                    .lock()
                    .unwrap()
                    .recv_timeout(Duration::from_secs(5))
                    .unwrap();
                Ok(DecodedRgbaImage {
                    width: 1,
                    height: 1,
                    rgba: vec![255; 4],
                })
            }
        }

        let (started, started_rx) = mpsc::channel();
        let (release, release_rx) = mpsc::channel();
        let release_rx = Arc::new(Mutex::new(release_rx));
        let decoder = ThumbnailDecoder::spawn(|| GatedProvider {
            started: started.clone(),
            release: Arc::clone(&release_rx),
        })
        .unwrap();
        let mut cache = ThumbnailCache::default();
        let mut renderer = DummyRenderer::default();
        let mut backend = ThumbnailBackend {
            decoder: &decoder,
            renderer: &mut renderer,
        };

        let path = Path::new("/a.png");
        cache.request_visible(path, [64, 64]);
        cache.maintain(&mut backend);
        let worker = started_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_ne!(worker, thread::current().id());
        assert_eq!(cache.stats().in_flight, 1);
        assert_eq!(cache.texture_id(path), None);
        release.send(()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while cache.texture_id(path).is_none() {
            assert!(
                Instant::now() < deadline,
                "decoder did not return a thumbnail"
            );
            cache.maintain(&mut backend);
            thread::yield_now();
        }
        assert_eq!(renderer.next, 1);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn replacing_a_decoder_resubmits_in_flight_work_without_caching_failure() {
        use std::sync::{Arc, Mutex, mpsc};
        use std::thread;
        use std::time::{Duration, Instant};

        struct Provider(Option<Arc<Mutex<mpsc::Receiver<()>>>>);

        impl ThumbnailProvider for Provider {
            fn decode(&mut self, _request: &ThumbnailRequest) -> Result<DecodedRgbaImage, String> {
                if let Some(gate) = &self.0 {
                    let _ = gate.lock().unwrap().recv();
                    return Err("old decoder".into());
                }
                Ok(DecodedRgbaImage {
                    width: 1,
                    height: 1,
                    rgba: vec![255; 4],
                })
            }
        }

        let replacement = ThumbnailDecoder::spawn(|| Provider(None)).unwrap();
        for keep_old_alive in [false, true] {
            let (release, gate) = mpsc::channel();
            let gate = Arc::new(Mutex::new(gate));
            let previous = ThumbnailDecoder::spawn(|| Provider(Some(Arc::clone(&gate)))).unwrap();
            let mut cache = ThumbnailCache::default();
            let mut renderer = DummyRenderer::default();
            let path = Path::new("/pending.png");
            cache.request_visible(path, [64, 64]);
            cache.maintain(&mut ThumbnailBackend {
                decoder: &previous,
                renderer: &mut renderer,
            });
            assert_eq!(cache.stats().in_flight, 1);
            let previous = keep_old_alive.then_some(previous);
            let mut backend = ThumbnailBackend {
                decoder: &replacement,
                renderer: &mut renderer,
            };
            let deadline = Instant::now() + Duration::from_secs(5);
            while cache.texture_id(path).is_none() {
                cache.maintain(&mut backend);
                assert_eq!(cache.stats().failed, 0);
                assert!(
                    Instant::now() < deadline,
                    "replacement did not get the request"
                );
                thread::yield_now();
            }
            assert_eq!(cache.stats().in_flight, 0);
            drop(release);
            drop(previous);
        }
    }
}
