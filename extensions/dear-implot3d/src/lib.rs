//! Dear ImPlot3D - Rust bindings (high level)
//!
//! Safe wrapper over `dear-implot3d-sys`, designed to integrate with
//! `dear-imgui-rs`. Mirrors `dear-implot` design: context + Ui facade,
//! builder-style helpers, optional `mint` inputs.
//!
//! # Quick Start
//!
//! ```no_run
//! use dear_imgui_rs::*;
//! use dear_implot3d::*;
//!
//! let mut imgui_ctx = Context::create();
//! let plot3d_ctx = Plot3DContext::create(&imgui_ctx);
//!
//! // In your main loop:
//! let ui = imgui_ctx.frame();
//! let plot_ui = plot3d_ctx.get_plot_ui(&ui);
//!
//! if let Some(_token) = plot_ui.begin_plot("3D Plot").build() {
//!     let xs = [0.0, 1.0, 2.0];
//!     let ys = [0.0, 1.0, 0.0];
//!     let zs = [0.0, 0.5, 1.0];
//!     plot_ui.plot_line_f32("Line", &xs, &ys, &zs, Line3DFlags::NONE);
//! }
//! ```
//!
//! # Features
//!
//! - **mint**: Enable support for `mint` math types (Point3, Vector3)
//!
//! # Architecture
//!
//! This crate follows the same design patterns as `dear-implot`:
//! - `Plot3DContext`: Manages the ImPlot3D context (create once)
//! - `Plot3DUi`: Per-frame access to plotting functions
//! - RAII tokens: `Plot3DToken` automatically calls `EndPlot` on drop
//! - Builder pattern: Fluent API for configuring plots
//! - Type-safe flags: Using `bitflags!` for compile-time safety

use dear_imgui_rs::texture::TextureRef;
pub use dear_imgui_rs::{Context, Ui};
use dear_implot3d_sys as sys;

mod flags;
mod style;
mod ui_ext;

pub use flags::*;
pub use style::*;
pub use ui_ext::*;
pub mod meshes;
pub mod plots;

/// Show upstream ImPlot3D demos (from C++ demo)
///
/// This displays all available ImPlot3D demos in a single window.
/// Useful for learning and testing the library.
pub fn show_all_demos() {
    unsafe { sys::ImPlot3D_ShowAllDemos() }
}

/// Show the main ImPlot3D demo window (C++ upstream)
///
/// This displays the main demo window with tabs for different plot types.
/// Pass `None` to always show, or `Some(&mut bool)` to control visibility.
///
/// # Example
///
/// ```no_run
/// use dear_implot3d::*;
///
/// let mut show_demo = true;
/// show_demo_window_with_flag(&mut show_demo);
/// ```
pub fn show_demo_window() {
    unsafe { sys::ImPlot3D_ShowDemoWindow(std::ptr::null_mut()) }
}

/// Show the main ImPlot3D demo window with a visibility flag
pub fn show_demo_window_with_flag(p_open: &mut bool) {
    unsafe { sys::ImPlot3D_ShowDemoWindow(p_open as *mut bool) }
}

/// Show the ImPlot3D style editor window
///
/// This displays a window for editing ImPlot3D style settings in real-time.
/// Pass `None` to use the current style, or `Some(&mut ImPlot3DStyle)` to edit a specific style.
pub fn show_style_editor() {
    unsafe { sys::ImPlot3D_ShowStyleEditor(std::ptr::null_mut()) }
}

/// Show the ImPlot3D metrics/debugger window
///
/// This displays performance metrics and debugging information.
/// Pass `None` to always show, or `Some(&mut bool)` to control visibility.
pub fn show_metrics_window() {
    unsafe { sys::ImPlot3D_ShowMetricsWindow(std::ptr::null_mut()) }
}

/// Show the ImPlot3D metrics/debugger window with a visibility flag
pub fn show_metrics_window_with_flag(p_open: &mut bool) {
    unsafe { sys::ImPlot3D_ShowMetricsWindow(p_open as *mut bool) }
}

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
    owned: bool,
}

impl Plot3DContext {
    /// Create a new ImPlot3D context
    ///
    /// This should be called once after creating your ImGui context.
    pub fn create(_imgui: &Context) -> Self {
        unsafe {
            let ctx = sys::ImPlot3D_CreateContext();
            // Ensure our new context is set as current even if another existed
            sys::ImPlot3D_SetCurrentContext(ctx);
        }
        Self { owned: true }
    }

    /// Get a raw pointer to the current ImPlot3D style
    ///
    /// This is an advanced function for direct style manipulation.
    /// Prefer using the safe style functions in the `style` module.
    pub fn raw_style_mut() -> *mut sys::ImPlot3DStyle {
        unsafe { sys::ImPlot3D_GetStyle() }
    }

    /// Get a per-frame plotting interface
    ///
    /// Call this once per frame to get access to plotting functions.
    /// The returned `Plot3DUi` is tied to the lifetime of the `Ui` frame.
    pub fn get_plot_ui<'ui>(&self, ui: &'ui Ui) -> Plot3DUi<'ui> {
        Plot3DUi { _ui: ui }
    }
}

impl Drop for Plot3DContext {
    fn drop(&mut self) {
        if self.owned {
            unsafe {
                sys::ImPlot3D_DestroyContext(std::ptr::null_mut());
            }
        }
    }
}

