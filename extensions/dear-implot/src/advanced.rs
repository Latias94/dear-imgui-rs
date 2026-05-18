//! Advanced plotting features for complex visualizations
//!
//! This module provides high-level functionality for creating complex plots
//! with multiple subplots, legends, and advanced layout management.

use crate::context::PlotScopeGuard;
use crate::{AxisFlags, YAxis, plots::PlotError, sys};
use std::ffi::CString;
use std::marker::PhantomData;
use std::rc::Rc;

fn validate_size(caller: &str, size: [f32; 2]) -> Result<(), PlotError> {
    if size[0].is_finite() && size[1].is_finite() {
        Ok(())
    } else {
        Err(PlotError::InvalidData(format!(
            "{caller} size must be finite"
        )))
    }
}

fn count_to_i32(caller: &str, name: &str, value: usize) -> Result<i32, PlotError> {
    if value == 0 {
        return Err(PlotError::InvalidData(format!(
            "{caller} {name} must be positive"
        )));
    }

    i32::try_from(value)
        .map_err(|_| PlotError::InvalidData(format!("{caller} {name} exceeded ImPlot's i32 range")))
}

fn validate_ratios(caller: &str, name: &str, ratios: &[f32]) -> Result<(), PlotError> {
    if ratios.iter().all(|value| value.is_finite() && *value > 0.0) {
        Ok(())
    } else {
        Err(PlotError::InvalidData(format!(
            "{caller} {name} must contain only positive finite values"
        )))
    }
}

fn validate_range(caller: &str, min: f64, max: f64) -> Result<(), PlotError> {
    if min.is_finite() && max.is_finite() && min != max {
        Ok(())
    } else {
        Err(PlotError::InvalidData(format!(
            "{caller} range values must be finite and distinct"
        )))
    }
}

/// Multi-plot layout manager for creating subplot grids
pub struct SubplotGrid<'a> {
    title: &'a str,
    rows: usize,
    cols: usize,
    size: Option<[f32; 2]>,
    flags: SubplotFlags,
    row_ratios: Option<Vec<f32>>,
    col_ratios: Option<Vec<f32>>,
}

bitflags::bitflags! {
    /// Flags for subplot configuration
    pub struct SubplotFlags: u32 {
        const NONE = 0;
        const NO_TITLE = 1 << 0;
        const NO_RESIZE = 1 << 1;
        const NO_ALIGN = 1 << 2;
        const SHARE_ITEMS = 1 << 3;
        const LINK_ROWS = 1 << 4;
        const LINK_COLS = 1 << 5;
        const LINK_ALL_X = 1 << 6;
        const LINK_ALL_Y = 1 << 7;
        const COLUMN_MAJOR = 1 << 8;
    }
}

impl<'a> SubplotGrid<'a> {
    /// Create a new subplot grid
    pub fn new(title: &'a str, rows: usize, cols: usize) -> Self {
        Self {
            title,
            rows,
            cols,
            size: None,
            flags: SubplotFlags::NONE,
            row_ratios: None,
            col_ratios: None,
        }
    }

    /// Set the size of the subplot grid
    pub fn with_size(mut self, size: [f32; 2]) -> Self {
        self.size = Some(size);
        self
    }

