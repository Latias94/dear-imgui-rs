use crate::{Axis, AxisFlags, PlotCond, XAxis, YAxis, sys};
use dear_imgui_rs::{
    Context as ImGuiContext, Ui, with_scratch_txt, with_scratch_txt_slice, with_scratch_txt_two,
};
use dear_imgui_sys as imgui_sys;
use std::os::raw::c_char;
use std::{cell::RefCell, rc::Rc};

fn assert_finite_f64(caller: &str, name: &str, value: f64) {
    assert!(value.is_finite(), "{caller} {name} must be finite");
}

fn assert_finite_vec2(caller: &str, name: &str, value: [f32; 2]) {
    assert!(
        value[0].is_finite() && value[1].is_finite(),
        "{caller} {name} must be finite"
    );
}

fn assert_finite_f64_slice(caller: &str, name: &str, values: &[f64]) {
    assert!(
        values.iter().all(|value| value.is_finite()),
        "{caller} {name} must contain only finite values"
    );
}

fn assert_axis_limit_range(caller: &str, min: f64, max: f64) {
    assert_finite_f64(caller, "min", min);
    assert_finite_f64(caller, "max", max);
    assert!(min != max, "{caller} min and max must differ");
}

fn assert_axis_constraint_range(caller: &str, min: f64, max: f64) {
    assert_finite_f64(caller, "min", min);
    assert_finite_f64(caller, "max", max);
    assert!(min <= max, "{caller} min must be <= max");
}

fn assert_axis_zoom_range(caller: &str, min: f64, max: f64) {
    assert_finite_f64(caller, "min", min);
    assert_finite_f64(caller, "max", max);
    assert!(min > 0.0, "{caller} min must be positive");
    assert!(min <= max, "{caller} min must be <= max");
}

fn axis_tick_count_to_i32(caller: &str, n_ticks: usize) -> i32 {
    assert!(n_ticks > 0, "{caller} n_ticks must be positive");
    i32::try_from(n_ticks)
        .unwrap_or_else(|_| panic!("{caller} n_ticks exceeded ImPlot's i32 range"))
}

/// ImPlot context that manages the plotting state
///
/// This context is separate from the Dear ImGui context but works alongside it.
/// You need both contexts to create plots.
pub struct PlotContext {
    raw: *mut sys::ImPlotContext,
    imgui_ctx_raw: *mut imgui_sys::ImGuiContext,
    imgui_alive: Option<dear_imgui_rs::ContextAliveToken>,
    owns_context: bool,
}

#[derive(Clone, Copy)]
pub(crate) struct PlotContextBinding {
    plot_ctx_raw: *mut sys::ImPlotContext,
    imgui_ctx_raw: *mut imgui_sys::ImGuiContext,
}

impl PlotContextBinding {
    pub(crate) fn bind(self, caller: &str) {
        assert!(
            !self.imgui_ctx_raw.is_null(),
            "{caller} requires an active ImGui context"
        );
        assert!(
            !self.plot_ctx_raw.is_null(),
            "{caller} requires an active ImPlot context"
        );
        assert_eq!(
            unsafe { imgui_sys::igGetCurrentContext() },
            self.imgui_ctx_raw,
            "{caller} must be used with the currently-active ImGui context"
        );
        unsafe {
            sys::ImPlot_SetImGuiContext(self.imgui_ctx_raw);
            sys::ImPlot_SetCurrentContext(self.plot_ctx_raw);
        }
    }
}

impl PlotContext {
    /// Try to create a new ImPlot context
    ///
    /// This should be called after creating the Dear ImGui context.
    /// The ImPlot context will use the same Dear ImGui context internally.
    pub fn try_create(imgui_ctx: &ImGuiContext) -> dear_imgui_rs::ImGuiResult<Self> {
        let imgui_ctx_raw = imgui_ctx.as_raw();
        let imgui_alive = Some(imgui_ctx.alive_token());
        assert_eq!(
            unsafe { imgui_sys::igGetCurrentContext() },
            imgui_ctx_raw,
            "dear-implot: PlotContext must be created with the currently-active ImGui context"
        );

        // Bind ImPlot to the ImGui context before creating.
        // On some toolchains/platforms, not setting this can lead to crashes
        // if ImPlot initialization queries ImGui state during CreateContext.
        unsafe { sys::ImPlot_SetImGuiContext(imgui_ctx_raw) };

        let raw = unsafe { sys::ImPlot_CreateContext() };
        if raw.is_null() {
            return Err(dear_imgui_rs::ImGuiError::context_creation(
                "ImPlot_CreateContext returned null",
            ));
        }

        // Ensure the newly created context is current (defensive, CreateContext should do this).
        unsafe { sys::ImPlot_SetCurrentContext(raw) };

        Ok(Self {
            raw,
            imgui_ctx_raw,
            imgui_alive,
            owns_context: true,
        })
    }

    /// Create a new ImPlot context (panics on error)
    pub fn create(imgui_ctx: &ImGuiContext) -> Self {
        Self::try_create(imgui_ctx).expect("Failed to create ImPlot context")
    }

