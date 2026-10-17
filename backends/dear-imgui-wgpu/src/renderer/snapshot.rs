//! Context publication stays on the UI thread; the executor owns only GPU state and metadata.

use std::{
    collections::{HashMap, HashSet},
    num::NonZeroU64,
    sync::Arc,
};

use dear_imgui_rs::{
    Context, ContextId, Id,
    platform_io::NativeWindowTarget,
    render::{
        DetachedRendererConsumer, FrameSnapshot, SnapshotCommitError, SnapshotEpoch,
        TextureFeedback,
    },
};

use super::{
    FramebufferExtent, WgpuRenderer, core::RendererContextState, draw_source::DrawSource,
    gpu::GpuState,
};
use crate::{
    ExternalTextureBinding, ExternalTextureId, GammaMode, RendererError, RendererResult, Uniforms,
    WgpuDrawRecording, WgpuFrameScope, WgpuFrameSkipReason, WgpuInitInfo,
    texture::ManagedRequestOutcome, wgpu,
};

/// UI-thread publication and sole detached consumer capability for one Context.
///
/// Keep this value on the UI thread. Move the executor returned by [`Self::new`] to the render
/// thread and send [`WgpuSnapshotSubmission`] values through an application-owned bounded channel.
/// No Context binding, platform pointer, or raw draw callback enters the executor.
///
/// ```compile_fail
/// fn require_send<T: Send>() {}
/// require_send::<dear_imgui_wgpu::WgpuSnapshotRenderer>();
/// ```
///
/// Dropping this UI owner clears its exact publication and starts the core consumer's draining
/// phase. Already captured snapshots retain their own completion tickets. Join the worker and
/// call [`Self::shutdown`] when that live Context must accept another renderer immediately.
pub struct WgpuSnapshotRenderer {
    state: Option<RendererContextState>,
    consumer: Option<DetachedRendererConsumer>,
}

impl WgpuSnapshotRenderer {
    /// Publishes the WGPU draw contract and creates its separate GPU executor.
    pub fn new(
        init: WgpuInitInfo,
        context: &mut Context,
    ) -> RendererResult<(Self, WgpuSnapshotExecutor)> {
        WgpuRenderer::ensure_context_available(context)?;
        let mut gpu = GpuState::empty();
        gpu.initialize(init)?;
        let (flags, name) = WgpuRenderer::configure_imgui_context(context)?;
        let state = match RendererContextState::publish(context, flags) {
            Ok(state) => state,
            Err(error) => {
                WgpuRenderer::clear_unbound_imgui_context(context, flags, name);
                return Err(error);
            }
        };
        let consumer = match context.create_detached_renderer_consumer() {
            Ok(consumer) => consumer,
            Err(error) => {
                state.clear_with_context(context);
                return Err(error.into());
            }
        };
        let reset = match context.prepare_renderer_texture_reset(&consumer) {
            Ok(reset) => reset,
            Err(error) => {
                drop(consumer);
                state.clear_with_context(context);
                return Err(error.into());
            }
        };
        reset.commit();
        let executor = WgpuSnapshotExecutor {
            gpu,
            context: consumer.context_id(),
            generation: consumer.generation(),
            targets: HashMap::new(),
            next_target_generation: 1,
            last_epoch: None,
        };
        Ok((
            Self {
                state: Some(state),
                consumer: Some(consumer),
            },
            executor,
        ))
    }

    /// Consumer passed to the core's `FrameToken::render_snapshot` or platform snapshot capture.
    pub fn renderer_consumer(&self) -> RendererResult<&DetachedRendererConsumer> {
        let state = self.state.as_ref().ok_or(RendererError::ContextNotBound)?;
        state.ensure_renderer_contract()?;
        self.consumer.as_ref().ok_or(RendererError::ContextNotBound)
    }

