use std::collections::HashMap;
use std::fmt;
use std::num::NonZeroU64;

use bevy_ecs::message::Message;
use dear_imgui_rs::{Context, ContextId, SuspendedContext};

use super::lifecycle::ImguiAppLifecycle;
use super::{
    ContextOwner, ImguiContextRemovalPendingReason, ImguiContextRetirementSink, ImguiPass,
    ImguiPassError, ImguiPrimaryPass, PassIdentity,
};

/// Generation-qualified identity of one managed Context retirement request.
///
/// Retain this value instead of matching a delayed completion by [`ContextId`] alone. The
/// generation identifies the registry admission that owned the Context when retirement began.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ImguiContextRetirementId {
    context_id: ContextId,
    generation: NonZeroU64,
}

impl ImguiContextRetirementId {
    pub(crate) const fn new(context_id: ContextId, generation: NonZeroU64) -> Self {
        Self {
            context_id,
            generation,
        }
    }

    /// Return the Context identity that was registered when removal was requested.
    #[must_use]
    pub const fn context_id(self) -> ContextId {
        self.context_id
    }

    /// Return the registry generation that distinguishes this request from a reused slot.
    #[must_use]
    pub const fn generation(self) -> NonZeroU64 {
        self.generation
    }
}

/// One-shot notification that a managed Context retirement completed.
#[derive(Clone, Copy, Debug, Eq, Message, PartialEq)]
pub struct ImguiContextRetired {
    retirement: ImguiContextRetirementId,
}

impl ImguiContextRetired {
    /// Return the generation-qualified retirement that completed.
    #[must_use]
    pub const fn retirement(self) -> ImguiContextRetirementId {
        self.retirement
    }

    /// Return the retired Context identity.
    #[must_use]
    pub const fn context_id(self) -> ContextId {
        self.retirement.context_id()
    }

    pub(crate) const fn new(retirement: ImguiContextRetirementId) -> Self {
        Self { retirement }
    }
}

/// Result of atomically selecting a new primary Context.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImguiPrimaryChange {
    previous: Option<ContextId>,
    current: ContextId,
}

impl ImguiPrimaryChange {
    /// Return the Context that was primary before the transaction.
    #[must_use]
    pub const fn previous(self) -> Option<ContextId> {
        self.previous
    }

    /// Return the Context selected as primary by the transaction.
    #[must_use]
    pub const fn current(self) -> ContextId {
        self.current
    }
}

/// Per-Context lifecycle and private UI pass configuration.
///
/// In headless builds without the `render` feature, every configured Context pass is driven,
/// but only the primary Context receives implicit primary-window input and capture updates.
/// Explicit input routing for additional Contexts requires `render`.
#[derive(Clone, Debug)]
pub struct ImguiContextConfig {
    pass: PassIdentity,
    docking: bool,
    multi_viewport: bool,
}

impl ImguiContextConfig {
    /// Create an additional-Context configuration bound to an application-owned pass.
    #[must_use]
    pub fn new<P: 'static>(pass: &ImguiPass<P>) -> Self {
        Self {
            pass: pass.identity(),
            docking: true,
            multi_viewport: false,
        }
    }

    /// Configure docking for this Context.
    #[must_use]
    pub fn with_docking(mut self, docking: bool) -> Self {
        self.docking = docking;
        self
    }

    /// Configure native Dear ImGui platform windows for this Context.
    #[must_use]
    pub fn with_multi_viewport(mut self, multi_viewport: bool) -> Self {
        self.multi_viewport = multi_viewport;
        self
    }

    /// Return the Rust type name used to brand this Context pass.
    #[must_use]
    pub const fn pass_name(&self) -> &'static str {
        self.pass.brand_name()
    }

    /// Return whether docking is enabled.
    #[must_use]
    pub fn docking(&self) -> bool {
        self.docking
    }

    /// Return whether native platform windows are requested.
    #[must_use]
    pub fn multi_viewport(&self) -> bool {
        self.multi_viewport
    }

    pub(crate) fn primary(pass: &ImguiPass<ImguiPrimaryPass>) -> Self {
        Self {
            pass: pass.identity(),
            docking: true,
            multi_viewport: false,
        }
    }

    pub(crate) const fn pass(&self) -> PassIdentity {
        self.pass
    }
}