    /// Get the current ImPlot context as a non-owning raw-context wrapper.
    ///
    /// Returns None if no context is current
    ///
    /// # Safety
    ///
    /// The returned value does not own the current ImPlot context and cannot prove that the
    /// associated ImGui context remains alive. The caller must ensure the raw ImPlot and ImGui
    /// contexts outlive the returned wrapper and are used on the same thread/context stack.
    pub unsafe fn current() -> Option<Self> {
        let raw = unsafe { sys::ImPlot_GetCurrentContext() };
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

    /// Set this context as the current ImPlot context
    pub fn set_as_current(&self) {
        self.assert_imgui_alive();
        self.binding().bind("dear-implot: PlotContext");
    }

    fn assert_imgui_alive(&self) {
        if let Some(alive) = &self.imgui_alive {
            assert!(
                alive.is_alive(),
                "dear-implot: ImGui context has been dropped"
            );
        }
    }

    fn binding(&self) -> PlotContextBinding {
        PlotContextBinding {
            plot_ctx_raw: self.raw,
            imgui_ctx_raw: self.imgui_ctx_raw,
        }
    }

    /// Get a PlotUi for creating plots
    ///
    /// This borrows both the ImPlot context and the Dear ImGui Ui,
    /// ensuring that plots can only be created when both are available.
    pub fn get_plot_ui<'ui>(&'ui self, ui: &'ui Ui) -> PlotUi<'ui> {
        self.set_as_current();
        PlotUi { context: self, ui }
    }

    /// Get the raw ImPlot context pointer
    ///
    /// # Safety
    ///
    /// The caller must ensure the pointer is used safely and not stored
    /// beyond the lifetime of this context.
    pub unsafe fn raw(&self) -> *mut sys::ImPlotContext {
        self.raw
    }
}

impl Drop for PlotContext {
    fn drop(&mut self) {
        if !self.owns_context || self.raw.is_null() {
            return;
        }

        if let Some(alive) = &self.imgui_alive {
            if !alive.is_alive() {
                // Avoid calling into ImGui allocators after the context has been dropped.
                // Best-effort: leak the ImPlot context instead of risking UB.
                return;
            }
        }

        unsafe {
            let prev_imgui = imgui_sys::igGetCurrentContext();
            imgui_sys::igSetCurrentContext(self.imgui_ctx_raw);
            sys::ImPlot_SetImGuiContext(self.imgui_ctx_raw);

            if sys::ImPlot_GetCurrentContext() == self.raw {
                sys::ImPlot_SetCurrentContext(std::ptr::null_mut());
            }
            sys::ImPlot_DestroyContext(self.raw);

            imgui_sys::igSetCurrentContext(prev_imgui);
        }
    }
}

// ImPlot context is tied to Dear ImGui and not thread-safe to send/share.

/// A temporary reference for building plots
///
/// This struct ensures that plots can only be created when both ImGui and ImPlot
/// contexts are available and properly set up.
pub struct PlotUi<'ui> {
    #[allow(dead_code)]
    context: &'ui PlotContext,
    #[allow(dead_code)]
    ui: &'ui Ui,
}

impl<'ui> PlotUi<'ui> {
    #[inline]
    pub(crate) fn bind(&self) {
        self.context.assert_imgui_alive();
        self.context.binding().bind("dear-implot: PlotUi");
    }

    /// Begin a new plot with the given title
    ///
    /// Returns a PlotToken if the plot was successfully started.
    /// The plot will be automatically ended when the token is dropped.
    pub fn begin_plot(&self, title: &str) -> Option<PlotToken<'_>> {
        let size = sys::ImVec2_c { x: -1.0, y: 0.0 };
        if title.contains('\0') {
            return None;
        }
        self.bind();
        let started = with_scratch_txt(title, |ptr| unsafe { sys::ImPlot_BeginPlot(ptr, size, 0) });