/// Per-frame access helper mirroring `dear-implot`
///
/// This provides access to all 3D plotting functions. It is tied to the lifetime
/// of the current ImGui frame and should be obtained via `Plot3DContext::get_plot_ui()`.
///
/// # Example
///
/// ```no_run
/// use dear_implot3d::*;
///
/// # let plot_ui: Plot3DUi = todo!();
/// if let Some(_token) = plot_ui.begin_plot("My 3D Plot").build() {
///     plot_ui.setup_axes("X", "Y", "Z", Axis3DFlags::NONE, Axis3DFlags::NONE, Axis3DFlags::NONE);
///
///     let xs = [0.0, 1.0, 2.0];
///     let ys = [0.0, 1.0, 0.0];
///     let zs = [0.0, 0.5, 1.0];
///     plot_ui.plot_line_f32("Line", &xs, &ys, &zs, Line3DFlags::NONE);
/// }
/// ```
pub struct Plot3DUi<'ui> {
    _ui: &'ui Ui,
}

/// RAII token that ends the plot on drop
///
/// This token is returned by `Plot3DBuilder::build()` and automatically calls
/// `ImPlot3D_EndPlot()` when it goes out of scope, ensuring proper cleanup.
pub struct Plot3DToken;

impl<'ui> Plot3DUi<'ui> {
    /// Builder to configure and begin a 3D plot
    ///
    /// Returns a `Plot3DBuilder` that allows you to configure the plot before calling `.build()`.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use dear_implot3d::*;
    ///
    /// # let plot_ui: Plot3DUi = todo!();
    /// if let Some(_token) = plot_ui
    ///     .begin_plot("My Plot")
    ///     .size([600.0, 400.0])
    ///     .flags(Plot3DFlags::NO_LEGEND)
    ///     .build()
    /// {
    ///     // Plot content here
    /// }
    /// ```
    pub fn begin_plot<S: AsRef<str>>(&self, title: S) -> Plot3DBuilder {
        Plot3DBuilder {
            title: title.as_ref().into(),
            size: None,
            flags: Plot3DFlags::empty(),
        }
    }

    /// Convenience: plot a simple 3D line (f32)
    ///
    /// This is a quick way to plot a line without using the builder pattern.
    /// For more control, use the `plots::Line3D` builder.
    ///
    /// # Arguments
    ///
    /// * `label` - Label for the legend
    /// * `xs` - X coordinates
    /// * `ys` - Y coordinates
    /// * `zs` - Z coordinates
    /// * `flags` - Line flags (e.g., `Line3DFlags::SEGMENTS`, `Line3DFlags::LOOP`)
    ///
    /// # Example
    ///
    /// ```no_run
    /// use dear_implot3d::*;
    ///
    /// # let plot_ui: Plot3DUi = todo!();
    /// let xs = [0.0, 1.0, 2.0];
    /// let ys = [0.0, 1.0, 0.0];
    /// let zs = [0.0, 0.5, 1.0];
    /// plot_ui.plot_line_f32("Line", &xs, &ys, &zs, Line3DFlags::NONE);
    /// ```
    pub fn plot_line_f32<S: AsRef<str>>(
        &self,
        label: S,
        xs: &[f32],
        ys: &[f32],
        zs: &[f32],
        flags: Line3DFlags,
    ) {
        if xs.len() != ys.len() || ys.len() != zs.len() {
            return;
        }
        let label_c = match std::ffi::CString::new(label.as_ref()) {
            Ok(s) => s,
            Err(_) => return,
        };
        unsafe {
            sys::ImPlot3D_PlotLine_FloatPtr(
                label_c.as_ptr(),
                xs.as_ptr(),
                ys.as_ptr(),
                zs.as_ptr(),
                xs.len() as i32,
                flags.bits() as i32,
                0,
                0,
            );
        }
    }

    /// Raw line plot (f32) with offset/stride
    pub fn plot_line_f32_raw<S: AsRef<str>>(
        &self,
        label: S,
        xs: &[f32],
        ys: &[f32],
        zs: &[f32],
        flags: Line3DFlags,
        offset: i32,
        stride: i32,
    ) {
        if xs.len() != ys.len() || ys.len() != zs.len() {
            return;
        }
        let label_c = match std::ffi::CString::new(label.as_ref()) {
            Ok(s) => s,
            Err(_) => return,
        };
        unsafe {
            sys::ImPlot3D_PlotLine_FloatPtr(
                label_c.as_ptr(),
                xs.as_ptr(),
                ys.as_ptr(),
                zs.as_ptr(),
                xs.len() as i32,
                flags.bits() as i32,
                offset,
                stride,
            );
        }
    }

    /// Convenience: plot a simple 3D line (f64)
    pub fn plot_line_f64<S: AsRef<str>>(
        &self,
        label: S,
        xs: &[f64],
        ys: &[f64],
        zs: &[f64],
        flags: Line3DFlags,
    ) {
        if xs.len() != ys.len() || ys.len() != zs.len() {
            return;
        }
        let label_c = match std::ffi::CString::new(label.as_ref()) {
            Ok(s) => s,
            Err(_) => return,
        };
        unsafe {
            sys::ImPlot3D_PlotLine_doublePtr(
                label_c.as_ptr(),
                xs.as_ptr(),
                ys.as_ptr(),
                zs.as_ptr(),
                xs.len() as i32,
                flags.bits() as i32,
                0,
                0,
            );
        }
    }

