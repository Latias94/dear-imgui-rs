//! Main-thread ownership of one suspended Dear ImGui Context.

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use std::rc::Rc;

#[cfg(feature = "render")]
use crate::render;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use crate::viewport;

use super::backend_contract::{
    BackendAttachment, ImguiActiveRendererContextError, ImguiBackendOwnership,
    ImguiContextRemovalPendingReason, clear_backend_data, preflight_backend_context_claims,
    sync_backend_context_config,
};
#[cfg(feature = "render")]
use super::backend_contract::{
    preflight_renderer_teardown_ownership, validate_active_renderer_ownership,
    validate_renderer_teardown_ownership,
};
use super::retirement::{ContextRetirement, ImguiContextRetirementSink};
#[cfg(not(all(feature = "multi-viewport", not(target_arch = "wasm32"))))]
use super::viewport_attachment::ImguiPlatformCompletionError;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use super::viewport_attachment::{
    ImguiPlatformCompletionError, ImguiViewportBridgeLifecycle, ImguiViewportBridgeOwner,
    ImguiViewportBridgePhase, advance_viewport_drain, clear_viewport_backend_contract,
    complete_platform_frame_if_needed, finish_viewport_detach, validate_viewport_bridge,
};
use super::{ImguiContextConfig, ImguiContextError};

pub(crate) struct ContextOwner {
    pub(super) context: Option<dear_imgui_rs::SuspendedContext>,
    backend_ownership: ImguiBackendOwnership,
    #[cfg(feature = "render")]
    snapshot_mailbox: Option<super::ImguiFrameMailbox>,
    #[cfg(feature = "render")]
    renderer_consumer: Option<dear_imgui_rs::render::DetachedRendererConsumer>,
    #[cfg(feature = "render")]
    renderer_release: Option<render::ImguiRendererReleaseLease>,
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    viewport_bridge: ImguiViewportBridgeLifecycle,
    retirement_sink: Option<ImguiContextRetirementSink>,
}

impl ContextOwner {
    pub(crate) fn new(context: dear_imgui_rs::SuspendedContext) -> Self {
        Self {
            context: Some(context),
            backend_ownership: ImguiBackendOwnership::default(),
            #[cfg(feature = "render")]
            snapshot_mailbox: None,
            #[cfg(feature = "render")]
            renderer_consumer: None,
            #[cfg(feature = "render")]
            renderer_release: None,
            #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
            viewport_bridge: ImguiViewportBridgeLifecycle::default(),
            retirement_sink: None,
        }
    }

    pub(crate) fn set_retirement_sink(&mut self, sink: ImguiContextRetirementSink) {
        #[cfg(feature = "render")]
        {
            self.snapshot_mailbox = sink.snapshot_mailbox();
        }
        self.retirement_sink = Some(sink);
    }

    pub(super) fn is_unattached(&self) -> bool {
        self.backend_ownership.flags_added.is_empty()
            && self.backend_ownership.platform_name.is_none()
            && self.backend_ownership.renderer_name.is_none()
            && !self.backend_ownership.standard_draw_callbacks
            && !self.backend_ownership.viewport_contract
            && {
                #[cfg(feature = "render")]
                {
                    self.renderer_consumer.is_none() && self.renderer_release.is_none()
                }
                #[cfg(not(feature = "render"))]
                {
                    true
                }
            }
            && {
                #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
                {
                    self.viewport_bridge.is_detached()
                }
                #[cfg(not(all(feature = "multi-viewport", not(target_arch = "wasm32"))))]
                {
                    true
                }
            }
    }

    fn take_for_retirement(&mut self) -> ContextOwner {
        let sink = self.retirement_sink.clone().unwrap_or_default();
        ContextOwner {
            context: self.context.take(),
            backend_ownership: std::mem::take(&mut self.backend_ownership),
            #[cfg(feature = "render")]
            snapshot_mailbox: self.snapshot_mailbox.take(),
            #[cfg(feature = "render")]
            renderer_consumer: self.renderer_consumer.take(),
            #[cfg(feature = "render")]
            renderer_release: self.renderer_release.take(),
            #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
            viewport_bridge: std::mem::take(&mut self.viewport_bridge),
            retirement_sink: Some(sink),
        }
    }

