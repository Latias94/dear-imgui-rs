use crate::{imgui_sys, sys};

#[derive(Clone, Copy)]
pub(crate) struct Plot3DContextBinding {
    pub(crate) plot_ctx_raw: *mut sys::ImPlot3DContext,
    pub(crate) imgui_ctx_raw: *mut imgui_sys::ImGuiContext,
}

#[must_use = "dropping the guard restores the previous Dear ImGui/ImPlot3D contexts"]
pub(crate) struct Plot3DContextBindingGuard {
    prev_imgui_ctx_raw: *mut imgui_sys::ImGuiContext,
    prev_plot_ctx_raw: *mut sys::ImPlot3DContext,
    restore_imgui: bool,
    restore_plot: bool,
}

impl Plot3DContextBinding {
    pub(crate) fn bind(&self) -> Plot3DContextBindingGuard {
        assert!(
            !self.imgui_ctx_raw.is_null(),
            "dear-implot3d: Plot3DUi requires an active ImGui context"
        );
        assert!(
            !self.plot_ctx_raw.is_null(),
            "dear-implot3d: Plot3DUi requires an active ImPlot3D context"
        );
        let prev_imgui_ctx_raw = unsafe { imgui_sys::igGetCurrentContext() };
        let prev_plot_ctx_raw = unsafe { sys::ImPlot3D_GetCurrentContext() };
        let restore_imgui = prev_imgui_ctx_raw != self.imgui_ctx_raw;
        let restore_plot = prev_plot_ctx_raw != self.plot_ctx_raw;
        unsafe {
            if restore_imgui {
                imgui_sys::igSetCurrentContext(self.imgui_ctx_raw);
            }
            sys::ImPlot3D_SetCurrentContext(self.plot_ctx_raw);
        }
        Plot3DContextBindingGuard {
            prev_imgui_ctx_raw,
            prev_plot_ctx_raw,
            restore_imgui,
            restore_plot,
        }
    }
}

impl Drop for Plot3DContextBindingGuard {
    fn drop(&mut self) {
        unsafe {
            if self.restore_plot {
                sys::ImPlot3D_SetCurrentContext(self.prev_plot_ctx_raw);
            }
            if self.restore_imgui {
                imgui_sys::igSetCurrentContext(self.prev_imgui_ctx_raw);
            }
        }
    }
}