    /// Set subplot flags
    pub fn with_flags(mut self, flags: SubplotFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Set row height ratios
    pub fn with_row_ratios(mut self, ratios: &[f32]) -> Self {
        self.row_ratios = if ratios.is_empty() {
            None
        } else {
            Some(ratios.to_vec())
        };
        self
    }

    /// Set column width ratios
    pub fn with_col_ratios(mut self, ratios: &[f32]) -> Self {
        self.col_ratios = if ratios.is_empty() {
            None
        } else {
            Some(ratios.to_vec())
        };
        self
    }

    /// Begin the subplot grid and return a token
    pub fn begin(self) -> Result<SubplotToken<'a>, PlotError> {
        let rows = count_to_i32("SubplotGrid::begin()", "rows", self.rows)?;
        let cols = count_to_i32("SubplotGrid::begin()", "cols", self.cols)?;
        let title_cstr =
            CString::new(self.title).map_err(|e| PlotError::StringConversion(e.to_string()))?;

        let size = self.size.unwrap_or([-1.0, -1.0]);
        validate_size("SubplotGrid::begin()", size)?;
        let size_vec = sys::ImVec2_c {
            x: size[0],
            y: size[1],
        };

        // The C API takes `float*` for ratios. Keep owned copies alive in the token to avoid
        // casting away constness and to stay sound even if the backend ever writes to them.
        let mut row_ratios = self.row_ratios;
        let mut col_ratios = self.col_ratios;
        if let Some(row_ratios) = &row_ratios {
            if row_ratios.len() != self.rows {
                return Err(PlotError::InvalidData(format!(
                    "SubplotGrid::begin() row_ratios length must equal rows ({})",
                    self.rows
                )));
            }
            validate_ratios("SubplotGrid::begin()", "row_ratios", row_ratios)?;
        }
        if let Some(col_ratios) = &col_ratios {
            if col_ratios.len() != self.cols {
                return Err(PlotError::InvalidData(format!(
                    "SubplotGrid::begin() col_ratios length must equal cols ({})",
                    self.cols
                )));
            }
            validate_ratios("SubplotGrid::begin()", "col_ratios", col_ratios)?;
        }
        let row_ratios_ptr = row_ratios
            .as_mut()
            .map(|r| r.as_mut_ptr())
            .unwrap_or(std::ptr::null_mut());
        let col_ratios_ptr = col_ratios
            .as_mut()
            .map(|c| c.as_mut_ptr())
            .unwrap_or(std::ptr::null_mut());

        let success = unsafe {
            sys::ImPlot_BeginSubplots(
                title_cstr.as_ptr(),
                rows,
                cols,
                size_vec,
                self.flags.bits() as i32,
                row_ratios_ptr,
                col_ratios_ptr,
            )
        };

        if success {
            Ok(SubplotToken {
                _title: title_cstr,
                _row_ratios: row_ratios,
                _col_ratios: col_ratios,
                _phantom: PhantomData,
                _not_send_or_sync: PhantomData,
            })
        } else {
            Err(PlotError::PlotCreationFailed(
                "Failed to begin subplots".to_string(),
            ))
        }
    }
}

/// Token representing an active subplot grid
pub struct SubplotToken<'a> {
    _title: CString,
    _row_ratios: Option<Vec<f32>>,
    _col_ratios: Option<Vec<f32>>,
    _phantom: PhantomData<&'a ()>,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl<'a> SubplotToken<'a> {
    /// End the subplot grid
    pub fn end(self) {
        // The actual ending happens in Drop.
    }
}

impl<'a> Drop for SubplotToken<'a> {
    fn drop(&mut self) {
        unsafe {
            sys::ImPlot_EndSubplots();
        }
    }
}

/// Multi-axis plot support
pub struct MultiAxisPlot<'a> {
    title: &'a str,
    size: Option<[f32; 2]>,
    y_axes: Vec<YAxisConfig<'a>>,
}

/// Configuration for a Y-axis
pub struct YAxisConfig<'a> {
    pub label: Option<&'a str>,
    pub flags: AxisFlags,
    pub range: Option<(f64, f64)>,
}

impl<'a> MultiAxisPlot<'a> {
    /// Create a new multi-axis plot
    pub fn new(title: &'a str) -> Self {
        Self {
            title,
            size: None,
            y_axes: Vec::new(),
        }
    }

    /// Set the plot size
    pub fn with_size(mut self, size: [f32; 2]) -> Self {
        self.size = Some(size);
        self
    }

    /// Add a Y-axis
    pub fn add_y_axis(mut self, config: YAxisConfig<'a>) -> Self {
        self.y_axes.push(config);
        self
    }

    /// Begin the multi-axis plot
    pub fn begin(self) -> Result<MultiAxisToken<'a>, PlotError> {
        let title_cstr =
            CString::new(self.title).map_err(|e| PlotError::StringConversion(e.to_string()))?;

        for axis in &self.y_axes {
            if let Some(label) = axis.label
                && label.contains('\0')
            {
                return Err(PlotError::StringConversion(
                    "Axis label contained an interior NUL byte".to_string(),
                ));
            }
            if let Some((min, max)) = axis.range {
                validate_range("MultiAxisPlot::begin()", min, max)?;
            }
        }
        if self.y_axes.len() > 3 {
            return Err(PlotError::InvalidData(
                "MultiAxisPlot::begin() supports at most 3 Y axes".to_string(),
            ));
        }

