use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use bevy_ecs::resource::Resource;

/// App-scoped lifecycle shared by every pass handle and Context registry created for one backend.
#[derive(Clone, Default, Resource)]
pub(crate) struct ImguiAppLifecycle {
    registry_claimed: Arc<AtomicBool>,
    terminal: Arc<AtomicBool>,
}

impl ImguiAppLifecycle {
    pub(crate) fn try_claim_registry(&self) -> bool {
        self.registry_claimed
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    pub(crate) fn registry_claimed(&self) -> bool {
        self.registry_claimed.load(Ordering::Acquire)
    }

    pub(crate) fn is_terminal(&self) -> bool {
        self.terminal.load(Ordering::Acquire)
    }

    pub(crate) fn commit_terminal(&self) {
        self.terminal.store(true, Ordering::Release);
    }
}
