use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};

use dear_imgui_rs::texture::TextureId;

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
/// Implementations own the GPU lifecycle of `TextureId`.
pub trait ThumbnailRenderer {
    /// Upload an RGBA8 thumbnail image to the GPU and return a `TextureId`.
    fn upload_rgba8(&mut self, image: &DecodedRgbaImage) -> Result<TextureId, String>;
    /// Destroy a previously created `TextureId`.
    fn destroy(&mut self, texture_id: TextureId);
}

/// Convenience wrapper passed to [`ThumbnailCache::maintain`].
pub struct ThumbnailBackend<'a> {
    /// Decoder/provider.
    pub provider: &'a mut dyn ThumbnailProvider,
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
}

#[derive(Clone, Debug)]
enum ThumbnailState {
    Queued,
    InFlight,
    Ready { texture_id: TextureId },
    Failed,
}

#[derive(Clone, Debug)]
struct ThumbnailEntry {
    state: ThumbnailState,
    lru_stamp: u64,
}

/// An in-memory thumbnail request queue + LRU cache.
///
/// This type is renderer-agnostic: the application is expected to:
/// 1) call [`advance_frame`](Self::advance_frame) once per UI frame,
/// 2) drive visibility by calling [`request_visible`](Self::request_visible) for entries that are
///    currently visible,
/// 3) drain requests via [`take_requests`](Self::take_requests), decode/upload thumbnails in user
///    code, then call [`fulfill`](Self::fulfill),
/// 4) destroy evicted GPU textures from [`take_pending_destroys`](Self::take_pending_destroys).
#[derive(Clone, Debug)]
pub struct ThumbnailCache {
    /// Cache configuration.
    pub config: ThumbnailCacheConfig,

    frame_index: u64,
    issued_this_frame: usize,
    next_stamp: u64,