    pub(crate) fn try_with_active_context<T, E>(
        &mut self,
        operation: impl FnOnce(&mut dear_imgui_rs::Context) -> Result<T, E>,
    ) -> Result<T, dear_imgui_rs::ScopedActivationError<E>> {
        self.context
            .as_mut()
            .expect("Context owner must retain its suspended Context")
            .try_with_active(operation)
    }

    #[cfg(all(feature = "render", test))]
    pub(crate) fn try_with_active_renderer_context<T, E>(
        &mut self,
        multi_viewport: bool,
        operation: impl FnOnce(
            &mut dear_imgui_rs::Context,
            Option<&dear_imgui_rs::render::DetachedRendererConsumer>,
        ) -> Result<T, E>,
    ) -> Result<T, E> {
        match self.try_with_active_renderer_context_checked(multi_viewport, operation) {
            Ok(value) => Ok(value),
            Err(ImguiActiveRendererContextError::Operation(error)) => Err(error),
            Err(ImguiActiveRendererContextError::ContextScope(error)) => {
                panic!("dear-imgui-bevy active Context scope failed: {error}")
            }
            Err(ImguiActiveRendererContextError::RendererOwnership(error)) => {
                panic!("dear-imgui-bevy renderer ownership changed: {error}")
            }
            #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
            Err(ImguiActiveRendererContextError::ViewportBridge(error)) => {
                panic!("dear-imgui-bevy viewport bridge failed: {error}")
            }
        }
    }