    /// Raw line plot (f64) with offset/stride
    pub fn plot_line_f64_raw<S: AsRef<str>>(
        &self,
        label: S,
        xs: &[f64],
        ys: &[f64],
        zs: &[f64],
        flags: Line3DFlags,
        offset: i32,
        stride: i32,
    ) {
        if xs.len() != ys.len() || ys.len() != zs.len() {
            return;
        }
        let label_c = match std::ffi::CString::new(label.as_ref()) {
            Ok(s) => s,
            Err(_) => return,
        };
        unsafe {
            sys::ImPlot3D_PlotLine_doublePtr(
                label_c.as_ptr(),
                xs.as_ptr(),
                ys.as_ptr(),
                zs.as_ptr(),
                xs.len() as i32,
                flags.bits() as i32,
                offset,
                stride,
            );
        }
    }

    /// Convenience: plot a 3D scatter (f32)
    pub fn plot_scatter_f32<S: AsRef<str>>(
        &self,
        label: S,
        xs: &[f32],
        ys: &[f32],
        zs: &[f32],
        flags: Scatter3DFlags,
    ) {
        if xs.len() != ys.len() || ys.len() != zs.len() {
            return;
        }
        let label_c = match std::ffi::CString::new(label.as_ref()) {
            Ok(s) => s,
            Err(_) => return,
        };
        unsafe {
            sys::ImPlot3D_PlotScatter_FloatPtr(
                label_c.as_ptr(),
                xs.as_ptr(),
                ys.as_ptr(),
                zs.as_ptr(),
                xs.len() as i32,
                flags.bits() as i32,
                0,
                0,
            );
        }
    }

    /// Raw scatter plot (f32) with offset/stride
    pub fn plot_scatter_f32_raw<S: AsRef<str>>(
        &self,
        label: S,
        xs: &[f32],
        ys: &[f32],
        zs: &[f32],
        flags: Scatter3DFlags,
        offset: i32,
        stride: i32,
    ) {
        if xs.len() != ys.len() || ys.len() != zs.len() {
            return;
        }
        let label_c = match std::ffi::CString::new(label.as_ref()) {
            Ok(s) => s,
            Err(_) => return,
        };
        unsafe {
            sys::ImPlot3D_PlotScatter_FloatPtr(
                label_c.as_ptr(),
                xs.as_ptr(),
                ys.as_ptr(),
                zs.as_ptr(),
                xs.len() as i32,
                flags.bits() as i32,
                offset,
                stride,
            );
        }
    }

    /// Convenience: plot a 3D scatter (f64)
    pub fn plot_scatter_f64<S: AsRef<str>>(
        &self,
        label: S,
        xs: &[f64],
        ys: &[f64],
        zs: &[f64],
        flags: Scatter3DFlags,
    ) {
        if xs.len() != ys.len() || ys.len() != zs.len() {
            return;
        }
        let label_c = match std::ffi::CString::new(label.as_ref()) {
            Ok(s) => s,
            Err(_) => return,
        };
        unsafe {
            sys::ImPlot3D_PlotScatter_doublePtr(
                label_c.as_ptr(),
                xs.as_ptr(),
                ys.as_ptr(),
                zs.as_ptr(),
                xs.len() as i32,
                flags.bits() as i32,
                0,
                0,
            );
        }
    }

    /// Raw scatter plot (f64) with offset/stride
    pub fn plot_scatter_f64_raw<S: AsRef<str>>(
        &self,
        label: S,
        xs: &[f64],
        ys: &[f64],
        zs: &[f64],
        flags: Scatter3DFlags,
        offset: i32,
        stride: i32,
    ) {
        if xs.len() != ys.len() || ys.len() != zs.len() {
            return;
        }
        let label_c = match std::ffi::CString::new(label.as_ref()) {
            Ok(s) => s,
            Err(_) => return,
        };
        unsafe {
            sys::ImPlot3D_PlotScatter_doublePtr(
                label_c.as_ptr(),
                xs.as_ptr(),
                ys.as_ptr(),
                zs.as_ptr(),
                xs.len() as i32,
                flags.bits() as i32,
                offset,
                stride,
            );
        }
    }

    /// Convenience: plot triangles from interleaved xyz arrays (count must be multiple of 3)
    pub fn plot_triangles_f32<S: AsRef<str>>(
        &self,
        label: S,
        xs: &[f32],
        ys: &[f32],
        zs: &[f32],
        flags: Triangle3DFlags,
    ) {
        if xs.len() != ys.len() || ys.len() != zs.len() {
            return;
        }
        let label_c = match std::ffi::CString::new(label.as_ref()) {
            Ok(s) => s,
            Err(_) => return,
        };
        unsafe {
            sys::ImPlot3D_PlotTriangle_FloatPtr(
                label_c.as_ptr(),
                xs.as_ptr(),
                ys.as_ptr(),
                zs.as_ptr(),
                xs.len() as i32,
                flags.bits() as i32,
                0,
                0,
            );
        }
    }

    pub fn plot_triangles_f32_raw<S: AsRef<str>>(
        &self,
        label: S,
        xs: &[f32],
        ys: &[f32],
        zs: &[f32],
        flags: Triangle3DFlags,
        offset: i32,
        stride: i32,
    ) {
        if xs.len() != ys.len() || ys.len() != zs.len() {
            return;
        }
        let label_c = match std::ffi::CString::new(label.as_ref()) {
            Ok(s) => s,
            Err(_) => return,
        };
        unsafe {
            sys::ImPlot3D_PlotTriangle_FloatPtr(
                label_c.as_ptr(),
                xs.as_ptr(),
                ys.as_ptr(),
                zs.as_ptr(),
                xs.len() as i32,
                flags.bits() as i32,
                offset,
                stride,
            );
        }
    }