    /// Resets managed bindings and clears publication after the worker returns its executor.
    ///
    /// Outstanding snapshots reject shutdown without changing either owner. Poll the core's
    /// completion channel first. Queue submission and GPU completion remain host responsibilities.
    pub fn shutdown(
        &mut self,
        context: &mut Context,
        executor: &mut WgpuSnapshotExecutor,
    ) -> RendererResult<()> {
        self.state
            .as_ref()
            .ok_or(RendererError::ContextNotBound)?
            .ensure_matches(context)?;
        let consumer = self
            .consumer
            .as_ref()
            .ok_or(RendererError::ContextNotBound)?;
        if executor.context != consumer.context_id() || executor.generation != consumer.generation()
        {
            return Err(RendererError::ContextMismatch);
        }
        let reset = context.prepare_renderer_texture_reset(consumer)?;
        executor.gpu = GpuState::empty();
        executor.targets.clear();
        reset.commit();
        self.consumer.take();
        if let Some(state) = self.state.take() {
            state.clear_with_context(context);
        }
        Ok(())
    }
}

/// Frozen association of one viewport with an executor's target-resource generation.
///
/// This is pure `Send + Sync` metadata. It owns no native window, Surface, texture, or GPU lease.
/// Host code must retain those resources while recording and submitting their commands.
///
/// ```compile_fail
/// use dear_imgui_wgpu::WgpuSnapshotTarget;
/// let forged = WgpuSnapshotTarget { resource_generation: std::num::NonZeroU64::MIN };
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WgpuSnapshotTarget {
    context: ContextId,
    consumer_generation: u64,
    resource_generation: NonZeroU64,
    viewport: Id,
    native: Option<NativeWindowTarget>,
}

impl WgpuSnapshotTarget {
    /// Captured viewport routing identity.
    pub const fn viewport_id(self) -> Id {
        self.viewport
    }
    /// Exact native registration, when supplied by the host integration.
    pub const fn native_target(self) -> Option<NativeWindowTarget> {
        self.native
    }
    /// Monotonic resource identity. Replacing or re-registering a target never reuses it.
    pub const fn resource_generation(self) -> NonZeroU64 {
        self.resource_generation
    }
}

/// Owned core snapshot plus the target associations captured when the UI published it.
pub struct WgpuSnapshotSubmission {
    snapshot: FrameSnapshot,
    targets: Vec<WgpuSnapshotTarget>,
}

impl WgpuSnapshotSubmission {
    /// Freezes metadata without consulting the executor or a live Context.
    ///
    /// Supply exactly one target for a main-only snapshot, or one for each captured platform
    /// viewport. The executor checks completeness, duplicate viewports, and resource generations.
    pub fn new(
        snapshot: FrameSnapshot,
        targets: impl IntoIterator<Item = WgpuSnapshotTarget>,
    ) -> Self {
        Self {
            snapshot,
            targets: targets.into_iter().collect(),
        }
    }
}

/// Evidence for one target recorded from a detached snapshot.
#[derive(Clone, Debug)]
pub struct WgpuSnapshotTargetRecording {
    /// Frozen association; never resolved against the current viewport registry afterwards.
    pub target: WgpuSnapshotTarget,
    /// Recording evidence only. Host submit, present, and GPU completion are not observed here.
    pub draw: WgpuDrawRecording,
}

/// Owned metadata retained on success and on errors, including partially recorded snapshots.
#[derive(Clone, Debug)]
pub struct WgpuSnapshotReport {
    /// Core snapshot epoch.
    pub epoch: SnapshotEpoch,
    /// Frozen targets in publication order. Unrecorded targets carry `recorded = false`.
    pub targets: Vec<WgpuSnapshotTargetRecording>,
}

/// Snapshot failure with any earlier recording evidence and completion-delivery failure.
#[derive(Debug, thiserror::Error)]
#[error("detached WGPU snapshot failed: {error}")]
pub struct WgpuSnapshotError {
    /// Immediate renderer failure.
    pub error: Arc<RendererError>,
    /// Earlier observations, with no claim about queue submission or GPU completion.
    pub report: WgpuSnapshotReport,
    /// Failure to deliver feedback, usually because the Context was destroyed.
    pub completion_error: Option<SnapshotCommitError>,
}

