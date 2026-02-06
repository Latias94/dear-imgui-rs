use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};

use dear_imgui_rs::texture::TextureId;

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
    Queued {
        max_size: [u32; 2],
    },
    InFlight {
        max_size: [u32; 2],
    },
    Ready {
        texture_id: TextureId,
        max_size: [u32; 2],
    },
    Failed {
        message: String,
    },
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
                state: ThumbnailState::Queued { max_size },
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
            ThumbnailState::Ready { texture_id, .. } => Some(*texture_id),
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
                if let ThumbnailState::Queued { max_size } = entry.state {
                    entry.state = ThumbnailState::InFlight { max_size };
                }
            }
            out.push(req);
        }
        out
    }

    /// Complete a request with either a ready texture id or an error string.
    ///
    /// Returns any evicted texture ids that should be destroyed by the renderer.
    pub fn fulfill(&mut self, path: &Path, result: Result<TextureId, String>, max_size: [u32; 2]) {
        let key = path.to_path_buf();
        let stamp = self.alloc_stamp();
        let state = match result {
            Ok(texture_id) => ThumbnailState::Ready {
                texture_id,
                max_size,
            },
            Err(message) => ThumbnailState::Failed { message },
        };

        if let Some(old) = self.entries.insert(
            key.clone(),
            ThumbnailEntry {
                state,
                lru_stamp: stamp,
            },
        ) {
            if let ThumbnailState::Ready { texture_id, .. } = old.state {
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
                if let ThumbnailState::Ready { texture_id, .. } = removed.state {
                    self.pending_destroys.push(texture_id);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
