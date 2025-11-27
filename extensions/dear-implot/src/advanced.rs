//! Advanced plotting features for complex visualizations
//!
//! This module provides high-level functionality for creating complex plots
//! with multiple subplots, legends, and advanced layout management.

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
    row_ratios: Option<&'a [f32]>,
    col_ratios: Option<&'a [f32]>,
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
    pub fn with_row_ratios(mut self, ratios: &'a [f32]) -> Self {
        self.row_ratios = Some(ratios);
        self
    }

    /// Set column width ratios
    pub fn with_col_ratios(mut self, ratios: &'a [f32]) -> Self {
        self.col_ratios = Some(ratios);
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

        let row_ratios_ptr = self
            .row_ratios
            .map(|r| r.as_ptr() as *mut f32)
            .unwrap_or(std::ptr::null_mut());

        let col_ratios_ptr = self
            .col_ratios
            .map(|c| c.as_ptr() as *mut f32)
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
    _phantom: PhantomData<&'a ()>,
}

impl<'a> SubplotToken<'a> {
    /// End the subplot grid
    pub fn end(self) {
        unsafe {
            sys::ImPlot_EndSubplots();
        }
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

        let size = self.size.unwrap_or([-1.0, -1.0]);
        let size_vec = sys::ImVec2_c {
            x: size[0],
            y: size[1],
        };

        let success = unsafe { sys::ImPlot_BeginPlot(title_cstr.as_ptr(), size_vec, 0) };

        if success {
            // Setup additional Y-axes
            for (i, axis_config) in self.y_axes.iter().enumerate() {
                if i > 0 {
                    // Skip the first axis (it's the default)
                    let label_cstr = if let Some(label) = axis_config.label {
                        Some(
                            CString::new(label)
                                .map_err(|e| PlotError::StringConversion(e.to_string()))?,
                        )
                    } else {
                        None
                    };

                    let label_ptr = label_cstr
                        .as_ref()
                        .map(|cstr| cstr.as_ptr())
                        .unwrap_or(std::ptr::null());

                    unsafe {
                        let axis_enum = (i as i32) + 3; // ImAxis_Y1 = 3
                        sys::ImPlot_SetupAxis(
                            axis_enum,
                            label_ptr,
                            axis_config.flags.bits() as i32,
                        );

                        if let Some((min, max)) = axis_config.range {
                            sys::ImPlot_SetupAxisLimits(axis_enum, min, max, 0);
                        }
                    }
                }
            }

            Ok(MultiAxisToken {
                _title: title_cstr,
                _axis_labels: self
                    .y_axes
                    .into_iter()
                    .filter_map(|config| config.label)
                    .map(|label| CString::new(label).unwrap())
                    .collect(),
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
        unsafe {
            sys::ImPlot_EndPlot();
        }
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

/// Legend location options
#[repr(i32)]
pub enum LegendLocation {
    Center = 0,
    North = 1,
    South = 2,
    West = 4,
    East = 8,
    NorthWest = 5,
    NorthEast = 9,
    SouthWest = 6,
    SouthEast = 10,
}

bitflags::bitflags! {
    /// Flags for legend configuration
    pub struct LegendFlags: u32 {
        const NONE = 0;
        const NO_BUTTONS = 1 << 0;
        const NO_HIGHLIGHT_ITEM = 1 << 1;
        const NO_HIGHLIGHT_AXIS = 1 << 2;
        const NO_MENUS = 1 << 3;
        const OUTSIDE = 1 << 4;
        const HORIZONTAL = 1 << 5;
        const SORT = 1 << 6;
    }
}

/// Token representing an active legend
pub struct LegendToken {
    _label: CString,
}

impl LegendToken {
    /// End the legend
    pub fn end(self) {
        unsafe {
            sys::ImPlot_EndLegendPopup();
        }
    }
}

impl Drop for LegendToken {
    fn drop(&mut self) {
        unsafe {
            sys::ImPlot_EndLegendPopup();
        }
    }
}
