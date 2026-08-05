//! Native multi-viewport attachment ownership and teardown state machine.

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use crate::viewport;

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use super::backend_contract::{ImguiBackendOwnership, ImguiContextRemovalPendingReason};

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(super) type ImguiPlatformCompletionError = viewport::ImguiViewportRuntimeError;
#[cfg(not(all(feature = "multi-viewport", not(target_arch = "wasm32"))))]
pub(super) type ImguiPlatformCompletionError = std::convert::Infallible;

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ImguiViewportBridgePhase {
    Detached,
    Attached,
    EcsReleasePending,
    ViewportDrained,
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(super) struct ImguiViewportBridgeOwner {
    pub(super) keepalive: viewport::ImguiViewportBridgeKeepalive,
    pub(super) attachment: dear_imgui_rs::ContextAttachmentLease,
    pub(super) registration: Option<viewport::ImguiViewportBridgeRegistration>,
    pub(super) context_id: dear_imgui_rs::ContextId,
    pub(super) capabilities_still_owned: bool,
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(super) struct ImguiViewportBridgeLifecycle {
    pub(super) phase: ImguiViewportBridgePhase,
    pub(super) owner: Option<ImguiViewportBridgeOwner>,
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
impl Default for ImguiViewportBridgeLifecycle {
    fn default() -> Self {
        Self {
            phase: ImguiViewportBridgePhase::Detached,
            owner: None,
        }
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
impl ImguiViewportBridgeLifecycle {
    pub(super) fn attached_keepalive(&self) -> Option<&viewport::ImguiViewportBridgeKeepalive> {
        (self.phase == ImguiViewportBridgePhase::Attached).then(|| {
            &self
                .owner
                .as_ref()
                .expect("an attached bridge must retain its owner")
                .keepalive
        })
    }

    pub(super) fn is_detached(&self) -> bool {
        self.phase == ImguiViewportBridgePhase::Detached
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(super) fn validate_viewport_bridge(
    context: &mut dear_imgui_rs::Context,
    keepalive: &viewport::ImguiViewportBridgeKeepalive,
) -> Result<(), viewport::ImguiViewportRuntimeError> {
    if let Some(error) = viewport::platform_callback_error(keepalive) {
        return Err(error);
    }
    viewport::platform_callback_ownership(context, keepalive)
        .map_err(viewport::ImguiViewportRuntimeError::CallbackOwnership)
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(super) fn complete_platform_frame_if_needed(
    context: &mut dear_imgui_rs::Context,
    keepalive: &viewport::ImguiViewportBridgeKeepalive,
) -> Result<(), viewport::ImguiViewportRuntimeError> {
    let _ = context.end_frame();
    // SAFETY: the owner keeps this Context active and current for the whole completion check.
    let platform_frame_pending = unsafe {
        let raw = &*context.as_raw();
        raw.FrameCount > 0
            && raw.FrameCountEnded == raw.FrameCount
            && raw.FrameCountPlatformEnded < raw.FrameCount
    };
    if !platform_frame_pending {
        return viewport::platform_callback_error(keepalive).map_or(Ok(()), Err);
    }
    validate_viewport_bridge(context, keepalive)?;
    context.update_platform_windows();
    viewport::platform_callback_error(keepalive).map_or(Ok(()), Err)
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(super) fn clear_viewport_backend_contract(
    context: &mut dear_imgui_rs::Context,
    ownership: &mut ImguiBackendOwnership,
    capabilities_still_owned: bool,
) {
    let viewport_flags = dear_imgui_rs::BackendFlags::PLATFORM_HAS_VIEWPORTS
        | dear_imgui_rs::BackendFlags::RENDERER_HAS_VIEWPORTS
        | dear_imgui_rs::BackendFlags::HAS_MOUSE_HOVERED_VIEWPORT;
    let added_viewport_flags = ownership.flags_added & viewport_flags;
    ownership.flags_added.remove(viewport_flags);

    if ownership.viewport_contract && capabilities_still_owned {
        let mut config_flags = context.io().config_flags();
        config_flags.remove(dear_imgui_rs::ConfigFlags::VIEWPORTS_ENABLE);
        context.io_mut().set_config_flags(config_flags);
    }
    ownership.viewport_contract = false;

    if capabilities_still_owned {
        let current_flags = context.io().backend_flags();
        context
            .io_mut()
            .set_backend_flags(current_flags & !added_viewport_flags);
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(super) fn advance_viewport_drain(
    context: &mut dear_imgui_rs::Context,
    lifecycle: &mut ImguiViewportBridgeLifecycle,
) -> Result<(), ImguiContextRemovalPendingReason> {
    match lifecycle.phase {
        ImguiViewportBridgePhase::Detached | ImguiViewportBridgePhase::ViewportDrained => Ok(()),
        ImguiViewportBridgePhase::Attached => {
            let owner = lifecycle
                .owner
                .as_ref()
                .expect("an attached bridge must retain its owner");
            let capabilities_still_owned =
                viewport::platform_capabilities_still_owned(context, &owner.keepalive);
            let ownership_error =
                viewport::begin_owned_bridge_release(context, &owner.keepalive).err();
            let ecs_release_pending = viewport::viewport_ecs_release_pending(&owner.keepalive);
            lifecycle
                .owner
                .as_mut()
                .expect("an attached bridge must retain its owner")
                .capabilities_still_owned = capabilities_still_owned;
            lifecycle.phase = if ecs_release_pending {
                ImguiViewportBridgePhase::EcsReleasePending
            } else {
                ImguiViewportBridgePhase::ViewportDrained
            };
            if let Some(error) = ownership_error
                && capabilities_still_owned
            {
                return Err(ImguiContextRemovalPendingReason::ViewportCallbackOwnership(
                    error,
                ));
            }
            if ecs_release_pending {
                return Err(ImguiContextRemovalPendingReason::ViewportWorldReleasePending);
            }
            Ok(())
        }
        ImguiViewportBridgePhase::EcsReleasePending => {
            let owner = lifecycle
                .owner
                .as_ref()
                .expect("a draining bridge must retain its owner");
            if viewport::viewport_ecs_release_pending(&owner.keepalive) {
                return Err(ImguiContextRemovalPendingReason::ViewportWorldReleasePending);
            }
            lifecycle.phase = ImguiViewportBridgePhase::ViewportDrained;
            Ok(())
        }
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(super) fn finish_viewport_detach(
    context: &mut dear_imgui_rs::Context,
    lifecycle: &mut ImguiViewportBridgeLifecycle,
) -> bool {
    if lifecycle.phase == ImguiViewportBridgePhase::Detached {
        return false;
    }
    assert_eq!(
        lifecycle.phase,
        ImguiViewportBridgePhase::ViewportDrained,
        "viewport detach cannot finish before the ECS viewport world drains"
    );
    let owner = lifecycle
        .owner
        .as_mut()
        .expect("a drained bridge must retain its owner");
    viewport::finish_owned_bridge_release(context, &owner.keepalive);
    let _ = owner
        .attachment
        .detach()
        .expect("the drained Bevy viewport bridge has no renderer attachment dependency");
    viewport::finish_viewport_ecs_release(&owner.keepalive);
    if let Some(registration) = owner.registration.as_ref() {
        registration.unregister_context(owner.context_id, &owner.keepalive);
    }
    let capabilities_still_owned = owner.capabilities_still_owned;
    drop(lifecycle.owner.take());
    lifecycle.phase = ImguiViewportBridgePhase::Detached;
    capabilities_still_owned
}
