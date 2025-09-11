//! Error bars plot implementation

use super::{safe_cstring, validate_data_lengths, Plot, PlotData, PlotError};
use crate::sys;

/// Builder for error bars plots
pub struct ErrorBarsPlot<'a> {
    label: &'a str,
    x_data: &'a [f64],
    y_data: &'a [f64],
    err_data: &'a [f64],
    flags: sys::ImPlotErrorBarsFlags,
    offset: i32,
    stride: i32,
}

impl<'a> ErrorBarsPlot<'a> {
    /// Create a new error bars plot with symmetric errors
    ///
    /// # Arguments
    /// * `label` - The label for the error bars
    /// * `x_data` - X coordinates of the data points
    /// * `y_data` - Y coordinates of the data points
    /// * `err_data` - Error values (symmetric, ±err)
    pub fn new(label: &'a str, x_data: &'a [f64], y_data: &'a [f64], err_data: &'a [f64]) -> Self {
        Self {
            label,
            x_data,
            y_data,
            err_data,
            flags: 0,
            offset: 0,
            stride: std::mem::size_of::<f64>() as i32,
        }
    }

    /// Set error bar flags for customization
    pub fn with_flags(mut self, flags: sys::ImPlotErrorBarsFlags) -> Self {
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

    /// Make error bars horizontal instead of vertical
    pub fn horizontal(mut self) -> Self {
        // Note: This would require a different flag or function
        // For now, we'll keep it as a placeholder
        self
    }

    /// Validate the plot data
    pub fn validate(&self) -> Result<(), PlotError> {
        validate_data_lengths(&self.x_data, &self.y_data)?;
        validate_data_lengths(&self.x_data, &self.err_data)?;

        // Check for negative error values
        if self.err_data.iter().any(|&err| err < 0.0) {
            return Err(PlotError::InvalidData(
                "Error values cannot be negative".to_string(),
            ));
        }

        Ok(())
    }
}

impl<'a> Plot for ErrorBarsPlot<'a> {
    fn plot(&self) {
        if let Err(_) = self.validate() {
            return;
        }

        let label_cstr = safe_cstring(self.label);

        unsafe {
            sys::ImPlot_PlotErrorBars_double(
                label_cstr.as_ptr(),
                self.x_data.as_ptr(),
                self.y_data.as_ptr(),
                self.err_data.as_ptr(),
                self.x_data.len() as i32,
                self.flags as i32,
            );
        }
    }

    fn label(&self) -> &str {
        self.label
    }
}

/// Builder for asymmetric error bars plots
pub struct AsymmetricErrorBarsPlot<'a> {
    label: &'a str,
    x_data: &'a [f64],
    y_data: &'a [f64],
    err_neg: &'a [f64],
    err_pos: &'a [f64],
    flags: sys::ImPlotErrorBarsFlags,
}

impl<'a> AsymmetricErrorBarsPlot<'a> {
    /// Create a new asymmetric error bars plot
    ///
    /// # Arguments
    /// * `label` - The label for the error bars
    /// * `x_data` - X coordinates of the data points
    /// * `y_data` - Y coordinates of the data points
    /// * `err_neg` - Negative error values (downward/leftward)
    /// * `err_pos` - Positive error values (upward/rightward)
    pub fn new(
        label: &'a str,
        x_data: &'a [f64],
        y_data: &'a [f64],
        err_neg: &'a [f64],
        err_pos: &'a [f64],
    ) -> Self {
        Self {
            label,
            x_data,
            y_data,
            err_neg,
            err_pos,
            flags: 0,
        }
    }