/// Context-free WGPU state. On native WGPU backends it is automatically `Send`.
///
/// Move this value to the render thread. Target generation changes, external registrations, and
/// device resources all belong to that thread. It never binds or dereferences a live Context.
pub struct WgpuSnapshotExecutor {
    gpu: GpuState,
    context: ContextId,
    generation: u64,
    targets: HashMap<Id, WgpuSnapshotTarget>,
    next_target_generation: u64,
    last_epoch: Option<u64>,
}

impl WgpuSnapshotExecutor {
    /// Registers or replaces the resources associated with a viewport.
    ///
    /// Call this whenever the host replaces its render-target allocation or native registration.
    /// Queued submissions carrying an earlier capability are rejected before texture mutation.
    pub fn set_target(
        &mut self,
        viewport: Id,
        native: Option<NativeWindowTarget>,
    ) -> RendererResult<WgpuSnapshotTarget> {
        if native.is_some_and(|target| {
            target.instance_id.context_id() != self.context || target.viewport_id != viewport
        }) {
            return Err(RendererError::ContextMismatch);
        }
        let generation = NonZeroU64::new(self.next_target_generation)
            .ok_or(RendererError::SnapshotTargetGenerationExhausted)?;
        self.next_target_generation = self
            .next_target_generation
            .checked_add(1)
            .ok_or(RendererError::SnapshotTargetGenerationExhausted)?;
        let target = WgpuSnapshotTarget {
            context: self.context,
            consumer_generation: self.generation,
            resource_generation: generation,
            viewport,
            native,
        };
        self.targets.insert(viewport, target);
        Ok(target)
    }

    /// Retires an exact generation. Stale or foreign capabilities cannot retire a newer target.
    pub fn retire_target(&mut self, target: WgpuSnapshotTarget) -> RendererResult<()> {
        self.validate_target(target)?;
        self.targets.remove(&target.viewport);
        Ok(())
    }

    fn validate_target(&self, target: WgpuSnapshotTarget) -> RendererResult<()> {
        if target.context != self.context
            || target.consumer_generation != self.generation
            || self.targets.get(&target.viewport) != Some(&target)
        {
            return Err(RendererError::StaleSnapshotTarget {
                viewport: target.viewport,
            });
        }
        Ok(())
    }

    /// Selects the same gamma policy used by the synchronous renderer.
    pub fn set_gamma_mode(&mut self, mode: GammaMode) {
        self.gpu.gamma_mode = mode;
    }

    /// Registers an external view on this executor's device. The application owns its contents.
    pub fn register_external_texture(
        &mut self,
        view: &wgpu::TextureView,
    ) -> RendererResult<ExternalTextureId> {
        self.gpu.texture_manager.register_external_view(view)
    }

    /// Returns the current external view revision, independently of its content frame.
    pub fn external_texture_binding(
        &self,
        texture: ExternalTextureId,
    ) -> RendererResult<ExternalTextureBinding> {
        self.gpu
            .texture_manager
            .external_binding(texture.texture_id())
            .ok_or(RendererError::ExternalTextureNotFound(texture.texture_id()))
    }

    /// Replaces a view and invalidates its cached bind group after a successful revision change.
    pub fn update_external_texture(
        &mut self,
        texture: ExternalTextureId,
        view: &wgpu::TextureView,
    ) -> RendererResult<()> {
        self.gpu
            .texture_manager
            .update_external_view(texture, view)?;
        self.gpu
            .backend_data
            .as_mut()
            .ok_or(RendererError::ContextNotBound)?
            .render_resources
            .remove_image_bind_group(texture.texture_id());
        Ok(())
    }

    /// Removes an external mapping. The host must retain resources used by in-flight GPU work.
    pub fn unregister_external_texture(
        &mut self,
        texture: ExternalTextureId,
    ) -> RendererResult<()> {
        self.gpu.texture_manager.remove_external_view(texture)?;
        self.gpu
            .backend_data
            .as_mut()
            .ok_or(RendererError::ContextNotBound)?
            .render_resources
            .remove_image_bind_group(texture.texture_id());
        Ok(())
    }

