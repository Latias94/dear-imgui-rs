use std::collections::HashMap;

use dear_imgui_rs::{ContextId, SnapshotTextureId, TextureRect};

use crate::BackendError;

/// Renderer resources indexed by the Core's complete managed texture identity.
///
/// A destroy tombstone rejects uploads for that identity until its latest destroy is acknowledged
/// and older requests can no longer execute. Backends keep applying the Core's frame ordering
/// rules after tombstones are pruned. They decide when to transfer resources to a retirement
/// queue, release GPU allocations, and report feedback.
#[derive(Debug)]
pub struct ManagedTextureTable<GpuTex> {
    resources: HashMap<SnapshotTextureId, GpuTex>,
    destroyed: HashMap<SnapshotTextureId, DestroyTombstone>,
}

#[derive(Debug)]
struct DestroyTombstone {
    epoch: u64,
    acknowledged: bool,
}

impl<GpuTex> Default for ManagedTextureTable<GpuTex> {
    fn default() -> Self {
        Self {
            resources: HashMap::new(),
            destroyed: HashMap::new(),
        }
    }
}

impl<GpuTex> ManagedTextureTable<GpuTex> {
    /// Look up a resource. A sealed texture remains visible until the backend removes it.
    #[must_use]
    pub fn get(&self, id: SnapshotTextureId) -> Option<&GpuTex> {
        self.resources.get(&id)
    }

    /// Borrow an existing resource for in-place uploads while the identity is unsealed.
    #[must_use]
    pub fn get_mut(&mut self, id: SnapshotTextureId) -> Option<&mut GpuTex> {
        if self.is_destroyed(id) {
            return None;
        }
        self.resources.get_mut(&id)
    }

    /// Install a resource and return the previous resource, if any.
    ///
    /// Returns the supplied resource in `Err` if the identity is sealed by a destroy request.
    pub fn insert(
        &mut self,
        id: SnapshotTextureId,
        resource: GpuTex,
    ) -> Result<Option<GpuTex>, GpuTex> {
        if self.is_destroyed(id) {
            return Err(resource);
        }
        Ok(self.resources.insert(id, resource))
    }

    /// Take a resource for backend-owned retirement or release. Any tombstone stays in place.
    #[must_use]
    pub fn remove(&mut self, id: SnapshotTextureId) -> Option<GpuTex> {
        self.resources.remove(&id)
    }

    /// Whether a destroy request currently blocks uploads for this exact identity.
    #[must_use]
    pub fn is_destroyed(&self, id: SnapshotTextureId) -> bool {
        self.destroyed.contains_key(&id)
    }

    /// Block uploads as soon as a destroy request is observed.
    ///
    /// An acknowledgement from an older epoch cannot clear a newer request. Re-observing the
    /// same or an older request preserves any acknowledgement already recorded.
    pub fn seal_destroy(&mut self, id: SnapshotTextureId, epoch: u64) {
        self.record_destroy(id, epoch, false);
    }

    /// Record a destroy acknowledgement when the backend is ready to return destroy feedback.
    ///
    /// Backends that can destroy and acknowledge immediately can use this on its own. A later
    /// request may still require an acknowledgement at its own epoch.
    pub fn acknowledge_destroy(&mut self, id: SnapshotTextureId, epoch: u64) {
        self.record_destroy(id, epoch, true);
    }

    fn record_destroy(&mut self, id: SnapshotTextureId, epoch: u64, acknowledged: bool) {
        let tombstone = self.destroyed.entry(id).or_insert(DestroyTombstone {
            epoch,
            acknowledged,
        });
        if epoch > tombstone.epoch || (epoch == tombstone.epoch && acknowledged) {
            *tombstone = DestroyTombstone {
                epoch,
                acknowledged,
            };
        }
    }

    /// Prune acknowledged tombstones through the supplied epoch for each Context.
    ///
    /// Use the Core's contiguous completion watermark or an execution boundary that rejects all
    /// epochs through that sequence before texture processing. Return `None` to keep a Context's
    /// tombstones. A newer destroy request or a pending acknowledgement keeps the identity sealed.
    pub fn prune_tombstones(&mut self, mut through: impl FnMut(ContextId) -> Option<u64>) {
        self.destroyed.retain(|id, tombstone| {
            !tombstone.acknowledged
                || through(texture_context(*id)).is_none_or(|epoch| epoch < tombstone.epoch)
        });
    }

