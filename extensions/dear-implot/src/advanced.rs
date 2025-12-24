//! Advanced plotting features for complex visualizations
//!
//! This module provides high-level functionality for creating complex plots
//! with multiple subplots, legends, and advanced layout management.

use crate::context::PlotScopeGuard;
use crate::{AxisFlags, plots::PlotError, sys};
use std::ffi::CString;
use std::marker::PhantomData;

/// Multi-plot layout manager for creating subplot grids
pub struct SubplotGrid<'a> {
    title: &'a str,
    rows: i32,
    cols: i32,
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
    pub fn new(title: &'a str, rows: i32, cols: i32) -> Self {
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
        let title_cstr =
            CString::new(self.title).map_err(|e| PlotError::StringConversion(e.to_string()))?;

        let size = self.size.unwrap_or([-1.0, -1.0]);
        let size_vec = sys::ImVec2_c {
            x: size[0],
            y: size[1],
        };

        // The C API takes `float*` for ratios. Keep owned copies alive in the token to avoid
        // casting away constness and to stay sound even if the backend ever writes to them.
        let mut row_ratios = self.row_ratios;
        let mut col_ratios = self.col_ratios;
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
                self.rows,
                self.cols,
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
        }

        let size = self.size.unwrap_or([-1.0, -1.0]);
        let size_vec = sys::ImVec2_c {
            x: size[0],
            y: size[1],
        };

        let success = unsafe { sys::ImPlot_BeginPlot(title_cstr.as_ptr(), size_vec, 0) };

        if success {
            let mut axis_labels: Vec<CString> = Vec::new();

            // Setup Y-axes (Y1..), matching `token.set_y_axis(0..)` convention.
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
    pub fn set_y_axis(&self, axis: i32) {
        unsafe {
            sys::ImPlot_SetAxes(
                0,        // ImAxis_X1
                axis + 3, // ImAxis_Y1 = 3
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
            Ok(LegendToken { _label: label_cstr })
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
