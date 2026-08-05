use dear_imgui_rs::platform_io::Viewport;

use super::super::registry::{ViewportIdentity, resolve_viewport};
use super::RuntimeControl;

impl RuntimeControl {
    pub(in super::super) fn mark_viewport_create_failed(&self, viewport: &mut Viewport) {
        self.failed_viewports
            .borrow_mut()
            .insert(ViewportIdentity::from_viewport(
                self.context_raw(),
                viewport,
            ));
        viewport.set_platform_request_close(true);
    }

    pub(in super::super) fn clear_viewport_create_failure(&self, viewport: &Viewport) {
        self.failed_viewports
            .borrow_mut()
            .remove(&ViewportIdentity::from_viewport(
                self.context_raw(),
                viewport,
            ));
    }

    pub(in super::super) fn clear_viewport_create_failures(&self) {
        self.failed_viewports.borrow_mut().clear();
    }

    pub(in super::super) fn reassert_failed_viewport_closures(&self) {
        let failed_viewports = self
            .failed_viewports
            .borrow()
            .iter()
            .copied()
            .collect::<Vec<_>>();
        if failed_viewports.is_empty() {
            return;
        }
        for identity in failed_viewports {
            // A failed secondary viewport may be hidden from `PlatformIO.Viewports` while Dear
            // ImGui still owns it internally. Reassert the close request through the complete
            // internal lookup so its platform/renderer sidecars can be destroyed normally.
            if let Some(viewport) = resolve_viewport(identity) {
                // SAFETY: this runtime's Context is current while contract drift is checked.
                unsafe { Viewport::from_raw_mut(viewport) }.set_platform_request_close(true);
            } else {
                self.failed_viewports.borrow_mut().remove(&identity);
            }
        }
    }
}
