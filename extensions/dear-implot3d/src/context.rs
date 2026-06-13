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
    pub(crate) fn binding(&self) -> Plot3DContextBinding {
        Plot3DContextBinding {
            plot_ctx_raw: self.raw,
            imgui_ctx_raw: self.imgui_ctx_raw,
        }
    }

    pub(crate) fn assert_imgui_alive(&self, caller: &str) {
        if let Some(alive) = &self.imgui_alive {
            assert!(alive.is_alive(), "{caller}: ImGui context has been dropped");
        }
    }

    /// Try to create a new ImPlot3D context.
    ///
    /// This should be called once after creating your ImGui context.
    pub fn try_create(imgui: &Context) -> dear_imgui_rs::ImGuiResult<Self> {
        let imgui_ctx_raw = imgui.as_raw();
        let imgui_alive = Some(imgui.alive_token());
        let prev_imgui = unsafe { imgui_sys::igGetCurrentContext() };
        let prev_plot = unsafe { sys::ImPlot3D_GetCurrentContext() };
        unsafe {
            if prev_imgui != imgui_ctx_raw {
                imgui_sys::igSetCurrentContext(imgui_ctx_raw);
            }
            let ctx = sys::ImPlot3D_CreateContext();
            if sys::ImPlot3D_GetCurrentContext() != prev_plot {
                sys::ImPlot3D_SetCurrentContext(prev_plot);
            }
            if prev_imgui != imgui_ctx_raw {
                imgui_sys::igSetCurrentContext(prev_imgui);
            }
            if ctx.is_null() {
                return Err(dear_imgui_rs::ImGuiError::context_creation(
                    "ImPlot3D_CreateContext returned null",
                ));
            }

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

    /// Set this context as the current ImPlot3D context.
    /// Get a raw pointer to the current ImPlot3D style
    ///
    /// This is an advanced function for direct style manipulation.
    /// Prefer using the safe style functions in the `style` module.
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
        let ui_ctx_raw = ui.with_bound_context(|| unsafe { imgui_sys::igGetCurrentContext() });
        assert_eq!(
            ui_ctx_raw, self.imgui_ctx_raw,
            "dear-implot3d: Plot3DContext::get_plot_ui() requires a Ui from the owning ImGui context"
        );
        Plot3DUi {
            _ui: ui,
            binding: self.binding(),
            imgui_alive: self.imgui_alive.clone(),
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
            let prev_plot = sys::ImPlot3D_GetCurrentContext();
            let restore_plot = if prev_plot == self.raw {
                std::ptr::null_mut()
            } else {
                prev_plot
            };
            imgui_sys::igSetCurrentContext(self.imgui_ctx_raw);
            sys::ImPlot3D_DestroyContext(self.raw);
            sys::ImPlot3D_SetCurrentContext(restore_plot);
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
    use std::sync::{Mutex, OnceLock};

    fn test_guard() -> std::sync::MutexGuard<'static, ()> {
        static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
        GUARD
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|err| err.into_inner())
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

        {
            let _guard = Plot3DContextBinding {
                plot_ctx_raw: plot_a.raw,
                imgui_ctx_raw: plot_a.imgui_ctx_raw,
            }
            .bind();

            assert_eq!(unsafe { sys::ImPlot3D_GetCurrentContext() }, raw_a);
        }

        assert_eq!(unsafe { sys::ImPlot3D_GetCurrentContext() }, raw_b);

        drop(plot_b);
        drop(plot_a);
    }

    #[test]
    fn plot3d_tokens_bind_own_context_before_drop() {
        let _guard = test_guard();
        let mut imgui = Context::create();
        {
            use dear_imgui_rs::BackendFlags;
            let io = imgui.io_mut();
            io.set_display_size([800.0, 600.0]);
            io.set_delta_time(1.0 / 60.0);
            io.set_backend_flags(io.backend_flags() | BackendFlags::RENDERER_HAS_TEXTURES);
        }
        let plot_a = Plot3DContext::create(&imgui);
        let plot_b = Plot3DContext::create(&imgui);
        let raw_b = plot_b.raw;

        {
            let ui = imgui.frame();
            let plot_ui = plot_a.get_plot_ui(&ui);
            let style = plot_ui.push_style_var_f32(crate::Plot3DStyleVar::FillAlpha, 0.5);
            unsafe { sys::ImPlot3D_SetCurrentContext(raw_b) };
            drop(style);
            assert_eq!(unsafe { sys::ImPlot3D_GetCurrentContext() }, raw_b);

            let token = plot_ui
                .begin_plot("token")
                .build()
                .expect("failed to begin 3D plot");
            unsafe { sys::ImPlot3D_SetCurrentContext(raw_b) };
            drop(token);
            assert_eq!(unsafe { sys::ImPlot3D_GetCurrentContext() }, raw_b);
        }
        let _ = imgui.render();

        drop(plot_b);
        drop(plot_a);
    }

    #[test]
    fn dropping_current_plot3d_context_clears_current_context() {
        let _guard = test_guard();
        let imgui = Context::create();
        let plot = Plot3DContext::create(&imgui);
        let raw = plot.raw;

        unsafe { sys::ImPlot3D_SetCurrentContext(raw) };
        drop(plot);

        assert!(unsafe { sys::ImPlot3D_GetCurrentContext() }.is_null());
    }

    #[test]
    fn dropping_non_current_plot3d_context_restores_previous_context() {
        let _guard = test_guard();
        let imgui = Context::create();
        let plot_a = Plot3DContext::create(&imgui);
        let plot_b = Plot3DContext::create(&imgui);
        let raw_b = plot_b.raw;

        unsafe { sys::ImPlot3D_SetCurrentContext(raw_b) };
        drop(plot_a);

        assert_eq!(unsafe { sys::ImPlot3D_GetCurrentContext() }, raw_b);
        drop(plot_b);
    }

    #[test]
    fn plot3d_ui_binds_owner_context() {
        let _guard = test_guard();
        let imgui_a = Context::create();
        let plot_a = Plot3DContext::create(&imgui_a);
        let suspended_a = imgui_a.suspend();
        let imgui_b = Context::create();

        assert_eq!(
            unsafe { dear_imgui_rs::sys::igGetCurrentContext() },
            imgui_b.as_raw()
        );
        {
            let _guard = Plot3DContextBinding {
                plot_ctx_raw: plot_a.raw,
                imgui_ctx_raw: plot_a.imgui_ctx_raw,
            }
            .bind();
            assert_eq!(
                unsafe { dear_imgui_rs::sys::igGetCurrentContext() },
                plot_a.imgui_ctx_raw
            );
        }
        assert_eq!(
            unsafe { dear_imgui_rs::sys::igGetCurrentContext() },
            imgui_b.as_raw()
        );
        drop(suspended_a);
        drop(plot_a);
        drop(imgui_b);
    }
}