    #[cfg(feature = "render")]
    pub(crate) fn try_with_active_renderer_context_checked<T, E>(
        &mut self,
        multi_viewport: bool,
        operation: impl FnOnce(
            &mut dear_imgui_rs::Context,
            Option<&dear_imgui_rs::render::DetachedRendererConsumer>,
        ) -> Result<T, E>,
    ) -> Result<T, ImguiActiveRendererContextError<E>> {
        let consumer = self.renderer_consumer.as_ref();
        let renderer_ownership = &mut self.backend_ownership;
        #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
        let viewport_keepalive = if multi_viewport {
            Some(
                self.viewport_bridge
                    .attached_keepalive()
                    .expect("dear-imgui-bevy viewport bridge is not attached"),
            )
        } else {
            None
        };
        #[cfg(not(all(feature = "multi-viewport", not(target_arch = "wasm32"))))]
        let _ = multi_viewport;
        self.context
            .as_mut()
            .expect("Context owner must retain its suspended Context")
            .try_with_active(|context| {
                validate_active_renderer_ownership(context, renderer_ownership)
                    .map_err(ImguiActiveRendererContextError::RendererOwnership)?;
                #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
                if let Some(keepalive) = viewport_keepalive {
                    validate_viewport_bridge(context, keepalive)
                        .map_err(ImguiActiveRendererContextError::ViewportBridge)?;
                }
                let operation = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    operation(context, consumer)
                }));
                let platform_completion =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
                        if let Some(keepalive) = viewport_keepalive {
                            return complete_platform_frame_if_needed(context, keepalive);
                        }
                        Ok::<(), ImguiPlatformCompletionError>(())
                    }));
                match operation {
                    Ok(result) => {
                        match platform_completion {
                            Ok(Ok(())) => {}
                            #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
                            Ok(Err(error)) => {
                                return Err(ImguiActiveRendererContextError::ViewportBridge(error));
                            }
                            #[cfg(not(all(
                                feature = "multi-viewport",
                                not(target_arch = "wasm32")
                            )))]
                            Ok(Err(_)) => unreachable!("platform completion is disabled"),
                            Err(payload) => std::panic::resume_unwind(payload),
                        }
                        result.map_err(ImguiActiveRendererContextError::Operation)
                    }
                    Err(payload) => {
                        drop(platform_completion);
                        std::panic::resume_unwind(payload);
                    }
                }
            })
            .map_err(ImguiActiveRendererContextError::from_scoped)
    }

    #[cfg(not(feature = "render"))]
    pub(crate) fn try_with_active_renderer_context_checked<T, E>(
        &mut self,
        multi_viewport: bool,
        operation: impl FnOnce(&mut dear_imgui_rs::Context, Option<&()>) -> Result<T, E>,
    ) -> Result<T, ImguiActiveRendererContextError<E>> {
        #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
        let viewport_keepalive = if multi_viewport {
            Some(
                self.viewport_bridge
                    .attached_keepalive()
                    .expect("dear-imgui-bevy viewport bridge is not attached"),
            )
        } else {
            None
        };
        #[cfg(not(all(feature = "multi-viewport", not(target_arch = "wasm32"))))]
        let _ = multi_viewport;
        self.context
            .as_mut()
            .expect("Context owner must retain its suspended Context")
            .try_with_active(|context| {
                #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
                if let Some(keepalive) = viewport_keepalive {
                    validate_viewport_bridge(context, keepalive)
                        .map_err(ImguiActiveRendererContextError::ViewportBridge)?;
                }
                let operation = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    operation(context, None)
                }));
                let platform_completion =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
                        if let Some(keepalive) = viewport_keepalive {
                            return complete_platform_frame_if_needed(context, keepalive);
                        }
                        Ok::<(), ImguiPlatformCompletionError>(())
                    }));
                match operation {
                    Ok(result) => {
                        match platform_completion {
                            Ok(Ok(())) => {}
                            #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
                            Ok(Err(error)) => {
                                return Err(ImguiActiveRendererContextError::ViewportBridge(error));
                            }
                            #[cfg(not(all(
                                feature = "multi-viewport",
                                not(target_arch = "wasm32")
                            )))]
                            Ok(Err(_)) => unreachable!("platform completion is disabled"),
                            Err(payload) => std::panic::resume_unwind(payload),
                        }
                        result.map_err(ImguiActiveRendererContextError::Operation)
                    }
                    Err(payload) => {
                        drop(platform_completion);
                        std::panic::resume_unwind(payload);
                    }
                }
            })
            .map_err(ImguiActiveRendererContextError::from_scoped)
    }

    pub(crate) fn preflight_backend_attachment(
        &mut self,
        backend: &BackendAttachment,
        config: &ImguiContextConfig,
    ) -> Result<(), ImguiContextError> {
        let context_id = self
            .context
            .as_ref()
            .expect("Context owner must retain its suspended Context")
            .id();

        #[cfg(not(all(feature = "multi-viewport", not(target_arch = "wasm32"))))]
        if config.multi_viewport() {
            return Err(ImguiContextError::NativeMultiViewportUnavailable { context_id });
        }

        #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
        if config.multi_viewport() {
            if backend.viewport_bridge_registration.is_none() {
                return Err(ImguiContextError::BackendOwnershipConflict {
                    context_id,
                    field: "ViewportBridge",
                });
            }
            match self.viewport_bridge.phase {
                ImguiViewportBridgePhase::Attached => {}
                ImguiViewportBridgePhase::EcsReleasePending
                | ImguiViewportBridgePhase::ViewportDrained => {
                    return Err(ImguiContextError::TeardownInProgress { context_id });
                }
                ImguiViewportBridgePhase::Detached => {
                    let result = self
                        .context
                        .as_mut()
                        .expect("Context owner must retain its suspended Context")
                        .try_with_active(|context| {
                            context
                                .preflight_attachment_registration::<
                                    viewport::ImguiViewportBridgeAttachmentMarker,
                                >(dear_imgui_rs::ContextAttachmentRole::Platform)
                                .map_err(|_| "ContextAttachment")?;
                            viewport::preflight_owned_platform_callbacks(context)
                                .map_err(|_| "PlatformIO")
                        });
                    if let Err(error) = result {
                        return Err(ImguiContextError::from_scoped_activation(
                            context_id,
                            error,
                            |field| ImguiContextError::BackendOwnershipConflict {
                                context_id,
                                field,
                            },
                        ));
                    }
                }
            }
        }

        let ownership = &self.backend_ownership;
        let result = self
            .context
            .as_mut()
            .expect("Context owner must retain its suspended Context")
            .try_with_active(|context| {
                preflight_backend_context_claims(
                    context,
                    ownership,
                    backend.render_integration_installed,
                )
            });
        result.map_err(|error| {
            ImguiContextError::from_scoped_activation(context_id, error, |field| {
                ImguiContextError::BackendOwnershipConflict { context_id, field }
            })
        })
    }

    #[cfg(feature = "render")]
    pub(crate) fn preflight_renderer_admission(
        &mut self,
        backend: &BackendAttachment,
    ) -> Result<(), ImguiContextError> {
        if !backend.render_integration_installed || self.renderer_consumer.is_some() {
            return Ok(());
        }
        let context_id = self
            .context
            .as_ref()
            .expect("Context owner must retain its suspended Context")
            .id();
        self.context
            .as_mut()
            .expect("Context owner must retain its suspended Context")
            .try_with_active(|context| context.preflight_renderer_consumer())
            .map_err(|error| {
                ImguiContextError::from_scoped_activation(context_id, error, |source| {
                    ImguiContextError::RendererAdmission { context_id, source }
                })
            })
    }

    #[cfg(not(feature = "render"))]
    pub(crate) fn preflight_renderer_admission(
        &mut self,
        _backend: &BackendAttachment,
    ) -> Result<(), ImguiContextError> {
        Ok(())
    }

    #[cfg(feature = "render")]
    pub(crate) fn commit_renderer_admission(
        &mut self,
        backend: &BackendAttachment,
    ) -> Result<(), ImguiContextError> {
        if !backend.render_integration_installed || self.renderer_consumer.is_some() {
            return Ok(());
        }
        let context_id = self
            .context
            .as_ref()
            .expect("Context owner must retain its suspended Context")
            .id();
        let consumer = self
            .context
            .as_mut()
            .expect("Context owner must retain its suspended Context")
            .try_with_active(|context| {
                let consumer =
                    context
                        .create_detached_renderer_consumer()
                        .unwrap_or_else(|error| {
                            panic!("renderer admission changed after its global preflight: {error}")
                        });
                let reset = context
                    .prepare_renderer_texture_reset(&consumer)
                    .unwrap_or_else(|error| {
                        panic!("a newly admitted renderer consumer must be idle: {error}")
                    });
                reset.commit();
                Ok::<_, std::convert::Infallible>(consumer)
            })
            .map_err(|error| {
                ImguiContextError::from_scoped_activation(context_id, error, |never| match never {})
            })?;
        self.renderer_consumer = Some(consumer);
        let releases = backend
            .renderer_releases
            .as_ref()
            .expect("installed Bevy rendering must provide a Context release registry");
        self.renderer_release = Some(releases.admit(context_id));
        Ok(())
    }

    #[cfg(not(feature = "render"))]
    pub(crate) fn commit_renderer_admission(
        &mut self,
        _backend: &BackendAttachment,
    ) -> Result<(), ImguiContextError> {
        Ok(())
    }

    pub(crate) fn commit_backend_attachment(
        &mut self,
        backend: &BackendAttachment,
        config: &ImguiContextConfig,
    ) -> Result<(), ImguiContextError> {
        let context_id = self
            .context
            .as_ref()
            .expect("Context owner must retain its suspended Context")
            .id();
        let ownership = &mut self.backend_ownership;
        #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
        let viewport_keepalive = self.viewport_bridge.attached_keepalive();
        self.context
            .as_mut()
            .expect("Context owner must retain its suspended Context")
            .try_with_active(|context| {
                sync_backend_context_config(context, ownership, backend, config);
                #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
                if let Some(keepalive) = viewport_keepalive {
                    viewport::record_owned_platform_name(context, keepalive);
                }
                Ok::<_, std::convert::Infallible>(())
            })
            .map_err(|error| {
                ImguiContextError::from_scoped_activation(context_id, error, |never| match never {})
            })?;
        Ok(())
    }

    pub(crate) fn attach_backend(
        &mut self,
        backend: &BackendAttachment,
        config: &ImguiContextConfig,
    ) -> Result<(), ImguiContextError> {
        self.preflight_backend_attachment(backend, config)?;
        self.preflight_renderer_admission(backend)?;
        #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
        if config.multi_viewport() {
            let registration = backend
                .viewport_bridge_registration
                .as_ref()
                .ok_or_else(|| ImguiContextError::BackendOwnershipConflict {
                    context_id: self.context_id(),
                    field: "ViewportBridge",
                })?;
            self.attach_context_viewport_bridge(registration)?;
        }
        self.commit_renderer_admission(backend)?;
        self.commit_backend_attachment(backend, config)
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    fn context_id(&self) -> dear_imgui_rs::ContextId {
        self.context
            .as_ref()
            .expect("Context owner must retain its suspended Context")
            .id()
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    pub(crate) fn attach_context_viewport_bridge(
        &mut self,
        registration: &viewport::ImguiViewportBridgeRegistration,
    ) -> Result<(), ImguiContextError> {
        let context_id = self.context_id();
        match self.viewport_bridge.phase {
            ImguiViewportBridgePhase::Attached => {
                let installed = &mut self
                    .viewport_bridge
                    .owner
                    .as_mut()
                    .expect("an attached bridge must retain its owner")
                    .registration;
                if installed.is_none() {
                    *installed = Some(registration.clone());
                }
                return Ok(());
            }
            ImguiViewportBridgePhase::EcsReleasePending
            | ImguiViewportBridgePhase::ViewportDrained => {
                return Err(ImguiContextError::TeardownInProgress { context_id });
            }
            ImguiViewportBridgePhase::Detached => {}
        }

        let keepalive = Rc::new(viewport::ImguiViewportBridgeShared::default());
        let attachment = self
            .try_with_active_context(|context| {
                let attachment = context
                    .register_attachment::<viewport::ImguiViewportBridgeAttachmentMarker>(
                        dear_imgui_rs::ContextAttachmentRole::Platform,
                        viewport::viewport_bridge_teardown_attachment(Rc::clone(&keepalive)),
                    )
                    .map_err(|_| "ContextAttachment")?;
                // SAFETY: the keepalive is retained by both the Context attachment and the owner
                // lifecycle before callback pointers can be observed by Dear ImGui.
                unsafe { viewport::install_owned_platform_callbacks(context, &keepalive) }
                    .map_err(|_| "PlatformIO")?;
                Ok::<_, &'static str>(attachment)
            })
            .map_err(|error| {
                ImguiContextError::from_scoped_activation(context_id, error, |field| {
                    ImguiContextError::BackendOwnershipConflict { context_id, field }
                })
            })?;

        registration.register_context(context_id, Rc::clone(&keepalive));
        self.attach_viewport_bridge_with_registration(
            keepalive,
            attachment,
            Some(registration.clone()),
        );
        Ok(())
    }

    pub(crate) fn into_unattached_context(
        mut self,
    ) -> Result<dear_imgui_rs::SuspendedContext, Box<Self>> {
        if self.is_unattached() {
            Ok(self
                .context
                .take()
                .expect("Context owner must retain its suspended Context"))
        } else {
            Err(Box::new(self))
        }
    }

    pub(crate) fn into_suspended(mut self) -> dear_imgui_rs::SuspendedContext {
        self.context
            .take()
            .expect("detached Context owner must retain its suspended Context")
    }

    #[cfg(all(test, feature = "multi-viewport", not(target_arch = "wasm32")))]
    pub(crate) fn attach_viewport_bridge(
        &mut self,
        keepalive: viewport::ImguiViewportBridgeKeepalive,
        attachment: dear_imgui_rs::ContextAttachmentLease,
    ) {
        self.attach_viewport_bridge_with_registration(keepalive, attachment, None);
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    fn attach_viewport_bridge_with_registration(
        &mut self,
        keepalive: viewport::ImguiViewportBridgeKeepalive,
        attachment: dear_imgui_rs::ContextAttachmentLease,
        registration: Option<viewport::ImguiViewportBridgeRegistration>,
    ) {
        assert!(
            self.viewport_bridge.is_detached(),
            "dear-imgui-bevy viewport bridge was attached more than once"
        );
        self.backend_ownership.viewport_contract = true;
        self.backend_ownership.flags_added |= dear_imgui_rs::BackendFlags::PLATFORM_HAS_VIEWPORTS
            | dear_imgui_rs::BackendFlags::RENDERER_HAS_VIEWPORTS
            | dear_imgui_rs::BackendFlags::HAS_MOUSE_HOVERED_VIEWPORT;
        let context_id = self.context_id();
        self.viewport_bridge.owner = Some(ImguiViewportBridgeOwner {
            keepalive,
            attachment,
            registration,
            context_id,
            capabilities_still_owned: false,
        });
        self.viewport_bridge.phase = ImguiViewportBridgePhase::Attached;
    }

    #[cfg(feature = "render")]
    pub(crate) fn try_recover_renderer(
        &mut self,
    ) -> Result<(), ImguiActiveRendererContextError<dear_imgui_rs::render::RendererConsumerError>>
    {
        let renderer_consumer = self
            .renderer_consumer
            .as_ref()
            .expect("renderer recovery requires an admitted consumer");
        let renderer_release = self
            .renderer_release
            .as_ref()
            .expect("renderer recovery requires a release lease");
        let renderer_ownership = &mut self.backend_ownership;
        self.context
            .as_mut()
            .expect("Context owner must retain its suspended Context")
            .try_with_active(|context| {
                validate_active_renderer_ownership(context, renderer_ownership)
                    .map_err(ImguiActiveRendererContextError::RendererOwnership)?;
                let reset = context
                    .prepare_renderer_texture_reset(renderer_consumer)
                    .map_err(ImguiActiveRendererContextError::Operation)?;
                renderer_release.release_renderer_resources();
                reset.commit();
                renderer_release.finish_device_recovery();
                Ok(())
            })
            .map_err(ImguiActiveRendererContextError::from_scoped)
    }

    /// Validate every backend-owned field needed by teardown without starting either renderer or
    /// viewport release. Shutdown runs this for every registered Context before committing any
    /// irreversible world changes.
    pub(crate) fn preflight_backend_detach(
        &mut self,
    ) -> Result<(), ImguiContextRemovalPendingReason> {
        if self.context.is_none() {
            return Ok(());
        }

        #[cfg(any(
            feature = "render",
            all(feature = "multi-viewport", not(target_arch = "wasm32"))
        ))]
        {
            #[cfg(feature = "render")]
            let renderer_ownership = &self.backend_ownership;
            #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
            let viewport_keepalive = self.viewport_bridge.attached_keepalive().cloned();

            self.context
                .as_mut()
                .expect("Context owner must retain its suspended Context")
                .try_with_active(|context| {
                    #[cfg(feature = "render")]
                    validate_renderer_teardown_ownership(context, renderer_ownership)
                        .map_err(ImguiContextRemovalPendingReason::RendererOwnership)?;
                    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
                    if let Some(keepalive) = viewport_keepalive.as_ref() {
                        viewport::preflight_platform_callback_ownership(context, keepalive)
                            .map_err(ImguiContextRemovalPendingReason::ViewportCallbackOwnership)?;
                    }
                    Ok(())
                })
                .map_err(ImguiContextRemovalPendingReason::from_scoped)
        }

        #[cfg(not(any(
            feature = "render",
            all(feature = "multi-viewport", not(target_arch = "wasm32"))
        )))]
        Ok(())
    }

    pub(crate) fn try_detach_backend(&mut self) -> Result<(), ImguiContextRemovalPendingReason> {
        if self.context.is_none() {
            return Ok(());
        }
        #[cfg(feature = "render")]
        if let Some(snapshot_mailbox) = self.snapshot_mailbox.as_ref() {
            let context_id = self
                .context
                .as_ref()
                .expect("Context owner must retain its suspended Context")
                .id();
            snapshot_mailbox.clear(context_id);
        }
        #[cfg(feature = "render")]
        {
            let ownership = &mut self.backend_ownership;
            self.context
                .as_mut()
                .expect("Context owner must retain its suspended Context")
                .try_with_active(|context| {
                    preflight_renderer_teardown_ownership(context, ownership)
                        .map_err(ImguiContextRemovalPendingReason::RendererOwnership)
                })
                .map_err(ImguiContextRemovalPendingReason::from_scoped)?;
        }
        #[cfg(feature = "render")]
        // Request release before ECS despawn establishes a fail-closed extraction barrier.
        let renderer_release_acknowledged = self
            .renderer_release
            .as_ref()
            .is_none_or(|release| release.request_release());
        #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
        {
            let viewport_bridge = &mut self.viewport_bridge;
            self.context
                .as_mut()
                .expect("Context owner must retain its suspended Context")
                .try_with_active(|context| advance_viewport_drain(context, viewport_bridge))
                .map_err(ImguiContextRemovalPendingReason::from_scoped)?;
        }
        #[cfg(feature = "render")]
        if !renderer_release_acknowledged {
            return Err(ImguiContextRemovalPendingReason::RenderWorldReleasePending);
        }

        let ownership = &mut self.backend_ownership;
        #[cfg(feature = "render")]
        let consumer = &mut self.renderer_consumer;
        #[cfg(feature = "render")]
        let renderer_release = self.renderer_release.as_ref();
        #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
        let viewport_bridge = &mut self.viewport_bridge;
        let result = self
            .context
            .as_mut()
            .expect("Context owner must retain its suspended Context")
            .try_with_active(|context| {
                #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
                {
                    let viewport_capabilities_still_owned =
                        finish_viewport_detach(context, viewport_bridge);
                    clear_viewport_backend_contract(
                        context,
                        ownership,
                        viewport_capabilities_still_owned,
                    );
                }
                #[cfg(feature = "render")]
                if let Some(renderer_consumer) = consumer.as_ref() {
                    let reset = context
                        .prepare_renderer_texture_reset(renderer_consumer)
                        .map_err(ImguiContextRemovalPendingReason::Renderer)?;
                    renderer_release
                        .expect("an admitted Bevy renderer consumer must retain its release lease")
                        .release_renderer_resources();
                    reset.commit();
                }
                #[cfg(feature = "render")]
                {
                    drop(consumer.take());
                    let _ = context
                        .poll_snapshot_completions()
                        .map_err(ImguiContextRemovalPendingReason::Renderer)?;
                }
                clear_backend_data(context, ownership);
                Ok(())
            })
            .map_err(ImguiContextRemovalPendingReason::from_scoped);
        #[cfg(feature = "render")]
        if result.is_ok()
            && let Some(renderer_release) = self.renderer_release.take()
        {
            renderer_release.retire();
        }
        result
    }
}

