//! Heatmap plot implementation

use super::{Plot, PlotError, safe_cstring};
use crate::sys;

/// Builder for heatmap plots with extensive customization options
pub struct HeatmapPlot<'a> {
    label: &'a str,
    values: &'a [f64],
    rows: i32,
    cols: i32,
    scale_min: f64,
    scale_max: f64,
    label_fmt: Option<&'a str>,
    bounds_min: sys::ImPlotPoint,
    bounds_max: sys::ImPlotPoint,
    flags: sys::ImPlotHeatmapFlags,
}

impl<'a> HeatmapPlot<'a> {
    /// Create a new heatmap plot with the given label and data
    ///
    /// # Arguments
    /// * `label` - The label for the heatmap
    /// * `values` - The data values in row-major order (unless ColMajor flag is set)
    /// * `rows` - Number of rows in the data
    /// * `cols` - Number of columns in the data
    pub fn new(label: &'a str, values: &'a [f64], rows: usize, cols: usize) -> Self {
        Self {
            label,
            values,
            rows: rows as i32,
            cols: cols as i32,
            scale_min: 0.0,
            scale_max: 0.0, // Auto-scale when both are 0
            label_fmt: Some("%.1f"),
            bounds_min: sys::ImPlotPoint { x: 0.0, y: 0.0 },
            bounds_max: sys::ImPlotPoint { x: 1.0, y: 1.0 },
            flags: 0,
        }
    }

    /// Set the color scale range (min, max)
    /// If both are 0.0, auto-scaling will be used
    pub fn with_scale(mut self, min: f64, max: f64) -> Self {
        self.scale_min = min;
        self.scale_max = max;
        self
    }

    /// Set the label format for values (e.g., "%.2f", "%.1e")
    /// Set to None to disable labels
    pub fn with_label_format(mut self, fmt: Option<&'a str>) -> Self {
        self.label_fmt = fmt;
        self
    }

    /// Set the drawing area bounds in plot coordinates
    pub fn with_bounds(mut self, min_x: f64, min_y: f64, max_x: f64, max_y: f64) -> Self {
        self.bounds_min = sys::ImPlotPoint { x: min_x, y: min_y };
        self.bounds_max = sys::ImPlotPoint { x: max_x, y: max_y };
        self
    }

    /// Set the drawing area bounds using ImPlotPoint
    pub fn with_bounds_points(mut self, min: sys::ImPlotPoint, max: sys::ImPlotPoint) -> Self {
        self.bounds_min = min;
        self.bounds_max = max;
        self
    }

    /// Set heatmap flags for customization
    pub fn with_flags(mut self, flags: sys::ImPlotHeatmapFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Use column-major data ordering instead of row-major
    pub fn column_major(mut self) -> Self {
        self.flags |= sys::ImPlotHeatmapFlags_ColMajor;
        self
    }

    /// Validate the plot data
    pub fn validate(&self) -> Result<(), PlotError> {
        if self.values.is_empty() {
            return Err(PlotError::EmptyData);
        }

        let expected_size = (self.rows * self.cols) as usize;
        if self.values.len() != expected_size {
            return Err(PlotError::DataLengthMismatch {
                x_len: expected_size,
                y_len: self.values.len(),
            });
        }

        if self.rows <= 0 || self.cols <= 0 {
            return Err(PlotError::InvalidData(
                "Rows and columns must be positive".to_string(),
            ));
        }

        Ok(())
    }
}

impl<'a> Plot for HeatmapPlot<'a> {
    fn plot(&self) {
        if self.validate().is_err() {
            return; // Skip plotting if data is invalid
        }

        let label_cstr = safe_cstring(self.label);

        let label_fmt_cstr = self.label_fmt.map(safe_cstring);
        let label_fmt_ptr = label_fmt_cstr
            .as_ref()
            .map(|cstr| cstr.as_ptr())
            .unwrap_or(std::ptr::null());

        unsafe {
            sys::ImPlot_PlotHeatmap_doublePtr(
                label_cstr.as_ptr(),
                self.values.as_ptr(),
                self.rows,
                self.cols,
                self.scale_min,
                self.scale_max,
                label_fmt_ptr,
                self.bounds_min,
                self.bounds_max,
                self.flags,
            );
        }
    }

    fn label(&self) -> &str {
        self.label
    }
}

/// Float version of heatmap for better performance with f32 data
pub struct HeatmapPlotF32<'a> {
    label: &'a str,
    values: &'a [f32],
    rows: i32,
    cols: i32,
    scale_min: f64,
    scale_max: f64,
    label_fmt: Option<&'a str>,
    bounds_min: sys::ImPlotPoint,
    bounds_max: sys::ImPlotPoint,
    flags: sys::ImPlotHeatmapFlags,
}

