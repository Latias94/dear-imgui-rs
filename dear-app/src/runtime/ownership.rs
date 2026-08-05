//! Ordered ownership release and fail-stop teardown transactions.

use tracing::error;

use super::recovery::{GenerationRelease, RuntimeGenerations};
use super::state::{RuntimeGeneration, UiState, WindowState};
use crate::RunError;

pub(super) struct RuntimeOwnership {
    pub(super) window: WindowState,
    pub(super) ui: UiState,
    pub(super) generations: RuntimeGenerations<RuntimeGeneration>,
}

pub(super) trait RuntimeOwnershipLifecycle: Sized {
    fn release_renderer(&mut self) -> Result<(), RunError>;
    fn release_platform(&mut self) -> Result<(), RunError>;
    fn teardown_after_backend_release(self);
}

pub(super) struct OrderedRuntimeOwner<T: RuntimeOwnershipLifecycle> {
    ownership: Option<T>,
}

/// Quarantines the ownership graph unless every Context-bound backend release reaches its commit
/// point.
struct BackendReleaseTransaction<T> {
    ownership: Option<T>,
}

impl<T> BackendReleaseTransaction<T> {
    fn new(ownership: T) -> Self {
        Self {
            ownership: Some(ownership),
        }
    }

    fn ownership_mut(&mut self) -> &mut T {
        self.ownership
            .as_mut()
            .expect("renderer release transaction owns the runtime graph")
    }

    fn commit(mut self) -> T {
        self.ownership
            .take()
            .expect("renderer release transaction can commit only once")
    }
}

impl<T> Drop for BackendReleaseTransaction<T> {
    fn drop(&mut self) {
        if let Some(ownership) = self.ownership.take() {
            std::mem::forget(ownership);
        }
    }
}

impl<T: RuntimeOwnershipLifecycle> OrderedRuntimeOwner<T> {
    pub(super) fn new(ownership: T) -> Self {
        Self {
            ownership: Some(ownership),
        }
    }

    pub(super) fn get(&self) -> &T {
        self.ownership
            .as_ref()
            .expect("runtime ownership is available until teardown starts")
    }

    pub(super) fn get_mut(&mut self) -> &mut T {
        self.ownership
            .as_mut()
            .expect("runtime ownership is available until teardown starts")
    }

    pub(super) fn teardown(mut self) -> Result<(), RunError> {
        let ownership = self
            .ownership
            .take()
            .expect("runtime ownership can be consumed only once");
        release_then_teardown_or_quarantine(ownership)
    }
}

impl<T: RuntimeOwnershipLifecycle> Drop for OrderedRuntimeOwner<T> {
    fn drop(&mut self) {
        let Some(ownership) = self.ownership.take() else {
            return;
        };
        if let Err(error) = release_then_teardown_or_quarantine(ownership) {
            error!("Dear App quarantined runtime ownership after backend release failed: {error}");
        }
    }
}

fn release_then_teardown_or_quarantine<T: RuntimeOwnershipLifecycle>(
    ownership: T,
) -> Result<(), RunError> {
    let mut transaction = BackendReleaseTransaction::new(ownership);
    // The transaction quarantines the complete graph if renderer resources still borrow the
    // Context, window, or GPU generation.
    transaction.ownership_mut().release_renderer()?;
    // A platform ownership conflict leaves Context attachment state uncertain. Context drop must
    // not run its fallback teardown after an explicit release failure.
    transaction.ownership_mut().release_platform()?;
    let ownership = transaction.commit();
    ownership.teardown_after_backend_release();
    Ok(())
}

impl RuntimeOwnershipLifecycle for RuntimeOwnership {
    fn release_renderer(&mut self) -> Result<(), RunError> {
        let mut release = RuntimeRelease { ui: &mut self.ui };
        self.generations.shutdown(&mut release)
    }

    fn release_platform(&mut self) -> Result<(), RunError> {
        self.ui.release_platform()
    }

    fn teardown_after_backend_release(self) {
        let Self {
            window,
            ui,
            generations,
        } = self;
        drop(generations);
        ui.teardown_after_platform_release();
        drop(window);
    }
}

struct RuntimeRelease<'a> {
    ui: &'a mut UiState,
}

impl GenerationRelease<RuntimeGeneration> for RuntimeRelease<'_> {
    fn release_generation(&mut self, generation: &mut RuntimeGeneration) -> Result<(), RunError> {
        generation.gpu.release_renderer(&mut self.ui.context)
    }
}
