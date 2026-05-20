use crate::{imgui_sys, sys};

#[derive(Clone, Copy)]
pub(crate) struct Plot3DContextBinding {
    pub(crate) plot_ctx_raw: *mut sys::ImPlot3DContext,
    pub(crate) imgui_ctx_raw: *mut imgui_sys::ImGuiContext,
}

impl Plot3DContextBinding {
    pub(crate) fn bind(self) {
        assert!(
            !self.imgui_ctx_raw.is_null(),
            "dear-implot3d: Plot3DUi requires an active ImGui context"
        );
        assert!(
            !self.plot_ctx_raw.is_null(),
            "dear-implot3d: Plot3DUi requires an active ImPlot3D context"
        );
        assert_eq!(
            unsafe { imgui_sys::igGetCurrentContext() },
            self.imgui_ctx_raw,
            "dear-implot3d: Plot3DUi must be used with the currently-active ImGui context"
        );
        unsafe { sys::ImPlot3D_SetCurrentContext(self.plot_ctx_raw) };
    }
}
