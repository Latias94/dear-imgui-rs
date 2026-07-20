use std::collections::{BTreeMap, HashSet, VecDeque};
use std::num::NonZeroU64;
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};

#[cfg(feature = "multi-viewport")]
use crate::render::snapshot::capture_platform_io;
use crate::render::snapshot::{
    FrameSnapshot, PendingSnapshot, PendingTextureRequest, RendererConsumer, RendererConsumerError,
    SnapshotCompletionOutcome, SnapshotCompletionProgress, SnapshotEpoch, SnapshotError,
    SnapshotMessage, SnapshotTextureId, TextureFeedback, TextureFeedbackResult, TextureRequest,
    TextureRequestKey, TextureRequestKind, capture_draw_data, capture_texture_requests_only,
    finalize_texture_requests,
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
    Unclaimed,
    Synchronous,
    Detached,
}

#[derive(Debug)]
struct OutstandingEpoch {
    epoch: SnapshotEpoch,
    expected: HashSet<TextureRequestKey>,
    completion: Option<SnapshotCompletionOutcome>,
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
        }
    }

    pub(super) const fn completion_watermark(&self) -> u64 {
        self.completion_watermark
    }

    pub(super) fn attach_consumer(&mut self) -> Result<RendererConsumer, RendererConsumerError> {
        match self.phase {
            ConsumerPhase::Active { .. } => {
                return Err(RendererConsumerError::ConsumerAlreadyActive);
            }
            ConsumerPhase::Draining(_) => return Err(RendererConsumerError::ConsumerDraining),
            ConsumerPhase::Unbound => {}
        }
        let generation = self
            .next_consumer_generation
            .take()
            .ok_or(RendererConsumerError::ConsumerGenerationExhausted)?;
        self.next_consumer_generation = generation.get().checked_add(1).and_then(NonZeroU64::new);
        self.phase = ConsumerPhase::Active {
            generation,
            mode: ConsumerMode::Unclaimed,
        };
        Ok(RendererConsumer::new(
            self.context,
            generation,
            self.sender.clone(),
        ))
    }

    pub(super) fn begin_snapshot(
        &mut self,
        consumer: &RendererConsumer,
        pending: PendingSnapshot,
        registry: &mut ManagedTextureRegistry,
    ) -> Result<FrameSnapshot, SnapshotError> {
        let generation = self.validate_consumer(consumer, ConsumerMode::Detached)?;
        let sequence = self.allocate_epoch()?;
        let epoch = SnapshotEpoch::new(self.context, generation, sequence);
        let referenced = pending.referenced_user_textures();
        registry.record_snapshot_references(&referenced, sequence.get())?;
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
        pending: Vec<PendingTextureRequest>,
    ) -> Result<(SnapshotEpoch, Vec<TextureRequest>), RendererConsumerError> {
        let generation = self.claim_active_mode(ConsumerMode::Synchronous)?;
        if !self.outstanding.is_empty() {
            return Err(RendererConsumerError::ConsumerModeMismatch);
        }
        let sequence = self.allocate_epoch()?;
        let epoch = SnapshotEpoch::new(self.context, generation, sequence);
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
        self.set_direct_completion(epoch, SnapshotCompletionOutcome::Committed(feedback))?;
        self.advance(registry, &atlas)
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
        consumer: &RendererConsumer,
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
            } if expected != consumer.generation_raw() => {
                Err(RendererConsumerError::StaleConsumerGeneration {
                    expected: expected.get(),
                    actual: consumer.generation(),
                })
            }
            ConsumerPhase::Active { generation, .. } => Ok(generation),
        }?;
        self.claim_mode(mode)?;
        Ok(generation)
    }

    fn claim_active_mode(
        &mut self,
        mode: ConsumerMode,
    ) -> Result<NonZeroU64, RendererConsumerError> {
        let ConsumerPhase::Active { generation, .. } = self.phase else {
            return Err(match self.phase {
                ConsumerPhase::Unbound => RendererConsumerError::NoActiveConsumer,
                ConsumerPhase::Draining(_) => RendererConsumerError::ConsumerDraining,
                ConsumerPhase::Active { .. } => unreachable!(),
            });
        };
        self.claim_mode(mode)?;
        Ok(generation)
    }

    fn claim_mode(&mut self, requested: ConsumerMode) -> Result<(), RendererConsumerError> {
        let ConsumerPhase::Active { mode, .. } = &mut self.phase else {
            return Err(RendererConsumerError::NoActiveConsumer);
        };
        match *mode {
            ConsumerMode::Unclaimed => {
                *mode = requested;
                Ok(())
            }
            current if current == requested => Ok(()),
            _ => Err(RendererConsumerError::ConsumerModeMismatch),
        }
    }

    pub(super) fn validate_idle_consumer(
        &self,
        consumer: &RendererConsumer,
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
            ConsumerPhase::Active { generation, .. } if generation != consumer.generation_raw() => {
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
        atlas: FontAtlasSnapshotTarget,
    ) -> Result<SnapshotCompletionProgress, RendererConsumerError> {
        self.drain_messages();
        self.advance(registry, &atlas)
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
                    match validate_feedback(&outstanding, &feedback)
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

        registry.reap_destroyed(self.completion_watermark);
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
    }
}

