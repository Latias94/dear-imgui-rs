pub(super) struct CurrentContextGuard {
    previous: *mut dear_imgui_rs::sys::ImGuiContext,
    target: *mut dear_imgui_rs::sys::ImGuiContext,
}

impl CurrentContextGuard {
    pub(super) unsafe fn bind(target: *mut dear_imgui_rs::sys::ImGuiContext) -> Self {
        let previous = unsafe { dear_imgui_rs::sys::igGetCurrentContext() };
        if previous != target {
            unsafe { dear_imgui_rs::sys::igSetCurrentContext(target) };
        }
        Self { previous, target }
    }
}

impl Drop for CurrentContextGuard {
    fn drop(&mut self) {
        if self.previous != self.target {
            unsafe { dear_imgui_rs::sys::igSetCurrentContext(self.previous) };
        }
    }
}