        if started {
            Some(PlotToken::new(
                self.context.binding(),
                self.context.imgui_alive.clone(),
            ))
        } else {
            None
        }
    }

    /// Begin a plot with custom size
    pub fn begin_plot_with_size(&self, title: &str, size: [f32; 2]) -> Option<PlotToken<'_>> {
        assert_finite_vec2("PlotUi::begin_plot_with_size()", "size", size);
        let plot_size = sys::ImVec2_c {
            x: size[0],
            y: size[1],
        };
        if title.contains('\0') {
            return None;
        }
        self.bind();
        let started = with_scratch_txt(title, |ptr| unsafe {
            sys::ImPlot_BeginPlot(ptr, plot_size, 0)
        });

        if started {
            Some(PlotToken::new(
                self.context.binding(),
                self.context.imgui_alive.clone(),
            ))
        } else {
            None
        }
    }

    /// Plot a line with the given label and data
    ///
    /// This is a convenience method that can be called within a plot.
    pub fn plot_line(&self, label: &str, x_data: &[f64], y_data: &[f64]) {
        if x_data.len() != y_data.len() {
            return; // Data length mismatch
        }
        let count = match i32::try_from(x_data.len()) {
            Ok(v) => v,
            Err(_) => return,
        };

        let label = if label.contains('\0') { "" } else { label };
        self.bind();
        with_scratch_txt(label, |ptr| unsafe {
            let spec = crate::plots::plot_spec_from(0, crate::plots::PlotDataLayout::DEFAULT);
            sys::ImPlot_PlotLine_doublePtrdoublePtr(
                ptr,
                x_data.as_ptr(),
                y_data.as_ptr(),
                count,
                spec,
            );
        })
    }

    /// Plot a scatter plot with the given label and data
    pub fn plot_scatter(&self, label: &str, x_data: &[f64], y_data: &[f64]) {
        if x_data.len() != y_data.len() {
            return; // Data length mismatch
        }
        let count = match i32::try_from(x_data.len()) {
            Ok(v) => v,
            Err(_) => return,
        };

        let label = if label.contains('\0') { "" } else { label };
        self.bind();
        with_scratch_txt(label, |ptr| unsafe {
            let spec = crate::plots::plot_spec_from(0, crate::plots::PlotDataLayout::DEFAULT);
            sys::ImPlot_PlotScatter_doublePtrdoublePtr(
                ptr,
                x_data.as_ptr(),
                y_data.as_ptr(),
                count,
                spec,
            );
        })
    }

    /// Plot a polygon with the given label and vertex data.
    pub fn plot_polygon(&self, label: &str, x_data: &[f64], y_data: &[f64]) {
        if x_data.len() != y_data.len() {
            return;
        }
        let count = match i32::try_from(x_data.len()) {
            Ok(v) => v,
            Err(_) => return,
        };

        let label = if label.contains('\0') { "" } else { label };
        self.bind();
        with_scratch_txt(label, |ptr| unsafe {
            let spec = crate::plots::plot_spec_from(0, crate::plots::PlotDataLayout::DEFAULT);
            sys::ImPlot_PlotPolygon_doublePtr(ptr, x_data.as_ptr(), y_data.as_ptr(), count, spec);
        })
    }

    /// Check if the plot area is hovered
    pub fn is_plot_hovered(&self) -> bool {
        self.bind();
        unsafe { sys::ImPlot_IsPlotHovered() }
    }

    /// Get the mouse position in plot coordinates
    pub fn get_plot_mouse_pos(&self, y_axis: Option<crate::YAxisChoice>) -> sys::ImPlotPoint {
        let y_axis_i32 = crate::y_axis_choice_option_to_i32(y_axis);
        let y_axis = match y_axis_i32 {
            0 => 3,
            1 => 4,
            2 => 5,
            _ => 3,
        };
        self.bind();
        unsafe { sys::ImPlot_GetPlotMousePos(0, y_axis) }
    }

    /// Get the mouse position in plot coordinates for specific axes
    pub fn get_plot_mouse_pos_axes(&self, x_axis: XAxis, y_axis: YAxis) -> sys::ImPlotPoint {
        self.bind();
        unsafe { sys::ImPlot_GetPlotMousePos(x_axis as i32, y_axis as i32) }
    }

    /// Set current axes for subsequent plot submissions
    pub fn set_axes(&self, x_axis: XAxis, y_axis: YAxis) {
        self.bind();
        unsafe { sys::ImPlot_SetAxes(x_axis as i32, y_axis as i32) }
    }

    /// Setup a specific X axis
    pub fn setup_x_axis(&self, axis: XAxis, label: Option<&str>, flags: AxisFlags) {
        self.bind();
        let label = label.filter(|s| !s.contains('\0'));
        match label {
            Some(label) => with_scratch_txt(label, |ptr| unsafe {
                sys::ImPlot_SetupAxis(
                    axis as sys::ImAxis,
                    ptr,
                    flags.bits() as sys::ImPlotAxisFlags,
                )
            }),
            None => unsafe {
                sys::ImPlot_SetupAxis(
                    axis as sys::ImAxis,
                    std::ptr::null(),
                    flags.bits() as sys::ImPlotAxisFlags,
                )
            },
        }
    }

    /// Setup a specific Y axis
    pub fn setup_y_axis(&self, axis: YAxis, label: Option<&str>, flags: AxisFlags) {
        self.bind();
        let label = label.filter(|s| !s.contains('\0'));
        match label {
            Some(label) => with_scratch_txt(label, |ptr| unsafe {
                sys::ImPlot_SetupAxis(
                    axis as sys::ImAxis,
                    ptr,
                    flags.bits() as sys::ImPlotAxisFlags,
                )
            }),
            None => unsafe {
                sys::ImPlot_SetupAxis(
                    axis as sys::ImAxis,
                    std::ptr::null(),
                    flags.bits() as sys::ImPlotAxisFlags,
                )
            },
        }
    }

    /// Setup axis limits for a specific X axis
    pub fn setup_x_axis_limits(&self, axis: XAxis, min: f64, max: f64, cond: PlotCond) {
        assert_axis_limit_range("PlotUi::setup_x_axis_limits()", min, max);
        self.bind();
        unsafe {
            sys::ImPlot_SetupAxisLimits(axis as sys::ImAxis, min, max, cond as sys::ImPlotCond)
        }
    }

    /// Setup axis limits for a specific Y axis
    pub fn setup_y_axis_limits(&self, axis: YAxis, min: f64, max: f64, cond: PlotCond) {
        assert_axis_limit_range("PlotUi::setup_y_axis_limits()", min, max);
        self.bind();
        unsafe {
            sys::ImPlot_SetupAxisLimits(axis as sys::ImAxis, min, max, cond as sys::ImPlotCond)
        }
    }

    /// Link an axis to external min/max values (live binding)
    pub fn setup_axis_links(
        &self,
        axis: Axis,
        link_min: Option<&mut f64>,
        link_max: Option<&mut f64>,
    ) {
        let pmin = link_min.map_or(std::ptr::null_mut(), |r| r as *mut f64);
        let pmax = link_max.map_or(std::ptr::null_mut(), |r| r as *mut f64);
        self.bind();
        unsafe { sys::ImPlot_SetupAxisLinks(axis.to_sys(), pmin, pmax) }
    }

    /// Link a raw axis to external min/max values (live binding).
    ///
    /// # Safety
    ///
    /// `axis` must be a valid ImPlot `ImAxis` value for the active plot. Passing an
    /// out-of-range value lets ImPlot index internal axis arrays out of bounds.
    pub unsafe fn setup_axis_links_unchecked(
        &self,
        axis: sys::ImAxis,
        link_min: Option<&mut f64>,
        link_max: Option<&mut f64>,
    ) {
        let pmin = link_min.map_or(std::ptr::null_mut(), |r| r as *mut f64);
        let pmax = link_max.map_or(std::ptr::null_mut(), |r| r as *mut f64);
        self.bind();
        unsafe { sys::ImPlot_SetupAxisLinks(axis, pmin, pmax) }
    }

    /// Setup both axes labels/flags at once
    pub fn setup_axes(
        &self,
        x_label: Option<&str>,
        y_label: Option<&str>,
        x_flags: AxisFlags,
        y_flags: AxisFlags,
    ) {
        self.bind();
        let x_label = x_label.filter(|s| !s.contains('\0'));
        let y_label = y_label.filter(|s| !s.contains('\0'));

        match (x_label, y_label) {
            (Some(x_label), Some(y_label)) => {
                with_scratch_txt_two(x_label, y_label, |xp, yp| unsafe {
                    sys::ImPlot_SetupAxes(
                        xp,
                        yp,
                        x_flags.bits() as sys::ImPlotAxisFlags,
                        y_flags.bits() as sys::ImPlotAxisFlags,
                    )
                })
            }
            (Some(x_label), None) => with_scratch_txt(x_label, |xp| unsafe {
                sys::ImPlot_SetupAxes(
                    xp,
                    std::ptr::null(),
                    x_flags.bits() as sys::ImPlotAxisFlags,
                    y_flags.bits() as sys::ImPlotAxisFlags,
                )
            }),
            (None, Some(y_label)) => with_scratch_txt(y_label, |yp| unsafe {
                sys::ImPlot_SetupAxes(
                    std::ptr::null(),
                    yp,
                    x_flags.bits() as sys::ImPlotAxisFlags,
                    y_flags.bits() as sys::ImPlotAxisFlags,
                )
            }),
            (None, None) => unsafe {
                sys::ImPlot_SetupAxes(
                    std::ptr::null(),
                    std::ptr::null(),
                    x_flags.bits() as sys::ImPlotAxisFlags,
                    y_flags.bits() as sys::ImPlotAxisFlags,
                )
            },
        }
    }

    /// Setup axes limits (both) at once
    pub fn setup_axes_limits(
        &self,
        x_min: f64,
        x_max: f64,
        y_min: f64,
        y_max: f64,
        cond: PlotCond,
    ) {
        assert_axis_limit_range("PlotUi::setup_axes_limits() x axis", x_min, x_max);
        assert_axis_limit_range("PlotUi::setup_axes_limits() y axis", y_min, y_max);
        self.bind();
        unsafe { sys::ImPlot_SetupAxesLimits(x_min, x_max, y_min, y_max, cond as sys::ImPlotCond) }
    }

    /// Call after axis setup to finalize configuration
    pub fn setup_finish(&self) {
        self.bind();
        unsafe { sys::ImPlot_SetupFinish() }
    }

    /// Set next frame limits for a specific axis
    pub fn set_next_x_axis_limits(&self, axis: XAxis, min: f64, max: f64, cond: PlotCond) {
        assert_axis_limit_range("PlotUi::set_next_x_axis_limits()", min, max);
        self.bind();
        unsafe {
            sys::ImPlot_SetNextAxisLimits(axis as sys::ImAxis, min, max, cond as sys::ImPlotCond)
        }
    }

    /// Set next frame limits for a specific axis
    pub fn set_next_y_axis_limits(&self, axis: YAxis, min: f64, max: f64, cond: PlotCond) {
        assert_axis_limit_range("PlotUi::set_next_y_axis_limits()", min, max);
        self.bind();
        unsafe {
            sys::ImPlot_SetNextAxisLimits(axis as sys::ImAxis, min, max, cond as sys::ImPlotCond)
        }
    }

    /// Link an axis to external min/max for next frame
    pub fn set_next_axis_links(
        &self,
        axis: Axis,
        link_min: Option<&mut f64>,
        link_max: Option<&mut f64>,
    ) {
        let pmin = link_min.map_or(std::ptr::null_mut(), |r| r as *mut f64);
        let pmax = link_max.map_or(std::ptr::null_mut(), |r| r as *mut f64);
        self.bind();
        unsafe { sys::ImPlot_SetNextAxisLinks(axis.to_sys(), pmin, pmax) }
    }

    /// Link a raw axis to external min/max for the next frame.
    ///
    /// # Safety
    ///
    /// `axis` must be a valid ImPlot `ImAxis` value. Passing an out-of-range
    /// value lets ImPlot index internal next-plot arrays out of bounds.
    pub unsafe fn set_next_axis_links_unchecked(
        &self,
        axis: sys::ImAxis,
        link_min: Option<&mut f64>,
        link_max: Option<&mut f64>,
    ) {
        let pmin = link_min.map_or(std::ptr::null_mut(), |r| r as *mut f64);
        let pmax = link_max.map_or(std::ptr::null_mut(), |r| r as *mut f64);
        self.bind();
        unsafe { sys::ImPlot_SetNextAxisLinks(axis, pmin, pmax) }
    }

    /// Set next frame limits for both axes
    pub fn set_next_axes_limits(
        &self,
        x_min: f64,
        x_max: f64,
        y_min: f64,
        y_max: f64,
        cond: PlotCond,
    ) {
        assert_axis_limit_range("PlotUi::set_next_axes_limits() x axis", x_min, x_max);
        assert_axis_limit_range("PlotUi::set_next_axes_limits() y axis", y_min, y_max);
        self.bind();
        unsafe {
            sys::ImPlot_SetNextAxesLimits(x_min, x_max, y_min, y_max, cond as sys::ImPlotCond)
        }
    }

    /// Fit next frame both axes
    pub fn set_next_axes_to_fit(&self) {
        self.bind();
        unsafe { sys::ImPlot_SetNextAxesToFit() }
    }

    /// Fit next frame a specific axis
    pub fn set_next_axis_to_fit(&self, axis: Axis) {
        self.bind();
        unsafe { sys::ImPlot_SetNextAxisToFit(axis.to_sys()) }
    }

    /// Fit next frame a raw axis.
    ///
    /// # Safety
    ///
    /// `axis` must be a valid ImPlot `ImAxis` value. Passing an out-of-range
    /// value lets ImPlot index internal next-plot arrays out of bounds.
    pub unsafe fn set_next_axis_to_fit_unchecked(&self, axis: sys::ImAxis) {
        self.bind();
        unsafe { sys::ImPlot_SetNextAxisToFit(axis) }
    }

    /// Fit next frame a specific X axis
    pub fn set_next_x_axis_to_fit(&self, axis: XAxis) {
        self.bind();
        unsafe { sys::ImPlot_SetNextAxisToFit(axis as sys::ImAxis) }
    }

    /// Fit next frame a specific Y axis
    pub fn set_next_y_axis_to_fit(&self, axis: YAxis) {
        self.bind();
        unsafe { sys::ImPlot_SetNextAxisToFit(axis as sys::ImAxis) }
    }

    /// Setup ticks with explicit positions and optional labels for an X axis.
    ///
    /// If `labels` is provided, it must have the same length as `values`.
    pub fn setup_x_axis_ticks_positions(
        &self,
        axis: XAxis,
        values: &[f64],
        labels: Option<&[&str]>,
        keep_default: bool,
    ) {
        assert_finite_f64_slice("PlotUi::setup_x_axis_ticks_positions()", "values", values);
        self.bind();
        let count = match i32::try_from(values.len()) {
            Ok(v) => v,
            Err(_) => return,
        };
        if let Some(labels) = labels {
            if labels.len() != values.len() {
                return;
            }
            let cleaned: Vec<&str> = labels
                .iter()
                .map(|&s| if s.contains('\0') { "" } else { s })
                .collect();
            with_scratch_txt_slice(&cleaned, |ptrs| unsafe {
                sys::ImPlot_SetupAxisTicks_doublePtr(
                    axis as sys::ImAxis,
                    values.as_ptr(),
                    count,
                    ptrs.as_ptr() as *const *const c_char,
                    keep_default,
                )
            })
        } else {
            unsafe {
                sys::ImPlot_SetupAxisTicks_doublePtr(
                    axis as sys::ImAxis,
                    values.as_ptr(),
                    count,
                    std::ptr::null(),
                    keep_default,
                )
            }
        }
    }

    /// Setup ticks with explicit positions and optional labels for a Y axis.
    ///
    /// If `labels` is provided, it must have the same length as `values`.
    pub fn setup_y_axis_ticks_positions(
        &self,
        axis: YAxis,
        values: &[f64],
        labels: Option<&[&str]>,
        keep_default: bool,
    ) {
        assert_finite_f64_slice("PlotUi::setup_y_axis_ticks_positions()", "values", values);
        self.bind();
        let count = match i32::try_from(values.len()) {
            Ok(v) => v,
            Err(_) => return,
        };
        if let Some(labels) = labels {
            if labels.len() != values.len() {
                return;
            }
            let cleaned: Vec<&str> = labels
                .iter()
                .map(|&s| if s.contains('\0') { "" } else { s })
                .collect();
            with_scratch_txt_slice(&cleaned, |ptrs| unsafe {
                sys::ImPlot_SetupAxisTicks_doublePtr(
                    axis as sys::ImAxis,
                    values.as_ptr(),
                    count,
                    ptrs.as_ptr() as *const *const c_char,
                    keep_default,
                )
            })
        } else {
            unsafe {
                sys::ImPlot_SetupAxisTicks_doublePtr(
                    axis as sys::ImAxis,
                    values.as_ptr(),
                    count,
                    std::ptr::null(),
                    keep_default,
                )
            }
        }
    }

    /// Setup ticks on a range with tick count and optional labels for an X axis.
    ///
    /// If `labels` is provided, it must have length `n_ticks`.
    pub fn setup_x_axis_ticks_range(
        &self,
        axis: XAxis,
        v_min: f64,
        v_max: f64,
        n_ticks: usize,
        labels: Option<&[&str]>,
        keep_default: bool,
    ) {
        assert_axis_limit_range("PlotUi::setup_x_axis_ticks_range()", v_min, v_max);
        let n_ticks_i32 = axis_tick_count_to_i32("PlotUi::setup_x_axis_ticks_range()", n_ticks);
        self.bind();
        if let Some(labels) = labels {
            if labels.len() != n_ticks {
                return;
            }
            let cleaned: Vec<&str> = labels
                .iter()
                .map(|&s| if s.contains('\0') { "" } else { s })
                .collect();
            with_scratch_txt_slice(&cleaned, |ptrs| unsafe {
                sys::ImPlot_SetupAxisTicks_double(
                    axis as sys::ImAxis,
                    v_min,
                    v_max,
                    n_ticks_i32,
                    ptrs.as_ptr() as *const *const c_char,
                    keep_default,
                )
            })
        } else {
            unsafe {
                sys::ImPlot_SetupAxisTicks_double(
                    axis as sys::ImAxis,
                    v_min,
                    v_max,
                    n_ticks_i32,
                    std::ptr::null(),
                    keep_default,
                )
            }
        }
    }

    /// Setup ticks on a range with tick count and optional labels for a Y axis.
    ///
    /// If `labels` is provided, it must have length `n_ticks`.
    pub fn setup_y_axis_ticks_range(
        &self,
        axis: YAxis,
        v_min: f64,
        v_max: f64,
        n_ticks: usize,
        labels: Option<&[&str]>,
        keep_default: bool,
    ) {
        assert_axis_limit_range("PlotUi::setup_y_axis_ticks_range()", v_min, v_max);
        let n_ticks_i32 = axis_tick_count_to_i32("PlotUi::setup_y_axis_ticks_range()", n_ticks);
        self.bind();
        if let Some(labels) = labels {
            if labels.len() != n_ticks {
                return;
            }
            let cleaned: Vec<&str> = labels
                .iter()
                .map(|&s| if s.contains('\0') { "" } else { s })
                .collect();
            with_scratch_txt_slice(&cleaned, |ptrs| unsafe {
                sys::ImPlot_SetupAxisTicks_double(
                    axis as sys::ImAxis,
                    v_min,
                    v_max,
                    n_ticks_i32,
                    ptrs.as_ptr() as *const *const c_char,
                    keep_default,
                )
            })
        } else {
            unsafe {
                sys::ImPlot_SetupAxisTicks_double(
                    axis as sys::ImAxis,
                    v_min,
                    v_max,
                    n_ticks_i32,
                    std::ptr::null(),
                    keep_default,
                )
            }
        }
    }

    /// Setup tick label format string for a specific X axis
    pub fn setup_x_axis_format(&self, axis: XAxis, fmt: &str) {
        if fmt.contains('\0') {
            return;
        }
        self.bind();
        with_scratch_txt(fmt, |ptr| unsafe {
            sys::ImPlot_SetupAxisFormat_Str(axis as sys::ImAxis, ptr)
        })
    }

    /// Setup tick label format string for a specific Y axis
    pub fn setup_y_axis_format(&self, axis: YAxis, fmt: &str) {
        if fmt.contains('\0') {
            return;
        }
        self.bind();
        with_scratch_txt(fmt, |ptr| unsafe {
            sys::ImPlot_SetupAxisFormat_Str(axis as sys::ImAxis, ptr)
        })
    }

    /// Setup scale for a specific X axis (pass sys::ImPlotScale variant)
    pub fn setup_x_axis_scale(&self, axis: XAxis, scale: sys::ImPlotScale) {
        self.bind();
        unsafe { sys::ImPlot_SetupAxisScale_PlotScale(axis as sys::ImAxis, scale) }
    }

    /// Setup scale for a specific Y axis (pass sys::ImPlotScale variant)
    pub fn setup_y_axis_scale(&self, axis: YAxis, scale: sys::ImPlotScale) {
        self.bind();
        unsafe { sys::ImPlot_SetupAxisScale_PlotScale(axis as sys::ImAxis, scale) }
    }

    /// Setup axis limits constraints
    pub fn setup_axis_limits_constraints(&self, axis: Axis, v_min: f64, v_max: f64) {
        assert_axis_constraint_range("PlotUi::setup_axis_limits_constraints()", v_min, v_max);
        self.bind();
        unsafe { sys::ImPlot_SetupAxisLimitsConstraints(axis.to_sys(), v_min, v_max) }
    }

    /// Setup raw axis limits constraints.
    ///
    /// # Safety
    ///
    /// `axis` must be a valid ImPlot `ImAxis` value for the active plot. Passing an
    /// out-of-range value lets ImPlot index internal axis arrays out of bounds.
    pub unsafe fn setup_axis_limits_constraints_unchecked(
        &self,
        axis: sys::ImAxis,
        v_min: f64,
        v_max: f64,
    ) {
        assert_axis_constraint_range(
            "PlotUi::setup_axis_limits_constraints_unchecked()",
            v_min,
            v_max,
        );
        self.bind();
        unsafe { sys::ImPlot_SetupAxisLimitsConstraints(axis, v_min, v_max) }
    }

    /// Setup axis zoom constraints
    pub fn setup_axis_zoom_constraints(&self, axis: Axis, z_min: f64, z_max: f64) {
        assert_axis_zoom_range("PlotUi::setup_axis_zoom_constraints()", z_min, z_max);
        self.bind();
        unsafe { sys::ImPlot_SetupAxisZoomConstraints(axis.to_sys(), z_min, z_max) }
    }

    /// Setup raw axis zoom constraints.
    ///
    /// # Safety
    ///
    /// `axis` must be a valid ImPlot `ImAxis` value for the active plot. Passing an
    /// out-of-range value lets ImPlot index internal axis arrays out of bounds.
    pub unsafe fn setup_axis_zoom_constraints_unchecked(
        &self,
        axis: sys::ImAxis,
        z_min: f64,
        z_max: f64,
    ) {
        assert_axis_zoom_range(
            "PlotUi::setup_axis_zoom_constraints_unchecked()",
            z_min,
            z_max,
        );
        self.bind();
        unsafe { sys::ImPlot_SetupAxisZoomConstraints(axis, z_min, z_max) }
    }

    // -------- Formatter (closure) --------
    /// Setup tick label formatter using a Rust closure.
    ///
    /// The closure is kept alive until the current plot ends.
    pub fn setup_x_axis_format_closure<F>(&self, axis: XAxis, f: F) -> AxisFormatterToken
    where
        F: Fn(f64) -> String + Send + Sync + 'static,
    {
        self.bind();
        AxisFormatterToken::new(axis as sys::ImAxis, f)
    }

    /// Setup tick label formatter using a Rust closure.
    ///
    /// The closure is kept alive until the current plot ends.
    pub fn setup_y_axis_format_closure<F>(&self, axis: YAxis, f: F) -> AxisFormatterToken
    where
        F: Fn(f64) -> String + Send + Sync + 'static,
    {
        self.bind();
        AxisFormatterToken::new(axis as sys::ImAxis, f)
    }

    // -------- Transform (closure) --------
    /// Setup custom axis transform using Rust closures (forward/inverse).
    ///
    /// The closures are kept alive until the current plot ends.
    pub fn setup_x_axis_transform_closure<FW, INV>(
        &self,
        axis: XAxis,
        forward: FW,
        inverse: INV,
    ) -> AxisTransformToken
    where
        FW: Fn(f64) -> f64 + Send + Sync + 'static,
        INV: Fn(f64) -> f64 + Send + Sync + 'static,
    {
        self.bind();
        AxisTransformToken::new(axis as sys::ImAxis, forward, inverse)
    }

    /// Setup custom axis transform for Y axis using closures
    pub fn setup_y_axis_transform_closure<FW, INV>(
        &self,
        axis: YAxis,
        forward: FW,
        inverse: INV,
    ) -> AxisTransformToken
    where
        FW: Fn(f64) -> f64 + Send + Sync + 'static,
        INV: Fn(f64) -> f64 + Send + Sync + 'static,
    {
        self.bind();
        AxisTransformToken::new(axis as sys::ImAxis, forward, inverse)
    }
}

