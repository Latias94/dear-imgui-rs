use std::collections::HashMap;
use std::sync::Arc;

use crate::render::snapshot::{PendingTextureRequest, SnapshotTextureId, TextureOp};
use crate::texture::TextureRect;

#[derive(Default)]
pub(super) struct TextureSyncQueue {
    entries: HashMap<SnapshotTextureId, PendingTexture>,
    next_revision: u64,
}

struct PendingTexture {
    revision: u64,
    operation: Option<Arc<TextureOp>>,
}

impl TextureSyncQueue {
    pub(super) fn operation(&self, id: SnapshotTextureId) -> Option<&TextureOp> {
        self.entries.get(&id)?.operation.as_deref()
    }

    pub(super) fn stage(&mut self, id: SnapshotTextureId, operation: Arc<TextureOp>) {
        if matches!(operation.as_ref(), TextureOp::Destroy)
            && matches!(self.operation(id), Some(TextureOp::Destroy))
        {
            return;
        }
        self.next_revision = self
            .next_revision
            .checked_add(1)
            .expect("texture revision exhausted");
        self.entries.insert(
            id,
            PendingTexture {
                revision: self.next_revision,
                operation: Some(operation),
            },
        );
    }

    pub(super) fn requests(&self) -> Vec<PendingTextureRequest> {
        self.entries
            .iter()
            .filter_map(|(&texture, pending)| {
                Some(PendingTextureRequest {
                    texture,
                    revision: pending.revision,
                    op: Arc::clone(pending.operation.as_ref()?),
                })
            })
            .collect()
    }

    pub(super) fn acknowledge(&mut self, id: SnapshotTextureId, revision: u64) -> bool {
        let Some(entry) = self.entries.get_mut(&id) else {
            return false;
        };
        if entry.revision != revision {
            return false;
        }
        // A later epoch can confirm the same upload with a newer backend binding.
        // Keep only its receipt identity; the potentially large payload is released.
        entry.operation = None;
        true
    }

    pub(super) fn forget(&mut self, id: SnapshotTextureId) {
        self.entries.remove(&id);
    }

    pub(super) fn clear(&mut self) {
        self.entries.clear();
    }
}

pub(super) fn cumulative_update_rect(
    previous: &TextureOp,
    incoming: &TextureOp,
) -> Option<TextureRect> {
    let TextureOp::Update { rects: old, .. } = previous else {
        return None;
    };
    let TextureOp::Update { rects: new, .. } = incoming else {
        return None;
    };
    let mut rects = old.iter().chain(new).map(|upload| upload.rect);
    let first = rects.next()?;
    let (mut x, mut y, mut right, mut bottom) = (
        first.x,
        first.y,
        u32::from(first.x) + u32::from(first.w),
        u32::from(first.y) + u32::from(first.h),
    );
    for rect in rects {
        x = x.min(rect.x);
        y = y.min(rect.y);
        right = right.max(u32::from(rect.x) + u32::from(rect.w));
        bottom = bottom.max(u32::from(rect.y) + u32::from(rect.h));
    }
    Some(TextureRect {
        x,
        y,
        w: u16::try_from(right - u32::from(x)).ok()?,
        h: u16::try_from(bottom - u32::from(y)).ok()?,
    })
}

pub(super) fn update_pending_create(
    previous: &TextureOp,
    incoming: &TextureOp,
) -> Option<TextureOp> {
    let TextureOp::Create {
        format,
        width,
        height,
        row_pitch,
        pixels,
    } = previous
    else {
        return None;
    };
    let TextureOp::Update { rects, .. } = incoming else {
        return None;
    };
    let mut pixels = pixels.clone();
    let bpp = crate::texture::get_format_bytes_per_pixel(*format);
    for upload in rects {
        for row in 0..usize::from(upload.rect.h) {
            let target =
                (usize::from(upload.rect.y) + row) * row_pitch + usize::from(upload.rect.x) * bpp;
            let source = row * upload.row_pitch;
            pixels[target..target + upload.row_pitch]
                .copy_from_slice(&upload.data[source..source + upload.row_pitch]);
        }
    }
    Some(TextureOp::Create {
        format: *format,
        width: *width,
        height: *height,
        row_pitch: *row_pitch,
        pixels,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::ContextId;
    use crate::render::snapshot::TextureUploadRect;
    use crate::texture::TextureFormat;

    fn id() -> SnapshotTextureId {
        SnapshotTextureId::FontAtlas {
            context: ContextId::allocate().unwrap(),
            stamp: 1,
            generation: 1,
        }
    }

    fn create() -> TextureOp {
        TextureOp::Create {
            format: TextureFormat::Alpha8,
            width: 4,
            height: 1,
            row_pitch: 4,
            pixels: vec![0; 4],
        }
    }

    fn update(x: u16, value: u8) -> TextureOp {
        TextureOp::Update {
            format: TextureFormat::Alpha8,
            width: 4,
            height: 1,
            rects: vec![TextureUploadRect {
                rect: TextureRect {
                    x,
                    y: 0,
                    w: 1,
                    h: 1,
                },
                row_pitch: 1,
                data: vec![value],
            }],
        }
    }

    #[test]
    fn retries_share_payload_and_stale_feedback_cannot_consume_newer_work() {
        let mut queue = TextureSyncQueue::default();
        let id = id();
        queue.stage(id, Arc::new(create()));
        let first = queue.requests().pop().unwrap();
        let retry = queue.requests().pop().unwrap();
        assert!(Arc::ptr_eq(&first.op, &retry.op));
        let merged = update_pending_create(queue.operation(id).unwrap(), &update(2, 7)).unwrap();
        queue.stage(id, Arc::new(merged));
        assert!(!queue.acknowledge(id, first.revision));
        let latest = queue.requests().pop().unwrap();
        assert!(
            matches!(latest.op.as_ref(), TextureOp::Create { pixels, .. } if pixels == &[0, 0, 7, 0])
        );
        assert!(queue.acknowledge(id, latest.revision));
        assert!(queue.requests().is_empty());
        assert!(queue.entries[&id].operation.is_none());
    }

    #[test]
    fn reset_never_reuses_an_upload_identity_and_destroy_is_stable() {
        let mut queue = TextureSyncQueue::default();
        let id = id();
        queue.stage(id, Arc::new(create()));
        let first = queue.requests().pop().unwrap();
        queue.clear();
        queue.stage(id, Arc::new(create()));
        assert_ne!(first.revision, queue.requests()[0].revision);
        queue.stage(id, Arc::new(TextureOp::Destroy));
        let destroy = queue.requests().pop().unwrap();
        queue.stage(id, Arc::new(TextureOp::Destroy));
        assert_eq!(destroy.revision, queue.requests()[0].revision);
        assert!(!queue.acknowledge(id, first.revision));
    }

    #[test]
    fn dirty_update_union_is_bounded_by_one_rectangle() {
        assert_eq!(
            cumulative_update_rect(&update(0, 1), &update(3, 2)),
            Some(TextureRect {
                x: 0,
                y: 0,
                w: 4,
                h: 1
            })
        );
    }
}