    /// Convenience: plot quads from interleaved xyz arrays (count must be multiple of 4)
    pub fn plot_quads_f32<S: AsRef<str>>(
        &self,
        label: S,
        xs: &[f32],
        ys: &[f32],
        zs: &[f32],
        flags: Quad3DFlags,
    ) {
        if xs.len() != ys.len() || ys.len() != zs.len() {
            return;
        }
        let label_c = match std::ffi::CString::new(label.as_ref()) {
            Ok(s) => s,
            Err(_) => return,
        };
        unsafe {
            sys::ImPlot3D_PlotQuad_FloatPtr(
                label_c.as_ptr(),
                xs.as_ptr(),
                ys.as_ptr(),
                zs.as_ptr(),
                xs.len() as i32,
                flags.bits() as i32,
                0,
                0,
            );
        }
    }

    pub fn plot_quads_f32_raw<S: AsRef<str>>(
        &self,
        label: S,
        xs: &[f32],
        ys: &[f32],
        zs: &[f32],
        flags: Quad3DFlags,
        offset: i32,
        stride: i32,
    ) {
        if xs.len() != ys.len() || ys.len() != zs.len() {
            return;
        }
        let label_c = match std::ffi::CString::new(label.as_ref()) {
            Ok(s) => s,
            Err(_) => return,
        };
        unsafe {
            sys::ImPlot3D_PlotQuad_FloatPtr(
                label_c.as_ptr(),
                xs.as_ptr(),
                ys.as_ptr(),
                zs.as_ptr(),
                xs.len() as i32,
                flags.bits() as i32,
                offset,
                stride,
            );
        }
    }

    /// Convenience: plot triangles from interleaved xyz arrays (f64)
    pub fn plot_triangles_f64<S: AsRef<str>>(
        &self,
        label: S,
        xs: &[f64],
        ys: &[f64],
        zs: &[f64],
        flags: Triangle3DFlags,
    ) {
        if xs.len() != ys.len() || ys.len() != zs.len() {
            return;
        }
        let label_c = match std::ffi::CString::new(label.as_ref()) {
            Ok(s) => s,
            Err(_) => return,
        };
        unsafe {
            sys::ImPlot3D_PlotTriangle_doublePtr(
                label_c.as_ptr(),
                xs.as_ptr(),
                ys.as_ptr(),
                zs.as_ptr(),
                xs.len() as i32,
                flags.bits() as i32,
                0,
                0,
            );
        }
    }

    pub fn plot_triangles_f64_raw<S: AsRef<str>>(
        &self,
        label: S,
        xs: &[f64],
        ys: &[f64],
        zs: &[f64],
        flags: Triangle3DFlags,
        offset: i32,
        stride: i32,
    ) {
        if xs.len() != ys.len() || ys.len() != zs.len() {
            return;
        }
        let label_c = match std::ffi::CString::new(label.as_ref()) {
            Ok(s) => s,
            Err(_) => return,
        };
        unsafe {
            sys::ImPlot3D_PlotTriangle_doublePtr(
                label_c.as_ptr(),
                xs.as_ptr(),
                ys.as_ptr(),
                zs.as_ptr(),
                xs.len() as i32,
                flags.bits() as i32,
                offset,
                stride,
            );
        }
    }

    /// Convenience: plot quads from interleaved xyz arrays (f64)
    pub fn plot_quads_f64<S: AsRef<str>>(
        &self,
        label: S,
        xs: &[f64],
        ys: &[f64],
        zs: &[f64],
        flags: Quad3DFlags,
    ) {
        if xs.len() != ys.len() || ys.len() != zs.len() {
            return;
        }
        let label_c = match std::ffi::CString::new(label.as_ref()) {
            Ok(s) => s,
            Err(_) => return,
        };
        unsafe {
            sys::ImPlot3D_PlotQuad_doublePtr(
                label_c.as_ptr(),
                xs.as_ptr(),
                ys.as_ptr(),
                zs.as_ptr(),
                xs.len() as i32,
                flags.bits() as i32,
                0,
                0,
            );
        }
    }

    pub fn plot_quads_f64_raw<S: AsRef<str>>(
        &self,
        label: S,
        xs: &[f64],
        ys: &[f64],
        zs: &[f64],
        flags: Quad3DFlags,
        offset: i32,
        stride: i32,
    ) {
        if xs.len() != ys.len() || ys.len() != zs.len() {
            return;
        }
        let label_c = match std::ffi::CString::new(label.as_ref()) {
            Ok(s) => s,
            Err(_) => return,
        };
        unsafe {
            sys::ImPlot3D_PlotQuad_doublePtr(
                label_c.as_ptr(),
                xs.as_ptr(),
                ys.as_ptr(),
                zs.as_ptr(),
                xs.len() as i32,
                flags.bits() as i32,
                offset,
                stride,
            );
        }
    }
}

impl Drop for Plot3DToken {
    fn drop(&mut self) {
        unsafe {
            sys::ImPlot3D_EndPlot();
        }
    }
}

/// Plot builder for configuring the 3D plot
pub struct Plot3DBuilder {
    title: String,
    size: Option<[f32; 2]>,
    flags: Plot3DFlags,
}

