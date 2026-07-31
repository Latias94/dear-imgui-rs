use std::collections::HashMap;
use std::hash::Hash;
use std::num::NonZeroU64;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_QUEUE_ID: AtomicU64 = AtomicU64::new(1);

/// Monotonic batch containing managed texture resources waiting for GPU-safe destruction.
///
/// Associate this value with GPU completion that covers every renderer operation which can still
/// reference a texture in the batch. This includes work recorded after the token is returned, such
/// as secondary viewport draws from the same Dear ImGui frame. Pass the completed batch to
/// [`AshRenderer::wait_for_texture_retirements`](super::AshRenderer::wait_for_texture_retirements)
/// before expecting Dear ImGui destroy requests to be acknowledged. A batch can also contain old
/// Vulkan images superseded by copy-on-write managed texture updates. Completing a batch
/// invalidates recorded but unsubmitted command buffers that reference its resources; such
/// command buffers must not be submitted afterwards.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[must_use]
pub struct TextureRetirementBatch {
    queue: NonZeroU64,
    sequence: NonZeroU64,
}

impl TextureRetirementBatch {
    /// Monotonic renderer-local batch sequence.
    pub const fn sequence(self) -> u64 {
        self.sequence.get()
    }
}

#[derive(Debug)]
struct Retiring<T> {
    batch: TextureRetirementBatch,
    value: T,
}

#[derive(Debug)]
pub(super) struct RetirementReservation {
    batch: TextureRetirementBatch,
}

impl RetirementReservation {
    pub(super) const fn batch(&self) -> TextureRetirementBatch {
        self.batch
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RetirementRequest {
    Queued(TextureRetirementBatch),
    Pending,
    Retired,
}

#[derive(Debug)]
pub(super) struct RetirementQueue<K, T> {
    entries: HashMap<K, Retiring<T>>,
    queue_id: Option<NonZeroU64>,
    next_batch: Option<NonZeroU64>,
    last_issued: u64,
}

impl<K, T> RetirementQueue<K, T>
where
    K: Copy + Eq + Hash,
{
    pub(super) fn new() -> Self {
        let queue_id = NEXT_QUEUE_ID
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(1)
            })
            .ok()
            .and_then(NonZeroU64::new);
        Self {
            entries: HashMap::new(),
            queue_id,
            next_batch: NonZeroU64::new(1),
            last_issued: 0,
        }
    }

    #[cfg(test)]
    fn enqueue(&mut self, key: K, value: T) -> Result<TextureRetirementBatch, T> {
        debug_assert!(!self.entries.contains_key(&key));
        if self.entries.contains_key(&key) {
            return Err(value);
        }
        let Some(reservation) = self.reserve() else {
            return Err(value);
        };
        Ok(self.commit(reservation, key, value))
    }

    pub(super) fn reserve(&mut self) -> Option<RetirementReservation> {
        let (Some(queue), Some(sequence)) = (self.queue_id, self.next_batch.take()) else {
            return None;
        };
        let batch = TextureRetirementBatch { queue, sequence };
        self.next_batch = sequence.get().checked_add(1).and_then(NonZeroU64::new);
        self.last_issued = sequence.get();
        Some(RetirementReservation { batch })
    }

    pub(super) fn commit(
        &mut self,
        reservation: RetirementReservation,
        key: K,
        value: T,
    ) -> TextureRetirementBatch {
        assert_eq!(Some(reservation.batch.queue), self.queue_id);
        assert!(reservation.batch.sequence() <= self.last_issued);
        assert!(!self.entries.contains_key(&key));
        self.entries.insert(
            key,
            Retiring {
                batch: reservation.batch,
                value,
            },
        );
        reservation.batch
    }

    #[cfg(test)]
    fn request_retirement(
        &mut self,
        active: &mut HashMap<K, T>,
        key: K,
    ) -> Result<RetirementRequest, ()> {
        if self.contains_key(&key) {
            return Ok(RetirementRequest::Pending);
        }
        let Some(value) = active.remove(&key) else {
            return Ok(RetirementRequest::Retired);
        };
        match self.enqueue(key, value) {
            Ok(batch) => Ok(RetirementRequest::Queued(batch)),
            Err(value) => {
                active.insert(key, value);
                Err(())
            }
        }
    }

    pub(super) fn contains_key(&self, key: &K) -> bool {
        self.entries.contains_key(key)
    }

    pub(super) fn get(&self, key: &K) -> Option<&T> {
        self.entries.get(key).map(|entry| &entry.value)
    }

    pub(super) fn pending_batch(&self) -> Option<TextureRetirementBatch> {
        self.entries.values().map(|entry| entry.batch).max()
    }

    pub(super) fn complete_through(
        &mut self,
        batch: TextureRetirementBatch,
    ) -> Option<Vec<(K, T)>> {
        if Some(batch.queue) != self.queue_id || batch.sequence() > self.last_issued {
            return None;
        }
        let keys = self
            .entries
            .iter()
            .filter_map(|(key, entry)| (entry.batch <= batch).then_some(*key))
            .collect::<Vec<_>>();
        Some(
            keys.into_iter()
                .filter_map(|key| self.entries.remove(&key).map(|entry| (key, entry.value)))
                .collect(),
        )
    }