/// Typed failure from Context lookup, pass admission, configuration, or teardown.
#[derive(Debug)]
#[non_exhaustive]
pub enum ImguiContextError {
    /// Explicit shutdown committed this App's terminal Dear ImGui lifecycle.
    AppTerminated,
    /// The App's private pass registry could not serve Context admission.
    PassRegistry(ImguiPassError),
    /// The App already owns a Context registry.
    ContextRegistryAlreadyInstalled,
    /// No registered Context has this process identity.
    UnknownContext { context_id: ContextId },
    /// This exact core Context identity is already registered.
    AlreadyRegistered { context_id: ContextId },
    /// Another Context already owns the requested UI pass.
    DuplicatePass {
        pass: &'static str,
        owner: ContextId,
    },
    /// The requested pass belongs to a different Bevy App.
    ForeignPass { pass: &'static str },
    /// The requested Context is being removed.
    TeardownInProgress { context_id: ContextId },
    /// Raw Context mutation was requested while its UI frame was live.
    RawMutationWhileFrameOpen { context_id: ContextId },
    /// A temporary active-Context scope could not be entered or completed.
    ScopedActivation {
        context_id: ContextId,
        source: super::ImguiContextScopeError,
    },
    /// The legacy/headless font-atlas capability could not be acquired before opening a frame.
    FontAtlasMode {
        context_id: ContextId,
        source: dear_imgui_rs::FontAtlasModeError,
    },
    /// Managed renderer admission failed before backend fields were mutated.
    #[cfg(feature = "render")]
    RendererAdmission {
        context_id: ContextId,
        source: dear_imgui_rs::render::RendererConsumerError,
    },
    /// Device recovery could not reset this Context's renderer generation.
    #[cfg(feature = "render")]
    RendererRecovery {
        context_id: ContextId,
        source: dear_imgui_rs::render::RendererConsumerError,
    },
    /// Renderer fields changed after this backend acquired them.
    #[cfg(feature = "render")]
    RendererOwnership {
        context_id: ContextId,
        source: super::ImguiRendererOwnershipError,
    },
    /// Detached snapshot completion failed for this Context.
    #[cfg(feature = "render")]
    RendererCompletion {
        context_id: ContextId,
        source: dear_imgui_rs::render::RendererConsumerError,
    },
    /// Render-world delivery of a detached snapshot outcome failed for this Context.
    #[cfg(feature = "render")]
    SnapshotCommit {
        context_id: ContextId,
        source: dear_imgui_rs::render::snapshot::SnapshotCommitError,
    },
    /// The completed Dear ImGui frame could not be captured for the render world.
    #[cfg(feature = "render")]
    SnapshotCapture {
        context_id: ContextId,
        source: dear_imgui_rs::render::snapshot::SnapshotError,
    },
    /// Native viewport callbacks or their deferred command bridge failed for this Context.
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    ViewportBridge {
        context_id: ContextId,
        source: crate::viewport::ImguiViewportRuntimeError,
    },
    /// Native multi-viewport has no live application window to host this Context's main viewport.
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    PlatformHostUnavailable { context_id: ContextId },
    /// A foreign integration already owns a backend field Bevy needs.
    BackendOwnershipConflict {
        context_id: ContextId,
        field: &'static str,
    },
    /// Native multi-viewport was requested, but this build cannot provide native windows.
    NativeMultiViewportUnavailable { context_id: ContextId },
    /// Core Context construction failed.
    ContextCreation(dear_imgui_rs::ImGuiError),
    /// The registry exhausted its monotonic slot generations.
    ContextGenerationExhausted,
    /// Managed removal was requested before the plugin installed its retirement queue.
    RetirementQueueUnavailable { context_id: ContextId },
    /// Context removal cannot begin or finish while backend-owned state is still live.
    RemovalPending {
        context_id: ContextId,
        reason: super::ImguiContextRemovalPendingReason,
    },
}

impl ImguiContextError {
    pub(crate) fn from_scoped_activation<E>(
        context_id: ContextId,
        error: dear_imgui_rs::ScopedActivationError<E>,
        map_closure: impl FnOnce(E) -> Self,
    ) -> Self {
        match super::backend_contract::separate_scoped_error(error) {
            Ok(source) => Self::ScopedActivation { context_id, source },
            Err(error) => map_closure(error),
        }
    }
}

impl fmt::Display for ImguiContextError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AppTerminated => formatter
                .write_str("the Dear ImGui integration is terminal after explicit App shutdown"),
            Self::PassRegistry(error) => error.fmt(formatter),
            Self::ContextRegistryAlreadyInstalled => {
                formatter.write_str("the Bevy App already owns a Dear ImGui Context registry")
            }
            Self::UnknownContext { context_id } => {
                write!(formatter, "unknown Dear ImGui Context {context_id:?}")
            }
            Self::AlreadyRegistered { context_id } => {
                write!(
                    formatter,
                    "Dear ImGui Context {context_id:?} is already registered"
                )
            }
            Self::DuplicatePass { pass, owner } => {
                write!(
                    formatter,
                    "Dear ImGui pass {pass} is already owned by Context {owner:?}"
                )
            }
            Self::ForeignPass { pass } => {
                write!(
                    formatter,
                    "Dear ImGui pass {pass} belongs to another Bevy App"
                )
            }
            Self::TeardownInProgress { context_id } => {
                write!(formatter, "Context {context_id:?} teardown is in progress")
            }
            Self::RawMutationWhileFrameOpen { context_id } => write!(
                formatter,
                "Context {context_id:?} cannot be configured while its Ui is live"
            ),
            Self::ScopedActivation { context_id, source } => write!(
                formatter,
                "Context {context_id:?} could not complete an active access scope: {source}"
            ),
            Self::FontAtlasMode { context_id, source } => write!(
                formatter,
                "Context {context_id:?} cannot enter legacy font-atlas mode: {source}"
            ),
            #[cfg(feature = "render")]
            Self::RendererAdmission { context_id, source } => write!(
                formatter,
                "Context {context_id:?} cannot enter managed renderer mode: {source}"
            ),
            #[cfg(feature = "render")]
            Self::RendererRecovery { context_id, source } => write!(
                formatter,
                "Context {context_id:?} could not recover its managed renderer state: {source}"
            ),
            #[cfg(feature = "render")]
            Self::RendererOwnership { context_id, source } => {
                write!(
                    formatter,
                    "Context {context_id:?} stopped because renderer ownership changed: {source}"
                )
            }
            #[cfg(feature = "render")]
            Self::RendererCompletion { context_id, source } => write!(
                formatter,
                "Context {context_id:?} stopped because snapshot completion failed: {source}"
            ),
            #[cfg(feature = "render")]
            Self::SnapshotCommit { context_id, source } => write!(
                formatter,
                "Context {context_id:?} stopped because its render-world snapshot outcome could not be committed: {source}"
            ),
            #[cfg(feature = "render")]
            Self::SnapshotCapture { context_id, source } => write!(
                formatter,
                "Context {context_id:?} stopped because its frame snapshot could not be captured: {source}"
            ),
            #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
            Self::ViewportBridge { context_id, source } => write!(
                formatter,
                "Context {context_id:?} stopped because its native viewport bridge failed: {source}"
            ),
            #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
            Self::PlatformHostUnavailable { context_id } => write!(
                formatter,
                "Context {context_id:?} has no live window route for its native platform main viewport"
            ),
            Self::BackendOwnershipConflict { context_id, field } => write!(
                formatter,
                "Context {context_id:?} backend field `{field}` is owned by another integration"
            ),
            Self::NativeMultiViewportUnavailable { context_id } => write!(
                formatter,
                "Context {context_id:?} requests native multi-viewport, but this build cannot provide native windows"
            ),
            Self::ContextCreation(error) => error.fmt(formatter),
            Self::ContextGenerationExhausted => {
                formatter.write_str("Dear ImGui Context registry generations are exhausted")
            }
            Self::RetirementQueueUnavailable { context_id } => write!(
                formatter,
                "Context {context_id:?} cannot enter managed retirement before ImguiPlugin installation"
            ),
            Self::RemovalPending { context_id, reason } => write!(
                formatter,
                "Context {context_id:?} removal cannot proceed yet: {reason}"
            ),
        }
    }
}

