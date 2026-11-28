use crate::sys;
use crate::ui::Plot3DUi;
use dear_imgui_rs::{Context, ContextBinding, ContextThreadLease, Ui};
use std::cell::{RefCell, RefMut};

/// Plot3D context wrapper
///
/// Plotting requires the originating Dear ImGui context. The native plot allocation
/// reserves its owner thread until this `Plot3DContext` is dropped.
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
/// let plot_ui = ui.implot3d(&plot3d_ctx);
/// ```
pub struct Plot3DContext {
    pub(crate) raw: *mut sys::ImPlot3DContext,
    pub(crate) imgui_binding: ContextBinding,
    plot_scope: RefCell<()>,
    _thread_lease: ContextThreadLease,
}

impl Plot3DContext {
    pub(crate) fn enter_plot(&self) -> RefMut<'_, ()> {
        self.plot_scope
            .try_borrow_mut()
            .expect("ImPlot3D requires a context with no open plot")
    }

    /// Try to create a new ImPlot3D context.
    ///
    /// This should be called once after creating your ImGui context.
    pub fn try_create(imgui: &Context) -> dear_imgui_rs::ImGuiResult<Self> {
        let thread_lease = ContextThreadLease::acquire()?;
        let imgui_binding = imgui.binding();
        let ctx = imgui_binding.with_bound_context(|| unsafe {
            let prev_plot = sys::ImPlot3D_GetCurrentContext();
            let ctx = sys::ImPlot3D_CreateContext();
            if sys::ImPlot3D_GetCurrentContext() != prev_plot {
                sys::ImPlot3D_SetCurrentContext(prev_plot);
            }
            ctx
        });
        if ctx.is_null() {
            return Err(dear_imgui_rs::ContextError::Creation {
                reason: "ImPlot3D_CreateContext returned null".into(),
            }
            .into());
        }

        Ok(Self {
            raw: ctx,
            imgui_binding,
            plot_scope: RefCell::default(),
            _thread_lease: thread_lease,
        })
    }

    /// Create a new ImPlot3D context (panics on error).
    pub fn create(imgui: &Context) -> Self {
        Self::try_create(imgui).expect("Failed to create ImPlot3D context")
    }

    /// Get the raw ImPlot3D context pointer without changing the current context.
    ///
    /// # Safety
    ///
    /// Both contexts must be alive and correctly bound. Keep wrapper-owned plots, cached
    /// items and style stacks intact. Balance raw Begin/End and Push/Pop pairs within their
    /// own scopes. This wrapper retains sole ownership of the native allocation.
    pub unsafe fn raw(&self) -> *mut sys::ImPlot3DContext {
        self.raw
    }

    /// Get a per-frame plotting interface
    ///
    /// Call this once per frame to get access to plotting functions.
    /// The returned `Plot3DUi` borrows both this plot context and the frame.
    ///
    /// ```compile_fail,E0515
    /// use dear_imgui_rs::Ui;
    /// use dear_implot3d::{Plot3DContext, Plot3DUi};
    /// fn view(ui: &Ui, plot: Plot3DContext) -> Plot3DUi<'_> {
    ///     plot.get_plot_ui(ui)
    /// }
    /// ```
    pub fn get_plot_ui<'ui>(&'ui self, ui: &'ui Ui) -> Plot3DUi<'ui> {
        assert_eq!(
            ui.context_id(),
            self.imgui_binding.id(),
            "dear-implot3d: Plot3DContext::get_plot_ui() requires a Ui from the owning ImGui context"
        );
        Plot3DUi {
            imgui: ui,
            context: self,
        }
    }
}

