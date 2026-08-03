use std::collections::HashMap;
use std::fmt;

use dear_imgui_rs::{Context, ContextId, SuspendedContext};

use super::ownership::{ContextOwner, ImguiContextRetirementSink};
use super::{ImguiPass, ImguiPrimaryPass, PassIdentity};

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
    pub fn new<P: 'static>(pass: ImguiPass<P>) -> Self {
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

    pub(crate) fn primary(pass: ImguiPass<ImguiPrimaryPass>) -> Self {
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
    /// The legacy/headless font atlas could not be built before opening a frame.
    FontAtlasBuildFailed { context_id: ContextId },
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
        source: super::ownership::ImguiRendererOwnershipError,
    },
    /// Detached snapshot completion failed for this Context.
    #[cfg(feature = "render")]
    RendererCompletion {
        context_id: ContextId,
        source: dear_imgui_rs::render::RendererConsumerError,
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
    /// Context removal has begun but backend-owned work is still live.
    RemovalPending {
        context_id: ContextId,
        reason: super::ownership::ImguiContextRemovalPendingReason,
    },
}

impl fmt::Display for ImguiContextError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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
            Self::FontAtlasBuildFailed { context_id } => {
                write!(
                    formatter,
                    "Context {context_id:?} could not build its legacy font atlas"
                )
            }
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
            Self::RemovalPending { context_id, reason } => {
                write!(
                    formatter,
                    "Context {context_id:?} removal is pending: {reason}"
                )
            }
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
            Self::SnapshotCapture { source, .. } => Some(source),
            #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
            Self::ViewportBridge { source, .. } => Some(source),
            Self::ContextCreation(error) => Some(error),
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
    pass_registry_id: u64,
    primary: Option<ContextId>,
    slots: HashMap<ContextId, ContextSlot>,
    order: Vec<ContextId>,
    pass_owners: HashMap<super::pass::PassKey, ContextId>,
    backend: Option<super::ownership::BackendAttachment>,
    retirement_sink: Option<ImguiContextRetirementSink>,
}

impl ImguiContexts {
    /// Create a registry adopting `primary` as the primary Context.
    #[must_use]
    pub fn with_primary(primary: SuspendedContext, pass: ImguiPass<ImguiPrimaryPass>) -> Self {
        let primary_id = primary.id();
        let config = ImguiContextConfig::primary(pass);
        let pass = config.pass;
        let mut slots = HashMap::new();
        slots.insert(
            primary_id,
            ContextSlot {
                config,
                owner: Some(ContextOwner::new(primary)),
                frame_index: 0,
                state: ContextSlotState::Ready,
                last_error: None,
            },
        );
        Self {
            pass_registry_id: pass.registry_id(),
            primary: Some(primary_id),
            slots,
            order: vec![primary_id],
            pass_owners: HashMap::from([(pass.key(), primary_id)]),
            backend: None,
            retirement_sink: None,
        }
    }

    /// Return the primary Context identity.
    #[must_use]
    pub fn primary_id(&self) -> Option<ContextId> {
        self.primary
    }