impl Drop for ContextOwner {
    fn drop(&mut self) {
        if self.context.is_none() {
            return;
        }
        if self.retirement_sink.is_none() && self.is_unattached() {
            return;
        }
        if self.try_detach_backend().is_ok() {
            return;
        }
        let sink = self.retirement_sink.clone().unwrap_or_default();
        let owner = self.take_for_retirement();
        drop(ContextRetirement::new(owner, sink));
    }
}
#[cfg(all(test, feature = "multi-viewport", not(target_arch = "wasm32")))]
mod tests {
    use std::rc::Rc;

    use bevy_ecs::prelude::World;

    use super::ContextOwner;
    use crate::context::retirement::ImguiContextRetirements;
    use crate::test_util::imgui_context_guard as context_guard;
    use crate::viewport;

    fn viewport_owner() -> ContextOwner {
        let mut context = dear_imgui_rs::Context::create();
        context.io_mut().set_config_input_trickle_event_queue(false);
        context
            .font_atlas()
            .try_claim_legacy_renderer()
            .expect("the standalone viewport fixture uses legacy rendering")
            .build();
        let _ = context.set_ini_filename::<std::path::PathBuf>(None);

        let keepalive = Rc::new(viewport::ImguiViewportBridgeShared::default());
        let attachment = context
            .register_attachment::<viewport::ImguiViewportBridgeAttachmentMarker>(
                dear_imgui_rs::ContextAttachmentRole::Platform,
                viewport::viewport_bridge_teardown_attachment(Rc::clone(&keepalive)),
            )
            .unwrap();
        // SAFETY: the owner retains both the callback allocation and its Context attachment.
        unsafe { viewport::install_owned_platform_callbacks(&mut context, &keepalive) }.unwrap();
        let context_bridge = viewport::ImguiViewportBridgeContext {
            context_id: context.id(),
            inner: Rc::clone(&keepalive),
        };
        let primary_window =
            bevy_ecs::entity::Entity::from_raw_u32(1).expect("test entity index should be valid");
        viewport::prepare_platform_viewports_for_frame(
            &mut context,
            &context_bridge,
            primary_window,
            &bevy_window::Window::default(),
            &[],
            std::iter::empty(),
            viewport::NativeViewportFrameSupport::new(
                true,
                viewport::native_window::DesktopPositionSupport::Available,
            ),
        )
        .expect("the viewport fixture should complete the real platform-frame preparation");

        let mut owner = ContextOwner::new(context.suspend_or_panic());
        owner.attach_viewport_bridge(keepalive, attachment);
        owner
    }