impl<'a> HeatmapPlotF32<'a> {
    /// Create a new f32 heatmap plot
    pub fn new(label: &'a str, values: &'a [f32], rows: usize, cols: usize) -> Self {
        Self {
            label,
            values,
            rows: rows as i32,
            cols: cols as i32,
            scale_min: 0.0,
            scale_max: 0.0,
            label_fmt: Some("%.1f"),
            bounds_min: sys::ImPlotPoint { x: 0.0, y: 0.0 },
            bounds_max: sys::ImPlotPoint { x: 1.0, y: 1.0 },
            flags: 0,
        }
    }

    /// Set the color scale range (min, max)
    pub fn with_scale(mut self, min: f64, max: f64) -> Self {
        self.scale_min = min;
        self.scale_max = max;
        self
    }

    /// Set the label format for values
    pub fn with_label_format(mut self, fmt: Option<&'a str>) -> Self {
        self.label_fmt = fmt;
        self
    }

    /// Set the drawing area bounds in plot coordinates
    pub fn with_bounds(mut self, min_x: f64, min_y: f64, max_x: f64, max_y: f64) -> Self {
        self.bounds_min = sys::ImPlotPoint { x: min_x, y: min_y };
        self.bounds_max = sys::ImPlotPoint { x: max_x, y: max_y };
        self
    }

    /// Set heatmap flags for customization
    pub fn with_flags(mut self, flags: sys::ImPlotHeatmapFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Use column-major data ordering
    pub fn column_major(mut self) -> Self {
        self.flags |= sys::ImPlotHeatmapFlags_ColMajor;
        self
    }

    /// Validate the plot data
    pub fn validate(&self) -> Result<(), PlotError> {
        if self.values.is_empty() {
            return Err(PlotError::EmptyData);
        }

        let expected_size = (self.rows * self.cols) as usize;
        if self.values.len() != expected_size {
            return Err(PlotError::DataLengthMismatch {
                x_len: expected_size,
                y_len: self.values.len(),
            });
        }

        if self.rows <= 0 || self.cols <= 0 {
            return Err(PlotError::InvalidData(
                "Rows and columns must be positive".to_string(),
            ));
        }

        Ok(())
    }
}

impl<'a> Plot for HeatmapPlotF32<'a> {
    fn plot(&self) {
        if self.validate().is_err() {
            return;
        }

        let label_cstr = safe_cstring(self.label);

        let label_fmt_cstr = self.label_fmt.map(safe_cstring);
        let label_fmt_ptr = label_fmt_cstr
            .as_ref()
            .map(|cstr| cstr.as_ptr())
            .unwrap_or(std::ptr::null());

        unsafe {
            sys::ImPlot_PlotHeatmap_FloatPtr(
                label_cstr.as_ptr(),
                self.values.as_ptr(),
                self.rows,
                self.cols,
                self.scale_min,
                self.scale_max,
                label_fmt_ptr,
                self.bounds_min,
                self.bounds_max,
                self.flags,
            );
        }
    }

    fn label(&self) -> &str {
        self.label
    }
}

/// Convenience functions for quick heatmap plotting
impl<'ui> crate::PlotUi<'ui> {
    /// Plot a heatmap with f64 data
    pub fn heatmap_plot(
        &self,
        label: &str,
        values: &[f64],
        rows: usize,
        cols: usize,
    ) -> Result<(), PlotError> {
        let plot = HeatmapPlot::new(label, values, rows, cols);
        plot.validate()?;
        plot.plot();
        Ok(())
    }

    /// Plot a heatmap with f32 data
    pub fn heatmap_plot_f32(
        &self,
        label: &str,
        values: &[f32],
        rows: usize,
        cols: usize,
    ) -> Result<(), PlotError> {
        let plot = HeatmapPlotF32::new(label, values, rows, cols);
        plot.validate()?;
        plot.plot();
        Ok(())
    }

    /// Plot a heatmap with custom scale and bounds
    pub fn heatmap_plot_scaled(
        &self,
        label: &str,
        values: &[f64],
        rows: usize,
        cols: usize,
        scale_min: f64,
        scale_max: f64,
        bounds_min: sys::ImPlotPoint,
        bounds_max: sys::ImPlotPoint,
    ) -> Result<(), PlotError> {
        let plot = HeatmapPlot::new(label, values, rows, cols)
            .with_scale(scale_min, scale_max)
            .with_bounds_points(bounds_min, bounds_max);
        plot.validate()?;
        plot.plot();
        Ok(())
    }
}