// Plot-scope callback storage -------------------------------------------------
//
// ImPlot's axis formatter/transform APIs take function pointers + `user_data`
// pointers, and may call them at any point until the current plot ends.
//
// Returning a standalone token that owns the closure is unsound: safe Rust code
// could drop the token early, leaving ImPlot with a dangling `user_data` pointer.
//
// To keep the safe API sound without forcing users to manually retain tokens,
// we store callback holders in thread-local, plot-scoped storage that is
// created when a plot begins and destroyed when the plot ends.

#[derive(Default)]
struct PlotScopeStorage {
    formatters: Vec<Box<FormatterHolder>>,
    transforms: Vec<Box<TransformHolder>>,
}

thread_local! {
    static PLOT_SCOPE_STACK: RefCell<Vec<PlotScopeStorage>> = const { RefCell::new(Vec::new()) };
}

fn with_plot_scope_storage<T>(f: impl FnOnce(&mut PlotScopeStorage) -> T) -> Option<T> {
    PLOT_SCOPE_STACK.with(|stack| {
        let mut stack = stack.borrow_mut();
        stack.last_mut().map(f)
    })
}

pub(crate) struct PlotScopeGuard {
    _not_send_or_sync: std::marker::PhantomData<Rc<()>>,
}

impl PlotScopeGuard {
    pub(crate) fn new() -> Self {
        PLOT_SCOPE_STACK.with(|stack| stack.borrow_mut().push(PlotScopeStorage::default()));
        Self {
            _not_send_or_sync: std::marker::PhantomData,
        }
    }
}

