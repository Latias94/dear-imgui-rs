use std::collections::{BTreeMap, HashSet, VecDeque};
use std::num::NonZeroU64;
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};

#[cfg(feature = "multi-viewport")]
use crate::render::snapshot::capture_platform_io;
use crate::render::snapshot::{
    DetachedRendererConsumer, FrameSnapshot, PendingSnapshot, PendingTextureRequest,
    RendererConsumerCapability, RendererConsumerError, SnapshotCompletionOutcome,
    SnapshotCompletionProgress, SnapshotEpoch, SnapshotError, SnapshotMessage, SnapshotTextureId,
    SynchronousRendererConsumer, TextureFeedback, TextureRequest, TextureRequestKey,
    capture_draw_data, capture_texture_requests_only, finalize_texture_requests,
    validate_texture_feedback,
};

use super::binding::CTX_MUTEX;
use super::texture_registry::{
    FontAtlasSnapshotTarget, FontAtlasTextureTarget, ManagedTextureRegistry,
};
use super::{Context, ContextId};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum ConsumerPhase {
    Unbound,
    Active {
        generation: NonZeroU64,
        mode: ConsumerMode,
    },
    Draining(NonZeroU64),
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum ConsumerMode {
    Synchronous,
    Detached,
}

fn consumer_generation_raw(consumer: &(impl RendererConsumerCapability + ?Sized)) -> NonZeroU64 {
    NonZeroU64::new(consumer.generation())
        .expect("renderer consumer generations are always non-zero")
}

#[derive(Debug)]
struct OutstandingEpoch {
    epoch: SnapshotEpoch,
    expected: HashSet<TextureRequestKey>,
    completion: Option<SnapshotCompletionOutcome>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum SynchronousFrameStatus {
    Pending,
    Reconciled,
    Abandoned,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct SynchronousFrameState {
    native_frame_count: i32,
    epoch: Option<SnapshotEpoch>,
    status: SynchronousFrameStatus,
}

#[derive(Debug)]
pub(super) struct SnapshotHub {
    context: ContextId,
    sender: Sender<SnapshotMessage>,
    receiver: Receiver<SnapshotMessage>,
    phase: ConsumerPhase,
    next_consumer_generation: Option<NonZeroU64>,
    next_epoch: Option<NonZeroU64>,
    completion_watermark: u64,
    outstanding: BTreeMap<u64, OutstandingEpoch>,
    pending_errors: VecDeque<RendererConsumerError>,
    synchronous_frame: Option<SynchronousFrameState>,
}

impl SnapshotHub {
    pub(super) fn new(context: ContextId) -> Self {
        let (sender, receiver) = channel();
        Self {
            context,
            sender,
            receiver,
            phase: ConsumerPhase::Unbound,
            next_consumer_generation: Some(NonZeroU64::MIN),
            next_epoch: Some(NonZeroU64::MIN),
            completion_watermark: 0,
            outstanding: BTreeMap::new(),
            pending_errors: VecDeque::new(),
            synchronous_frame: None,
        }
    }

    pub(super) const fn completion_watermark(&self) -> u64 {
        self.completion_watermark
    }

    pub(super) fn begin_synchronous_native_frame(&mut self, native_frame_count: i32) {
        self.synchronous_frame = Some(SynchronousFrameState {
            native_frame_count,
            epoch: None,
            status: SynchronousFrameStatus::Pending,
        });
    }

    #[cfg(feature = "multi-viewport")]
    pub(super) fn is_synchronous_frame_reconciled(&self, native_frame_count: i32) -> bool {
        self.synchronous_frame.is_some_and(|frame| {
            frame.native_frame_count == native_frame_count
                && frame.epoch.is_some()
                && frame.status == SynchronousFrameStatus::Reconciled
        })
    }

    pub(super) fn validate_consumer_admission(&self) -> Result<NonZeroU64, RendererConsumerError> {
        if let Some(error) = self.pending_errors.front().copied() {
            return Err(error);
        }
        match self.phase {
            ConsumerPhase::Active { .. } => {
                return Err(RendererConsumerError::ConsumerAlreadyActive);
            }
            ConsumerPhase::Draining(_) => return Err(RendererConsumerError::ConsumerDraining),
            ConsumerPhase::Unbound => {}
        }
        self.next_consumer_generation
            .ok_or(RendererConsumerError::ConsumerGenerationExhausted)
    }

    fn commit_consumer_admission(&mut self, generation: NonZeroU64, mode: ConsumerMode) {
        debug_assert_eq!(
            self.validate_consumer_admission(),
            Ok(generation),
            "renderer consumer admission must be validated before it is committed"
        );
        let claimed_generation = self
            .next_consumer_generation
            .take()
            .expect("validated renderer consumer generation must remain available");
        assert_eq!(
            claimed_generation, generation,
            "renderer consumer generation changed after admission validation"
        );
        self.next_consumer_generation = generation.get().checked_add(1).and_then(NonZeroU64::new);
        self.phase = ConsumerPhase::Active { generation, mode };
    }

    pub(super) fn commit_synchronous_consumer_admission(
        &mut self,
        generation: NonZeroU64,
    ) -> SynchronousRendererConsumer {
        self.commit_consumer_admission(generation, ConsumerMode::Synchronous);
        SynchronousRendererConsumer::new(self.context, generation, self.sender.clone())
    }

    pub(super) fn commit_detached_consumer_admission(
        &mut self,
        generation: NonZeroU64,
    ) -> DetachedRendererConsumer {
        self.commit_consumer_admission(generation, ConsumerMode::Detached);
        DetachedRendererConsumer::new(self.context, generation, self.sender.clone())
    }

    pub(super) fn begin_snapshot(
        &mut self,
        consumer: &DetachedRendererConsumer,
        pending: PendingSnapshot,
        registry: &mut ManagedTextureRegistry,
        atlas: &FontAtlasSnapshotTarget,
    ) -> Result<FrameSnapshot, SnapshotError> {
        let generation = self.validate_consumer(consumer, ConsumerMode::Detached)?;
        let sequence = self.allocate_epoch()?;
        let epoch = SnapshotEpoch::new(self.context, generation, sequence);
        let referenced = pending.referenced_user_textures();
        registry.record_snapshot_references(&referenced, sequence.get())?;
        for request in &pending.texture_requests {
            if matches!(request.texture, SnapshotTextureId::FontAtlas { .. }) {
                atlas.record_request_reference(request.texture, sequence.get());
            }
        }
        let (snapshot, expected) = pending.into_frame(epoch, self.sender.clone());
        let previous = self.outstanding.insert(
            sequence.get(),
            OutstandingEpoch {
                epoch,
                expected,
                completion: None,
            },
        );
        debug_assert!(previous.is_none(), "snapshot epoch was allocated twice");
        Ok(snapshot)
    }

    pub(super) fn begin_synchronous(
        &mut self,
        consumer: &SynchronousRendererConsumer,
        pending: Vec<PendingTextureRequest>,
        atlas: &FontAtlasSnapshotTarget,
    ) -> Result<(SnapshotEpoch, Vec<TextureRequest>), RendererConsumerError> {
        let generation = self.validate_consumer(consumer, ConsumerMode::Synchronous)?;
        debug_assert!(self.outstanding.is_empty());
        let sequence = self.allocate_epoch()?;
        let epoch = SnapshotEpoch::new(self.context, generation, sequence);
        if let Some(frame) = self.synchronous_frame.as_mut() {
            frame.epoch = Some(epoch);
        }
        for request in &pending {
            if matches!(request.texture, SnapshotTextureId::FontAtlas { .. }) {
                atlas.record_request_reference(request.texture, sequence.get());
            }
        }
        let (requests, expected) = finalize_texture_requests(pending, epoch);
        self.outstanding.insert(
            sequence.get(),
            OutstandingEpoch {
                epoch,
                expected,
                completion: None,
            },
        );
        Ok((epoch, requests))
    }

    pub(super) fn complete_synchronous(
        &mut self,
        epoch: SnapshotEpoch,
        feedback: Vec<TextureFeedback>,
        registry: &mut ManagedTextureRegistry,
        atlas: FontAtlasSnapshotTarget,
    ) -> Result<SnapshotCompletionProgress, RendererConsumerError> {
        let result = self
            .set_direct_completion(epoch, SnapshotCompletionOutcome::Committed(feedback))
            .and_then(|()| self.advance(registry, &atlas));
        self.finish_synchronous_frame(epoch, result.is_ok());
        result
    }

    pub(super) fn abandon_synchronous(
        &mut self,
        epoch: SnapshotEpoch,
        registry: &mut ManagedTextureRegistry,
        atlas: FontAtlasSnapshotTarget,
    ) {
        if self
            .set_direct_completion(epoch, SnapshotCompletionOutcome::Abandoned)
            .is_ok()
        {
            let _ = self.advance(registry, &atlas);
        }
        self.finish_synchronous_frame(epoch, false);
    }

    fn finish_synchronous_frame(&mut self, epoch: SnapshotEpoch, reconciled: bool) {
        let Some(frame) = self.synchronous_frame.as_mut() else {
            return;
        };
        if frame.epoch != Some(epoch) {
            return;
        }
        frame.status = if reconciled {
            SynchronousFrameStatus::Reconciled
        } else {
            SynchronousFrameStatus::Abandoned
        };
    }

    fn set_direct_completion(
        &mut self,
        epoch: SnapshotEpoch,
        outcome: SnapshotCompletionOutcome,
    ) -> Result<(), RendererConsumerError> {
        let Some(outstanding) = self.outstanding.get_mut(&epoch.sequence()) else {
            return Err(RendererConsumerError::UnknownEpoch {
                epoch: epoch.sequence(),
            });
        };
        if outstanding.epoch != epoch {
            return Err(RendererConsumerError::StaleConsumerGeneration {
                expected: outstanding.epoch.consumer_generation(),
                actual: epoch.consumer_generation(),
            });
        }
        if outstanding.completion.is_some() {
            return Err(RendererConsumerError::EpochAlreadyCompleted {
                epoch: epoch.sequence(),
            });
        }
        if let SnapshotCompletionOutcome::Committed(feedback) = &outcome {
            validate_texture_feedback(epoch, &outstanding.expected, feedback)?;
        }
        outstanding.completion = Some(outcome);
        Ok(())
    }

    fn allocate_epoch(&mut self) -> Result<NonZeroU64, RendererConsumerError> {
        let sequence = self
            .next_epoch
            .take()
            .ok_or(RendererConsumerError::EpochExhausted)?;
        self.next_epoch = sequence.get().checked_add(1).and_then(NonZeroU64::new);
        Ok(sequence)
    }

    fn validate_consumer(
        &mut self,
        consumer: &impl RendererConsumerCapability,
        mode: ConsumerMode,
    ) -> Result<NonZeroU64, RendererConsumerError> {
        if consumer.context_id() != self.context {
            return Err(RendererConsumerError::ForeignContext {
                expected: self.context,
                actual: consumer.context_id(),
            });
        }
        let generation = match self.phase {
            ConsumerPhase::Unbound => Err(RendererConsumerError::NoActiveConsumer),
            ConsumerPhase::Draining(_) => Err(RendererConsumerError::ConsumerDraining),
            ConsumerPhase::Active {
                generation: expected,
                ..
            } if expected != consumer_generation_raw(consumer) => {
                Err(RendererConsumerError::StaleConsumerGeneration {
                    expected: expected.get(),
                    actual: consumer.generation(),
                })
            }
            ConsumerPhase::Active {
                generation,
                mode: active_mode,
            } => {
                debug_assert_eq!(active_mode, mode);
                Ok(generation)
            }
        }?;
        Ok(generation)
    }

    pub(super) fn validate_idle_consumer(
        &self,
        consumer: &impl RendererConsumerCapability,
    ) -> Result<(), RendererConsumerError> {
        if consumer.context_id() != self.context {
            return Err(RendererConsumerError::ForeignContext {
                expected: self.context,
                actual: consumer.context_id(),
            });
        }
        match self.phase {
            ConsumerPhase::Unbound => return Err(RendererConsumerError::NoActiveConsumer),
            ConsumerPhase::Draining(_) => return Err(RendererConsumerError::ConsumerDraining),
            ConsumerPhase::Active { generation, .. }
                if generation != consumer_generation_raw(consumer) =>
            {
                return Err(RendererConsumerError::StaleConsumerGeneration {
                    expected: generation.get(),
                    actual: consumer.generation(),
                });
            }
            ConsumerPhase::Active { .. } => {}
        }
        if !self.outstanding.is_empty() {
            return Err(RendererConsumerError::OutstandingEpochs {
                count: self.outstanding.len(),
            });
        }
        Ok(())
    }

    pub(super) fn poll(
        &mut self,
        registry: &mut ManagedTextureRegistry,
        atlas: &FontAtlasSnapshotTarget,
    ) -> Result<SnapshotCompletionProgress, RendererConsumerError> {
        self.drain_messages();
        self.advance(registry, atlas)
    }

    fn advance(
        &mut self,
        registry: &mut ManagedTextureRegistry,
        atlas: &FontAtlasSnapshotTarget,
    ) -> Result<SnapshotCompletionProgress, RendererConsumerError> {
        let mut progress = SnapshotCompletionProgress {
            watermark: self.completion_watermark,
            ..Default::default()
        };
        let previous_watermark = self.completion_watermark;

        while let Some((&sequence, outstanding)) = self.outstanding.first_key_value() {
            if outstanding.completion.is_none() {
                break;
            }
            let mut outstanding = self
                .outstanding
                .remove(&sequence)
                .expect("first outstanding epoch still exists");
            let outcome = outstanding
                .completion
                .take()
                .expect("completed epoch contains an outcome");
            match outcome {
                SnapshotCompletionOutcome::Committed(feedback) => {
                    match validate_texture_feedback(
                        outstanding.epoch,
                        &outstanding.expected,
                        &feedback,
                    )
                    .and_then(|()| registry.apply_snapshot_feedback(&feedback, atlas, sequence))
                    {
                        Ok(applied) => {
                            progress.committed += 1;
                            progress.feedback_applied += applied;
                        }
                        Err(error) => {
                            self.pending_errors.push_back(error);
                            progress.abandoned += 1;
                        }
                    }
                }
                SnapshotCompletionOutcome::Abandoned => {
                    progress.abandoned += 1;
                }
            }
            self.completion_watermark = sequence;
            progress.watermark = sequence;
        }

        if self.completion_watermark != previous_watermark {
            registry.reap_destroyed(self.completion_watermark);
            atlas.prune_tombstones(self.completion_watermark);
        }
        if matches!(self.phase, ConsumerPhase::Draining(_)) && self.outstanding.is_empty() {
            self.phase = ConsumerPhase::Unbound;
        }
        if let Some(error) = self.pending_errors.pop_front() {
            Err(error)
        } else {
            Ok(progress)
        }
    }

    fn drain_messages(&mut self) {
        loop {
            match self.receiver.try_recv() {
                Ok(SnapshotMessage::Completion(completion)) => {
                    let sequence = completion.epoch.sequence();
                    if completion.epoch.context_id() != self.context {
                        self.pending_errors
                            .push_back(RendererConsumerError::ForeignContext {
                                expected: self.context,
                                actual: completion.epoch.context_id(),
                            });
                        continue;
                    }
                    let Some(outstanding) = self.outstanding.get_mut(&sequence) else {
                        self.pending_errors
                            .push_back(RendererConsumerError::UnknownEpoch { epoch: sequence });
                        continue;
                    };
                    if outstanding.epoch != completion.epoch {
                        self.pending_errors.push_back(
                            RendererConsumerError::StaleConsumerGeneration {
                                expected: outstanding.epoch.consumer_generation(),
                                actual: completion.epoch.consumer_generation(),
                            },
                        );
                        continue;
                    }
                    if outstanding.completion.is_some() {
                        self.pending_errors.push_back(
                            RendererConsumerError::EpochAlreadyCompleted { epoch: sequence },
                        );
                        continue;
                    }
                    outstanding.completion = Some(completion.outcome);
                }
                Ok(SnapshotMessage::Detach {
                    context,
                    generation,
                }) => {
                    if context != self.context {
                        self.pending_errors
                            .push_back(RendererConsumerError::ForeignContext {
                                expected: self.context,
                                actual: context,
                            });
                        continue;
                    }
                    match self.phase {
                        ConsumerPhase::Active {
                            generation: active, ..
                        } if active == generation => {
                            self.phase = ConsumerPhase::Draining(generation);
                        }
                        ConsumerPhase::Draining(active) if active == generation => {}
                        ConsumerPhase::Active {
                            generation: active, ..
                        }
                        | ConsumerPhase::Draining(active) => {
                            self.pending_errors.push_back(
                                RendererConsumerError::StaleConsumerGeneration {
                                    expected: active.get(),
                                    actual: generation.get(),
                                },
                            );
                        }
                        ConsumerPhase::Unbound => {
                            self.pending_errors
                                .push_back(RendererConsumerError::NoActiveConsumer);
                        }
                    }
                }
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
            }
        }
    }

    pub(super) fn close(&mut self) {
        self.outstanding.clear();
        self.phase = ConsumerPhase::Unbound;
        self.synchronous_frame = None;
    }
}

/// One-use permission to reset Context-owned renderer texture bindings.
///
/// [`Context::prepare_renderer_texture_reset`] validates that the matching renderer consumer is
/// idle before any GPU resource is released. The permit then keeps both the Context and consumer
/// borrowed while the backend destroys its texture map. Call [`Self::commit`] only after those GPU
/// resources are no longer reachable. Dropping the permit without committing leaves every native
/// binding unchanged.
#[must_use = "destroy the renderer texture map, then commit this reset permit"]
pub struct RendererTextureReset<'context, 'consumer> {
    context: &'context mut Context,
    _consumer: &'consumer dyn RendererConsumerCapability,
    watermark: u64,
}

impl RendererTextureReset<'_, '_> {
    /// Clear the bindings covered by this already-validated reset transaction.
    ///
    /// This operation is infallible because the permit exclusively borrows the Context, keeps the
    /// validated consumer alive, and was created only after all of its epochs completed.
    pub fn commit(self) {
        let binding = self.context.binding();
        binding.with_bound_context(|| self.commit_unlocked());
    }

    fn commit_unlocked(self) {
        self.context
            .commit_renderer_texture_reset_unlocked(self.watermark);
    }
}

impl Context {
    /// Validate whether this Context can attach a managed renderer consumer.
    ///
    /// This check is non-mutating: it neither reserves a consumer generation nor claims the font
    /// atlas for managed rendering. It is intended for integrations that must validate several
    /// Contexts before attaching any renderer. A successful preflight is only a snapshot of the
    /// current state; both consumer creation methods repeat the validation when they commit.
    ///
    /// Pending detached completions are not polled by this method. Call
    /// [`Self::poll_snapshot_completions`] first when retrying after a consumer entered its
    /// draining phase.
    pub fn preflight_renderer_consumer(&self) -> Result<(), RendererConsumerError> {
        let _guard = CTX_MUTEX.lock();
        self.validate_renderer_consumer_admission_unlocked("Context::preflight_renderer_consumer()")
            .map(|_| ())
    }

    /// Register the sole synchronous renderer consumer for this Context.
    ///
    /// The generation is fixed to synchronous rendering when it is created and cannot be used to
    /// build detached snapshots.
    ///
    /// A [`SharedFontAtlas`](crate::SharedFontAtlas) must be registered with exactly one context
    /// before it can enter managed renderer mode. If multiple contexts still share the atlas, this
    /// returns [`RendererConsumerError::SharedFontAtlasRequiresExclusiveContext`]. Multiple-context
    /// shared atlases remain available to legacy renderer-managed texture handling.
    pub fn create_synchronous_renderer_consumer(
        &mut self,
    ) -> Result<SynchronousRendererConsumer, RendererConsumerError> {
        let _guard = CTX_MUTEX.lock();
        let generation = self.commit_renderer_consumer_admission_unlocked(
            "Context::create_synchronous_renderer_consumer()",
        )?;
        Ok(self
            .snapshot_hub
            .commit_synchronous_consumer_admission(generation))
    }

    /// Register the sole detached renderer consumer for this Context.
    ///
    /// The generation is fixed to pointer-free snapshot rendering when it is created. Dropping the
    /// capability begins draining any outstanding snapshot epochs.
    pub fn create_detached_renderer_consumer(
        &mut self,
    ) -> Result<DetachedRendererConsumer, RendererConsumerError> {
        let _guard = CTX_MUTEX.lock();
        let generation = self.commit_renderer_consumer_admission_unlocked(
            "Context::create_detached_renderer_consumer()",
        )?;
        Ok(self
            .snapshot_hub
            .commit_detached_consumer_admission(generation))
    }

    fn commit_renderer_consumer_admission_unlocked(
        &mut self,
        caller: &str,
    ) -> Result<NonZeroU64, RendererConsumerError> {
        self.assert_current_context(caller);
        let atlas_target = self.font_atlas_snapshot_target();
        let _ = self.poll_snapshot_completions_with_target(&atlas_target)?;
        let (atlas, generation) = self.validate_renderer_consumer_admission_unlocked(caller)?;
        let _ = crate::fonts::claim_validated_font_atlas_managed_renderer(atlas, self.raw);
        Ok(generation)
    }

    fn validate_renderer_consumer_admission_unlocked(
        &self,
        caller: &str,
    ) -> Result<(*mut crate::sys::ImFontAtlas, NonZeroU64), RendererConsumerError> {
        self.assert_current_context(caller);
        let io = self.io_ptr(caller);
        let atlas = unsafe { (*io).Fonts };
        crate::fonts::validate_font_atlas_managed_renderer(atlas, self.raw)?;
        let generation = self.snapshot_hub.validate_consumer_admission()?;
        Ok((atlas, generation))
    }

    /// Merge all currently available detached completion messages.
    pub fn poll_snapshot_completions(
        &mut self,
    ) -> Result<SnapshotCompletionProgress, RendererConsumerError> {
        let _guard = CTX_MUTEX.lock();
        self.poll_snapshot_completions_unlocked()
    }

    pub(super) fn poll_snapshot_completions_unlocked(
        &mut self,
    ) -> Result<SnapshotCompletionProgress, RendererConsumerError> {
        self.assert_current_context("Context::poll_snapshot_completions()");
        let atlas = self.font_atlas_snapshot_target();
        self.poll_snapshot_completions_with_target(&atlas)
    }

    fn poll_snapshot_completions_with_target(
        &mut self,
        atlas: &FontAtlasSnapshotTarget,
    ) -> Result<SnapshotCompletionProgress, RendererConsumerError> {
        self.snapshot_hub
            .poll(&mut self.texture_registry.borrow_mut(), atlas)
    }

    /// Validate an idle renderer generation before destroying its complete GPU texture map.
    ///
    /// This two-phase transaction is the only safe renderer-reset path. Prepare the reset while
    /// the renderer is still intact, release every GPU resource keyed by this consumer, then call
    /// [`RendererTextureReset::commit`]. If preparation fails, the backend can return without
    /// partially destroying its resource map. Dropping the permit without commit does not mutate
    /// native texture state.
    ///
    /// A single-call reset is intentionally unavailable because the Context cannot prove that an
    /// external renderer released its GPU map first:
    ///
    /// ```compile_fail
    /// use dear_imgui_rs::Context;
    ///
    /// let mut context = Context::create();
    /// let consumer = context.create_synchronous_renderer_consumer().unwrap();
    /// let _ = context.reset_renderer_texture_bindings(&consumer);
    /// ```
    pub fn prepare_renderer_texture_reset<'context, 'consumer>(
        &'context mut self,
        consumer: &'consumer impl RendererConsumerCapability,
    ) -> Result<RendererTextureReset<'context, 'consumer>, RendererConsumerError> {
        let _guard = CTX_MUTEX.lock();
        self.prepare_renderer_texture_reset_unlocked(consumer)
    }

    /// Prepares a reset while `Context::drop` already owns the Context lock.
    ///
    /// The only caller is the phase-limited attachment capability. Not reacquiring the global
    /// lock avoids recursive locking during Context teardown while retaining the ordinary public
    /// transaction for all external renderers.
    pub(super) fn prepare_renderer_texture_reset_during_teardown(
        &mut self,
        consumer: &impl RendererConsumerCapability,
    ) -> Result<u64, RendererConsumerError> {
        self.validate_renderer_texture_reset_unlocked(consumer)
    }

    fn prepare_renderer_texture_reset_unlocked<'context, 'consumer>(
        &'context mut self,
        consumer: &'consumer impl RendererConsumerCapability,
    ) -> Result<RendererTextureReset<'context, 'consumer>, RendererConsumerError> {
        let watermark = self.validate_renderer_texture_reset_unlocked(consumer)?;
        Ok(RendererTextureReset {
            context: self,
            _consumer: consumer,
            watermark,
        })
    }

    fn validate_renderer_texture_reset_unlocked(
        &mut self,
        consumer: &impl RendererConsumerCapability,
    ) -> Result<u64, RendererConsumerError> {
        self.assert_current_context("Context::prepare_renderer_texture_reset()");
        let atlas = self.font_atlas_snapshot_target();
        let _ = self.poll_snapshot_completions_with_target(&atlas)?;
        self.snapshot_hub.validate_idle_consumer(consumer)?;
        Ok(self.snapshot_hub.completion_watermark())
    }

    pub(super) fn commit_renderer_texture_reset_during_teardown(&mut self, watermark: u64) {
        self.commit_renderer_texture_reset_unlocked(watermark);
    }

    fn commit_renderer_texture_reset_unlocked(&mut self, watermark: u64) {
        self.assert_current_context("RendererTextureReset::commit()");
        let atlas = self.font_atlas_snapshot_target();
        let _ = atlas.reset_renderer_bindings();
        let _ = self
            .texture_registry
            .borrow_mut()
            .reset_renderer_bindings(watermark);
    }

    pub(super) fn capture_main_snapshot(
        &mut self,
        consumer: &DetachedRendererConsumer,
        draw_data: *const crate::render::DrawData,
    ) -> Result<FrameSnapshot, SnapshotError> {
        let atlas = self.font_atlas_snapshot_target();
        let _ = self.poll_snapshot_completions_with_target(&atlas)?;
        let mut pending = {
            let registry = self.texture_registry.borrow();
            let mut resolve = |native| registry.resolve_snapshot_texture(native, &atlas);
            capture_draw_data(unsafe { &*draw_data }, &mut resolve)?
        };
        self.texture_registry
            .borrow_mut()
            .track_snapshot_operations(&mut pending.texture_requests, &atlas)?;
        self.snapshot_hub.begin_snapshot(
            consumer,
            pending,
            &mut self.texture_registry.borrow_mut(),
            &atlas,
        )
    }

    pub(super) fn begin_synchronous_render(
        &mut self,
        consumer: &SynchronousRendererConsumer,
        draw_data: *const crate::render::DrawData,
    ) -> Result<(SnapshotEpoch, Vec<TextureRequest>), SnapshotError> {
        let native_frame_count = unsafe { (*self.raw).FrameCount };
        self.snapshot_hub
            .begin_synchronous_native_frame(native_frame_count);
        let atlas = self.font_atlas_snapshot_target();
        let _ = self.poll_snapshot_completions_with_target(&atlas)?;
        let mut pending = {
            let registry = self.texture_registry.borrow();
            let mut resolve = |native| registry.resolve_snapshot_texture(native, &atlas);
            capture_texture_requests_only(unsafe { &*draw_data }, &mut resolve)?
        };
        self.texture_registry
            .borrow_mut()
            .track_snapshot_operations(&mut pending, &atlas)?;
        Ok(self
            .snapshot_hub
            .begin_synchronous(consumer, pending, &atlas)?)
    }

    pub(crate) fn complete_synchronous_render(
        &mut self,
        epoch: SnapshotEpoch,
        feedback: Vec<TextureFeedback>,
    ) -> Result<SnapshotCompletionProgress, RendererConsumerError> {
        let atlas = self.font_atlas_snapshot_target();
        self.snapshot_hub.complete_synchronous(
            epoch,
            feedback,
            &mut self.texture_registry.borrow_mut(),
            atlas,
        )
    }

    pub(crate) fn abandon_synchronous_render(&mut self, epoch: SnapshotEpoch) {
        let atlas = self.font_atlas_snapshot_target();
        self.snapshot_hub.abandon_synchronous(
            epoch,
            &mut self.texture_registry.borrow_mut(),
            atlas,
        );
    }

    #[cfg(feature = "multi-viewport")]
    pub(super) fn capture_platform_snapshot(
        &mut self,
        consumer: &DetachedRendererConsumer,
    ) -> Result<FrameSnapshot, SnapshotError> {
        let atlas = self.font_atlas_snapshot_target();
        let _ = self.poll_snapshot_completions_with_target(&atlas)?;
        let platform_io_ptr = self.platform_io_ptr("Context::capture_platform_snapshot()");
        let platform_io =
            unsafe { crate::platform_io::PlatformIo::from_raw(platform_io_ptr.cast_const()) };
        let mut pending = {
            let registry = self.texture_registry.borrow();
            let mut resolve = |native| registry.resolve_snapshot_texture(native, &atlas);
            // SAFETY: this Context owns the live rendered frame and PlatformIO draw pointers;
            // capture copies all data before either can be advanced or destroyed.
            unsafe { capture_platform_io(platform_io, &mut resolve)? }
        };
        self.texture_registry
            .borrow_mut()
            .track_snapshot_operations(&mut pending.texture_requests, &atlas)?;
        self.snapshot_hub.begin_snapshot(
            consumer,
            pending,
            &mut self.texture_registry.borrow_mut(),
            &atlas,
        )
    }

    pub(super) fn font_atlas_snapshot_target(&self) -> FontAtlasSnapshotTarget {
        let io = self.io_ptr("Context snapshot texture capture");
        let atlas = unsafe { (*io).Fonts };
        assert!(!atlas.is_null(), "Context has no font atlas");
        let textures = crate::fonts::font_atlas_snapshot_identities(atlas, self.raw)
            .into_iter()
            .map(|identity| {
                FontAtlasTextureTarget::new(
                    SnapshotTextureId::FontAtlas {
                        context: self.id(),
                        stamp: identity.stamp,
                        generation: identity.texture_generation,
                    },
                    identity.revision,
                    identity.texture,
                )
            })
            .collect();
        FontAtlasSnapshotTarget::new(atlas, self.id(), textures)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renderer_consumer_preflight_failure_does_not_claim_the_font_atlas() {
        let _guard = crate::test_support::imgui_context_guard();
        let atlas = crate::SharedFontAtlas::create();
        let first = Context::create_with_shared_font_atlas(atlas.clone());
        let suspended = first.suspend();
        let second = Context::create_with_shared_font_atlas(atlas.clone());

        assert_eq!(
            second.preflight_renderer_consumer(),
            Err(
                RendererConsumerError::SharedFontAtlasRequiresExclusiveContext {
                    registered_contexts: 2,
                }
            )
        );

        drop(second);
        let replacement = Context::try_create_with_shared_font_atlas(atlas.clone())
            .expect("preflight must not claim the shared font atlas");
        drop(replacement);
        drop(suspended);
    }

    #[test]
    fn renderer_consumer_hub_failure_does_not_claim_the_font_atlas() {
        let _guard = crate::test_support::imgui_context_guard();
        let atlas = crate::SharedFontAtlas::create();
        let mut context = Context::create_with_shared_font_atlas(atlas.clone());
        context.snapshot_hub.next_consumer_generation = None;

        assert!(matches!(
            context.create_synchronous_renderer_consumer(),
            Err(RendererConsumerError::ConsumerGenerationExhausted)
        ));

        let suspended = context.suspend();
        let second = Context::try_create_with_shared_font_atlas(atlas.clone())
            .expect("failed consumer admission must not claim the shared font atlas");
        drop(second);
        drop(suspended);
    }
}
