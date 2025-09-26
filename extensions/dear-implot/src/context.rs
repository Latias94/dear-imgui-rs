use crate::{AxisFlags, PlotCond, XAxis, YAxis, sys};
use dear_imgui::{Context as ImGuiContext, Ui};
use dear_imgui_sys as imgui_sys;

/// ImPlot context that manages the plotting state
///
/// This context is separate from the Dear ImGui context but works alongside it.
/// You need both contexts to create plots.
pub struct PlotContext {
    raw: *mut sys::ImPlotContext,
}

impl PlotContext {
    /// Try to create a new ImPlot context
    ///
    /// This should be called after creating the Dear ImGui context.
    /// The ImPlot context will use the same Dear ImGui context internally.
    pub fn try_create(_imgui_ctx: &ImGuiContext) -> dear_imgui::ImGuiResult<Self> {
        // Bind ImPlot to the current Dear ImGui context before creating.
        // On some toolchains/platforms, not setting this can lead to crashes
        // if ImPlot initialization queries ImGui state during CreateContext.
        unsafe {
            sys::ImPlot_SetImGuiContext(imgui_sys::igGetCurrentContext());
        }

        let raw = unsafe { sys::ImPlot_CreateContext() };
        if raw.is_null() {
            return Err(dear_imgui::ImGuiError::context_creation(
                "ImPlot_CreateContext returned null",
            ));
        }

        // Ensure the newly created context is current (defensive, CreateContext should do this).
        unsafe {
            sys::ImPlot_SetCurrentContext(raw);
        }

        Ok(Self { raw })
    }

    /// Create a new ImPlot context (panics on error)
    pub fn create(imgui_ctx: &ImGuiContext) -> Self {
        Self::try_create(imgui_ctx).expect("Failed to create ImPlot context")
    }

    /// Get the current ImPlot context
    ///
    /// Returns None if no context is current
    pub fn current() -> Option<Self> {
        let raw = unsafe { sys::ImPlot_GetCurrentContext() };
        if raw.is_null() {
            None
        } else {
            Some(Self { raw })
        }
    }

    /// Set this context as the current ImPlot context
    pub fn set_as_current(&self) {
        unsafe {
            sys::ImPlot_SetCurrentContext(self.raw);
        }
    }

