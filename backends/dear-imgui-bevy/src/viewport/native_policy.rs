use bevy_ecs::entity::Entity;
use dear_imgui_rs::ViewportFlags;
use winit::window::{Window, WindowId};

#[cfg(target_os = "windows")]
use dear_imgui_winit::native_support::{
    NativeWindowPolicy as WinitNativeWindowPolicy, WindowPolicyError, WindowPolicyLease,
};

/// The native policy requested by one stable Dear ImGui viewport instance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct DesiredNativeWindowPolicy {
    pub(super) accepts_pointer_input: bool,
    pub(super) no_focus_on_click: bool,
}

impl DesiredNativeWindowPolicy {
    pub(super) const fn from_flags(flags: ViewportFlags) -> Self {
        Self {
            accepts_pointer_input: !flags.contains(ViewportFlags::NO_INPUTS),
            no_focus_on_click: flags.contains(ViewportFlags::NO_FOCUS_ON_CLICK),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum NativeViewportPolicyFailure {
    NativeWindowPending,
    #[cfg(target_os = "windows")]
    WindowHandleUnavailable,
    #[cfg(target_os = "windows")]
    UnexpectedHandleKind,
    #[cfg(target_os = "windows")]
    WindowOwnerUnavailable,
    #[cfg(target_os = "windows")]
    WrongWindowThread,
    #[cfg(target_os = "windows")]
    InstallFailed,
    #[cfg(target_os = "windows")]
    HookDetached,
    #[cfg(target_os = "windows")]
    WindowDestroyed,
}

impl NativeViewportPolicyFailure {
    #[cfg(target_os = "windows")]
    fn from_error(error: &WindowPolicyError) -> Self {
        match error {
            WindowPolicyError::WindowHandleUnavailable { .. } => Self::WindowHandleUnavailable,
            WindowPolicyError::UnexpectedHandleKind => Self::UnexpectedHandleKind,
            WindowPolicyError::WindowOwnerUnavailable => Self::WindowOwnerUnavailable,
            WindowPolicyError::WrongWindowThread { .. } => Self::WrongWindowThread,
            WindowPolicyError::InstallFailed { .. } => Self::InstallFailed,
            WindowPolicyError::HookDetached => Self::HookDetached,
            WindowPolicyError::WindowDestroyed => Self::WindowDestroyed,
            _ => Self::InstallFailed,
        }
    }
}

#[derive(Default)]
pub(super) enum NativeViewportPolicyState {
    #[default]
    Unmapped,
    Ready {
        entity: Entity,
        window_id: WindowId,
        policy: DesiredNativeWindowPolicy,
        #[cfg(target_os = "windows")]
        lease: WindowPolicyLease,
    },
    #[cfg(target_os = "windows")]
    Failed {
        entity: Entity,
        window_id: WindowId,
        policy: DesiredNativeWindowPolicy,
        reason: NativeViewportPolicyFailure,
    },
}

impl NativeViewportPolicyState {
    pub(super) fn is_ready(&self) -> bool {
        matches!(self, Self::Ready { .. })
    }

    pub(super) fn diagnostic_failure(&self) -> Option<NativeViewportPolicyFailure> {
        match self {
            Self::Unmapped => Some(NativeViewportPolicyFailure::NativeWindowPending),
            Self::Ready { .. } => None,
            #[cfg(target_os = "windows")]
            Self::Failed { reason, .. } => Some(*reason),
        }
    }

    pub(super) fn release(&mut self) {
        let _previous = std::mem::take(self);
    }

    #[cfg(target_os = "windows")]
    fn same_failed_target(
        &self,
        entity: Entity,
        window_id: WindowId,
        policy: DesiredNativeWindowPolicy,
    ) -> bool {
        matches!(
            self,
            Self::Failed {
                entity: current_entity,
                window_id: current_window_id,
                policy: current_policy,
                ..
            } if *current_entity == entity
                && *current_window_id == window_id
                && *current_policy == policy
        )
    }

    fn same_ready_window(&self, entity: Entity, window: &Window) -> bool {
        let Self::Ready {
            entity: current_entity,
            window_id,
            #[cfg(target_os = "windows")]
            lease,
            ..
        } = self
        else {
            return false;
        };
        if *current_entity != entity || *window_id != window.id() {
            return false;
        }
        #[cfg(target_os = "windows")]
        if !lease.matches_window(window) {
            return false;
        }
        true
    }

    pub(super) fn sync(
        &mut self,
        entity: Entity,
        window: Option<&Window>,
        policy: DesiredNativeWindowPolicy,
    ) {
        let Some(window) = window else {
            self.release();
            return;
        };
        let window_id = window.id();
        #[cfg(target_os = "windows")]
        if self.same_failed_target(entity, window_id, policy) {
            return;
        }

        if self.same_ready_window(entity, window) {
            #[cfg(target_os = "windows")]
            let update_result = match self {
                Self::Ready {
                    policy: current_policy,
                    lease,
                    ..
                } if *current_policy != policy => Some(lease.update(WinitNativeWindowPolicy {
                    accepts_pointer_input: policy.accepts_pointer_input,
                    no_focus_on_click: policy.no_focus_on_click,
                })),
                _ => None,
            };
            #[cfg(target_os = "windows")]
            if let Some(update_result) = update_result {
                match update_result {
                    Ok(()) => {
                        if let Self::Ready {
                            policy: current_policy,
                            ..
                        } = self
                        {
                            *current_policy = policy;
                        }
                        return;
                    }
                    Err(error) => {
                        let reason = NativeViewportPolicyFailure::from_error(&error);
                        self.release();
                        *self = Self::Failed {
                            entity,
                            window_id,
                            policy,
                            reason,
                        };
                        return;
                    }
                }
            }
            #[cfg(not(target_os = "windows"))]
            if let Self::Ready {
                policy: current_policy,
                ..
            } = self
            {
                *current_policy = policy;
                return;
            }
            #[cfg(target_os = "windows")]
            if matches!(self, Self::Ready { policy: current_policy, .. } if *current_policy == policy)
            {
                return;
            }
        }

        self.release();
        #[cfg(target_os = "windows")]
        {
            let native_policy = WinitNativeWindowPolicy {
                accepts_pointer_input: policy.accepts_pointer_input,
                no_focus_on_click: policy.no_focus_on_click,
            };
            match WindowPolicyLease::install(window, native_policy) {
                Ok(lease) => {
                    *self = Self::Ready {
                        entity,
                        window_id,
                        policy,
                        lease,
                    };
                }
                Err(error) => {
                    *self = Self::Failed {
                        entity,
                        window_id,
                        policy,
                        reason: NativeViewportPolicyFailure::from_error(&error),
                    };
                }
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            *self = Self::Ready {
                entity,
                window_id,
                policy,
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_exact_window_remains_pending_and_fail_closed() {
        let entity = Entity::from_raw_u32(1).expect("the test entity index should be valid");
        let mut state = NativeViewportPolicyState::default();
        let policy = DesiredNativeWindowPolicy::from_flags(
            ViewportFlags::NO_INPUTS | ViewportFlags::NO_FOCUS_ON_CLICK,
        );

        state.sync(entity, None, policy);

        assert!(!state.is_ready());
        assert_eq!(
            state.diagnostic_failure(),
            Some(NativeViewportPolicyFailure::NativeWindowPending)
        );
    }

    #[test]
    fn viewport_flags_map_to_native_policy_without_inverting_input_semantics() {
        let policy = DesiredNativeWindowPolicy::from_flags(
            ViewportFlags::NO_INPUTS | ViewportFlags::NO_FOCUS_ON_CLICK,
        );

        assert!(!policy.accepts_pointer_input);
        assert!(policy.no_focus_on_click);
        assert!(
            DesiredNativeWindowPolicy::from_flags(ViewportFlags::empty()).accepts_pointer_input
        );
    }
}