    pub(super) fn drain(&mut self) -> impl Iterator<Item = (K, T)> + '_ {
        self.entries.drain().map(|(key, entry)| (key, entry.value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retirement_waits_for_the_matching_completion_batch() {
        let mut queue = RetirementQueue::new();
        let first = queue.enqueue(1_u32, "first").unwrap();
        let second = queue.enqueue(2_u32, "second").unwrap();

        assert_eq!(queue.pending_batch(), Some(second));
        assert_eq!(queue.get(&1), Some(&"first"));
        assert_eq!(queue.complete_through(first), Some(vec![(1, "first")]));
        assert!(queue.contains_key(&2));
        assert_eq!(queue.complete_through(second), Some(vec![(2, "second")]));
        assert_eq!(queue.pending_batch(), None);
    }

    #[test]
    fn retirement_rejects_unissued_future_batches() {
        let mut queue = RetirementQueue::new();
        let issued = queue.enqueue(1_u32, "texture").unwrap();
        let future = TextureRetirementBatch {
            queue: issued.queue,
            sequence: NonZeroU64::new(issued.sequence() + 1).unwrap(),
        };

        assert_eq!(queue.complete_through(future), None);
        assert!(queue.contains_key(&1));
    }

    #[test]
    fn repeated_completion_is_idempotent() {
        let mut queue = RetirementQueue::new();
        let batch = queue.enqueue(1_u32, "texture").unwrap();

        assert_eq!(queue.complete_through(batch), Some(vec![(1, "texture")]));
        assert_eq!(queue.complete_through(batch), Some(Vec::new()));
    }

    #[test]
    fn later_completion_may_arrive_before_an_earlier_batch() {
        let mut queue = RetirementQueue::new();
        let first = queue.enqueue(1_u32, "first").unwrap();
        let second = queue.enqueue(2_u32, "second").unwrap();

        let mut completed = queue.complete_through(second).unwrap();
        completed.sort_unstable_by_key(|(key, _)| *key);
        assert_eq!(completed, vec![(1, "first"), (2, "second")]);
        assert_eq!(queue.complete_through(first), Some(Vec::new()));
        assert_eq!(queue.pending_batch(), None);
    }

    #[test]
    fn retirement_rejects_a_batch_from_another_queue() {
        let mut first = RetirementQueue::new();
        let mut second = RetirementQueue::new();
        let first_batch = first.enqueue(1_u32, "first").unwrap();
        let second_batch = second.enqueue(2_u32, "second").unwrap();

        assert_eq!(first.complete_through(second_batch), None);
        assert_eq!(first.get(&1), Some(&"first"));
        assert_eq!(
            first.complete_through(first_batch),
            Some(vec![(1, "first")])
        );
    }

    #[test]
    fn destroy_is_acknowledged_only_after_gpu_retirement_completes() {
        let mut active = HashMap::from([(1_u32, "texture")]);
        let mut queue = RetirementQueue::new();

        let RetirementRequest::Queued(batch) = queue
            .request_retirement(&mut active, 1)
            .expect("first destroy should enter retirement")
        else {
            panic!("first destroy was acknowledged before retirement")
        };
        assert_eq!(
            queue.request_retirement(&mut active, 1),
            Ok(RetirementRequest::Pending)
        );

        assert_eq!(queue.complete_through(batch), Some(vec![(1, "texture")]));
        assert_eq!(
            queue.request_retirement(&mut active, 1),
            Ok(RetirementRequest::Retired)
        );
    }

    #[test]
    fn stale_generation_destroy_does_not_touch_a_reused_slot() {
        let old_key = (7_u32, 1_u32);
        let new_key = (7_u32, 2_u32);
        let mut active = HashMap::from([(old_key, "old")]);
        let mut queue = RetirementQueue::new();

        let RetirementRequest::Queued(old_batch) = queue
            .request_retirement(&mut active, old_key)
            .expect("the old generation should enter retirement")
        else {
            panic!("the old generation was acknowledged before retirement");
        };
        active.insert(new_key, "new");

        assert_eq!(
            queue.complete_through(old_batch),
            Some(vec![(old_key, "old")])
        );
        assert_eq!(
            queue.request_retirement(&mut active, old_key),
            Ok(RetirementRequest::Retired)
        );
        assert_eq!(active.get(&new_key), Some(&"new"));
    }

    #[test]
    fn superseded_texture_retires_before_a_later_active_destroy() {
        let mut queue = RetirementQueue::new();
        let replacement = queue.enqueue((7_u32, "replacement"), "old image").unwrap();
        let destroy = queue.enqueue((7_u32, "destroy"), "active image").unwrap();

        assert_eq!(
            queue.complete_through(replacement),
            Some(vec![((7, "replacement"), "old image")])
        );
        assert_eq!(queue.get(&(7, "destroy")), Some(&"active image"));
        assert_eq!(
            queue.complete_through(destroy),
            Some(vec![((7, "destroy"), "active image")])
        );
    }

    #[test]
    fn abandoned_reservation_does_not_block_later_retirement() {
        let mut queue = RetirementQueue::new();
        let abandoned_sequence = {
            let abandoned = queue.reserve().unwrap();
            abandoned.batch.sequence()
        };

        let batch = queue.enqueue(1_u32, "texture").unwrap();
        assert_eq!(batch.sequence(), abandoned_sequence + 1);
        assert_eq!(queue.pending_batch(), Some(batch));
        assert_eq!(queue.complete_through(batch), Some(vec![(1, "texture")]));
    }

    #[test]
    fn teardown_drain_makes_late_completion_a_noop() {
        let mut queue = RetirementQueue::new();
        let first = queue.enqueue(1_u32, "first").unwrap();
        let second = queue.enqueue(2_u32, "second").unwrap();

        let mut drained = queue.drain().collect::<Vec<_>>();
        drained.sort_unstable_by_key(|(key, _)| *key);
        assert_eq!(drained, vec![(1, "first"), (2, "second")]);
        assert_eq!(queue.pending_batch(), None);
        assert_eq!(queue.complete_through(second), Some(Vec::new()));
        assert_eq!(queue.complete_through(first), Some(Vec::new()));
    }
}