        let size = self.size.unwrap_or([-1.0, -1.0]);
        validate_size("MultiAxisPlot::begin()", size)?;
        let size_vec = sys::ImVec2_c {
            x: size[0],
            y: size[1],
        };

        let success = unsafe { sys::ImPlot_BeginPlot(title_cstr.as_ptr(), size_vec, 0) };

        if success {
            let mut axis_labels: Vec<CString> = Vec::new();

            // Setup Y-axes (Y1..), matching `token.set_y_axis(YAxis::Y*)` convention.
            for (i, axis_config) in self.y_axes.iter().enumerate() {
                let label_ptr = if let Some(label) = axis_config.label {
                    let cstr = CString::new(label)
                        .map_err(|e| PlotError::StringConversion(e.to_string()))?;
                    let ptr = cstr.as_ptr();
                    axis_labels.push(cstr);
                    ptr
                } else {
                    std::ptr::null()
                };

                unsafe {
                    let axis_enum = (i as i32) + 3; // ImAxis_Y1 = 3
                    sys::ImPlot_SetupAxis(axis_enum, label_ptr, axis_config.flags.bits() as i32);

                    if let Some((min, max)) = axis_config.range {
                        sys::ImPlot_SetupAxisLimits(axis_enum, min, max, 0);
                    }
                }
            }

            Ok(MultiAxisToken {
                _title: title_cstr,
                _axis_labels: axis_labels,
                _scope: PlotScopeGuard::new(),
                _phantom: PhantomData,
            })
        } else {
            Err(PlotError::PlotCreationFailed(
                "Failed to begin multi-axis plot".to_string(),
            ))
        }
    }
}

/// Token representing an active multi-axis plot
pub struct MultiAxisToken<'a> {
    _title: CString,
    _axis_labels: Vec<CString>,
    _scope: PlotScopeGuard,
    _phantom: PhantomData<&'a ()>,
}

impl<'a> MultiAxisToken<'a> {
    /// Set the current Y-axis for subsequent plots
    pub fn set_y_axis(&self, axis: YAxis) {
        unsafe {
            sys::ImPlot_SetAxes(
                0, // ImAxis_X1
                axis as i32,
            );
        }
    }

    /// Set the current raw Y-axis for subsequent plots.
    ///
    /// # Safety
    ///
    /// `axis` must be a valid ImPlot Y-axis value for the active plot. Passing an
    /// out-of-range value lets ImPlot index internal axis arrays out of bounds.
    pub unsafe fn set_y_axis_unchecked(&self, axis: sys::ImAxis) {
        unsafe {
            sys::ImPlot_SetAxes(
                0, // ImAxis_X1
                axis,
            );
        }
    }

    /// End the multi-axis plot
    pub fn end(self) {
        // The actual ending happens in Drop.
    }
}

impl<'a> Drop for MultiAxisToken<'a> {
    fn drop(&mut self) {
        unsafe {
            sys::ImPlot_EndPlot();
        }
    }
}

/// Legend management utilities
pub struct LegendManager;

impl LegendManager {
    /// Setup legend with custom position and flags
    pub fn setup(location: LegendLocation, flags: LegendFlags) {
        unsafe {
            sys::ImPlot_SetupLegend(location as i32, flags.bits() as i32);
        }
    }

    /// Begin a custom legend
    pub fn begin_custom(label: &str, _size: [f32; 2]) -> Result<LegendToken, PlotError> {
        let label_cstr =
            CString::new(label).map_err(|e| PlotError::StringConversion(e.to_string()))?;

        let success = unsafe {
            sys::ImPlot_BeginLegendPopup(
                label_cstr.as_ptr(),
                1, // mouse button
            )
        };

        if success {
            Ok(LegendToken {
                _label: label_cstr,
                _not_send_or_sync: PhantomData,
            })
        } else {
            Err(PlotError::PlotCreationFailed(
                "Failed to begin legend".to_string(),
            ))
        }
    }
}