impl Drop for PlotScopeGuard {
    fn drop(&mut self) {
        PLOT_SCOPE_STACK.with(|stack| {
            let popped = stack.borrow_mut().pop();
            debug_assert!(popped.is_some(), "dear-implot: plot scope stack underflow");
        });
    }
}

// =================== Formatter bridge ===================

struct FormatterHolder {
    func: Box<dyn Fn(f64) -> String + Send + Sync + 'static>,
}

#[must_use]
pub struct AxisFormatterToken {
    _private: (),
    _not_send_or_sync: std::marker::PhantomData<Rc<()>>,
}

impl AxisFormatterToken {
    fn new<F>(axis: sys::ImAxis, f: F) -> Self
    where
        F: Fn(f64) -> String + Send + Sync + 'static,
    {
        let configured = with_plot_scope_storage(|storage| {
            let holder = Box::new(FormatterHolder { func: Box::new(f) });
            let user = &*holder as *const FormatterHolder as *mut std::os::raw::c_void;
            storage.formatters.push(holder);
            unsafe {
                sys::ImPlot_SetupAxisFormat_PlotFormatter(
                    axis as sys::ImAxis,
                    Some(formatter_thunk),
                    user,
                )
            }
        })
        .is_some();

        debug_assert!(
            configured,
            "dear-implot: axis formatter closure must be set within an active plot"
        );

        Self {
            _private: (),
            _not_send_or_sync: std::marker::PhantomData,
        }
    }
}