impl std::error::Error for ImguiContextError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            #[cfg(feature = "render")]
            Self::RendererAdmission { source, .. } | Self::RendererRecovery { source, .. } => {
                Some(source)
            }
            #[cfg(feature = "render")]
            Self::RendererOwnership { source, .. } => Some(source),
            #[cfg(feature = "render")]
            Self::RendererCompletion { source, .. } => Some(source),
            #[cfg(feature = "render")]
            Self::SnapshotCommit { source, .. } => Some(source),
            #[cfg(feature = "render")]
            Self::SnapshotCapture { source, .. } => Some(source),
            #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
            Self::ViewportBridge { source, .. } => Some(source),
            Self::ScopedActivation { source, .. } => Some(source),
            Self::FontAtlasMode { source, .. } => Some(source),
            Self::ContextCreation(error) => Some(error),
            Self::PassRegistry(error) => Some(error),
            Self::RemovalPending { reason, .. } => Some(reason),
            _ => None,
        }
    }
}

/// Admission failure that returns ownership of the rejected suspended Context.
pub struct ImguiContextAdmissionError {
    inner: Box<ImguiContextAdmissionFailure>,
}

struct ImguiContextAdmissionFailure {
    error: ImguiContextError,
    context: SuspendedContext,
}

impl ImguiContextAdmissionError {
    /// Borrow the typed admission error.
    #[must_use]
    pub fn error(&self) -> &ImguiContextError {
        &self.inner.error
    }

    /// Recover the rejected suspended Context.
    #[must_use]
    pub fn into_context(self) -> SuspendedContext {
        let ImguiContextAdmissionFailure { context, .. } = *self.inner;
        context
    }

    pub(crate) fn new(error: ImguiContextError, context: SuspendedContext) -> Self {
        Self {
            inner: Box::new(ImguiContextAdmissionFailure { error, context }),
        }
    }

    fn into_error(self) -> ImguiContextError {
        let ImguiContextAdmissionFailure { error, .. } = *self.inner;
        error
    }
}

impl fmt::Debug for ImguiContextAdmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ImguiContextAdmissionError")
            .field("error", &self.inner.error)
            .finish_non_exhaustive()
    }
}

impl fmt::Display for ImguiContextAdmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.error.fmt(formatter)
    }
}

impl std::error::Error for ImguiContextAdmissionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.inner.error)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ContextSlotState {
    Ready,
    Driving,
    Teardown,
}

pub(crate) struct ContextSlot {
    pub(crate) config: ImguiContextConfig,
    pub(crate) owner: Option<ContextOwner>,
    generation: NonZeroU64,
    retirement: Option<ImguiContextRetirementId>,
    pub(crate) frame_index: u64,
    pub(crate) state: ContextSlotState,
    pub(crate) last_error: Option<ImguiContextError>,
}

/// Main-thread registry that owns all Bevy-managed Dear ImGui Contexts.
///
/// Slots retain Contexts as [`SuspendedContext`] owners. The private driver removes one owner at a
/// time before activating it, so running a private pass never overlaps a registry borrow with a
/// live Dear ImGui `Ui`.
pub struct ImguiContexts {
    lifecycle: ImguiAppLifecycle,
    pass_registry_id: u64,
    primary: Option<ContextId>,
    slots: HashMap<ContextId, ContextSlot>,
    order: Vec<ContextId>,
    pass_owners: HashMap<super::pass::PassKey, ContextId>,
    backend: Option<super::BackendAttachment>,
    retirement_sink: Option<ImguiContextRetirementSink>,
    next_slot_generation: u64,
}