    fn render_test_frame(context: &mut dear_imgui_rs::Context) {
        context.prepare_frame(dear_imgui_rs::FramePrepareOptions::new(
            [64.0, 64.0],
            1.0 / 60.0,
        ));
        let _ = context.frame();
        let _ = context.render_legacy();
    }

    fn assert_platform_frame_completed(owner: &mut ContextOwner) {
        owner
            .try_with_active_context(|context| {
                // SAFETY: the Context is current and remains active for this inspection.
                let raw = unsafe { &*context.as_raw() };
                assert_eq!(raw.FrameCountPlatformEnded, raw.FrameCount);
                Ok::<_, std::convert::Infallible>(())
            })
            .expect("the viewport fixture Context should activate for inspection");
    }

    #[test]
    fn pending_retirement_keeps_the_complete_viewport_owner_alive() {
        let _guard = context_guard();
        let retirements = ImguiContextRetirements::default();
        let mut owner = viewport_owner();
        owner.set_retirement_sink(retirements.sink());
        let keepalive = Rc::clone(
            &owner
                .viewport_bridge
                .owner
                .as_ref()
                .expect("the viewport fixture must retain its bridge owner")
                .keepalive,
        );
        let pending_entity = World::new().spawn_empty().id();
        viewport::track_viewport_ecs_despawn_for_test(&keepalive, pending_entity);
        let strong_count = Rc::strong_count(&keepalive);

        drop(owner);

        assert_eq!(retirements.sink().pending_len(), 1);
        assert_eq!(
            Rc::strong_count(&keepalive),
            strong_count,
            "queueing retirement must transfer rather than release the viewport payload"
        );
    }

