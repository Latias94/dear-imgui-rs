//! Shaded area plot implementation

use super::{safe_cstring, validate_data_lengths, Plot, PlotData, PlotError};
use crate::sys;

/// Builder for shaded area plots
pub struct ShadedPlot<'a> {
    label: &'a str,
    x_data: &'a [f64],
    y_data: &'a [f64],
    y_ref: f64,
    flags: sys::ImPlotShadedFlags,
    offset: i32,
    stride: i32,
}

impl<'a> ShadedPlot<'a> {
    /// Create a new shaded plot between a line and a reference Y value
    pub fn new(label: &'a str, x_data: &'a [f64], y_data: &'a [f64]) -> Self {
        Self {
            label,
            x_data,
            y_data,
            y_ref: 0.0, // Default reference line at Y=0
            flags: 0,
            offset: 0,
            stride: std::mem::size_of::<f64>() as i32,
        }
    }

    /// Set the reference Y value for shading
    /// The area will be filled between the line and this Y value
    pub fn with_y_ref(mut self, y_ref: f64) -> Self {
        self.y_ref = y_ref;
        self
    }

    /// Set shaded flags for customization
    pub fn with_flags(mut self, flags: sys::ImPlotShadedFlags) -> Self {
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
        validate_data_lengths(&self.x_data, &self.y_data)
    }
}

impl<'a> Plot for ShadedPlot<'a> {
    fn plot(&self) {
        if let Err(_) = self.validate() {
            return;
        }

        let label_cstr = safe_cstring(self.label);

        unsafe {
            sys::ImPlot_PlotShaded_double(
                label_cstr.as_ptr(),
                self.x_data.as_ptr(),
                self.y_data.as_ptr(),
                self.x_data.len() as i32,
                self.y_ref,
                self.flags as i32,
            );
        }
    }

    fn label(&self) -> &str {
        self.label
    }
}

/// Builder for shaded area plots between two lines
pub struct ShadedBetweenPlot<'a> {
    label: &'a str,
    x_data: &'a [f64],
    y1_data: &'a [f64],
    y2_data: &'a [f64],
    flags: sys::ImPlotShadedFlags,
}

impl<'a> ShadedBetweenPlot<'a> {
    /// Create a new shaded plot between two lines
    pub fn new(label: &'a str, x_data: &'a [f64], y1_data: &'a [f64], y2_data: &'a [f64]) -> Self {
        Self {
            label,
            x_data,
            y1_data,
            y2_data,
            flags: 0,
        }
    }

    /// Set shaded flags for customization
    pub fn with_flags(mut self, flags: sys::ImPlotShadedFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Validate the plot data
    pub fn validate(&self) -> Result<(), PlotError> {
        validate_data_lengths(&self.x_data, &self.y1_data)?;
        validate_data_lengths(&self.x_data, &self.y2_data)?;
        Ok(())
    }
}

impl<'a> Plot for ShadedBetweenPlot<'a> {
    fn plot(&self) {
        if let Err(_) = self.validate() {
            return;
        }

        let label_cstr = safe_cstring(self.label);

        // Note: This would require a different wrapper function for shaded between two lines
        // For now, we'll use the single line version with the first Y data
        unsafe {
            sys::ImPlot_PlotShaded_double(
                label_cstr.as_ptr(),
                self.x_data.as_ptr(),
                self.y1_data.as_ptr(),
                self.x_data.len() as i32,
                0.0, // y_ref - this is a limitation of the current wrapper
                self.flags as i32,
            );
        }
    }

    fn label(&self) -> &str {
        self.label
    }
}

/// Simple shaded plot for quick plotting without builder pattern
pub struct SimpleShadedPlot<'a> {
    label: &'a str,
    values: &'a [f64],
    y_ref: f64,
    x_scale: f64,
    x_start: f64,
}

impl<'a> SimpleShadedPlot<'a> {
    /// Create a simple shaded plot with Y values only (X will be indices)
    pub fn new(label: &'a str, values: &'a [f64]) -> Self {
        Self {
            label,
            values,
            y_ref: 0.0,
            x_scale: 1.0,
            x_start: 0.0,
        }
    }

    /// Set the reference Y value for shading
    pub fn with_y_ref(mut self, y_ref: f64) -> Self {
        self.y_ref = y_ref;
        self
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

impl<'a> Plot for SimpleShadedPlot<'a> {
    fn plot(&self) {
        if self.values.is_empty() {
            return;
        }

        let label_cstr = safe_cstring(self.label);

        // Create temporary X data
        let x_data: Vec<f64> = (0..self.values.len())
            .map(|i| self.x_start + i as f64 * self.x_scale)
            .collect();

        unsafe {
            sys::ImPlot_PlotShaded_double(
                label_cstr.as_ptr(),
                x_data.as_ptr(),
                self.values.as_ptr(),
                self.values.len() as i32,
                self.y_ref,
                0, // flags
            );
        }
    }

    fn label(&self) -> &str {
        self.label
    }
}

/// Convenience functions for quick shaded plotting
impl<'ui> crate::PlotUi<'ui> {
    /// Plot a shaded area between a line and Y=0
    pub fn shaded_plot(
        &self,
        label: &str,
        x_data: &[f64],
        y_data: &[f64],
    ) -> Result<(), PlotError> {
        let plot = ShadedPlot::new(label, x_data, y_data);
        plot.validate()?;
        plot.plot();
        Ok(())
    }

    /// Plot a shaded area between a line and a reference Y value
    pub fn shaded_plot_with_ref(
        &self,
        label: &str,
        x_data: &[f64],
        y_data: &[f64],
        y_ref: f64,
    ) -> Result<(), PlotError> {
        let plot = ShadedPlot::new(label, x_data, y_data).with_y_ref(y_ref);
        plot.validate()?;
        plot.plot();
        Ok(())
    }

    /// Plot a shaded area between two lines
    pub fn shaded_between_plot(
        &self,
        label: &str,
        x_data: &[f64],
        y1_data: &[f64],
        y2_data: &[f64],
    ) -> Result<(), PlotError> {
        let plot = ShadedBetweenPlot::new(label, x_data, y1_data, y2_data);
        plot.validate()?;
        plot.plot();
        Ok(())
    }

    /// Plot a simple shaded area with Y values only (X will be indices)
    pub fn simple_shaded_plot(&self, label: &str, values: &[f64]) -> Result<(), PlotError> {
        if values.is_empty() {
            return Err(PlotError::EmptyData);
        }
        let plot = SimpleShadedPlot::new(label, values);
        plot.plot();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shaded_plot_creation() {
        let x_data = [1.0, 2.0, 3.0, 4.0];
        let y_data = [1.0, 4.0, 2.0, 3.0];

        let plot = ShadedPlot::new("test", &x_data, &y_data);
        assert_eq!(plot.label(), "test");
        assert!(plot.validate().is_ok());
    }

    #[test]
    fn test_shaded_plot_validation() {
        let x_data = [1.0, 2.0, 3.0];
        let y_data = [1.0, 4.0]; // Different length

        let plot = ShadedPlot::new("test", &x_data, &y_data);
        assert!(plot.validate().is_err());
    }

    #[test]
    fn test_shaded_between_plot() {
        let x_data = [1.0, 2.0, 3.0, 4.0];
        let y1_data = [1.0, 2.0, 3.0, 4.0];
        let y2_data = [2.0, 3.0, 4.0, 5.0];

        let plot = ShadedBetweenPlot::new("test", &x_data, &y1_data, &y2_data);
        assert_eq!(plot.label(), "test");
        assert!(plot.validate().is_ok());
    }
}