    /// Get a PlotUi for creating plots
    ///
    /// This borrows both the ImPlot context and the Dear ImGui Ui,
    /// ensuring that plots can only be created when both are available.
    pub fn get_plot_ui<'ui>(&'ui self, ui: &'ui Ui) -> PlotUi<'ui> {
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
        if !self.raw.is_null() {
            unsafe {
                sys::ImPlot_DestroyContext(self.raw);
            }
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
    /// Begin a new plot with the given title
    ///
    /// Returns a PlotToken if the plot was successfully started.
    /// The plot will be automatically ended when the token is dropped.
    pub fn begin_plot(&self, title: &str) -> Option<PlotToken<'_>> {
        let title_cstr = std::ffi::CString::new(title).ok()?;

        let size = sys::ImVec2 { x: -1.0, y: 0.0 };
        let started = unsafe { sys::ImPlot_BeginPlot(title_cstr.as_ptr(), size, 0) };

        if started {
            Some(PlotToken::new())
        } else {
            None
        }
    }

    /// Begin a plot with custom size
    pub fn begin_plot_with_size(&self, title: &str, size: [f32; 2]) -> Option<PlotToken<'_>> {
        let title_cstr = std::ffi::CString::new(title).ok()?;

        let plot_size = sys::ImVec2 {
            x: size[0],
            y: size[1],
        };
        let started = unsafe { sys::ImPlot_BeginPlot(title_cstr.as_ptr(), plot_size, 0) };

        if started {
            Some(PlotToken::new())
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

        let label_cstr = std::ffi::CString::new(label).unwrap_or_default();

        unsafe {
            sys::ImPlot_PlotLine_doublePtrdoublePtr(
                label_cstr.as_ptr(),
                x_data.as_ptr(),
                y_data.as_ptr(),
                x_data.len() as i32,
                0,
                0,
                0,
            );
        }
    }

    /// Plot a scatter plot with the given label and data
    pub fn plot_scatter(&self, label: &str, x_data: &[f64], y_data: &[f64]) {
        if x_data.len() != y_data.len() {
            return; // Data length mismatch
        }

        let label_cstr = std::ffi::CString::new(label).unwrap_or_default();

        unsafe {
            sys::ImPlot_PlotScatter_doublePtrdoublePtr(
                label_cstr.as_ptr(),
                x_data.as_ptr(),
                y_data.as_ptr(),
                x_data.len() as i32,
                0,
                0,
                0,
            );
        }
    }

    /// Check if the plot area is hovered
    pub fn is_plot_hovered(&self) -> bool {
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
        let mut out = sys::ImPlotPoint { x: 0.0, y: 0.0 };
        unsafe {
            sys::ImPlot_GetPlotMousePos(&mut out as *mut sys::ImPlotPoint, 0, y_axis);
        }
        out
    }

    /// Get the mouse position in plot coordinates for specific axes
    pub fn get_plot_mouse_pos_axes(&self, x_axis: XAxis, y_axis: YAxis) -> sys::ImPlotPoint {
        let mut out = sys::ImPlotPoint { x: 0.0, y: 0.0 };
        unsafe {
            sys::ImPlot_GetPlotMousePos(
                &mut out as *mut sys::ImPlotPoint,
                x_axis as i32,
                y_axis as i32,
            )
        };
        out
    }

    /// Set current axes for subsequent plot submissions
    pub fn set_axes(&self, x_axis: XAxis, y_axis: YAxis) {
        unsafe { sys::ImPlot_SetAxes(x_axis as i32, y_axis as i32) }
    }

    /// Setup a specific X axis
    pub fn setup_x_axis(&self, axis: XAxis, label: Option<&str>, flags: AxisFlags) {
        let label_cstr = label.and_then(|s| std::ffi::CString::new(s).ok());
        let ptr = label_cstr
            .as_ref()
            .map(|c| c.as_ptr())
            .unwrap_or(std::ptr::null());
        unsafe {
            sys::ImPlot_SetupAxis(
                axis as sys::ImAxis,
                ptr,
                flags.bits() as sys::ImPlotAxisFlags,
            )
        }
    }

    /// Setup a specific Y axis
    pub fn setup_y_axis(&self, axis: YAxis, label: Option<&str>, flags: AxisFlags) {
        let label_cstr = label.and_then(|s| std::ffi::CString::new(s).ok());
        let ptr = label_cstr
            .as_ref()
            .map(|c| c.as_ptr())
            .unwrap_or(std::ptr::null());
        unsafe {
            sys::ImPlot_SetupAxis(
                axis as sys::ImAxis,
                ptr,
                flags.bits() as sys::ImPlotAxisFlags,
            )
        }
    }

    /// Setup axis limits for a specific X axis
    pub fn setup_x_axis_limits(&self, axis: XAxis, min: f64, max: f64, cond: PlotCond) {
        unsafe {
            sys::ImPlot_SetupAxisLimits(axis as sys::ImAxis, min, max, cond as sys::ImPlotCond)
        }
    }

    /// Setup axis limits for a specific Y axis
    pub fn setup_y_axis_limits(&self, axis: YAxis, min: f64, max: f64, cond: PlotCond) {
        unsafe {
            sys::ImPlot_SetupAxisLimits(axis as sys::ImAxis, min, max, cond as sys::ImPlotCond)
        }
    }

    /// Link an axis to external min/max values (live binding)
    pub fn setup_axis_links(
        &self,
        axis: i32,
        link_min: Option<&mut f64>,
        link_max: Option<&mut f64>,
    ) {
        let pmin = link_min.map_or(std::ptr::null_mut(), |r| r as *mut f64);
        let pmax = link_max.map_or(std::ptr::null_mut(), |r| r as *mut f64);
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
        let x_c = x_label.and_then(|s| std::ffi::CString::new(s).ok());
        let y_c = y_label.and_then(|s| std::ffi::CString::new(s).ok());
        let xp = x_c.as_ref().map(|c| c.as_ptr()).unwrap_or(std::ptr::null());
        let yp = y_c.as_ref().map(|c| c.as_ptr()).unwrap_or(std::ptr::null());
        unsafe {
            sys::ImPlot_SetupAxes(
                xp,
                yp,
                x_flags.bits() as sys::ImPlotAxisFlags,
                y_flags.bits() as sys::ImPlotAxisFlags,
            )
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
        unsafe { sys::ImPlot_SetupAxesLimits(x_min, x_max, y_min, y_max, cond as sys::ImPlotCond) }
    }

    /// Call after axis setup to finalize configuration
    pub fn setup_finish(&self) {
        unsafe { sys::ImPlot_SetupFinish() }
    }

    /// Set next frame limits for a specific axis
    pub fn set_next_x_axis_limits(&self, axis: XAxis, min: f64, max: f64, cond: PlotCond) {
        unsafe {
            sys::ImPlot_SetNextAxisLimits(axis as sys::ImAxis, min, max, cond as sys::ImPlotCond)
        }
    }

    /// Set next frame limits for a specific axis
    pub fn set_next_y_axis_limits(&self, axis: YAxis, min: f64, max: f64, cond: PlotCond) {
        unsafe {
            sys::ImPlot_SetNextAxisLimits(axis as sys::ImAxis, min, max, cond as sys::ImPlotCond)
        }
    }

    /// Link an axis to external min/max for next frame
    pub fn set_next_axis_links(
        &self,
        axis: i32,
        link_min: Option<&mut f64>,
        link_max: Option<&mut f64>,
    ) {
        let pmin = link_min.map_or(std::ptr::null_mut(), |r| r as *mut f64);
        let pmax = link_max.map_or(std::ptr::null_mut(), |r| r as *mut f64);
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
        unsafe {
            sys::ImPlot_SetNextAxesLimits(x_min, x_max, y_min, y_max, cond as sys::ImPlotCond)
        }
    }

    /// Fit next frame both axes
    pub fn set_next_axes_to_fit(&self) {
        unsafe { sys::ImPlot_SetNextAxesToFit() }
    }

    /// Fit next frame a specific axis (raw)
    pub fn set_next_axis_to_fit(&self, axis: i32) {
        unsafe { sys::ImPlot_SetNextAxisToFit(axis as sys::ImAxis) }
    }

    /// Fit next frame a specific X axis
    pub fn set_next_x_axis_to_fit(&self, axis: XAxis) {
        unsafe { sys::ImPlot_SetNextAxisToFit(axis as sys::ImAxis) }
    }

    /// Fit next frame a specific Y axis
    pub fn set_next_y_axis_to_fit(&self, axis: YAxis) {
        unsafe { sys::ImPlot_SetNextAxisToFit(axis as sys::ImAxis) }
    }

    /// Setup ticks with explicit positions and optional labels for an X axis
    pub fn setup_x_axis_ticks_positions(
        &self,
        axis: XAxis,
        values: &[f64],
        labels: Option<&[&str]>,
        keep_default: bool,
    ) {
        let cstrs: Option<Vec<std::ffi::CString>> = labels.map(|ls| {
            ls.iter()
                .map(|&s| std::ffi::CString::new(s).unwrap_or_default())
                .collect()
        });
        // To keep lifetimes, allocate a temp Vec if labels present
        if let Some(vec) = cstrs {
            let raw: Vec<*const i8> = vec.iter().map(|c| c.as_ptr()).collect();
            unsafe {
                sys::ImPlot_SetupAxisTicks_doublePtr(
                    axis as sys::ImAxis,
                    values.as_ptr(),
                    values.len() as i32,
                    raw.as_ptr(),
                    keep_default,
                )
            }
        } else {
            unsafe {
                sys::ImPlot_SetupAxisTicks_doublePtr(
                    axis as sys::ImAxis,
                    values.as_ptr(),
                    values.len() as i32,
                    std::ptr::null(),
                    keep_default,
                )
            }
        }
    }

    /// Setup ticks with explicit positions and optional labels for a Y axis
    pub fn setup_y_axis_ticks_positions(
        &self,
        axis: YAxis,
        values: &[f64],
        labels: Option<&[&str]>,
        keep_default: bool,
    ) {
        let cstrs: Option<Vec<std::ffi::CString>> = labels.map(|ls| {
            ls.iter()
                .map(|&s| std::ffi::CString::new(s).unwrap_or_default())
                .collect()
        });
        if let Some(cstrs) = cstrs {
            let raw: Vec<*const i8> = cstrs.iter().map(|c| c.as_ptr()).collect();
            unsafe {
                sys::ImPlot_SetupAxisTicks_doublePtr(
                    axis as sys::ImAxis,
                    values.as_ptr(),
                    values.len() as i32,
                    raw.as_ptr(),
                    keep_default,
                )
            }
        } else {
            unsafe {
                sys::ImPlot_SetupAxisTicks_doublePtr(
                    axis as sys::ImAxis,
                    values.as_ptr(),
                    values.len() as i32,
                    std::ptr::null(),
                    keep_default,
                )
            }
        }
    }

    /// Setup ticks on a range with tick count and optional labels for an X axis
    pub fn setup_x_axis_ticks_range(
        &self,
        axis: XAxis,
        v_min: f64,
        v_max: f64,
        n_ticks: i32,
        labels: Option<&[&str]>,
        keep_default: bool,
    ) {
        let cstrs: Option<Vec<std::ffi::CString>> = labels.map(|ls| {
            ls.iter()
                .map(|&s| std::ffi::CString::new(s).unwrap_or_default())
                .collect()
        });
        if let Some(cstrs) = cstrs {
            let raw: Vec<*const i8> = cstrs.iter().map(|c| c.as_ptr()).collect();
            unsafe {
                sys::ImPlot_SetupAxisTicks_double(
                    axis as sys::ImAxis,
                    v_min,
                    v_max,
                    n_ticks,
                    raw.as_ptr(),
                    keep_default,
                )
            }
        } else {
            unsafe {
                sys::ImPlot_SetupAxisTicks_double(
                    axis as sys::ImAxis,
                    v_min,
                    v_max,
                    n_ticks,
                    std::ptr::null(),
                    keep_default,
                )
            }
        }
    }

    /// Setup ticks on a range with tick count and optional labels for a Y axis
    pub fn setup_y_axis_ticks_range(
        &self,
        axis: YAxis,
        v_min: f64,
        v_max: f64,
        n_ticks: i32,
        labels: Option<&[&str]>,
        keep_default: bool,
    ) {
        let cstrs: Option<Vec<std::ffi::CString>> = labels.map(|ls| {
            ls.iter()
                .map(|&s| std::ffi::CString::new(s).unwrap_or_default())
                .collect()
        });
        if let Some(cstrs) = cstrs {
            let raw: Vec<*const i8> = cstrs.iter().map(|c| c.as_ptr()).collect();
            unsafe {
                sys::ImPlot_SetupAxisTicks_double(
                    axis as sys::ImAxis,
                    v_min,
                    v_max,
                    n_ticks,
                    raw.as_ptr(),
                    keep_default,
                )
            }
        } else {
            unsafe {
                sys::ImPlot_SetupAxisTicks_double(
                    axis as sys::ImAxis,
                    v_min,
                    v_max,
                    n_ticks,
                    std::ptr::null(),
                    keep_default,
                )
            }
        }
    }

    /// Setup tick label format string for a specific X axis
    pub fn setup_x_axis_format(&self, axis: XAxis, fmt: &str) {
        if let Ok(c) = std::ffi::CString::new(fmt) {
            unsafe { sys::ImPlot_SetupAxisFormat_Str(axis as sys::ImAxis, c.as_ptr()) }
        }
    }

    /// Setup tick label format string for a specific Y axis
    pub fn setup_y_axis_format(&self, axis: YAxis, fmt: &str) {
        if let Ok(c) = std::ffi::CString::new(fmt) {
            unsafe { sys::ImPlot_SetupAxisFormat_Str(axis as sys::ImAxis, c.as_ptr()) }
        }
    }

    /// Setup scale for a specific X axis (pass sys::ImPlotScale variant)
    pub fn setup_x_axis_scale(&self, axis: XAxis, scale: sys::ImPlotScale) {
        unsafe { sys::ImPlot_SetupAxisScale_PlotScale(axis as sys::ImAxis, scale) }
    }

    /// Setup scale for a specific Y axis (pass sys::ImPlotScale variant)
    pub fn setup_y_axis_scale(&self, axis: YAxis, scale: sys::ImPlotScale) {
        unsafe { sys::ImPlot_SetupAxisScale_PlotScale(axis as sys::ImAxis, scale) }
    }

    /// Setup axis limits constraints
    pub fn setup_axis_limits_constraints(&self, axis: i32, v_min: f64, v_max: f64) {
        unsafe { sys::ImPlot_SetupAxisLimitsConstraints(axis as sys::ImAxis, v_min, v_max) }
    }

    /// Setup axis zoom constraints
    pub fn setup_axis_zoom_constraints(&self, axis: i32, z_min: f64, z_max: f64) {
        unsafe { sys::ImPlot_SetupAxisZoomConstraints(axis as sys::ImAxis, z_min, z_max) }
    }

    // -------- Formatter (closure) --------
    /// Setup tick label formatter using a Rust closure (lives until token drop)
    pub fn setup_x_axis_format_closure<F>(&self, axis: XAxis, f: F) -> AxisFormatterToken
    where
        F: Fn(f64) -> String + Send + Sync + 'static,
    {
        AxisFormatterToken::new(axis as sys::ImAxis, f)
    }

    /// Setup tick label formatter using a Rust closure (lives until token drop)
    pub fn setup_y_axis_format_closure<F>(&self, axis: YAxis, f: F) -> AxisFormatterToken
    where
        F: Fn(f64) -> String + Send + Sync + 'static,
    {
        AxisFormatterToken::new(axis as sys::ImAxis, f)
    }

    // -------- Transform (closure) --------
    /// Setup custom axis transform using Rust closures (forward/inverse) valid until token drop
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
        AxisTransformToken::new(axis as sys::ImAxis, forward, inverse)
    }
}

// =================== Formatter bridge ===================

struct FormatterHolder {
    func: Box<dyn Fn(f64) -> String + Send + Sync + 'static>,
}

pub struct AxisFormatterToken {
    holder: Box<FormatterHolder>,
    axis: sys::ImAxis,
}

impl AxisFormatterToken {
    fn new<F>(axis: sys::ImAxis, f: F) -> Self
    where
        F: Fn(f64) -> String + Send + Sync + 'static,
    {
        let holder = Box::new(FormatterHolder { func: Box::new(f) });
        let user = &*holder as *const FormatterHolder as *mut std::os::raw::c_void;
        unsafe {
            sys::ImPlot_SetupAxisFormat_PlotFormatter(
                axis as sys::ImAxis,
                Some(formatter_thunk),
                user,
            )
        }
        Self { holder, axis }
    }
}

impl Drop for AxisFormatterToken {
    fn drop(&mut self) {
        // No explicit reset API; leaving plot scope ends usage. Holder drop frees closure.
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
    let holder = unsafe { &*(user_data as *const FormatterHolder) };
    let s = (holder.func)(value);
    let bytes = s.as_bytes();
    let max = (size - 1).max(0) as usize;
    let n = bytes.len().min(max);
    std::ptr::copy_nonoverlapping(bytes.as_ptr(), buff as *mut u8, n);
    *buff.add(n) = 0;
    n as std::os::raw::c_int
}

// =================== Transform bridge ===================

struct TransformHolder {
    forward: Box<dyn Fn(f64) -> f64 + Send + Sync + 'static>,
    inverse: Box<dyn Fn(f64) -> f64 + Send + Sync + 'static>,
}

pub struct AxisTransformToken {
    holder: Box<TransformHolder>,
    axis: sys::ImAxis,
}

impl AxisTransformToken {
    fn new<FW, INV>(axis: sys::ImAxis, forward: FW, inverse: INV) -> Self
    where
        FW: Fn(f64) -> f64 + Send + Sync + 'static,
        INV: Fn(f64) -> f64 + Send + Sync + 'static,
    {
        let holder = Box::new(TransformHolder {
            forward: Box::new(forward),
            inverse: Box::new(inverse),
        });
        let user = &*holder as *const TransformHolder as *mut std::os::raw::c_void;
        unsafe {
            sys::ImPlot_SetupAxisScale_PlotTransform(
                axis as sys::ImAxis,
                Some(transform_forward_thunk),
                Some(transform_inverse_thunk),
                user,
            )
        }
        Self { holder, axis }
    }
}

impl Drop for AxisTransformToken {
    fn drop(&mut self) {
        // No explicit reset; scope end ends usage.
    }
}

unsafe extern "C" fn transform_forward_thunk(
    value: f64,
    user_data: *mut std::os::raw::c_void,
) -> f64 {
    let holder = unsafe { &*(user_data as *const TransformHolder) };
    (holder.forward)(value)
}

unsafe extern "C" fn transform_inverse_thunk(
    value: f64,
    user_data: *mut std::os::raw::c_void,
) -> f64 {
    let holder = unsafe { &*(user_data as *const TransformHolder) };
    (holder.inverse)(value)
}

/// Token that represents an active plot
///
/// The plot will be automatically ended when this token is dropped.
pub struct PlotToken<'ui> {
    _lifetime: std::marker::PhantomData<&'ui ()>,
}

impl<'ui> PlotToken<'ui> {
    /// Create a new PlotToken (internal use only)
    pub(crate) fn new() -> Self {
        Self {
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
        unsafe {
            sys::ImPlot_EndPlot();
        }
    }
}