impl Drop for AxisFormatterToken {
    fn drop(&mut self) {
        // The actual callback lifetime is managed by PlotScopeGuard.
    }
}

unsafe extern "C" fn formatter_thunk(
    value: f64,
    buff: *mut std::os::raw::c_char,
    size: std::os::raw::c_int,
    user_data: *mut std::os::raw::c_void,
) -> std::os::raw::c_int {
    if user_data.is_null() || buff.is_null() || size <= 0 {
        return 0;
    }
    // Safety: ImPlot passes back the same pointer we provided in `AxisFormatterToken::new`.
    let holder = unsafe { &*(user_data as *const FormatterHolder) };
    let s = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (holder.func)(value))) {
        Ok(v) => v,
        Err(_) => {
            eprintln!("dear-implot: panic in axis formatter callback");
            std::process::abort();
        }
    };
    let bytes = s.as_bytes();
    let max = (size - 1).max(0) as usize;
    let n = bytes.len().min(max);

    // Safety: `buff` is assumed to point to a valid buffer of at least `size`
    // bytes, with space for a terminating null. This matches ImPlot's
    // formatter contract.
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), buff as *mut u8, n);
        *buff.add(n) = 0;
    }
    n as std::os::raw::c_int
}

// =================== Transform bridge ===================

struct TransformHolder {
    forward: Box<dyn Fn(f64) -> f64 + Send + Sync + 'static>,
    inverse: Box<dyn Fn(f64) -> f64 + Send + Sync + 'static>,
}