impl Plot3DBuilder {
    pub fn size(mut self, size: [f32; 2]) -> Self {
        self.size = Some(size);
        self
    }
    pub fn flags(mut self, flags: Plot3DFlags) -> Self {
        self.flags = flags;
        self
    }
    pub fn build(self) -> Option<Plot3DToken> {
        let title_c = std::ffi::CString::new(self.title).ok()?;
        let size = self.size.unwrap_or([0.0, 0.0]);
        let ok = unsafe {
            // Defensive: ensure style.Colormap is in range before plotting
            let style = sys::ImPlot3D_GetStyle();
            if !style.is_null() {
                let count = sys::ImPlot3D_GetColormapCount();
                if count > 0 {
                    if (*style).Colormap < 0 || (*style).Colormap >= count {
                        (*style).Colormap = 0;
                    }
                }
            }
            sys::ImPlot3D_BeginPlot(
                title_c.as_ptr(),
                sys::ImVec2 {
                    x: size[0],
                    y: size[1],
                },
                self.flags.bits() as i32,
            )
        };
        if ok { Some(Plot3DToken) } else { None }
    }
}

/// Optional mint support for inputs
///
/// When the `mint` feature is enabled, you can use `mint::Point3<f32>` and `mint::Vector3<f32>`
/// types directly with plotting functions. This provides interoperability with popular math
/// libraries like `glam`, `nalgebra`, `cgmath`, etc.
///
/// # Example
///
/// ```no_run
/// # #[cfg(feature = "mint")]
/// # {
/// use dear_implot3d::*;
/// use mint::Point3;
///
/// # let plot_ui: Plot3DUi = todo!();
/// let points = vec![
///     Point3 { x: 0.0, y: 0.0, z: 0.0 },
///     Point3 { x: 1.0, y: 1.0, z: 1.0 },
///     Point3 { x: 2.0, y: 0.0, z: 2.0 },
/// ];
///
/// if let Some(_token) = plot_ui.begin_plot("Mint Example").build() {
///     plot_ui.plot_line_mint("Line", &points, Line3DFlags::NONE);
/// }
/// # }
/// ```
#[cfg(feature = "mint")]
impl<'ui> Plot3DUi<'ui> {
    /// Plot a 3D line using `mint::Point3<f32>` points
    ///
    /// This is a convenience function that converts mint points to separate x, y, z arrays.
    pub fn plot_line_mint<S: AsRef<str>>(
        &self,
        label: S,
        pts: &[mint::Point3<f32>],
        flags: Line3DFlags,
    ) {
        let mut xs = Vec::with_capacity(pts.len());
        let mut ys = Vec::with_capacity(pts.len());
        let mut zs = Vec::with_capacity(pts.len());
        for p in pts {
            xs.push(p.x);
            ys.push(p.y);
            zs.push(p.z);
        }
        self.plot_line_f32(label, &xs, &ys, &zs, flags);
    }

    /// Plot a 3D scatter using `mint::Point3<f32>` points
    pub fn plot_scatter_mint<S: AsRef<str>>(
        &self,
        label: S,
        pts: &[mint::Point3<f32>],
        flags: Scatter3DFlags,
    ) {
        let mut xs = Vec::with_capacity(pts.len());
        let mut ys = Vec::with_capacity(pts.len());
        let mut zs = Vec::with_capacity(pts.len());
        for p in pts {
            xs.push(p.x);
            ys.push(p.y);
            zs.push(p.z);
        }
        self.plot_scatter_f32(label, &xs, &ys, &zs, flags);
    }

    /// Plot 3D text at a `mint::Point3<f32>` position
    pub fn plot_text_mint(
        &self,
        text: &str,
        pos: mint::Point3<f32>,
        angle: f32,
        pix_offset: [f32; 2],
    ) {
        self.plot_text(text, pos.x, pos.y, pos.z, angle, pix_offset);
    }

    /// Convert a `mint::Point3<f32>` to pixel coordinates
    pub fn plot_to_pixels_mint(&self, point: mint::Point3<f32>) -> [f32; 2] {
        self.plot_to_pixels([point.x, point.y, point.z])
    }
}

/// Surface (grid) plot builder (f32 variant)
pub struct Surface3DBuilder<'ui> {
    ui: &'ui Plot3DUi<'ui>,
    label: std::ffi::CString,
    xs: &'ui [f32],
    ys: &'ui [f32],
    zs: &'ui [f32],
    scale_min: f64,
    scale_max: f64,
    flags: Surface3DFlags,
}

impl<'ui> Surface3DBuilder<'ui> {
    pub fn scale(mut self, min: f64, max: f64) -> Self {
        self.scale_min = min;
        self.scale_max = max;
        self
    }
    pub fn flags(mut self, flags: Surface3DFlags) -> Self {
        self.flags = flags;
        self
    }
    pub fn plot(self) {
        let x_count = self.xs.len() as i32;
        let y_count = self.ys.len() as i32;
        let expected = (x_count as usize) * (y_count as usize);
        if self.zs.len() != expected {
            return;
        }
        unsafe {
            sys::ImPlot3D_PlotSurface_FloatPtr(
                self.label.as_ptr(),
                self.xs.as_ptr(),
                self.ys.as_ptr(),
                self.zs.as_ptr(),
                x_count,
                y_count,
                self.scale_min,
                self.scale_max,
                self.flags.bits() as i32,
                0,
                0,
            );
        }
    }
}