    #[test]
    fn renderer_error_after_render_still_completes_the_platform_frame() {
        let _guard = context_guard();
        let mut owner = viewport_owner();

        let result = owner.try_with_active_renderer_context(true, |context, _consumer| {
            render_test_frame(context);
            Err::<(), _>("snapshot capture failed")
        });

        assert_eq!(result, Err("snapshot capture failed"));
        assert_platform_frame_completed(&mut owner);
    }

    #[test]
    fn renderer_panic_after_render_preserves_payload_and_completes_the_platform_frame() {
        let _guard = context_guard();
        let mut owner = viewport_owner();

        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _: Result<(), ()> =
                owner.try_with_active_renderer_context(true, |context, _consumer| {
                    render_test_frame(context);
                    std::panic::panic_any(0xC0FFEE_u32);
                });
        }))
        .expect_err("the renderer panic must propagate");

        assert_eq!(panic.downcast_ref::<u32>(), Some(&0xC0FFEE));
        assert_platform_frame_completed(&mut owner);
    }

    #[test]
    fn ui_panic_ends_the_open_frame_before_platform_completion() {
        let _guard = context_guard();
        let mut owner = viewport_owner();

        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _: Result<(), ()> =
                owner.try_with_active_renderer_context(true, |context, _consumer| {
                    context.prepare_frame(dear_imgui_rs::FramePrepareOptions::new(
                        [64.0, 64.0],
                        1.0 / 60.0,
                    ));
                    let _ = context.frame();
                    std::panic::panic_any("original UI panic");
                });
        }))
        .expect_err("the UI panic must propagate");

        assert_eq!(
            panic.downcast_ref::<&'static str>(),
            Some(&"original UI panic")
        );
        assert_platform_frame_completed(&mut owner);
    }
}