#[must_use]
pub struct AxisTransformToken {
    _private: (),
    _not_send_or_sync: std::marker::PhantomData<Rc<()>>,
}

impl AxisTransformToken {
    fn new<FW, INV>(axis: sys::ImAxis, forward: FW, inverse: INV) -> Self
    where
        FW: Fn(f64) -> f64 + Send + Sync + 'static,
        INV: Fn(f64) -> f64 + Send + Sync + 'static,
    {
        let configured = with_plot_scope_storage(|storage| {
            let holder = Box::new(TransformHolder {
                forward: Box::new(forward),
                inverse: Box::new(inverse),
            });
            let user = &*holder as *const TransformHolder as *mut std::os::raw::c_void;
            storage.transforms.push(holder);
            unsafe {
                sys::ImPlot_SetupAxisScale_PlotTransform(
                    axis as sys::ImAxis,
                    Some(transform_forward_thunk),
                    Some(transform_inverse_thunk),
                    user,
                )
            }
        })
        .is_some();

        debug_assert!(
            configured,
            "dear-implot: axis transform closure must be set within an active plot"
        );

        Self {
            _private: (),
            _not_send_or_sync: std::marker::PhantomData,
        }
    }
}

impl Drop for AxisTransformToken {
    fn drop(&mut self) {
        // The actual callback lifetime is managed by PlotScopeGuard.
    }
}

unsafe extern "C" fn transform_forward_thunk(
    value: f64,
    user_data: *mut std::os::raw::c_void,
) -> f64 {
    if user_data.is_null() {
        return value;
    }
    let holder = unsafe { &*(user_data as *const TransformHolder) };
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (holder.forward)(value))) {
        Ok(v) => v,
        Err(_) => {
            eprintln!("dear-implot: panic in axis transform (forward) callback");
            std::process::abort();
        }
    }
}

unsafe extern "C" fn transform_inverse_thunk(
    value: f64,
    user_data: *mut std::os::raw::c_void,
) -> f64 {
    if user_data.is_null() {
        return value;
    }
    let holder = unsafe { &*(user_data as *const TransformHolder) };
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (holder.inverse)(value))) {
        Ok(v) => v,
        Err(_) => {
            eprintln!("dear-implot: panic in axis transform (inverse) callback");
            std::process::abort();
        }
    }
}

/// Token that represents an active plot
///
/// The plot will be automatically ended when this token is dropped.
pub struct PlotToken<'ui> {
    binding: PlotContextBinding,
    imgui_alive: Option<dear_imgui_rs::ContextAliveToken>,
    _scope: PlotScopeGuard,
    _lifetime: std::marker::PhantomData<&'ui ()>,
}

impl<'ui> PlotToken<'ui> {
    /// Create a new PlotToken (internal use only)
    pub(crate) fn new(
        binding: PlotContextBinding,
        imgui_alive: Option<dear_imgui_rs::ContextAliveToken>,
    ) -> Self {
        Self {
            binding,
            imgui_alive,
            _scope: PlotScopeGuard::new(),
            _lifetime: std::marker::PhantomData,
        }
    }

    /// Manually end the plot
    ///
    /// This is called automatically when the token is dropped,
    /// but you can call it manually if needed.
    pub fn end(self) {
        // The actual ending happens in Drop
    }
}