impl<'ui> Plot3DUi<'ui> {
    /// Start a surface plot (f32)
    pub fn surface_f32<S: AsRef<str>>(
        &'ui self,
        label: S,
        xs: &'ui [f32],
        ys: &'ui [f32],
        zs: &'ui [f32],
    ) -> Surface3DBuilder<'ui> {
        let label_c = std::ffi::CString::new(label.as_ref())
            .unwrap_or_else(|_| std::ffi::CString::new("surface").unwrap());
        Surface3DBuilder {
            ui: self,
            label: label_c,
            xs,
            ys,
            zs,
            scale_min: f64::NAN,
            scale_max: f64::NAN,
            flags: Surface3DFlags::NONE,
        }
    }

    /// Raw surface plot (f32) with offset/stride
    pub fn surface_f32_raw<S: AsRef<str>>(
        &self,
        label: S,
        xs: &[f32],
        ys: &[f32],
        zs: &[f32],
        scale_min: f64,
        scale_max: f64,
        flags: Surface3DFlags,
        offset: i32,
        stride: i32,
    ) {
        let x_count = xs.len() as i32;
        let y_count = ys.len() as i32;
        let label_c = match std::ffi::CString::new(label.as_ref()) {
            Ok(s) => s,
            Err(_) => return,
        };
        unsafe {
            sys::ImPlot3D_PlotSurface_FloatPtr(
                label_c.as_ptr(),
                xs.as_ptr(),
                ys.as_ptr(),
                zs.as_ptr(),
                x_count,
                y_count,
                scale_min,
                scale_max,
                flags.bits() as i32,
                offset,
                stride,
            );
        }
    }
}

/// Image by axes builder
pub struct Image3DByAxesBuilder<'ui> {
    _ui: &'ui Plot3DUi<'ui>,
    label: std::ffi::CString,
    tex_ref: sys::ImTextureRef,
    center: [f32; 3],
    axis_u: [f32; 3],
    axis_v: [f32; 3],
    uv0: [f32; 2],
    uv1: [f32; 2],
    tint: [f32; 4],
    flags: Image3DFlags,
}

impl<'ui> Image3DByAxesBuilder<'ui> {
    pub fn uv(mut self, uv0: [f32; 2], uv1: [f32; 2]) -> Self {
        self.uv0 = uv0;
        self.uv1 = uv1;
        self
    }
    pub fn tint(mut self, col: [f32; 4]) -> Self {
        self.tint = col;
        self
    }
    pub fn flags(mut self, flags: Image3DFlags) -> Self {
        self.flags = flags;
        self
    }
    pub fn plot(self) {
        unsafe {
            sys::ImPlot3D_PlotImage_Vec2(
                self.label.as_ptr(),
                self.tex_ref,
                sys::ImPlot3DPoint {
                    x: self.center[0],
                    y: self.center[1],
                    z: self.center[2],
                },
                sys::ImPlot3DPoint {
                    x: self.axis_u[0],
                    y: self.axis_u[1],
                    z: self.axis_u[2],
                },
                sys::ImPlot3DPoint {
                    x: self.axis_v[0],
                    y: self.axis_v[1],
                    z: self.axis_v[2],
                },
                sys::ImVec2 {
                    x: self.uv0[0],
                    y: self.uv0[1],
                },
                sys::ImVec2 {
                    x: self.uv1[0],
                    y: self.uv1[1],
                },
                sys::ImVec4 {
                    x: self.tint[0],
                    y: self.tint[1],
                    z: self.tint[2],
                    w: self.tint[3],
                },
                self.flags.bits() as i32,
            );
        }
    }
}

/// Image by corners builder
pub struct Image3DByCornersBuilder<'ui> {
    _ui: &'ui Plot3DUi<'ui>,
    label: std::ffi::CString,
    tex_ref: sys::ImTextureRef,
    p0: [f32; 3],
    p1: [f32; 3],
    p2: [f32; 3],
    p3: [f32; 3],
    uv0: [f32; 2],
    uv1: [f32; 2],
    uv2: [f32; 2],
    uv3: [f32; 2],
    tint: [f32; 4],
    flags: Image3DFlags,
}

impl<'ui> Image3DByCornersBuilder<'ui> {
    pub fn uvs(mut self, uv0: [f32; 2], uv1: [f32; 2], uv2: [f32; 2], uv3: [f32; 2]) -> Self {
        self.uv0 = uv0;
        self.uv1 = uv1;
        self.uv2 = uv2;
        self.uv3 = uv3;
        self
    }
    pub fn tint(mut self, col: [f32; 4]) -> Self {
        self.tint = col;
        self
    }
    pub fn flags(mut self, flags: Image3DFlags) -> Self {
        self.flags = flags;
        self
    }
    pub fn plot(self) {
        unsafe {
            sys::ImPlot3D_PlotImage_Plot3DPoInt(
                self.label.as_ptr(),
                self.tex_ref,
                sys::ImPlot3DPoint {
                    x: self.p0[0],
                    y: self.p0[1],
                    z: self.p0[2],
                },
                sys::ImPlot3DPoint {
                    x: self.p1[0],
                    y: self.p1[1],
                    z: self.p1[2],
                },
                sys::ImPlot3DPoint {
                    x: self.p2[0],
                    y: self.p2[1],
                    z: self.p2[2],
                },
                sys::ImPlot3DPoint {
                    x: self.p3[0],
                    y: self.p3[1],
                    z: self.p3[2],
                },
                sys::ImVec2 {
                    x: self.uv0[0],
                    y: self.uv0[1],
                },
                sys::ImVec2 {
                    x: self.uv1[0],
                    y: self.uv1[1],
                },
                sys::ImVec2 {
                    x: self.uv2[0],
                    y: self.uv2[1],
                },
                sys::ImVec2 {
                    x: self.uv3[0],
                    y: self.uv3[1],
                },
                sys::ImVec4 {
                    x: self.tint[0],
                    y: self.tint[1],
                    z: self.tint[2],
                    w: self.tint[3],
                },
                self.flags.bits() as i32,
            );
        }
    }
}

