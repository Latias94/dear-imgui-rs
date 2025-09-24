//! Line plot implementation

use super::{Plot, PlotError, safe_cstring, validate_data_lengths};
use crate::sys;

/// Builder for line plots with extensive customization options
pub struct LinePlot<'a> {
    label: &'a str,
    x_data: &'a [f64],
    y_data: &'a [f64],
    flags: sys::ImPlotLineFlags,
    offset: i32,
    stride: i32,
}

impl<'a> LinePlot<'a> {
    /// Create a new line plot with the given label and data
    pub fn new(label: &'a str, x_data: &'a [f64], y_data: &'a [f64]) -> Self {
        Self {
            label,
            x_data,
            y_data,
            flags: 0,
            offset: 0,
            stride: std::mem::size_of::<f64>() as i32,
        }
    }

    /// Set line flags for customization
    pub fn with_flags(mut self, flags: sys::ImPlotLineFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Set data offset for partial plotting
    pub fn with_offset(mut self, offset: i32) -> Self {
        self.offset = offset;
        self
    }

    /// Set data stride for non-contiguous data
    pub fn with_stride(mut self, stride: i32) -> Self {
        self.stride = stride;
        self
    }

    /// Validate the plot data
    pub fn validate(&self) -> Result<(), PlotError> {
        validate_data_lengths(self.x_data, self.y_data)
    }
}

impl<'a> Plot for LinePlot<'a> {
    fn plot(&self) {
        if self.validate().is_err() {
            return; // Skip plotting if data is invalid
        }

        let label_cstr = safe_cstring(self.label);

        unsafe {
            sys::ImPlot_PlotLine_doublePtrdoublePtr(
                label_cstr.as_ptr(),
                self.x_data.as_ptr(),
                self.y_data.as_ptr(),
                self.x_data.len() as i32,
                self.flags,
                self.offset,
                self.stride,
            );
        }
    }

    fn label(&self) -> &str {
        self.label
    }
}

/// Simple line plot for quick plotting without builder pattern
pub struct SimpleLinePlot<'a> {
    label: &'a str,
    values: &'a [f64],
    x_scale: f64,
    x_start: f64,
}

impl<'a> SimpleLinePlot<'a> {
    /// Create a simple line plot with Y values only (X will be indices)
    pub fn new(label: &'a str, values: &'a [f64]) -> Self {
        Self {
            label,
            values,
            x_scale: 1.0,
            x_start: 0.0,
        }
    }

    /// Set X scale factor
    pub fn with_x_scale(mut self, scale: f64) -> Self {
        self.x_scale = scale;
        self
    }

    /// Set X start value
    pub fn with_x_start(mut self, start: f64) -> Self {
        self.x_start = start;
        self
    }
}

impl<'a> Plot for SimpleLinePlot<'a> {
    fn plot(&self) {
        if self.values.is_empty() {
            return;
        }

        let label_cstr = safe_cstring(self.label);

        // For simple line plots, we use the single-array version
        // This would require a wrapper function in the sys crate
        // For now, we'll create temporary X data
        let x_data: Vec<f64> = (0..self.values.len())
            .map(|i| self.x_start + i as f64 * self.x_scale)
            .collect();

        unsafe {
            sys::ImPlot_PlotLine_doublePtrdoublePtr(
                label_cstr.as_ptr(),
                x_data.as_ptr(),
                self.values.as_ptr(),
                self.values.len() as i32,
                0,
                0,
                std::mem::size_of::<f64>() as i32,
            );
        }
    }

    fn label(&self) -> &str {
        self.label
    }
}

/// Convenience functions for quick line plotting
impl<'ui> crate::PlotUi<'ui> {
    /// Plot a line with X and Y data
    pub fn line_plot(&self, label: &str, x_data: &[f64], y_data: &[f64]) -> Result<(), PlotError> {
        let plot = LinePlot::new(label, x_data, y_data);
        plot.validate()?;
        plot.plot();
        Ok(())
    }

    /// Plot a simple line with Y values only (X will be indices)
    pub fn simple_line_plot(&self, label: &str, values: &[f64]) -> Result<(), PlotError> {
        if values.is_empty() {
            return Err(PlotError::EmptyData);
        }
        let plot = SimpleLinePlot::new(label, values);
        plot.plot();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_line_plot_creation() {
        let x_data = [1.0, 2.0, 3.0, 4.0];
        let y_data = [1.0, 4.0, 2.0, 3.0];

        let plot = LinePlot::new("test", &x_data, &y_data);
        assert_eq!(plot.label(), "test");
        assert!(plot.validate().is_ok());
    }

    #[test]
    fn test_line_plot_validation() {
        let x_data = [1.0, 2.0, 3.0];
        let y_data = [1.0, 4.0]; // Different length

        let plot = LinePlot::new("test", &x_data, &y_data);
        assert!(plot.validate().is_err());
    }

    #[test]
    fn test_simple_line_plot() {
        let values = [1.0, 2.0, 3.0, 4.0];
        let plot = SimpleLinePlot::new("test", &values);
        assert_eq!(plot.label(), "test");
    }
}