    /// Validates the captured identity and applies owned managed requests without recording.
    ///
    /// Texture feedback is committed for every request on failure. Earlier successful uploads
    /// retain their acknowledgements; the failing and remaining requests request retry. Stale
    /// epochs use superseded feedback so delayed work cannot resurrect retired GPU resources.
    pub fn prepare(
        &mut self,
        submission: WgpuSnapshotSubmission,
    ) -> Result<WgpuPreparedSnapshot<'_>, WgpuSnapshotError> {
        let WgpuSnapshotSubmission { snapshot, targets } = submission;
        let epoch = snapshot.epoch();
        let report = WgpuSnapshotReport {
            epoch,
            targets: targets
                .iter()
                .map(|target| {
                    let draw = snapshot
                        .viewport_draw(target.viewport)
                        .unwrap_or_else(|| snapshot.draw_data());
                    WgpuSnapshotTargetRecording {
                        target: *target,
                        draw: WgpuDrawRecording {
                            scope: WgpuFrameScope {
                                context_id: epoch.context_id(),
                                epoch: Some(epoch),
                                frame_count: draw.frame_count,
                            },
                            dependencies: Default::default(),
                            recorded: false,
                            skip_reason: None,
                        },
                    }
                })
                .collect(),
        };
        let validation = (|| {
            if epoch.context_id() != self.context {
                return Err(RendererError::ContextMismatch);
            }
            if epoch.consumer_generation() != self.generation {
                return Err(RendererError::SnapshotConsumerGeneration {
                    expected: self.generation,
                    actual: epoch.consumer_generation(),
                });
            }
            if self.last_epoch.is_some_and(|last| epoch.sequence() <= last) {
                return Err(RendererError::FrameEpochOutOfOrder {
                    active: self.last_epoch.unwrap_or(0),
                    received: epoch.sequence(),
                });
            }
            let mut seen = std::collections::HashSet::new();
            for target in &targets {
                self.validate_target(*target)?;
                if !seen.insert(target.viewport) {
                    return Err(RendererError::InvalidRenderState(
                        "duplicate detached viewport target".to_owned(),
                    ));
                }
            }
            if snapshot.viewports().is_empty() {
                if targets.len() != 1 {
                    return Err(RendererError::InvalidRenderState(
                        "a main-only snapshot requires exactly one target".to_owned(),
                    ));
                }
            } else if targets.len() != snapshot.viewports().len()
                || snapshot
                    .viewports()
                    .iter()
                    .any(|viewport| !seen.contains(&viewport.viewport_id))
            {
                return Err(RendererError::InvalidRenderState(
                    "snapshot targets do not match captured viewports".to_owned(),
                ));
            }
            for target in &targets {
                let captured_main = snapshot
                    .viewports()
                    .iter()
                    .find(|viewport| viewport.viewport_id == target.viewport)
                    .is_none_or(|viewport| viewport.is_main());
                if target
                    .native
                    .is_some_and(|native| native.is_main != captured_main)
                {
                    return Err(RendererError::InvalidRenderState(
                        "native target role does not match the captured viewport".to_owned(),
                    ));
                }
            }
            let backend = self
                .gpu
                .backend_data
                .as_mut()
                .ok_or(RendererError::ContextNotBound)?;
            let frame_count = i32::try_from(snapshot.draw_data().frame_count).map_err(|_| {
                RendererError::DrawBufferTooLarge {
                    buffer: "frame counter",
                }
            })?;
            backend.begin_frame(epoch.sequence(), frame_count)
        })();
        if let Err(error) = validation {
            let superseded = matches!(error, RendererError::FrameEpochOutOfOrder { .. });
            let feedback = snapshot
                .texture_requests()
                .iter()
                .map(|request| {
                    if superseded {
                        request.superseded()
                    } else {
                        request.retry()
                    }
                })
                .collect::<Vec<_>>();
            return Err(WgpuSnapshotError {
                error: Arc::new(error),
                report,
                completion_error: snapshot.commit(feedback).err(),
            });
        }
        // Every older epoch is rejected above before request application. Its destroy markers
        // therefore no longer protect an executable frame; this is an execution-order barrier,
        // not a claim about core completion feedback or GPU completion.
        if let Some(previous) = self.last_epoch {
            self.gpu
                .texture_manager
                .prune_destroyed_managed_textures(previous);
        }
        self.last_epoch = Some(epoch.sequence());
        let (backend_data, texture_manager) =
            (&mut self.gpu.backend_data, &mut self.gpu.texture_manager);
        let backend = backend_data.as_mut().expect("validated GPU state");
        let mut feedback = Vec::with_capacity(snapshot.texture_requests().len());
        let mut failure = None;
        for request in snapshot.texture_requests() {
            if failure.is_some() {
                feedback.push(request.retry());
                continue;
            }
            let result = texture_manager
                .apply_managed_request_at_epoch(
                    request.texture(),
                    request.operation(),
                    epoch.sequence(),
                    &backend.device,
                    &backend.queue,
                    &mut backend.render_resources,
                )
                .and_then(|outcome| {
                    Ok(match outcome {
                        ManagedRequestOutcome::Uploaded(id) => request.uploaded(id)?,
                        ManagedRequestOutcome::Destroyed => request.destroyed()?,
                        ManagedRequestOutcome::Superseded => request.superseded(),
                    })
                });
            match result {
                Ok(item) => feedback.push(item),
                Err(error) => {
                    feedback.push(request.retry());
                    failure = Some(error);
                }
            }
        }
        if let Some(error) = failure {
            return Err(WgpuSnapshotError {
                error: Arc::new(error),
                report,
                completion_error: snapshot.commit(feedback).err(),
            });
        }
        Ok(WgpuPreparedSnapshot {
            executor: self,
            snapshot: Some(snapshot),
            targets,
            feedback,
            report,
            failure: None,
            attempted: HashSet::new(),
        })
    }
}