impl Drop for Plot3DContext {
    fn drop(&mut self) {
        self.invalidate_saved_binding();
        // DestroyContext frees the plot's own allocations and clears GImPlot3D only if current.
        let raw = self.raw;
        if self
            .imgui_binding
            .try_with_bound_context(|| unsafe { sys::ImPlot3D_DestroyContext(raw) })
            .is_err()
        {
            // The thread lease still covers the allocator after the ImGui context has gone.
            unsafe { sys::ImPlot3D_DestroyContext(raw) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Plot3DContext;
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

        plot_a.with_bound_context(|| {
            assert_eq!(unsafe { sys::ImPlot3D_GetCurrentContext() }, raw_a);
        });

        assert_eq!(unsafe { sys::ImPlot3D_GetCurrentContext() }, raw_b);

        drop(plot_b);
        drop(plot_a);
    }

    #[test]
    fn scoped_styles_and_plot_cleanup_bind_their_owner() {
        let _guard = test_guard();
        let mut imgui = Context::create();
        {
            let io = imgui.io_mut();
            io.set_display_size([800.0, 600.0]);
            io.set_delta_time(1.0 / 60.0);
        }
        imgui
            .font_atlas()
            .try_claim_legacy_renderer()
            .expect("headless test requires the legacy font-atlas capability")
            .build();
        let plot_a = Plot3DContext::create(&imgui);
        let plot_b = Plot3DContext::create(&imgui);
        let raw_b = plot_b.raw;

        {
            let frame = imgui.begin_frame();
            let plot_ui = plot_a.get_plot_ui(frame.ui());
            let style = || plot_a.with_bound_context(|| unsafe { *sys::ImPlot3D_GetStyle() });
            let initial = style();
            plot_ui.with_style_vars(
                &[
                    crate::Plot3DStyleVar::FillAlpha(0.5),
                    crate::Plot3DStyleVar::PlotPadding([3.0, 4.0]),
                    crate::Plot3DStyleVar::Marker(crate::Marker3D::Square),
                ],
                || {
                    let applied = style();
                    assert_eq!(applied.FillAlpha, 0.5);
                    assert_eq!([applied.PlotPadding.x, applied.PlotPadding.y], [3.0, 4.0]);
                    assert_eq!(applied.Marker, crate::Marker3D::Square as i32);
                    unsafe { sys::ImPlot3D_SetCurrentContext(raw_b) };
                },
            );
            assert_eq!(unsafe { sys::ImPlot3D_GetCurrentContext() }, raw_b);
            assert_eq!(style().FillAlpha, initial.FillAlpha);
            assert_eq!(style().PlotPadding, initial.PlotPadding);
            assert_eq!(style().Marker, initial.Marker);
            plot_ui
                .begin_plot("plot")
                .build(|plot| {
                    assert!(plot.get_frame_size()[0] > 0.0);
                    assert_eq!(unsafe { sys::ImPlot3D_GetCurrentContext() }, raw_b);
                })
                .expect("visible plot");
            assert_eq!(unsafe { sys::ImPlot3D_GetCurrentContext() }, raw_b);
        }

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
    fn plot3d_owner_keeps_its_thread_and_destroys_the_native_plot_after_imgui_drops() {
        let _guard = test_guard();
        let imgui = Context::create();
        let plot = Plot3DContext::create(&imgui);
        unsafe { sys::ImPlot3D_SetCurrentContext(plot.raw) };
        drop(imgui);

        assert!(matches!(
            plot.imgui_binding.try_with_bound_context(|| ()),
            Err(dear_imgui_rs::ContextError::Binding(
                dear_imgui_rs::error::context::BindingKind::NativeDestroyed
            ))
        ));
        std::thread::spawn(|| {
            assert!(matches!(
                dear_imgui_rs::ContextThreadLease::acquire(),
                Err(dear_imgui_rs::ContextError::ThreadConflict)
            ))
        })
        .join()
        .unwrap();
        drop(plot);
        std::thread::spawn(|| {
            let _lease = dear_imgui_rs::ContextThreadLease::acquire().unwrap();
            assert!(unsafe { sys::ImPlot3D_GetCurrentContext() }.is_null());
        })
        .join()
        .unwrap();
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
    fn nested_plot_bindings_do_not_restore_a_destroyed_generation() {
        let _guard = test_guard();
        let imgui = Context::create();
        let previous = Plot3DContext::create(&imgui);
        let outer = Plot3DContext::create(&imgui);
        let inner = Plot3DContext::create(&imgui);
        unsafe { sys::ImPlot3D_SetCurrentContext(previous.raw) };

        let replacement = outer.with_bound_context(|| {
            let replacement = inner.with_bound_context(|| {
                drop(previous);
                Plot3DContext::create(&imgui)
            });
            assert_eq!(unsafe { sys::ImPlot3D_GetCurrentContext() }, outer.raw);
            replacement
        });
        let restored = unsafe { sys::ImPlot3D_GetCurrentContext() };
        unsafe { sys::ImPlot3D_SetCurrentContext(std::ptr::null_mut()) };
        assert!(restored.is_null());

        replacement.with_bound_context(|| {
            assert_eq!(
                unsafe { sys::ImPlot3D_GetCurrentContext() },
                replacement.raw
            );
        });
    }

    #[test]
    fn plot3d_ui_binds_owner_context() {
        let _guard = test_guard();
        let imgui_a = Context::create();
        let plot_a = Plot3DContext::create(&imgui_a);
        let imgui_a_raw = imgui_a.as_raw();
        let imgui_b = Context::create();
        unsafe { dear_imgui_rs::sys::igSetCurrentContext(imgui_b.as_raw()) };

        assert_eq!(
            unsafe { dear_imgui_rs::sys::igGetCurrentContext() },
            imgui_b.as_raw()
        );
        plot_a.with_bound_context(|| {
            assert_eq!(
                unsafe { dear_imgui_rs::sys::igGetCurrentContext() },
                imgui_a_raw
            );
        });
        assert_eq!(
            unsafe { dear_imgui_rs::sys::igGetCurrentContext() },
            imgui_b.as_raw()
        );
        drop(imgui_a);
        drop(plot_a);
        drop(imgui_b);
    }
}