impl<'ui> Drop for PlotToken<'ui> {
    fn drop(&mut self) {
        if let Some(alive) = &self.imgui_alive {
            assert!(
                alive.is_alive(),
                "dear-implot: ImGui context has been dropped"
            );
        }
        self.binding.bind("dear-implot: PlotToken");
        unsafe {
            sys::ImPlot_EndPlot();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{PlotContext, axis_tick_count_to_i32, sys};
    use crate::{Axis, PlotCond, XAxis, YAxis};
    use dear_imgui_rs::{BackendFlags, Context};
    use std::sync::{Mutex, OnceLock};

    fn test_guard() -> std::sync::MutexGuard<'static, ()> {
        static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
        GUARD.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    fn prepare_imgui(imgui: &mut Context) {
        let io = imgui.io_mut();
        io.set_display_size([800.0, 600.0]);
        io.set_delta_time(1.0 / 60.0);
        io.set_backend_flags(io.backend_flags() | BackendFlags::RENDERER_HAS_TEXTURES);
    }

    #[test]
    fn axis_ticks_range_count_is_checked_before_ffi() {
        assert_eq!(axis_tick_count_to_i32("test", 1), 1);
        assert_eq!(axis_tick_count_to_i32("test", i32::MAX as usize), i32::MAX);

        assert!(
            std::panic::catch_unwind(|| axis_tick_count_to_i32("test", 0)).is_err(),
            "zero tick counts must not cross the safe API boundary"
        );
        assert!(
            std::panic::catch_unwind(|| {
                axis_tick_count_to_i32("test", i32::MAX as usize + 1);
            })
            .is_err(),
            "oversized tick counts must not cross the safe API boundary"
        );
    }

    #[test]
    fn plot_ui_binds_own_context_before_calls() {
        let _guard = test_guard();
        let mut imgui = Context::create();
        prepare_imgui(&mut imgui);
        let plot_a = PlotContext::create(&imgui);
        let raw_a = plot_a.raw;
        let plot_b = PlotContext::create(&imgui);
        let raw_b = plot_b.raw;

        {
            let ui = imgui.frame();
            let plot_ui = plot_a.get_plot_ui(&ui);
            unsafe { sys::ImPlot_SetCurrentContext(raw_b) };

            plot_ui.set_next_axes_to_fit();

            assert_eq!(unsafe { sys::ImPlot_GetCurrentContext() }, raw_a);
        }
        let _ = imgui.render();

        drop(plot_b);
        drop(plot_a);
    }

    #[test]
    fn plot_token_binds_own_context_before_drop() {
        let _guard = test_guard();
        let mut imgui = Context::create();
        prepare_imgui(&mut imgui);
        let plot_a = PlotContext::create(&imgui);
        let raw_a = plot_a.raw;
        let plot_b = PlotContext::create(&imgui);
        let raw_b = plot_b.raw;

        {
            let ui = imgui.frame();
            let plot_ui = plot_a.get_plot_ui(&ui);
            let token = plot_ui.begin_plot("token").expect("failed to begin plot");

            unsafe { sys::ImPlot_SetCurrentContext(raw_b) };
            drop(token);

            assert_eq!(unsafe { sys::ImPlot_GetCurrentContext() }, raw_a);
        }
        let _ = imgui.render();

        drop(plot_b);
        drop(plot_a);
    }

    #[test]
    fn current_context_wrapper_is_non_owning() {
        let _guard = test_guard();
        let imgui = Context::create();
        let plot = PlotContext::create(&imgui);
        let raw = plot.raw;

        let borrowed = unsafe { PlotContext::current() }.expect("expected current ImPlot context");
        drop(borrowed);

        assert_eq!(unsafe { sys::ImPlot_GetCurrentContext() }, raw);
        plot.set_as_current();

        drop(plot);
    }

    #[test]
    #[should_panic(expected = "PlotUi::set_next_x_axis_limits() min must be finite")]
    fn set_next_axis_limits_rejects_non_finite_values_before_ffi() {
        let _guard = test_guard();
        let mut imgui = Context::create();
        prepare_imgui(&mut imgui);
        let plot = PlotContext::create(&imgui);

        {
            let ui = imgui.frame();
            let plot_ui = plot.get_plot_ui(&ui);
            plot_ui.set_next_x_axis_limits(XAxis::X1, f64::NAN, 1.0, PlotCond::Once);
        }
    }

    #[test]
    #[should_panic(expected = "PlotUi::setup_axis_zoom_constraints() min must be positive")]
    fn axis_zoom_constraints_reject_non_positive_min_before_ffi() {
        let _guard = test_guard();
        let mut imgui = Context::create();
        prepare_imgui(&mut imgui);
        let plot = PlotContext::create(&imgui);

        {
            let ui = imgui.frame();
            let plot_ui = plot.get_plot_ui(&ui);
            let token = plot_ui
                .begin_plot("constraints")
                .expect("failed to begin plot");
            plot_ui.setup_axis_zoom_constraints(Axis::Y1, 0.0, 10.0);
            token.end();
        }
    }

    #[test]
    fn typed_axis_apis_accept_valid_axes() {
        let _guard = test_guard();
        let mut imgui = Context::create();
        prepare_imgui(&mut imgui);
        let plot = PlotContext::create(&imgui);

        {
            let ui = imgui.frame();
            let plot_ui = plot.get_plot_ui(&ui);
            plot_ui.set_next_axis_to_fit(Axis::X1);

            let token = plot_ui
                .begin_plot("typed-axis")
                .expect("failed to begin plot");
            plot_ui.setup_x_axis(XAxis::X1, None, crate::AxisFlags::NONE);
            plot_ui.setup_y_axis(YAxis::Y1, None, crate::AxisFlags::NONE);
            let mut min = 0.0;
            let mut max = 1.0;
            plot_ui.setup_axis_links(Axis::Y1, Some(&mut min), Some(&mut max));
            plot_ui.setup_axis_limits_constraints(Axis::Y1, -10.0, 10.0);
            plot_ui.setup_axis_zoom_constraints(Axis::Y1, 0.1, 20.0);
            token.end();
        }

        let _ = imgui.render();
        drop(plot);
    }
}