    entries: HashMap<PathBuf, ThumbnailEntry>,
    lru: VecDeque<(PathBuf, u64)>,
    requests: VecDeque<ThumbnailRequest>,
    pending_destroys: Vec<TextureId>,
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
            frame_index: 0,
            issued_this_frame: 0,
            next_stamp: 1,
            entries: HashMap::new(),
            lru: VecDeque::new(),
            requests: VecDeque::new(),
            pending_destroys: Vec::new(),
        }
    }

    /// Advance per-frame bookkeeping.
    ///
    /// Call this once per UI frame before issuing visibility requests.
    pub fn advance_frame(&mut self) {
        self.frame_index = self.frame_index.wrapping_add(1);
        self.issued_this_frame = 0;
    }

    /// Returns the internal frame counter.
    pub fn frame_index(&self) -> u64 {
        self.frame_index
    }

    /// Request a thumbnail for a visible file.
    ///
    /// If the thumbnail is not already cached, a request may be queued depending on the per-frame
    /// request budget.
    pub fn request_visible(&mut self, path: &Path, max_size: [u32; 2]) {
        let key = path.to_path_buf();

        if let Some(e) = self.entries.get(&key) {
            // Touch existing entries so they are not evicted.
            self.touch_existing(&key, e.lru_stamp);
            return;
        }

        if self.issued_this_frame >= self.config.max_new_requests_per_frame {
            return;
        }
        self.issued_this_frame += 1;

        let stamp = self.alloc_stamp();
        self.entries.insert(
            key.clone(),
            ThumbnailEntry {
                state: ThumbnailState::Queued,
                lru_stamp: stamp,
            },
        );
        self.lru.push_back((key.clone(), stamp));
        self.requests.push_back(ThumbnailRequest {
            path: key,
            max_size,
        });
        self.evict_to_fit();
    }

    /// Returns the cached texture id for a path, if available.
    pub fn texture_id(&self, path: &Path) -> Option<TextureId> {
        self.entries.get(path).and_then(|e| match &e.state {
            ThumbnailState::Ready { texture_id } => Some(*texture_id),
            _ => None,
        })
    }

    /// Drain queued thumbnail requests.
    ///
    /// Drained requests are marked as "in flight" until [`fulfill`](Self::fulfill) is called.
    pub fn take_requests(&mut self) -> Vec<ThumbnailRequest> {
        let mut out = Vec::new();
        while let Some(req) = self.requests.pop_front() {
            if let Some(entry) = self.entries.get_mut(&req.path) {
                if let ThumbnailState::Queued = entry.state {
                    entry.state = ThumbnailState::InFlight;
                }
            }
            out.push(req);
        }
        out
    }

    /// Complete a request with either a ready texture id or an error string.
    ///
    /// Returns any evicted texture ids that should be destroyed by the renderer.
    pub fn fulfill(&mut self, path: &Path, result: Result<TextureId, String>, _max_size: [u32; 2]) {
        let key = path.to_path_buf();
        let stamp = self.alloc_stamp();
        let state = match result {
            Ok(texture_id) => ThumbnailState::Ready { texture_id },
            Err(_message) => ThumbnailState::Failed,
        };

        if let Some(old) = self.entries.insert(
            key.clone(),
            ThumbnailEntry {
                state,
                lru_stamp: stamp,
            },
        ) {
            if let ThumbnailState::Ready { texture_id } = old.state {
                self.pending_destroys.push(texture_id);
            }
        }
        self.lru.push_back((key, stamp));
        self.evict_to_fit();
    }

    /// Complete a previously issued request.
    pub fn fulfill_request(&mut self, req: &ThumbnailRequest, result: Result<TextureId, String>) {
        self.fulfill(&req.path, result, req.max_size);
    }

    /// Process queued requests and perform pending destroys.
    ///
    /// This is a convenience helper for applications that want `dear-file-browser` to drive the
    /// request lifecycle:
    /// - Decodes queued requests using [`ThumbnailProvider`],
    /// - Uploads them using [`ThumbnailRenderer`],
    /// - Fulfills the cache, and
    /// - Destroys evicted/replaced GPU textures via the renderer.
    ///
    /// If you prefer to manage decoding/upload externally, you can instead use
    /// [`take_requests`](Self::take_requests), [`fulfill_request`](Self::fulfill_request), and
    /// [`take_pending_destroys`](Self::take_pending_destroys).
    pub fn maintain(&mut self, backend: &mut ThumbnailBackend<'_>) {
        let requests = self.take_requests();
        for req in &requests {
            let decoded = backend.provider.decode(req);
            let uploaded = match decoded {
                Ok(img) => backend.renderer.upload_rgba8(&img),
                Err(e) => Err(e),
            };
            self.fulfill_request(req, uploaded);
        }

        let destroys = self.take_pending_destroys();
        for tex in destroys {
            backend.renderer.destroy(tex);
        }
    }

    /// Drain GPU textures that should be destroyed after eviction or replacement.
    pub fn take_pending_destroys(&mut self) -> Vec<TextureId> {
        std::mem::take(&mut self.pending_destroys)
    }

    fn alloc_stamp(&mut self) -> u64 {
        let s = self.next_stamp;
        self.next_stamp = self.next_stamp.wrapping_add(1);
        s
    }

    fn touch_existing(&mut self, key: &PathBuf, old_stamp: u64) {
        let stamp = self.alloc_stamp();
        if let Some(e) = self.entries.get_mut(key) {
            e.lru_stamp = stamp;
        }
        self.lru.push_back((key.clone(), stamp));

        // Avoid unbounded growth if the user constantly hovers a single entry.
        // This is a soft heuristic: clean a little when the queue gets too large.
        if self.lru.len() > self.config.max_entries.saturating_mul(8).max(64) {
            self.compact_lru(old_stamp);
        }
    }

    fn compact_lru(&mut self, _hint_stamp: u64) {
        // Drop stale LRU nodes from the front.
        let target = self.config.max_entries.saturating_mul(4).max(32);
        while self.lru.len() > target {
            let Some((k, s)) = self.lru.pop_front() else {
                break;
            };
            let keep = self.entries.get(&k).is_some_and(|e| e.lru_stamp == s);
            if keep {
                self.lru.push_front((k, s));
                break;
            }
        }
    }

    fn evict_to_fit(&mut self) {
        while self.entries.len() > self.config.max_entries {
            let Some((key, stamp)) = self.lru.pop_front() else {
                break;
            };
            let Some(entry) = self.entries.get(&key) else {
                continue;
            };
            if entry.lru_stamp != stamp {
                continue;
            }
            let removed = self.entries.remove(&key);
            if let Some(removed) = removed {
                if let ThumbnailState::Ready { texture_id } = removed.state {
                    self.pending_destroys.push(texture_id);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct DummyProvider;

    impl ThumbnailProvider for DummyProvider {
        fn decode(&mut self, _req: &ThumbnailRequest) -> Result<DecodedRgbaImage, String> {
            Ok(DecodedRgbaImage {
                width: 1,
                height: 1,
                rgba: vec![255, 0, 0, 255],
            })
        }
    }

    #[derive(Default)]
    struct DummyRenderer {
        next: u64,
        destroyed: Vec<TextureId>,
    }

    impl ThumbnailRenderer for DummyRenderer {
        fn upload_rgba8(&mut self, _image: &DecodedRgbaImage) -> Result<TextureId, String> {
            self.next += 1;
            Ok(TextureId::new(self.next))
        }

        fn destroy(&mut self, texture_id: TextureId) {
            self.destroyed.push(texture_id);
        }
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
    fn evicts_lru_and_collects_pending_destroys() {
        let mut c = ThumbnailCache::new(ThumbnailCacheConfig {
            max_entries: 1,
            max_new_requests_per_frame: 10,
        });
        c.advance_frame();
        c.request_visible(Path::new("/a.png"), [64, 64]);
        c.take_requests();
        c.fulfill(Path::new("/a.png"), Ok(TextureId::new(1)), [64, 64]);

        c.advance_frame();
        c.request_visible(Path::new("/b.png"), [64, 64]);
        c.take_requests();
        c.fulfill(Path::new("/b.png"), Ok(TextureId::new(2)), [64, 64]);

        let destroyed = c.take_pending_destroys();
        assert!(destroyed.contains(&TextureId::new(1)));
        assert!(c.texture_id(Path::new("/a.png")).is_none());
        assert_eq!(c.texture_id(Path::new("/b.png")), Some(TextureId::new(2)));
    }

    #[test]
    fn maintain_decodes_uploads_and_destroys() {
        let mut c = ThumbnailCache::new(ThumbnailCacheConfig {
            max_entries: 1,
            max_new_requests_per_frame: 10,
        });
        let mut provider = DummyProvider::default();
        let mut renderer = DummyRenderer::default();
        let mut backend = ThumbnailBackend {
            provider: &mut provider,
            renderer: &mut renderer,
        };

        c.advance_frame();
        c.request_visible(Path::new("/a.png"), [64, 64]);
        c.maintain(&mut backend);
        assert!(c.texture_id(Path::new("/a.png")).is_some());

        c.advance_frame();
        c.request_visible(Path::new("/b.png"), [64, 64]);
        c.maintain(&mut backend);
        assert!(renderer.destroyed.iter().any(|t| t == &TextureId::new(1)));
    }
}