impl ImguiContexts {
    pub(crate) fn with_primary(
        primary: SuspendedContext,
        pass: ImguiPass<ImguiPrimaryPass>,
        lifecycle: ImguiAppLifecycle,
    ) -> Self {
        assert!(
            !lifecycle.is_terminal(),
            "the Dear ImGui App lifecycle is terminal"
        );
        assert!(
            lifecycle.try_claim_context_registry(),
            "the Bevy App already created its Dear ImGui Context registry"
        );
        let primary_id = primary.id();
        let config = ImguiContextConfig::primary(&pass);
        let pass = config.pass;
        let mut slots = HashMap::new();
        slots.insert(
            primary_id,
            ContextSlot {
                config,
                owner: Some(ContextOwner::new(primary)),
                generation: NonZeroU64::MIN,
                retirement: None,
                frame_index: 0,
                state: ContextSlotState::Ready,
                last_error: None,
            },
        );
        Self {
            lifecycle,
            pass_registry_id: pass.registry_id(),
            primary: Some(primary_id),
            slots,
            order: vec![primary_id],
            pass_owners: HashMap::from([(pass.key(), primary_id)]),
            backend: None,
            retirement_sink: None,
            next_slot_generation: 2,
        }
    }

    /// Return the primary Context identity.
    ///
    /// Explicit App shutdown makes the retained registry terminal, so callers cannot confuse a
    /// completed lifecycle with an active registry that temporarily has no primary Context.
    /// Returns [`ImguiContextError::AppTerminated`] in that terminal state.
    #[must_use]
    pub fn primary_id(&self) -> Result<Option<ContextId>, ImguiContextError> {
        self.ensure_active()?;
        Ok(self.primary)
    }

    /// Select an existing idle Context as the primary input and fallback-window target.
    ///
    /// Pass ownership and per-Context docking or viewport configuration stay with each Context.
    /// The registry changes `primary` only after every precondition succeeds.
    pub fn promote_primary(
        &mut self,
        context_id: ContextId,
    ) -> Result<ImguiPrimaryChange, ImguiContextError> {
        self.ensure_active()?;
        if let Some(active) = self.driving_context() {
            return Err(ImguiContextError::RawMutationWhileFrameOpen { context_id: active });
        }
        let slot = self
            .slots
            .get(&context_id)
            .ok_or(ImguiContextError::UnknownContext { context_id })?;
        if slot.state != ContextSlotState::Ready {
            return Err(ImguiContextError::TeardownInProgress { context_id });
        }
        let previous = self.primary.replace(context_id);
        Ok(ImguiPrimaryChange {
            previous,
            current: context_id,
        })
    }