    /// Iterate Context identities in deterministic drive order.
    pub fn ids(&self) -> impl ExactSizeIterator<Item = ContextId> + '_ {
        self.order.iter().copied()
    }

    /// Return whether this registry recognizes `context_id`.
    #[must_use]
    pub fn contains(&self, context_id: ContextId) -> bool {
        self.slots.contains_key(&context_id)
    }

    /// Return a Context's latest completed frame index.
    pub fn frame_index(&self, context_id: ContextId) -> Result<u64, ImguiContextError> {
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
        self.slots
            .get(&context_id)
            .map(|slot| slot.last_error.as_ref())
            .ok_or(ImguiContextError::UnknownContext { context_id })
    }

    /// Create and insert an independent suspended Context.
    pub fn create(&mut self, config: ImguiContextConfig) -> Result<ContextId, ImguiContextError> {
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
                frame_index: 0,
                state: ContextSlotState::Ready,
                last_error: None,
            },
        );
        Ok(context_id)
    }

    /// Run an outside-frame configuration closure against one Context.
    ///
    /// A Context whose removal is pending remains configurable so the integration that changed an
    /// owned backend field can restore or finish replacing its state before [`Self::remove`] is
    /// retried. Teardown Contexts never open another frame.
    pub fn configure<T>(
        &mut self,
        context_id: ContextId,
        configure: impl FnOnce(&mut Context) -> T,
    ) -> Result<T, ImguiContextError> {
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
            .expect("a ready Context slot must retain its owner");
        match owner.try_with_active_context(|context| {
            Ok::<_, std::convert::Infallible>(configure(context))
        }) {
            Ok(value) => Ok(value),
            Err(never) => match never {},
        }
    }

    /// Retry Context-local backend teardown and remove the Context when it is idle.
    pub fn remove(&mut self, context_id: ContextId) -> Result<SuspendedContext, ImguiContextError> {
        if let Some(active) = self.driving_context() {
            return Err(ImguiContextError::RawMutationWhileFrameOpen { context_id: active });
        }
        let slot = self
            .slots
            .get_mut(&context_id)
            .ok_or(ImguiContextError::UnknownContext { context_id })?;
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

        let slot = self
            .slots
            .remove(&context_id)
            .expect("the validated Context slot must still exist");
        self.order.retain(|candidate| *candidate != context_id);
        self.pass_owners.remove(&slot.config.pass.key());
        if self.primary == Some(context_id) {
            self.primary = None;
        }
        Ok(slot
            .owner
            .expect("removed Context slot must retain its owner")
            .into_suspended())
    }

    #[cfg(feature = "render")]
    pub(crate) fn is_tearing_down(&self, context_id: ContextId) -> bool {
        self.slots
            .get(&context_id)
            .is_some_and(|slot| slot.state == ContextSlotState::Teardown)
    }

    pub(crate) fn drive_order(&self) -> Vec<ContextId> {
        self.order.clone()
    }

    pub(crate) const fn pass_registry_id(&self) -> u64 {
        self.pass_registry_id
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    pub(crate) fn native_viewport_context_ids(&self) -> Vec<ContextId> {
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

    pub(crate) fn take_for_drive(
        &mut self,
        context_id: ContextId,
    ) -> Result<(ContextOwner, ImguiContextConfig, u64), ImguiContextError> {
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
            Err(super::ownership::ImguiActiveRendererContextError::Operation(source)) => {
                Err(ImguiContextError::RendererRecovery { context_id, source })
            }
            Err(super::ownership::ImguiActiveRendererContextError::RendererOwnership(source)) => {
                slot.state = ContextSlotState::Teardown;
                Err(ImguiContextError::RendererOwnership { context_id, source })
            }
            #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
            Err(super::ownership::ImguiActiveRendererContextError::ViewportBridge(source)) => {
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

    pub(crate) fn attach_backend(
        &mut self,
        backend: super::ownership::BackendAttachment,
    ) -> Result<(), ImguiContextError> {
        if let Some(active) = self.driving_context() {
            return Err(ImguiContextError::RawMutationWhileFrameOpen { context_id: active });
        }
        for context_id in &self.order {
            let slot = self
                .slots
                .get_mut(context_id)
                .expect("drive order must reference a registered Context");
            slot.owner
                .as_mut()
                .expect("backend attachment requires idle Context owners")
                .preflight_backend_attachment(&backend, &slot.config)?;
        }
        for context_id in &self.order {
            let slot = self
                .slots
                .get_mut(context_id)
                .expect("drive order must reference a registered Context");
            let owner = slot
                .owner
                .as_mut()
                .expect("renderer admission requires idle Context owners");
            owner.preflight_renderer_admission(&backend)?;
        }
        #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
        for context_id in &self.order {
            let slot = self
                .slots
                .get_mut(context_id)
                .expect("drive order must reference a registered Context");
            if !slot.config.multi_viewport() {
                continue;
            }
            let registration = backend
                .viewport_bridge_registration
                .as_ref()
                .expect("multi-viewport preflight must provide a bridge registration");
            slot.owner
                .as_mut()
                .expect("backend attachment requires idle Context owners")
                .attach_context_viewport_bridge(registration)?;
        }
        for context_id in &self.order {
            let slot = self
                .slots
                .get_mut(context_id)
                .expect("drive order must reference a registered Context");
            slot.owner
                .as_mut()
                .expect("renderer admission requires idle Context owners")
                .commit_renderer_admission(&backend);
        }
        for context_id in &self.order {
            let slot = self
                .slots
                .get_mut(context_id)
                .expect("drive order must reference a registered Context");
            slot.owner
                .as_mut()
                .expect("backend attachment requires idle Context owners")
                .commit_backend_attachment(&backend, &slot.config)?;
        }
        self.backend = Some(backend);
        Ok(())
    }
}