/// Exclusive preparation scope for one snapshot; resources cannot be replaced while it exists.
///
/// Record each target at most once, then finish to commit request feedback. Dropping this value
/// also commits already-applied texture feedback; it does not submit or present any commands.
///
/// ```compile_fail
/// use dear_imgui_wgpu::{WgpuSnapshotExecutor, WgpuSnapshotSubmission};
/// fn replace_while_prepared(executor: &mut WgpuSnapshotExecutor, submission: WgpuSnapshotSubmission) {
///     let prepared = executor.prepare(submission).unwrap();
///     let _ = executor.set_target(dear_imgui_rs::Id::from(7_u32), None);
///     let _ = prepared.report();
/// }
/// ```
#[must_use = "record the snapshot and finish its completion feedback"]
pub struct WgpuPreparedSnapshot<'executor> {
    executor: &'executor mut WgpuSnapshotExecutor,
    snapshot: Option<FrameSnapshot>,
    targets: Vec<WgpuSnapshotTarget>,
    feedback: Vec<TextureFeedback>,
    report: WgpuSnapshotReport,
    failure: Option<Arc<RendererError>>,
    attempted: HashSet<Id>,
}

impl WgpuPreparedSnapshot<'_> {
    /// Returns accumulated owned evidence before completion.
    pub fn report(&self) -> &WgpuSnapshotReport {
        &self.report
    }

    /// Records one captured viewport into the host's render pass using the shared draw executor.
    ///
    /// The supplied target must match the frozen association. As with synchronous rendering,
    /// WGPU cannot inspect a pass's attachments: the host supplies their exact extent and retains
    /// any Surface/native leases. On error the scope becomes terminal, preserving earlier records.
    pub fn record(
        &mut self,
        target: WgpuSnapshotTarget,
        pass: &mut wgpu::RenderPass<'_>,
        extent: FramebufferExtent,
    ) -> Result<&WgpuDrawRecording, Arc<RendererError>> {
        if let Some(error) = &self.failure {
            return Err(Arc::clone(error));
        }
        match self.record_target(target, pass, extent) {
            Ok(recording) => {
                let record = self
                    .report
                    .targets
                    .iter_mut()
                    .find(|record| record.target == target)
                    .expect("validated captured target");
                record.draw = recording;
                Ok(&record.draw)
            }
            Err(error) => {
                let error = Arc::new(error);
                self.failure = Some(Arc::clone(&error));
                Err(error)
            }
        }
    }

    fn record_target(
        &mut self,
        target: WgpuSnapshotTarget,
        pass: &mut wgpu::RenderPass<'_>,
        extent: FramebufferExtent,
    ) -> RendererResult<WgpuDrawRecording> {
        self.executor.validate_target(target)?;
        if !self.targets.contains(&target) {
            return Err(RendererError::StaleSnapshotTarget {
                viewport: target.viewport,
            });
        }
        if !self.attempted.insert(target.viewport) {
            return Err(RendererError::InvalidRenderState(
                "snapshot viewport was already recorded".to_owned(),
            ));
        }
        let snapshot = self.snapshot.as_ref().expect("unfinished snapshot");
        let draw = if snapshot.viewports().is_empty() {
            snapshot.draw_data()
        } else {
            snapshot
                .viewport_draw(target.viewport)
                .ok_or(RendererError::StaleSnapshotTarget {
                    viewport: target.viewport,
                })?
        };
        let scope = WgpuFrameScope {
            context_id: snapshot.epoch().context_id(),
            epoch: Some(snapshot.epoch()),
            frame_count: draw.frame_count,
        };
        if extent.is_empty() {
            return Ok(WgpuDrawRecording::skipped(
                scope,
                WgpuFrameSkipReason::ZeroSize,
            ));
        }
        let gpu = &mut self.executor.gpu;
        let (backend_data, texture_manager) = (&mut gpu.backend_data, &gpu.texture_manager);
        let backend = backend_data
            .as_mut()
            .ok_or(RendererError::ContextNotBound)?;
        let source = DrawSource::Snapshot(draw);
        let prepared = WgpuRenderer::prepare_draw_source(
            texture_manager,
            &gpu.default_texture,
            source,
            extent,
            backend,
        )?;
        if prepared.is_empty() {
            return Ok(WgpuDrawRecording::skipped(
                scope,
                WgpuFrameSkipReason::NoDrawCommands,
            ));
        }
        let dependencies = prepared.dependencies().clone();
        // Preserve successfully prepared dependencies even if a later upload or encoder step fails.
        self.report
            .targets
            .iter_mut()
            .find(|record| record.target == target)
            .expect("validated captured target")
            .draw
            .dependencies = dependencies.clone();
        let gamma = match gpu.gamma_mode {
            GammaMode::Auto => Uniforms::gamma_for_format(backend.render_target_format),
            GammaMode::Linear => 1.0,
            GammaMode::Gamma22 => 2.2,
        };
        let state = WgpuRenderer::prepare_render_state_source(
            source,
            backend,
            gamma,
            prepared.has_elements(),
        )?;
        WgpuRenderer::execute_prepared_commands(prepared, &state, extent, pass, |_| {
            Err(RendererError::RawDrawCallbackUnsupported)
        })?;
        Ok(WgpuDrawRecording {
            scope,
            dependencies,
            recorded: true,
            skip_reason: None,
        })
    }

    /// Commits the complete request-bound feedback and returns owned recording evidence.
    ///
    /// Core completion means CPU-side snapshot processing has ended. It does not mean commands
    /// were submitted or completed on the GPU. Unrecorded targets are allowed for host skip policy.
    pub fn finish(mut self) -> Result<WgpuSnapshotReport, WgpuSnapshotError> {
        let completion_error = self.commit();
        let report = self.report.clone();
        if let Some(error) = self.failure.take() {
            return Err(WgpuSnapshotError {
                error,
                report,
                completion_error,
            });
        }
        if let Some(error) = completion_error {
            return Err(WgpuSnapshotError {
                error: Arc::new(RendererError::InvalidRenderState(
                    "snapshot completion delivery failed".to_owned(),
                )),
                report,
                completion_error: Some(error),
            });
        }
        Ok(report)
    }

    fn commit(&mut self) -> Option<SnapshotCommitError> {
        self.snapshot
            .take()
            .and_then(|snapshot| snapshot.commit(std::mem::take(&mut self.feedback)).err())
    }
}

impl Drop for WgpuPreparedSnapshot<'_> {
    fn drop(&mut self) {
        let _ = self.commit();
    }
}

#[cfg(test)]
mod tests;