impl<'ui> Plot3DUi<'ui> {
    /// Image oriented by center and axes
    pub fn image_by_axes<S: AsRef<str>, T: Into<TextureRef>>(
        &'ui self,
        label: S,
        tex: T,
        center: [f32; 3],
        axis_u: [f32; 3],
        axis_v: [f32; 3],
    ) -> Image3DByAxesBuilder<'ui> {
        let label_c = std::ffi::CString::new(label.as_ref())
            .unwrap_or_else(|_| std::ffi::CString::new("image").unwrap());
        let tr = tex.into().raw();
        let tex_ref = sys::ImTextureRef {
            _TexData: tr._TexData as *mut sys::ImTextureData,
            _TexID: tr._TexID as sys::ImTextureID,
        };
        Image3DByAxesBuilder {
            _ui: self,
            label: label_c,
            tex_ref,
            center,
            axis_u,
            axis_v,
            uv0: [0.0, 0.0],
            uv1: [1.0, 1.0],
            tint: [1.0, 1.0, 1.0, 1.0],
            flags: Image3DFlags::NONE,
        }
    }

    /// Image by 4 corner points (p0..p3)
    pub fn image_by_corners<S: AsRef<str>, T: Into<TextureRef>>(
        &'ui self,
        label: S,
        tex: T,
        p0: [f32; 3],
        p1: [f32; 3],
        p2: [f32; 3],
        p3: [f32; 3],
    ) -> Image3DByCornersBuilder<'ui> {
        let label_c = std::ffi::CString::new(label.as_ref())
            .unwrap_or_else(|_| std::ffi::CString::new("image").unwrap());
        let tr = tex.into().raw();
        let tex_ref = sys::ImTextureRef {
            _TexData: tr._TexData as *mut sys::ImTextureData,
            _TexID: tr._TexID as sys::ImTextureID,
        };
        Image3DByCornersBuilder {
            _ui: self,
            label: label_c,
            tex_ref,
            p0,
            p1,
            p2,
            p3,
            uv0: [0.0, 0.0],
            uv1: [1.0, 0.0],
            uv2: [1.0, 1.0],
            uv3: [0.0, 1.0],
            tint: [1.0, 1.0, 1.0, 1.0],
            flags: Image3DFlags::NONE,
        }
    }
}

/// Axis helpers
impl<'ui> Plot3DUi<'ui> {
    pub fn setup_axes(
        &self,
        x_label: &str,
        y_label: &str,
        z_label: &str,
        x_flags: Axis3DFlags,
        y_flags: Axis3DFlags,
        z_flags: Axis3DFlags,
    ) {
        let cx = std::ffi::CString::new(x_label).unwrap_or_default();
        let cy = std::ffi::CString::new(y_label).unwrap_or_default();
        let cz = std::ffi::CString::new(z_label).unwrap_or_default();
        unsafe {
            sys::ImPlot3D_SetupAxes(
                cx.as_ptr(),
                cy.as_ptr(),
                cz.as_ptr(),
                x_flags.bits() as i32,
                y_flags.bits() as i32,
                z_flags.bits() as i32,
            )
        }
    }

    pub fn setup_axis(&self, axis: Axis3D, label: &str, flags: Axis3DFlags) {
        let c = std::ffi::CString::new(label).unwrap_or_default();
        unsafe { sys::ImPlot3D_SetupAxis(axis as i32, c.as_ptr(), flags.bits() as i32) }
    }

    pub fn setup_axis_limits(&self, axis: Axis3D, min: f64, max: f64, cond: Plot3DCond) {
        unsafe { sys::ImPlot3D_SetupAxisLimits(axis as i32, min, max, cond as i32) }
    }

    pub fn setup_axes_limits(
        &self,
        x_min: f64,
        x_max: f64,
        y_min: f64,
        y_max: f64,
        z_min: f64,
        z_max: f64,
        cond: Plot3DCond,
    ) {
        unsafe {
            sys::ImPlot3D_SetupAxesLimits(x_min, x_max, y_min, y_max, z_min, z_max, cond as i32)
        }
    }

    pub fn setup_axis_limits_constraints(&self, axis: Axis3D, v_min: f64, v_max: f64) {
        unsafe { sys::ImPlot3D_SetupAxisLimitsConstraints(axis as i32, v_min, v_max) }
    }

    pub fn setup_axis_zoom_constraints(&self, axis: Axis3D, z_min: f64, z_max: f64) {
        unsafe { sys::ImPlot3D_SetupAxisZoomConstraints(axis as i32, z_min, z_max) }
    }