fn validate_feedback(
    outstanding: &OutstandingEpoch,
    feedback: &[TextureFeedback],
) -> Result<(), RendererConsumerError> {
    let mut seen = HashSet::with_capacity(feedback.len());
    for item in feedback {
        let key = item.key();
        if key.epoch.context_id() != outstanding.epoch.context_id() {
            return Err(RendererConsumerError::ForeignContext {
                expected: outstanding.epoch.context_id(),
                actual: key.epoch.context_id(),
            });
        }
        if key.epoch.consumer_generation_raw() != outstanding.epoch.consumer_generation_raw() {
            return Err(RendererConsumerError::StaleConsumerGeneration {
                expected: outstanding.epoch.consumer_generation(),
                actual: key.epoch.consumer_generation(),
            });
        }
        if key.epoch.sequence() != outstanding.epoch.sequence()
            || !outstanding.expected.contains(&key)
        {
            return Err(RendererConsumerError::FeedbackNotRequested {
                epoch: outstanding.epoch.sequence(),
                texture: key.texture,
            });
        }
        if !seen.insert(key) {
            return Err(RendererConsumerError::DuplicateFeedback {
                epoch: outstanding.epoch.sequence(),
                texture: key.texture,
            });
        }
        if !matches!(
            (key.kind, item.result()),
            (
                TextureRequestKind::Create | TextureRequestKind::Update,
                TextureFeedbackResult::Uploaded { .. }
            ) | (
                TextureRequestKind::Destroy,
                TextureFeedbackResult::Destroyed
            )
        ) {
            return Err(RendererConsumerError::InvalidFeedbackTransition {
                texture: key.texture,
            });
        }
    }
    Ok(())
}

impl Context {
    /// Register the sole renderer consumer for this Context.
    ///
    /// A consumer generation is claimed by its first synchronous render or detached snapshot and
    /// cannot switch modes. Dropping it begins draining any outstanding detached epochs.
    pub fn create_renderer_consumer(&mut self) -> Result<RendererConsumer, RendererConsumerError> {
        let _guard = CTX_MUTEX.lock();
        self.assert_current_context("Context::create_renderer_consumer()");
        let _ = self.poll_snapshot_completions()?;
        self.snapshot_hub.attach_consumer()
    }

    /// Merge all currently available detached completion messages.
    pub fn poll_snapshot_completions(
        &mut self,
    ) -> Result<SnapshotCompletionProgress, RendererConsumerError> {
        let _guard = CTX_MUTEX.lock();
        self.assert_current_context("Context::poll_snapshot_completions()");
        let atlas = self.font_atlas_snapshot_target(false);
        self.snapshot_hub
            .poll(&mut self.texture_registry.borrow_mut(), atlas)
    }

    /// Clear native bindings after this renderer has destroyed all of its GPU textures.
    ///
    /// The consumer must belong to this Context and have no outstanding synchronous frame or
    /// detached snapshot. Active textures will be requested again on a later frame; retiring
    /// textures receive a shutdown acknowledgement and can be reclaimed once their watermark is
    /// complete.
    pub fn reset_renderer_texture_bindings(
        &mut self,
        consumer: &RendererConsumer,
    ) -> Result<usize, RendererConsumerError> {
        let _guard = CTX_MUTEX.lock();
        self.assert_current_context("Context::reset_renderer_texture_bindings()");
        let _ = self.poll_snapshot_completions()?;
        self.snapshot_hub.validate_idle_consumer(consumer)?;
        let invalidated = self
            .platform_io_mut()
            .invalidate_renderer_texture_bindings();
        let watermark = self.snapshot_hub.completion_watermark();
        let mut registry = self.texture_registry.borrow_mut();
        registry.acknowledge_renderer_reset(watermark);
        Ok(invalidated)
    }

