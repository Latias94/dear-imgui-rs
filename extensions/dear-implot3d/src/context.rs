use crate::ui::{Plot3DContextBinding, Plot3DUi};
use crate::{imgui_sys, sys};
use dear_imgui_rs::{Context, Ui};

/// Plot3D context wrapper
///
/// This manages the ImPlot3D context lifetime. Create one instance per application
/// and keep it alive for the duration of your program.
///
/// # Example
///
/// ```no_run
/// use dear_imgui_rs::*;
/// use dear_implot3d::*;
///
/// let mut imgui_ctx = Context::create();
/// let plot3d_ctx = Plot3DContext::create(&imgui_ctx);
///
/// // In your main loop:
/// let ui = imgui_ctx.frame();
/// let plot_ui = plot3d_ctx.get_plot_ui(&ui);
/// ```
pub struct Plot3DContext {
    pub(crate) raw: *mut sys::ImPlot3DContext,
    pub(crate) imgui_ctx_raw: *mut imgui_sys::ImGuiContext,
    pub(crate) imgui_alive: Option<dear_imgui_rs::ContextAliveToken>,
    pub(crate) owns_context: bool,
}

impl Plot3DContext {
    /// Try to create a new ImPlot3D context.
    ///
    /// This should be called once after creating your ImGui context.
    pub fn try_create(imgui: &Context) -> dear_imgui_rs::ImGuiResult<Self> {
        let imgui_ctx_raw = imgui.as_raw();
        let imgui_alive = Some(imgui.alive_token());
        assert_eq!(
            unsafe { imgui_sys::igGetCurrentContext() },
            imgui_ctx_raw,
            "dear-implot3d: Plot3DContext must be created with the currently-active ImGui context"
        );
        unsafe {
            let ctx = sys::ImPlot3D_CreateContext();
            if ctx.is_null() {
                return Err(dear_imgui_rs::ImGuiError::context_creation(
                    "ImPlot3D_CreateContext returned null",
                ));
            }

            // Ensure our new context is set as current even if another existed.
            sys::ImPlot3D_SetCurrentContext(ctx);
            Ok(Self {
                raw: ctx,
                imgui_ctx_raw,
                imgui_alive,
                owns_context: true,
            })
        }
    }

    /// Create a new ImPlot3D context (panics on error).
    pub fn create(imgui: &Context) -> Self {
        Self::try_create(imgui).expect("Failed to create ImPlot3D context")
    }

    /// Get the current ImPlot3D context as a non-owning raw-context wrapper.
    ///
    /// Returns None if no context is current.
    ///
    /// # Safety
    ///
    /// The returned value does not own the current ImPlot3D context and cannot prove that the
    /// associated ImGui context remains alive. The caller must ensure the raw ImPlot3D and ImGui
    /// contexts outlive the returned wrapper and are used on the same thread/context stack.
    pub unsafe fn current() -> Option<Self> {
        let raw = unsafe { sys::ImPlot3D_GetCurrentContext() };
        if raw.is_null() {
            None
        } else {
            Some(Self {
                raw,
                imgui_ctx_raw: unsafe { imgui_sys::igGetCurrentContext() },
                imgui_alive: None,
                owns_context: false,
            })
        }
    }

    /// Set this context as the current ImPlot3D context.
    pub fn set_as_current(&self) {
        self.assert_imgui_alive();
        self.binding().bind();
    }

    /// Get a raw pointer to the current ImPlot3D style
    ///
    /// This is an advanced function for direct style manipulation.
    /// Prefer using the safe style functions in the `style` module.
    pub fn raw_style_mut() -> *mut sys::ImPlot3DStyle {
        unsafe { sys::ImPlot3D_GetStyle() }
    }

    /// Get the raw ImPlot3D context pointer.
    ///
    /// # Safety
    ///
    /// The caller must ensure the pointer is used safely and not stored beyond the lifetime of
    /// this context wrapper.
    pub unsafe fn raw(&self) -> *mut sys::ImPlot3DContext {
        self.raw
    }

    /// Get a per-frame plotting interface
    ///
    /// Call this once per frame to get access to plotting functions.
    /// The returned `Plot3DUi` is tied to the lifetime of the `Ui` frame.
    pub fn get_plot_ui<'ui>(&self, ui: &'ui Ui) -> Plot3DUi<'ui> {
        self.set_as_current();
        Plot3DUi {
            _ui: ui,
            binding: Plot3DContextBinding {
                plot_ctx_raw: self.raw,
                imgui_ctx_raw: self.imgui_ctx_raw,
            },
            imgui_alive: self.imgui_alive.clone(),
        }
    }

    fn assert_imgui_alive(&self) {
        if let Some(alive) = &self.imgui_alive {
            assert!(
                alive.is_alive(),
                "dear-implot3d: ImGui context has been dropped"
            );
        }
    }

    fn binding(&self) -> Plot3DContextBinding {
        Plot3DContextBinding {
            plot_ctx_raw: self.raw,
            imgui_ctx_raw: self.imgui_ctx_raw,
        }
    }
}