/// Legend location options (ImPlotLocation)
#[repr(i32)]
pub enum LegendLocation {
    Center = sys::ImPlotLocation_Center as i32,
    North = sys::ImPlotLocation_North as i32,
    South = sys::ImPlotLocation_South as i32,
    West = sys::ImPlotLocation_West as i32,
    East = sys::ImPlotLocation_East as i32,
    NorthWest = sys::ImPlotLocation_NorthWest as i32,
    NorthEast = sys::ImPlotLocation_NorthEast as i32,
    SouthWest = sys::ImPlotLocation_SouthWest as i32,
    SouthEast = sys::ImPlotLocation_SouthEast as i32,
}

bitflags::bitflags! {
    /// Flags for legend configuration (ImPlotLegendFlags)
    pub struct LegendFlags: u32 {
        const NONE              = sys::ImPlotLegendFlags_None as u32;
        const NO_BUTTONS        = sys::ImPlotLegendFlags_NoButtons as u32;
        const NO_HIGHLIGHT_ITEM = sys::ImPlotLegendFlags_NoHighlightItem as u32;
        const NO_HIGHLIGHT_AXIS = sys::ImPlotLegendFlags_NoHighlightAxis as u32;
        const NO_MENUS          = sys::ImPlotLegendFlags_NoMenus as u32;
        const OUTSIDE           = sys::ImPlotLegendFlags_Outside as u32;
        const HORIZONTAL        = sys::ImPlotLegendFlags_Horizontal as u32;
        const SORT              = sys::ImPlotLegendFlags_Sort as u32;
        // Note: ImPlotLegendFlags_Reverse is currently not exposed.
    }
}

/// Token representing an active legend
pub struct LegendToken {
    _label: CString,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl LegendToken {
    /// End the legend
    pub fn end(self) {
        // The actual ending happens in Drop.
    }
}

impl Drop for LegendToken {
    fn drop(&mut self) {
        unsafe {
            sys::ImPlot_EndLegendPopup();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{PlotError, SubplotGrid};
    use crate::PlotContext;
    use std::sync::{Mutex, OnceLock};

    fn test_guard() -> std::sync::MutexGuard<'static, ()> {
        static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
        GUARD.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    fn setup_context() -> (dear_imgui_rs::Context, PlotContext) {
        let mut imgui = dear_imgui_rs::Context::create();
        let _ = imgui.font_atlas_mut().build();
        imgui.io_mut().set_display_size([256.0, 256.0]);
        imgui.io_mut().set_delta_time(1.0 / 60.0);
        let plot = PlotContext::create(&imgui);
        (imgui, plot)
    }

    fn invalid_data_message(err: PlotError) -> String {
        match err {
            PlotError::InvalidData(message) => message,
            other => panic!("expected invalid data error, got {other:?}"),
        }
    }

    fn expect_invalid_data(result: Result<super::SubplotToken<'_>, PlotError>) -> String {
        match result {
            Err(err) => invalid_data_message(err),
            Ok(_) => panic!("expected SubplotGrid::begin() to reject invalid input"),
        }
    }

    #[test]
    fn subplot_grid_rejects_invalid_counts_before_ffi() {
        let _guard = test_guard();
        let (mut imgui, _plot) = setup_context();
        let _ui = imgui.frame();

        let rows = expect_invalid_data(SubplotGrid::new("bad_rows", 0, 1).begin());
        assert!(rows.contains("rows must be positive"));

        let cols = expect_invalid_data(SubplotGrid::new("bad_cols", 1, 0).begin());
        assert!(cols.contains("cols must be positive"));

        let overflow = expect_invalid_data(
            SubplotGrid::new("too_many_rows", i32::MAX as usize + 1, 1).begin(),
        );
        assert!(overflow.contains("rows exceeded"));
    }

    #[test]
    fn subplot_grid_ratio_lengths_follow_usize_counts() {
        let _guard = test_guard();
        let (mut imgui, _plot) = setup_context();
        let _ui = imgui.frame();

        let err = expect_invalid_data(
            SubplotGrid::new("bad_ratios", 2usize, 1usize)
                .with_row_ratios(&[1.0])
                .begin(),
        );

        assert!(err.contains("row_ratios length must equal rows (2)"));
    }
}