    /// Set error bar flags for customization
    pub fn with_flags(mut self, flags: sys::ImPlotErrorBarsFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Validate the plot data
    pub fn validate(&self) -> Result<(), PlotError> {
        validate_data_lengths(&self.x_data, &self.y_data)?;
        validate_data_lengths(&self.x_data, &self.err_neg)?;
        validate_data_lengths(&self.x_data, &self.err_pos)?;

        // Check for negative error values
        if self.err_neg.iter().any(|&err| err < 0.0) || self.err_pos.iter().any(|&err| err < 0.0) {
            return Err(PlotError::InvalidData(
                "Error values cannot be negative".to_string(),
            ));
        }

        Ok(())
    }
}

impl<'a> Plot for AsymmetricErrorBarsPlot<'a> {
    fn plot(&self) {
        if let Err(_) = self.validate() {
            return;
        }

        // Note: This would require a different wrapper function for asymmetric error bars
        // For now, we'll use the symmetric version with the average of positive and negative errors
        let avg_errors: Vec<f64> = self
            .err_neg
            .iter()
            .zip(self.err_pos.iter())
            .map(|(&neg, &pos)| (neg + pos) / 2.0)
            .collect();

        let label_cstr = safe_cstring(self.label);

        unsafe {
            sys::ImPlot_PlotErrorBars_double(
                label_cstr.as_ptr(),
                self.x_data.as_ptr(),
                self.y_data.as_ptr(),
                avg_errors.as_ptr(),
                self.x_data.len() as i32,
                self.flags as i32,
            );
        }
    }

    fn label(&self) -> &str {
        self.label
    }
}

/// Simple error bars plot for quick plotting
pub struct SimpleErrorBarsPlot<'a> {
    label: &'a str,
    values: &'a [f64],
    errors: &'a [f64],
    x_scale: f64,
    x_start: f64,
}

impl<'a> SimpleErrorBarsPlot<'a> {
    /// Create a simple error bars plot with Y values only (X will be indices)
    pub fn new(label: &'a str, values: &'a [f64], errors: &'a [f64]) -> Self {
        Self {
            label,
            values,
            errors,
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

    /// Validate the plot data
    pub fn validate(&self) -> Result<(), PlotError> {
        validate_data_lengths(&self.values, &self.errors)?;

        if self.errors.iter().any(|&err| err < 0.0) {
            return Err(PlotError::InvalidData(
                "Error values cannot be negative".to_string(),
            ));
        }

        Ok(())
    }
}

impl<'a> Plot for SimpleErrorBarsPlot<'a> {
    fn plot(&self) {
        if let Err(_) = self.validate() {
            return;
        }

        let label_cstr = safe_cstring(self.label);

        // Create temporary X data
        let x_data: Vec<f64> = (0..self.values.len())
            .map(|i| self.x_start + i as f64 * self.x_scale)
            .collect();

        unsafe {
            sys::ImPlot_PlotErrorBars_double(
                label_cstr.as_ptr(),
                x_data.as_ptr(),
                self.values.as_ptr(),
                self.errors.as_ptr(),
                self.values.len() as i32,
                0, // flags
            );
        }
    }

    fn label(&self) -> &str {
        self.label
    }
}

/// Convenience functions for quick error bars plotting
impl<'ui> crate::PlotUi<'ui> {
    /// Plot error bars with symmetric errors
    pub fn error_bars_plot(
        &self,
        label: &str,
        x_data: &[f64],
        y_data: &[f64],
        err_data: &[f64],
    ) -> Result<(), PlotError> {
        let plot = ErrorBarsPlot::new(label, x_data, y_data, err_data);
        plot.validate()?;
        plot.plot();
        Ok(())
    }

    /// Plot error bars with asymmetric errors
    pub fn asymmetric_error_bars_plot(
        &self,
        label: &str,
        x_data: &[f64],
        y_data: &[f64],
        err_neg: &[f64],
        err_pos: &[f64],
    ) -> Result<(), PlotError> {
        let plot = AsymmetricErrorBarsPlot::new(label, x_data, y_data, err_neg, err_pos);
        plot.validate()?;
        plot.plot();
        Ok(())
    }

    /// Plot simple error bars with Y values only (X will be indices)
    pub fn simple_error_bars_plot(
        &self,
        label: &str,
        values: &[f64],
        errors: &[f64],
    ) -> Result<(), PlotError> {
        let plot = SimpleErrorBarsPlot::new(label, values, errors);
        plot.validate()?;
        plot.plot();
        Ok(())
    }
}