    pub fn setup_axis_ticks_values(
        &self,
        axis: Axis3D,
        values: &[f64],
        labels: Option<&[&str]>,
        keep_default: bool,
    ) {
        let n_ticks = values.len() as i32;
        let labels_ptr = if let Some(lbls) = labels {
            let c_labels: Vec<std::ffi::CString> = lbls
                .iter()
                .map(|s| std::ffi::CString::new(*s).unwrap_or_default())
                .collect();
            let ptrs: Vec<*const i8> = c_labels.iter().map(|cs| cs.as_ptr()).collect();
            ptrs.as_ptr()
        } else {
            std::ptr::null()
        };
        unsafe {
            sys::ImPlot3D_SetupAxisTicks_doublePtr(
                axis as i32,
                values.as_ptr(),
                n_ticks,
                labels_ptr,
                keep_default,
            )
        }
    }

    pub fn setup_axis_ticks_range(
        &self,
        axis: Axis3D,
        v_min: f64,
        v_max: f64,
        n_ticks: i32,
        labels: Option<&[&str]>,
        keep_default: bool,
    ) {
        let labels_ptr = if let Some(lbls) = labels {
            let c_labels: Vec<std::ffi::CString> = lbls
                .iter()
                .map(|s| std::ffi::CString::new(*s).unwrap_or_default())
                .collect();
            let ptrs: Vec<*const i8> = c_labels.iter().map(|cs| cs.as_ptr()).collect();
            ptrs.as_ptr()
        } else {
            std::ptr::null()
        };
        unsafe {
            sys::ImPlot3D_SetupAxisTicks_double(
                axis as i32,
                v_min,
                v_max,
                n_ticks,
                labels_ptr,
                keep_default,
            )
        }
    }

    pub fn setup_box_scale(&self, x: f32, y: f32, z: f32) {
        unsafe { sys::ImPlot3D_SetupBoxScale(x, y, z) }
    }

    pub fn setup_box_rotation(
        &self,
        elevation: f32,
        azimuth: f32,
        animate: bool,
        cond: Plot3DCond,
    ) {
        unsafe { sys::ImPlot3D_SetupBoxRotation_Float(elevation, azimuth, animate, cond as i32) }
    }

    pub fn setup_box_initial_rotation(&self, elevation: f32, azimuth: f32) {
        unsafe { sys::ImPlot3D_SetupBoxInitialRotation_Float(elevation, azimuth) }
    }

    pub fn plot_text(&self, text: &str, x: f32, y: f32, z: f32, angle: f32, pix_offset: [f32; 2]) {
        let c_text = std::ffi::CString::new(text).unwrap_or_default();
        unsafe {
            sys::ImPlot3D_PlotText(
                c_text.as_ptr(),
                x,
                y,
                z,
                angle,
                sys::ImVec2 {
                    x: pix_offset[0],
                    y: pix_offset[1],
                },
            )
        }
    }

    pub fn plot_to_pixels(&self, point: [f32; 3]) -> [f32; 2] {
        unsafe {
            let mut out = sys::ImVec2 { x: 0.0, y: 0.0 };
            sys::ImPlot3D_PlotToPixels_double(
                &mut out,
                point[0] as f64,
                point[1] as f64,
                point[2] as f64,
            );
            [out.x, out.y]
        }
    }

    pub fn get_plot_draw_list(&self) -> *mut sys::ImDrawList {
        unsafe { sys::ImPlot3D_GetPlotDrawList() }
    }

    pub fn get_frame_pos(&self) -> [f32; 2] {
        unsafe {
            let mut out = sys::ImVec2 { x: 0.0, y: 0.0 };
            sys::ImPlot3D_GetPlotPos(&mut out);
            [out.x, out.y]
        }
    }

    pub fn get_frame_size(&self) -> [f32; 2] {
        unsafe {
            let mut out = sys::ImVec2 { x: 0.0, y: 0.0 };
            sys::ImPlot3D_GetPlotSize(&mut out);
            [out.x, out.y]
        }
    }
}

/// Mesh plot builder
pub struct Mesh3DBuilder<'ui> {
    _ui: &'ui Plot3DUi<'ui>,
    label: std::ffi::CString,
    vertices: &'ui [[f32; 3]],
    indices: &'ui [u32],
    flags: Mesh3DFlags,
}

impl<'ui> Mesh3DBuilder<'ui> {
    pub fn flags(mut self, flags: Mesh3DFlags) -> Self {
        self.flags = flags;
        self
    }
    pub fn plot(self) {
        // SAFETY: ImPlot3DPoint has (x,y,z) floats; we transmute from [[f32;3]] for FFI call
        // Layout compatibility assumed; if upstream changes, this needs revisiting.
        let vtx_count = self.vertices.len() as i32;
        let idx_count = self.indices.len() as i32;
        unsafe {
            let vtx_ptr = self.vertices.as_ptr() as *const sys::ImPlot3DPoint;
            sys::ImPlot3D_PlotMesh(
                self.label.as_ptr(),
                vtx_ptr,
                self.indices.as_ptr(),
                vtx_count,
                idx_count,
                self.flags.bits() as i32,
            );
        }
    }
}

impl<'ui> Plot3DUi<'ui> {
    /// Start a mesh plot from vertices (x,y,z) and triangle indices
    pub fn mesh<S: AsRef<str>>(
        &'ui self,
        label: S,
        vertices: &'ui [[f32; 3]],
        indices: &'ui [u32],
    ) -> Mesh3DBuilder<'ui> {
        let label_c = std::ffi::CString::new(label.as_ref())
            .unwrap_or_else(|_| std::ffi::CString::new("mesh").unwrap());
        Mesh3DBuilder {
            _ui: self,
            label: label_c,
            vertices,
            indices,
            flags: Mesh3DFlags::NONE,
        }
    }
}