    /// Extract resources owned by the selected Contexts for backend-managed release.
    ///
    /// Consume the iterator to remove all matches. Any unvisited resources and all destroy
    /// tombstones stay in the table. For a single-Context renderer, `|_| true` selects everything.
    pub fn extract_resources<'table>(
        &'table mut self,
        mut include: impl FnMut(ContextId) -> bool + 'table,
    ) -> impl Iterator<Item = (SnapshotTextureId, GpuTex)> + 'table {
        self.resources
            .extract_if(move |id, _| include(texture_context(*id)))
    }

    /// Clear tombstones for Contexts whose renderer texture reset has made old requests stale.
    ///
    /// Use [`Self::prune_tombstones`] during ordinary frame processing.
    pub fn clear_tombstones(&mut self, mut include: impl FnMut(ContextId) -> bool) {
        self.destroyed
            .retain(|id, _| !include(texture_context(*id)));
    }
}

fn texture_context(id: SnapshotTextureId) -> ContextId {
    match id {
        SnapshotTextureId::User(id) => id.context_id(),
        SnapshotTextureId::FontAtlas { context, .. } => context,
    }
}

/// Validates or coalesces Dear ImGui dirty rectangles for one texture extent.
#[derive(Clone, Copy, Debug)]
pub struct DirtyRectNormalizer {
    texture_size: [u32; 2],
}

impl DirtyRectNormalizer {
    /// Use the destination texture's `[width, height]` in pixels.
    #[must_use]
    pub const fn new(texture_size: [u32; 2]) -> Self {
        Self { texture_size }
    }

    /// Validate the geometry of an upload rectangle that already has an associated pixel payload.
    ///
    /// The caller validates its row pitch and bytes using the texture format.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError::InvalidTextureRect`] when the rectangle is empty or out of bounds.
    pub fn validate(&self, rect: TextureRect) -> Result<(), BackendError> {
        let [right, bottom] = rect_end(rect);
        if rect.w == 0
            || rect.h == 0
            || right > self.texture_size[0]
            || bottom > self.texture_size[1]
        {
            return Err(BackendError::InvalidTextureRect {
                rect,
                texture_size: self.texture_size,
            });
        }
        Ok(())
    }

    /// Intersect a dirty rectangle with this texture, skipping empty intersections.
    ///
    /// Use this when selecting pixels from a complete source image.
    #[must_use]
    pub fn clamp(&self, rect: TextureRect) -> Option<TextureRect> {
        let [right, bottom] = rect_end(rect);
        let width = right
            .min(self.texture_size[0])
            .checked_sub(u32::from(rect.x))?;
        let height = bottom
            .min(self.texture_size[1])
            .checked_sub(u32::from(rect.y))?;
        if width == 0 || height == 0 {
            return None;
        }
        Some(TextureRect {
            w: u16::try_from(width).ok()?,
            h: u16::try_from(height).ok()?,
            ..rect
        })
    }

    /// Clamp and merge dirty rectangles for uploads from a complete source image.
    ///
    /// Overlapping rectangles and rectangles that share an edge segment may be combined into
    /// their bounding rectangle, which can include extra pixels. If that rectangle is too large
    /// for [`TextureRect`], both uploads are retained. Output order is unspecified.
    #[must_use]
    pub fn normalize(&self, rects: impl IntoIterator<Item = TextureRect>) -> Vec<TextureRect> {
        let mut normalized = Vec::new();
        for mut rect in rects.into_iter().filter_map(|rect| self.clamp(rect)) {
            let mut index = 0;
            while index < normalized.len() {
                if let Some(merged) = merge_rects(rect, normalized[index]) {
                    rect = merged;
                    normalized.swap_remove(index);
                    index = 0;
                } else {
                    index += 1;
                }
            }
            normalized.push(rect);
        }
        normalized
    }
}

fn rect_end(rect: TextureRect) -> [u32; 2] {
    [
        u32::from(rect.x) + u32::from(rect.w),
        u32::from(rect.y) + u32::from(rect.h),
    ]
}

fn merge_rects(first: TextureRect, second: TextureRect) -> Option<TextureRect> {
    let [first_right, first_bottom] = rect_end(first);
    let [second_right, second_bottom] = rect_end(second);
    let overlap_x = u32::from(first.x.max(second.x));
    let overlap_y = u32::from(first.y.max(second.y));
    let overlap_right = first_right.min(second_right);
    let overlap_bottom = first_bottom.min(second_bottom);
    if overlap_x > overlap_right
        || overlap_y > overlap_bottom
        || (overlap_x == overlap_right && overlap_y == overlap_bottom)
    {
        return None;
    }

    let x = first.x.min(second.x);
    let y = first.y.min(second.y);
    Some(TextureRect {
        x,
        y,
        w: u16::try_from(first_right.max(second_right) - u32::from(x)).ok()?,
        h: u16::try_from(first_bottom.max(second_bottom) - u32::from(y)).ok()?,
    })
}

#[cfg(test)]
mod tests {
    use dear_imgui_rs::{Context, OwnedTextureData, TextureFormat};

    use super::*;