    pub(super) fn poll_snapshot_completions_or_panic(&mut self, caller: &str) {
        if let Err(error) = self.poll_snapshot_completions() {
            panic!("{caller} rejected detached renderer completion: {error}");
        }
    }

    pub(super) fn capture_main_snapshot(
        &mut self,
        consumer: &RendererConsumer,
        draw_data: *const crate::render::DrawData,
    ) -> Result<FrameSnapshot, SnapshotError> {
        let _ = self.poll_snapshot_completions()?;
        let atlas = self.font_atlas_snapshot_target(true);
        let pending = {
            let registry = self.texture_registry.borrow();
            let mut resolve = |native| registry.resolve_snapshot_texture(native, &atlas);
            capture_draw_data(unsafe { &*draw_data }, &mut resolve)?
        };
        self.snapshot_hub
            .begin_snapshot(consumer, pending, &mut self.texture_registry.borrow_mut())
    }

    pub(super) fn begin_synchronous_render(
        &mut self,
        draw_data: *const crate::render::DrawData,
    ) -> Result<(SnapshotEpoch, Vec<TextureRequest>), SnapshotError> {
        let _ = self.poll_snapshot_completions()?;
        let atlas = self.font_atlas_snapshot_target(true);
        let pending = {
            let registry = self.texture_registry.borrow();
            let mut resolve = |native| registry.resolve_snapshot_texture(native, &atlas);
            capture_texture_requests_only(unsafe { &*draw_data }, &mut resolve)?
        };
        Ok(self.snapshot_hub.begin_synchronous(pending)?)
    }

    pub(crate) fn complete_synchronous_render(
        &mut self,
        epoch: SnapshotEpoch,
        feedback: Vec<TextureFeedback>,
    ) -> Result<SnapshotCompletionProgress, RendererConsumerError> {
        let atlas = self.font_atlas_snapshot_target(false);
        self.snapshot_hub.complete_synchronous(
            epoch,
            feedback,
            &mut self.texture_registry.borrow_mut(),
            atlas,
        )
    }

    pub(crate) fn abandon_synchronous_render(&mut self, epoch: SnapshotEpoch) {
        let atlas = self.font_atlas_snapshot_target(false);
        self.snapshot_hub.abandon_synchronous(
            epoch,
            &mut self.texture_registry.borrow_mut(),
            atlas,
        );
    }

    #[cfg(feature = "multi-viewport")]
    pub(super) fn capture_platform_snapshot(
        &mut self,
        consumer: &RendererConsumer,
    ) -> Result<FrameSnapshot, SnapshotError> {
        let _ = self.poll_snapshot_completions()?;
        let atlas = self.font_atlas_snapshot_target(true);
        let platform_io_ptr = self.platform_io_ptr("Context::capture_platform_snapshot()");
        let platform_io =
            unsafe { crate::platform_io::PlatformIo::from_raw(platform_io_ptr.cast_const()) };
        let pending = {
            let registry = self.texture_registry.borrow();
            let mut resolve = |native| registry.resolve_snapshot_texture(native, &atlas);
            capture_platform_io(platform_io, &mut resolve)?
        };
        self.snapshot_hub
            .begin_snapshot(consumer, pending, &mut self.texture_registry.borrow_mut())
    }

    pub(super) fn font_atlas_snapshot_target(
        &self,
        advance_revision: bool,
    ) -> FontAtlasSnapshotTarget {
        let io = self.io_ptr("Context snapshot texture capture");
        let atlas = unsafe { (*io).Fonts };
        assert!(!atlas.is_null(), "Context has no font atlas");
        let textures = crate::fonts::font_atlas_snapshot_identities(atlas, advance_revision)
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
        FontAtlasSnapshotTarget::new(textures)
    }
}