impl Drop for Plot3DContext {
    fn drop(&mut self) {
        if !self.owns_context || self.raw.is_null() {
            return;
        }

        if let Some(alive) = &self.imgui_alive {
            if !alive.is_alive() {
                // Avoid calling into ImGui allocators after the context has been dropped.
                // Best-effort: leak the Plot3D context instead of risking UB.
                return;
            }
        }

        unsafe {
            let prev_imgui = imgui_sys::igGetCurrentContext();
            imgui_sys::igSetCurrentContext(self.imgui_ctx_raw);

            if sys::ImPlot3D_GetCurrentContext() == self.raw {
                sys::ImPlot3D_SetCurrentContext(std::ptr::null_mut());
            }
            sys::ImPlot3D_DestroyContext(self.raw);

            imgui_sys::igSetCurrentContext(prev_imgui);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Plot3DContext;
    use crate::ui::Plot3DContextBinding;
    use crate::{Context, sys};
    use std::mem::{align_of, size_of};
    use std::ptr;
    use std::sync::{Mutex, OnceLock};

    fn test_guard() -> std::sync::MutexGuard<'static, ()> {
        static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
        GUARD.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    #[test]
    fn ffi_layout_implot3d_point_is_3_f64() {
        assert_eq!(size_of::<sys::ImPlot3DPoint>(), 3 * size_of::<f64>());
        assert_eq!(align_of::<sys::ImPlot3DPoint>(), align_of::<f64>());
    }

    #[test]
    fn plot3d_ui_binds_own_context() {
        let _guard = test_guard();
        let imgui = Context::create();
        let plot_a = Plot3DContext::create(&imgui);
        let raw_a = plot_a.raw;
        let plot_b = Plot3DContext::create(&imgui);
        let raw_b = plot_b.raw;

        unsafe { sys::ImPlot3D_SetCurrentContext(raw_b) };

        Plot3DContextBinding {
            plot_ctx_raw: plot_a.raw,
            imgui_ctx_raw: plot_a.imgui_ctx_raw,
        }
        .bind();

        assert_eq!(unsafe { sys::ImPlot3D_GetCurrentContext() }, raw_a);
    }

    #[test]
    fn plot3d_current_returns_none_without_current_context() {
        let _guard = test_guard();
        let _imgui = Context::create();

        unsafe { sys::ImPlot3D_SetCurrentContext(ptr::null_mut()) };
        assert!(unsafe { Plot3DContext::current() }.is_none());
    }

    #[test]
    fn plot3d_current_wrapper_reports_current_context() {
        let _guard = test_guard();
        let imgui = Context::create();
        let plot = Plot3DContext::create(&imgui);
        let raw = plot.raw;

        let current =
            unsafe { Plot3DContext::current() }.expect("expected current ImPlot3D context");

        assert_eq!(unsafe { current.raw() }, raw);
    }

    #[test]
    fn plot3d_ui_rejects_wrong_imgui_context() {
        let _guard = test_guard();
        let imgui_a = Context::create();
        let plot_a = Plot3DContext::create(&imgui_a);
        let suspended_a = imgui_a.suspend();
        let imgui_b = Context::create();

        let previous = unsafe { dear_imgui_rs::sys::igGetCurrentContext() };
        struct RestoreCurrentContext(*mut dear_imgui_rs::sys::ImGuiContext);
        impl Drop for RestoreCurrentContext {
            fn drop(&mut self) {
                unsafe { dear_imgui_rs::sys::igSetCurrentContext(self.0) };
            }
        }
        let _restore = RestoreCurrentContext(previous);

        assert_eq!(
            unsafe { dear_imgui_rs::sys::igGetCurrentContext() },
            imgui_b.as_raw()
        );
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Plot3DContextBinding {
                plot_ctx_raw: plot_a.raw,
                imgui_ctx_raw: plot_a.imgui_ctx_raw,
            }
            .bind();
        }))
        .expect_err("expected wrong ImGui context to panic");

        let message = panic
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| panic.downcast_ref::<&'static str>().copied())
            .unwrap_or("");
        assert!(message.contains("Plot3DUi must be used with the currently-active ImGui context"));
        drop(plot_a);
        drop(_restore);
        drop(imgui_b);
        drop(suspended_a);
    }
}