    /// Iterate registered Context identities in deterministic registry order.
    ///
    /// A managed-retirement tombstone remains visible until its matching
    /// [`ImguiContextRetired`] completion is emitted, although the private driver no longer opens
    /// frames for it.
    ///
    /// Returns [`ImguiContextError::AppTerminated`] after explicit App shutdown.
    pub fn ids(&self) -> Result<impl ExactSizeIterator<Item = ContextId> + '_, ImguiContextError> {
        self.ensure_active()?;
        Ok(self.order.iter().copied())
    }

    /// Return whether this registry recognizes `context_id`.
    ///
    /// Returns [`ImguiContextError::AppTerminated`] after explicit App shutdown.
    #[must_use]
    pub fn contains(&self, context_id: ContextId) -> Result<bool, ImguiContextError> {
        self.ensure_active()?;
        Ok(self.slots.contains_key(&context_id))
    }

    /// Return a Context's latest completed frame index.
    pub fn frame_index(&self, context_id: ContextId) -> Result<u64, ImguiContextError> {
        self.ensure_active()?;
        self.slots
            .get(&context_id)
            .map(|slot| slot.frame_index)
            .ok_or(ImguiContextError::UnknownContext { context_id })
    }

    /// Borrow the latest non-panic driver error for one Context.
    pub fn last_error(
        &self,
        context_id: ContextId,
    ) -> Result<Option<&ImguiContextError>, ImguiContextError> {
        self.ensure_active()?;
        self.slots
            .get(&context_id)
            .map(|slot| slot.last_error.as_ref())
            .ok_or(ImguiContextError::UnknownContext { context_id })
    }

    /// Create and insert an independent suspended Context.
    pub fn create(&mut self, config: ImguiContextConfig) -> Result<ContextId, ImguiContextError> {
        self.ensure_active()?;
        if let Some(active) = self.driving_context() {
            return Err(ImguiContextError::RawMutationWhileFrameOpen { context_id: active });
        }
        let context = SuspendedContext::try_create().map_err(ImguiContextError::ContextCreation)?;
        self.insert_suspended(context, config)
            .map_err(ImguiContextAdmissionError::into_error)
    }

    /// Insert an existing suspended Context without sharing a managed font atlas implicitly.
    pub fn insert_suspended(
        &mut self,
        context: SuspendedContext,
        config: ImguiContextConfig,
    ) -> Result<ContextId, ImguiContextAdmissionError> {
        if let Err(error) = self.ensure_active() {
            return Err(ImguiContextAdmissionError::new(error, context));
        }
        if let Some(active) = self.driving_context() {
            return Err(ImguiContextAdmissionError::new(
                ImguiContextError::RawMutationWhileFrameOpen { context_id: active },
                context,
            ));
        }
        if config.pass.registry_id() != self.pass_registry_id {
            return Err(ImguiContextAdmissionError::new(
                ImguiContextError::ForeignPass {
                    pass: config.pass.brand_name(),
                },
                context,
            ));
        }
        if let Some(owner) = self.pass_owners.get(&config.pass.key()).copied() {
            return Err(ImguiContextAdmissionError::new(
                ImguiContextError::DuplicatePass {
                    pass: config.pass.brand_name(),
                    owner,
                },
                context,
            ));
        }
        let context_id = context.id();
        if self.slots.contains_key(&context_id) {
            return Err(ImguiContextAdmissionError::new(
                ImguiContextError::AlreadyRegistered { context_id },
                context,
            ));
        }

        let generation = match self.allocate_slot_generation() {
            Ok(generation) => generation,
            Err(error) => return Err(ImguiContextAdmissionError::new(error, context)),
        };

        let mut owner = ContextOwner::new(context);
        if let Some(sink) = self.retirement_sink.as_ref() {
            owner.set_retirement_sink(sink.clone());
        }
        if let Some(backend) = self.backend.as_ref()
            && let Err(error) = owner.attach_backend(backend, &config)
        {
            let context = match owner.into_unattached_context() {
                Ok(context) => context,
                Err(_) => {
                    panic!("failed admission mutated backend ownership before returning an error")
                }
            };
            return Err(ImguiContextAdmissionError::new(error, context));
        }

        self.pass_owners.insert(config.pass.key(), context_id);
        self.order.push(context_id);
        self.slots.insert(
            context_id,
            ContextSlot {
                config,
                owner: Some(owner),
                generation,
                retirement: None,
                frame_index: 0,
                state: ContextSlotState::Ready,
                last_error: None,
            },
        );
        Ok(context_id)
    }

    /// Admit a suspended Context and select it as primary in one transaction.
    ///
    /// The previous primary remains registered under its existing pass. If admission fails, the
    /// registry is unchanged and the returned [`ImguiContextAdmissionError`] retains `context`.
    pub fn replace_primary(
        &mut self,
        context: SuspendedContext,
        config: ImguiContextConfig,
    ) -> Result<ImguiPrimaryChange, ImguiContextAdmissionError> {
        if let Err(error) = self.ensure_active() {
            return Err(ImguiContextAdmissionError::new(error, context));
        }
        if let Some(active) = self.driving_context() {
            return Err(ImguiContextAdmissionError::new(
                ImguiContextError::RawMutationWhileFrameOpen { context_id: active },
                context,
            ));
        }
        let current = context.id();
        let previous = self.primary;
        self.insert_suspended(context, config)?;
        self.primary = Some(current);
        Ok(ImguiPrimaryChange { previous, current })
    }

    /// Run an outside-frame configuration closure against one Context.
    ///
    /// A Context whose immediate removal is pending remains configurable so the integration that
    /// changed an owned backend field can restore it before [`Self::try_remove_immediately`] is
    /// retried. A Context transferred to managed retirement cannot be configured.
    pub fn configure<T>(
        &mut self,
        context_id: ContextId,
        configure: impl FnOnce(&mut Context) -> T,
    ) -> Result<T, ImguiContextError> {
        self.ensure_active()?;
        if let Some(active) = self.driving_context() {
            return Err(ImguiContextError::RawMutationWhileFrameOpen { context_id: active });
        }
        let slot = self
            .slots
            .get_mut(&context_id)
            .ok_or(ImguiContextError::UnknownContext { context_id })?;
        match slot.state {
            ContextSlotState::Driving => {
                return Err(ImguiContextError::RawMutationWhileFrameOpen { context_id });
            }
            ContextSlotState::Ready | ContextSlotState::Teardown => {}
        }
        let owner = slot
            .owner
            .as_mut()
            .ok_or(ImguiContextError::TeardownInProgress { context_id })?;
        owner
            .try_with_active_context(|context| {
                Ok::<_, std::convert::Infallible>(configure(context))
            })
            .map_err(|error| {
                ImguiContextError::from_scoped_activation(context_id, error, |never| match never {})
            })
    }

    /// Request managed removal of one Context.
    ///
    /// The request transfers the complete Context owner to the plugin's retirement queue and
    /// returns immediately. Repeating the request while retirement is pending returns the same
    /// generation-qualified identity. Observe [`ImguiContextRetired`] for the one-shot completion;
    /// once accepted, applications do not need to poll or retry this method. A repairable
    /// backend-ownership conflict is returned before transfer, so [`Self::configure`] remains
    /// available.
    pub fn remove(
        &mut self,
        context_id: ContextId,
    ) -> Result<ImguiContextRetirementId, ImguiContextError> {
        self.ensure_active()?;
        if let Some(active) = self.driving_context() {
            return Err(ImguiContextError::RawMutationWhileFrameOpen { context_id: active });
        }
        if let Some(retirement) = self.slots.get(&context_id).and_then(|slot| slot.retirement) {
            return Ok(retirement);
        }
        if !self.slots.contains_key(&context_id) {
            return Err(ImguiContextError::UnknownContext { context_id });
        }
        let sink = self
            .retirement_sink
            .clone()
            .ok_or(ImguiContextError::RetirementQueueUnavailable { context_id })?;
        let (retirement, owner, previous_state) = {
            let slot = self
                .slots
                .get_mut(&context_id)
                .ok_or(ImguiContextError::UnknownContext { context_id })?;
            if slot.state == ContextSlotState::Driving {
                return Err(ImguiContextError::RawMutationWhileFrameOpen { context_id });
            }
            slot.owner
                .as_mut()
                .expect("an unqueued Context slot must retain its owner")
                .preflight_backend_detach()
                .map_err(|reason| ImguiContextError::RemovalPending { context_id, reason })?;
            let retirement = ImguiContextRetirementId::new(context_id, slot.generation);
            let previous_state = slot.state;
            slot.state = ContextSlotState::Teardown;
            let owner = slot
                .owner
                .take()
                .expect("an unqueued Context slot must retain its owner");
            (retirement, owner, previous_state)
        };
        match sink.try_enqueue(owner, retirement) {
            Ok(()) => {
                self.slots
                    .get_mut(&context_id)
                    .expect("a queued Context slot must remain registered")
                    .retirement = Some(retirement);
                if self.primary == Some(context_id) {
                    self.primary = None;
                }
                Ok(retirement)
            }
            Err(owner) => {
                let slot = self
                    .slots
                    .get_mut(&context_id)
                    .expect("a failed retirement enqueue must retain its slot");
                slot.owner = Some(*owner);
                slot.state = previous_state;
                Err(ImguiContextError::RetirementQueueUnavailable { context_id })
            }
        }
    }

    /// Attempt synchronous teardown and return the suspended Context to the caller.
    ///
    /// This is an advanced retry-oriented escape hatch for integrations that must recover the
    /// native Context owner. A pending renderer or viewport acknowledgement returns
    /// [`ImguiContextError::RemovalPending`]; the caller must advance the relevant Bevy schedules
    /// before retrying. Normal applications should call [`Self::remove`] once instead.
    pub fn try_remove_immediately(
        &mut self,
        context_id: ContextId,
    ) -> Result<SuspendedContext, ImguiContextError> {
        self.ensure_active()?;
        if let Some(active) = self.driving_context() {
            return Err(ImguiContextError::RawMutationWhileFrameOpen { context_id: active });
        }
        let slot = self
            .slots
            .get_mut(&context_id)
            .ok_or(ImguiContextError::UnknownContext { context_id })?;
        if slot.retirement.is_some() {
            return Err(ImguiContextError::TeardownInProgress { context_id });
        }
        if slot.state == ContextSlotState::Driving {
            return Err(ImguiContextError::RawMutationWhileFrameOpen { context_id });
        }
        slot.state = ContextSlotState::Teardown;
        let owner = slot
            .owner
            .as_mut()
            .expect("a teardown Context slot must retain its owner");
        if let Err(reason) = owner.try_detach_backend() {
            return Err(ImguiContextError::RemovalPending { context_id, reason });
        }

        let slot = self.remove_slot(context_id);
        Ok(slot
            .owner
            .expect("removed Context slot must retain its owner")
            .into_suspended())
    }

    /// Validate every registered Context before terminal shutdown commits world changes.
    ///
    /// The first failure is returned in deterministic drive order, but every Context is checked so
    /// this phase remains a complete, side-effect-free transaction preflight.
    pub(crate) fn preflight_backend_detach(
        &mut self,
    ) -> Result<(), (ContextId, ImguiContextRemovalPendingReason)> {
        let mut first_failure = None;
        for context_id in self.order.clone() {
            let slot = self
                .slots
                .get_mut(&context_id)
                .expect("drive order must reference a registered Context");
            debug_assert_ne!(slot.state, ContextSlotState::Driving);
            let Some(owner) = slot.owner.as_mut() else {
                debug_assert!(slot.retirement.is_some());
                continue;
            };
            if let Err(reason) = owner.preflight_backend_detach()
                && first_failure.is_none()
            {
                first_failure = Some((context_id, reason));
            }
        }
        first_failure.map_or(Ok(()), Err)
    }

    pub(crate) fn complete_retirement(&mut self, retirement: ImguiContextRetirementId) -> bool {
        let Some(slot) = self.slots.get(&retirement.context_id) else {
            return false;
        };
        if slot.retirement != Some(retirement) {
            return false;
        }
        let slot = self.remove_slot(retirement.context_id);
        debug_assert!(slot.owner.is_none());
        true
    }

    #[cfg(feature = "render")]
    pub(crate) fn is_tearing_down(&self, context_id: ContextId) -> bool {
        self.slots
            .get(&context_id)
            .is_some_and(|slot| slot.state == ContextSlotState::Teardown)
    }

    pub(crate) fn drive_order(&self) -> Vec<ContextId> {
        if self.lifecycle.is_terminal() {
            return Vec::new();
        }
        self.order.clone()
    }

    pub(crate) const fn pass_registry_id(&self) -> u64 {
        self.pass_registry_id
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    pub(crate) fn native_viewport_context_ids(&self) -> Vec<ContextId> {
        if self.lifecycle.is_terminal() {
            return Vec::new();
        }
        self.order
            .iter()
            .copied()
            .filter(|context_id| {
                self.slots.get(context_id).is_some_and(|slot| {
                    slot.state == ContextSlotState::Ready && slot.config.multi_viewport()
                })
            })
            .collect()
    }

    fn driving_context(&self) -> Option<ContextId> {
        self.order.iter().copied().find(|context_id| {
            self.slots
                .get(context_id)
                .is_some_and(|slot| slot.state == ContextSlotState::Driving)
        })
    }

    fn ensure_active(&self) -> Result<(), ImguiContextError> {
        if self.lifecycle.is_terminal() {
            Err(ImguiContextError::AppTerminated)
        } else {
            Ok(())
        }
    }

    pub(crate) fn take_for_drive(
        &mut self,
        context_id: ContextId,
    ) -> Result<(ContextOwner, ImguiContextConfig, u64), ImguiContextError> {
        self.ensure_active()?;
        let slot = self
            .slots
            .get_mut(&context_id)
            .ok_or(ImguiContextError::UnknownContext { context_id })?;
        match slot.state {
            ContextSlotState::Ready => {}
            ContextSlotState::Driving => {
                return Err(ImguiContextError::RawMutationWhileFrameOpen { context_id });
            }
            ContextSlotState::Teardown => {
                return Err(ImguiContextError::TeardownInProgress { context_id });
            }
        }
        slot.state = ContextSlotState::Driving;
        let next_frame = slot.frame_index.saturating_add(1);
        let owner = slot
            .owner
            .take()
            .expect("a ready Context slot must retain its owner");
        Ok((owner, slot.config.clone(), next_frame))
    }

    #[cfg(feature = "render")]
    pub(crate) fn recover_renderer(
        &mut self,
        context_id: ContextId,
    ) -> Result<(), ImguiContextError> {
        let slot = self
            .slots
            .get_mut(&context_id)
            .ok_or(ImguiContextError::UnknownContext { context_id })?;
        if slot.state != ContextSlotState::Ready {
            return Err(ImguiContextError::TeardownInProgress { context_id });
        }
        let owner = slot
            .owner
            .as_mut()
            .expect("a ready Context slot must retain its owner");
        match owner.try_recover_renderer() {
            Ok(()) => Ok(()),
            Err(super::ImguiActiveRendererContextError::Operation(source)) => {
                Err(ImguiContextError::RendererRecovery { context_id, source })
            }
            Err(super::ImguiActiveRendererContextError::ContextScope(source)) => {
                Err(ImguiContextError::ScopedActivation { context_id, source })
            }
            Err(super::ImguiActiveRendererContextError::RendererOwnership(source)) => {
                slot.state = ContextSlotState::Teardown;
                Err(ImguiContextError::RendererOwnership { context_id, source })
            }
            #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
            Err(super::ImguiActiveRendererContextError::ViewportBridge(source)) => {
                slot.state = ContextSlotState::Teardown;
                Err(ImguiContextError::ViewportBridge { context_id, source })
            }
        }
    }

    pub(crate) fn finish_drive(
        &mut self,
        context_id: ContextId,
        owner: ContextOwner,
        completed_frame: Option<u64>,
        error: Option<ImguiContextError>,
    ) {
        let slot = self
            .slots
            .get_mut(&context_id)
            .expect("a driven Context slot must remain registered");
        debug_assert_eq!(slot.state, ContextSlotState::Driving);
        debug_assert!(slot.owner.is_none());
        if let Some(frame_index) = completed_frame {
            slot.frame_index = frame_index;
        }
        #[cfg(feature = "render")]
        let renderer_contract_failed = matches!(
            error.as_ref(),
            Some(ImguiContextError::RendererOwnership { .. })
                | Some(ImguiContextError::RendererCompletion { .. })
                | Some(ImguiContextError::SnapshotCommit { .. })
        );
        #[cfg(not(feature = "render"))]
        let renderer_contract_failed = false;
        #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
        let viewport_bridge_failed = matches!(
            error.as_ref(),
            Some(ImguiContextError::ViewportBridge { .. })
        );
        #[cfg(not(all(feature = "multi-viewport", not(target_arch = "wasm32"))))]
        let viewport_bridge_failed = false;
        let backend_ownership_lost = renderer_contract_failed || viewport_bridge_failed;
        slot.last_error = error;
        slot.owner = Some(owner);
        slot.state = if backend_ownership_lost {
            ContextSlotState::Teardown
        } else {
            ContextSlotState::Ready
        };
    }

    #[cfg(feature = "render")]
    pub(crate) fn record_renderer_completion_error(
        &mut self,
        context_id: ContextId,
        source: dear_imgui_rs::render::RendererConsumerError,
    ) {
        let slot = self
            .slots
            .get_mut(&context_id)
            .expect("a polled Context must remain registered");
        debug_assert_ne!(slot.state, ContextSlotState::Driving);
        slot.last_error = Some(ImguiContextError::RendererCompletion { context_id, source });
        slot.state = ContextSlotState::Teardown;
    }

    fn allocate_slot_generation(&mut self) -> Result<NonZeroU64, ImguiContextError> {
        let generation = NonZeroU64::new(self.next_slot_generation)
            .ok_or(ImguiContextError::ContextGenerationExhausted)?;
        self.next_slot_generation = self.next_slot_generation.checked_add(1).unwrap_or(0);
        Ok(generation)
    }

    fn remove_slot(&mut self, context_id: ContextId) -> ContextSlot {
        let slot = self
            .slots
            .remove(&context_id)
            .expect("the validated Context slot must still exist");
        self.order.retain(|candidate| *candidate != context_id);
        self.pass_owners.remove(&slot.config.pass.key());
        if self.primary == Some(context_id) {
            self.primary = None;
        }
        slot
    }

    #[cfg(feature = "render")]
    pub(crate) fn record_snapshot_commit_error(
        &mut self,
        context_id: ContextId,
        source: dear_imgui_rs::render::snapshot::SnapshotCommitError,
    ) {
        let Some(slot) = self.slots.get_mut(&context_id) else {
            return;
        };
        debug_assert_ne!(slot.state, ContextSlotState::Driving);
        slot.last_error = Some(ImguiContextError::SnapshotCommit { context_id, source });
        slot.state = ContextSlotState::Teardown;
    }

    pub(crate) fn set_retirement_sink(&mut self, sink: ImguiContextRetirementSink) {
        for slot in self.slots.values_mut() {
            if let Some(owner) = slot.owner.as_mut() {
                owner.set_retirement_sink(sink.clone());
            }
        }
        self.retirement_sink = Some(sink);
    }

    pub(crate) fn set_primary_contract(&mut self, docking: bool, multi_viewport: bool) {
        if let Some(primary) = self.primary
            && let Some(slot) = self.slots.get_mut(&primary)
        {
            slot.config.docking = docking;
            slot.config.multi_viewport = multi_viewport;
        }
    }

    pub(crate) fn preflight_backend_attachment(
        &mut self,
        backend: &super::BackendAttachment,
        primary_contract: Option<(bool, bool)>,
    ) -> Result<(), ImguiContextError> {
        self.ensure_active()?;
        if let Some(active) = self.driving_context() {
            return Err(ImguiContextError::RawMutationWhileFrameOpen { context_id: active });
        }
        for context_id in &self.order {
            let slot = self
                .slots
                .get_mut(context_id)
                .expect("drive order must reference a registered Context");
            let Some(owner) = slot.owner.as_mut() else {
                debug_assert!(slot.retirement.is_some());
                continue;
            };
            let mut config = slot.config.clone();
            if self.primary == Some(*context_id)
                && let Some((docking, multi_viewport)) = primary_contract
            {
                config.docking = docking;
                config.multi_viewport = multi_viewport;
            }
            owner.preflight_backend_attachment(backend, &config)?;
        }
        for context_id in &self.order {
            let slot = self
                .slots
                .get_mut(context_id)
                .expect("drive order must reference a registered Context");
            let Some(owner) = slot.owner.as_mut() else {
                debug_assert!(slot.retirement.is_some());
                continue;
            };
            owner.preflight_renderer_admission(backend)?;
        }
        Ok(())
    }

    pub(crate) fn attach_backend(
        &mut self,
        backend: super::BackendAttachment,
    ) -> Result<(), ImguiContextError> {
        self.preflight_backend_attachment(&backend, None)?;
        #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
        for context_id in &self.order {
            let slot = self
                .slots
                .get_mut(context_id)
                .expect("drive order must reference a registered Context");
            let Some(owner) = slot.owner.as_mut() else {
                debug_assert!(slot.retirement.is_some());
                continue;
            };
            if !slot.config.multi_viewport() {
                continue;
            }
            let registration = backend
                .viewport_bridge_registration
                .as_ref()
                .expect("multi-viewport preflight must provide a bridge registration");
            owner.attach_context_viewport_bridge(registration)?;
        }
        for context_id in &self.order {
            let slot = self
                .slots
                .get_mut(context_id)
                .expect("drive order must reference a registered Context");
            let Some(owner) = slot.owner.as_mut() else {
                debug_assert!(slot.retirement.is_some());
                continue;
            };
            owner.commit_renderer_admission(&backend)?;
        }
        for context_id in &self.order {
            let slot = self
                .slots
                .get_mut(context_id)
                .expect("drive order must reference a registered Context");
            let Some(owner) = slot.owner.as_mut() else {
                debug_assert!(slot.retirement.is_some());
                continue;
            };
            owner.commit_backend_attachment(&backend, &slot.config)?;
        }
        self.backend = Some(backend);
        Ok(())
    }
}

#[cfg(test)]
mod retirement_generation_tests {
    use super::*;
    use crate::test_util::imgui_context_guard;

    #[test]
    fn stale_retirement_completion_cannot_remove_a_reused_slot_generation() {
        let _guard = imgui_context_guard();
        let mut app = bevy_app::App::new();
        let primary_pass = super::super::pass::primary_pass(&mut app).unwrap();
        let lifecycle = super::super::pass::lifecycle(app.world());
        let primary = SuspendedContext::create();
        let context_id = primary.id();
        let mut contexts = ImguiContexts::with_primary(primary, primary_pass, lifecycle);
        let stale = ImguiContextRetirementId::new(context_id, NonZeroU64::MIN);
        let current = ImguiContextRetirementId::new(context_id, NonZeroU64::new(2).unwrap());
        let slot = contexts
            .slots
            .get_mut(&context_id)
            .expect("the primary slot must exist");
        slot.generation = NonZeroU64::new(2).unwrap();
        slot.retirement = Some(current);

        assert!(!contexts.complete_retirement(stale));
        assert!(contexts.contains(context_id).unwrap());
        assert_eq!(
            contexts
                .slots
                .get(&context_id)
                .and_then(|slot| slot.retirement),
            Some(current)
        );
    }
}