    #[test]
    fn destroy_retries_keep_tombstones_scoped_to_the_exact_texture_and_context() {
        let owner = Context::create();
        let mut other = Context::create();
        let owner_id = owner.id();
        let other_id = other.id();
        let atlas = |generation| SnapshotTextureId::FontAtlas {
            context: owner_id,
            stamp: 1,
            generation,
        };
        let retired = atlas(1);
        let replacement = atlas(2);
        let pixels = OwnedTextureData::from_pixels(TextureFormat::RGBA32, 1, 1, &[0; 4]).unwrap();
        let other_texture = SnapshotTextureId::User(other.register_texture(pixels));
        let mut table = ManagedTextureTable::default();

        assert_eq!(table.insert(retired, 10), Ok(None));
        table.seal_destroy(retired, 8);
        assert_eq!(table.insert(retired, 20), Err(20));
        assert_eq!(table.get(retired), Some(&10));
        assert_eq!(table.get_mut(retired), None);
        assert_eq!(table.remove(retired), Some(10));
        assert_eq!(table.insert(replacement, 30), Ok(None));
        table.acknowledge_destroy(retired, 8);
        table.seal_destroy(retired, 7);
        table.seal_destroy(retired, 10);
        table.acknowledge_destroy(retired, 8);
        table.acknowledge_destroy(other_texture, 3);

        table.prune_tombstones(|context| (context == owner_id).then_some(10));
        assert!(table.is_destroyed(retired));
        assert!(table.is_destroyed(other_texture));
        table.acknowledge_destroy(retired, 11);
        table.seal_destroy(retired, 11);
        table.prune_tombstones(|context| (context == owner_id).then_some(10));
        assert!(table.is_destroyed(retired));
        table.prune_tombstones(|context| (context == owner_id).then_some(11));
        assert!(!table.is_destroyed(retired));
        assert!(table.is_destroyed(other_texture));

        let other_atlas = SnapshotTextureId::FontAtlas {
            context: other_id,
            stamp: 1,
            generation: 2,
        };
        assert_eq!(table.insert(other_atlas, 40), Ok(None));
        table.seal_destroy(replacement, 12);
        let released: Vec<_> = table
            .extract_resources(|context| context == owner_id)
            .collect();
        assert_eq!(released, vec![(replacement, 30)]);
        assert_eq!(table.get(other_atlas), Some(&40));
        assert!(table.is_destroyed(replacement));
        table.clear_tombstones(|context| context == owner_id);
        assert!(!table.is_destroyed(replacement));
        assert!(table.is_destroyed(other_texture));
        table.prune_tombstones(|context| (context == other_id).then_some(3));
        assert!(!table.is_destroyed(other_texture));
    }

    fn rect(x: u16, y: u16, w: u16, h: u16) -> TextureRect {
        TextureRect { x, y, w, h }
    }

    #[test]
    fn strict_validation_preserves_boundaries_and_clamping_checks_wide_endpoints() {
        let max = u16::MAX;
        let texture_size = [u32::from(max) + 2, 10];
        let normalizer = DirtyRectNormalizer::new(texture_size);
        let valid = rect(max, 8, 2, 2);
        assert!(normalizer.validate(valid).is_ok());
        assert_eq!(normalizer.clamp(valid), Some(valid));

        let overhang = rect(max, 8, 5, 4);
        assert!(matches!(
            normalizer.validate(overhang),
            Err(BackendError::InvalidTextureRect { rect, texture_size: size })
                if rect == overhang && size == texture_size
        ));
        assert_eq!(normalizer.clamp(overhang), Some(valid));
        assert!(normalizer.validate(rect(0, 0, 0, 1)).is_err());
        assert_eq!(normalizer.clamp(rect(0, 0, 0, 1)), None);
        assert_eq!(normalizer.clamp(rect(0, 10, 1, 1)), None);
        assert_eq!(normalizer.clamp(rect(0, 11, 1, 1)), None);
    }

    #[test]
    fn clamped_rectangles_merge_transitively_without_merging_corner_contacts() {
        let normalized = DirtyRectNormalizer::new([7, 7]).normalize([
            rect(4, 0, 2, 2),
            rect(0, 0, 2, 5),
            rect(2, 3, 2, 2),
            rect(6, 5, 2, 2),
            rect(7, 7, 1, 1),
        ]);
        assert_eq!(normalized, [rect(0, 0, 6, 5), rect(6, 5, 1, 2)]);
    }

    #[test]
    fn merge_keeps_separate_rectangles_when_the_union_does_not_fit_the_native_type() {
        let max = u16::MAX;
        let rects = [rect(0, 0, max, 1), rect(max - 1, 0, 2, 1)];
        let normalizer = DirtyRectNormalizer::new([u32::from(max) + 1, 1]);
        assert_eq!(normalizer.normalize(rects), rects);
    }
}
