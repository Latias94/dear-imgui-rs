#[cfg(feature = "multi-viewport-winit")]
use super::super::SurfaceCreateError;
use super::super::SurfaceSupportError;
use super::super::callbacks::{
    detect_runtime_contract_drift, revoke_renderer_viewport_capability_if_owned,
};
use super::{AshViewportError, RuntimeControl};
use crate::RendererError;

fn entry_fault_is_terminal(fault: &AshViewportError) -> bool {
    match fault {
        AshViewportError::CallbackPanicked { .. }
        | AshViewportError::DeviceLost { .. }
        | AshViewportError::RendererUserDataOwnershipLost { .. }
        | AshViewportError::Renderer(
            RendererError::Vulkan(ash::vk::Result::ERROR_DEVICE_LOST)
            | RendererError::RendererStateReplaced { .. }
            | RendererError::RendererStateDrift { .. },
        ) => true,
        #[cfg(feature = "multi-viewport-winit")]
        AshViewportError::SurfaceCreate(SurfaceCreateError::Vulkan(
            ash::vk::Result::ERROR_DEVICE_LOST,
        )) => true,
        AshViewportError::SurfaceUnsupported(
            SurfaceSupportError::PresentSupportQuery(ash::vk::Result::ERROR_DEVICE_LOST)
            | SurfaceSupportError::CapabilitiesQuery(ash::vk::Result::ERROR_DEVICE_LOST)
            | SurfaceSupportError::FormatsQuery(ash::vk::Result::ERROR_DEVICE_LOST)
            | SurfaceSupportError::PresentModesQuery(ash::vk::Result::ERROR_DEVICE_LOST),
        ) => true,
        _ => false,
    }
}

impl RuntimeControl {
    pub(in super::super) fn record_fault(&self, fault: AshViewportError) {
        self.faults.borrow_mut().record_non_terminal(fault);
    }

    /// Classifies a fault raised by a native callback or callback-scoped Vulkan operation.
    pub(in super::super) fn record_entry_fault(&self, fault: AshViewportError) {
        if entry_fault_is_terminal(&fault) {
            self.record_runtime_contract_fault(fault);
        } else {
            self.record_fault(fault);
        }
    }

    pub(in super::super) fn record_runtime_contract_fault(&self, fault: AshViewportError) {
        let _ = self.binding.try_with_bound_context(|| {
            revoke_renderer_viewport_capability_if_owned(self);
        });
        self.faults.borrow_mut().record_terminal(fault);
        self.begin_shutdown();
    }

    pub(super) fn detect_and_take_fault(&self) -> Option<AshViewportError> {
        detect_runtime_contract_drift(self);
        self.faults.borrow_mut().take_next()
    }
}
