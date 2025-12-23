//! Bar plot implementation

use super::{Plot, PlotError, with_plot_str_or_empty};
use crate::{BarsFlags, sys};

/// Builder for bar plots with customization options
pub struct BarPlot<'a> {
    label: &'a str,
    values: &'a [f64],
    bar_size: f64,
    shift: f64,
    flags: BarsFlags,
    offset: i32,
    stride: i32,
}

impl<'a> BarPlot<'a> {
    /// Create a new bar plot with the given label and values
    pub fn new(label: &'a str, values: &'a [f64]) -> Self {
        Self {
            label,
            values,
            bar_size: 0.67, // Default bar width
            shift: 0.0,
            flags: BarsFlags::NONE,
            offset: 0,
            stride: std::mem::size_of::<f64>() as i32,
        }
    }

    /// Set the bar width (in plot units)
    pub fn with_bar_size(mut self, bar_size: f64) -> Self {
        self.bar_size = bar_size;
        self
    }

    /// Set the bar shift (in plot units)
    pub fn with_shift(mut self, shift: f64) -> Self {
        self.shift = shift;
        self
    }

    /// Set bar flags for customization
    pub fn with_flags(mut self, flags: BarsFlags) -> Self {
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
        if self.values.is_empty() {
            Err(PlotError::EmptyData)
        } else {
            Ok(())
        }
    }
}

impl<'a> Plot for BarPlot<'a> {
    fn plot(&self) {
        if self.validate().is_err() {
            return; // Skip plotting if data is invalid
        }
        let Ok(count) = i32::try_from(self.values.len()) else {
            return;
        };

        with_plot_str_or_empty(self.label, |label_ptr| unsafe {
            sys::ImPlot_PlotBars_doublePtrInt(
                label_ptr,
                self.values.as_ptr(),
                count,
                self.bar_size,
                self.shift,
                self.flags.bits() as i32,
                self.offset,
                self.stride,
            );
        })
    }

    fn label(&self) -> &str {
        self.label
    }
}

/// Bar plot with explicit X positions
pub struct PositionalBarPlot<'a> {
    label: &'a str,
    x_data: &'a [f64],
    y_data: &'a [f64],
    bar_size: f64,
    flags: BarsFlags,
}

impl<'a> PositionalBarPlot<'a> {
    /// Create a new positional bar plot with explicit X and Y data
    pub fn new(label: &'a str, x_data: &'a [f64], y_data: &'a [f64]) -> Self {
        Self {
            label,
            x_data,
            y_data,
            bar_size: 0.67,
            flags: BarsFlags::NONE,
        }
    }

    /// Set the bar width (in plot units)
    pub fn with_bar_size(mut self, bar_size: f64) -> Self {
        self.bar_size = bar_size;
        self
    }

    /// Set bar flags for customization
    pub fn with_flags(mut self, flags: BarsFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Validate the plot data
    pub fn validate(&self) -> Result<(), PlotError> {
        super::validate_data_lengths(self.x_data, self.y_data)
    }
}

impl<'a> Plot for PositionalBarPlot<'a> {
    fn plot(&self) {
        if self.validate().is_err() {
            return; // Skip plotting if data is invalid
        }
        let Ok(count) = i32::try_from(self.y_data.len()) else {
            return;
        };

        with_plot_str_or_empty(self.label, |label_ptr| unsafe {
            sys::ImPlot_PlotBars_doublePtrdoublePtr(
                label_ptr,
                self.x_data.as_ptr(),
                self.y_data.as_ptr(),
                count,
                self.bar_size,
                self.flags.bits() as i32,
                0,
                std::mem::size_of::<f64>() as i32,
            );
        })
    }

    fn label(&self) -> &str {
        self.label
    }
}

/// Convenience functions for quick bar plotting
impl<'ui> crate::PlotUi<'ui> {
    /// Plot a bar chart with values (X will be indices)
    pub fn bar_plot(&self, label: &str, values: &[f64]) -> Result<(), PlotError> {
        let plot = BarPlot::new(label, values);
        plot.validate()?;
        plot.plot();
        Ok(())
    }

    /// Plot a bar chart with custom bar width
    pub fn bar_plot_with_width(
        &self,
        label: &str,
        values: &[f64],
        width: f64,
    ) -> Result<(), PlotError> {
        let plot = BarPlot::new(label, values).with_bar_size(width);
        plot.validate()?;
        plot.plot();
        Ok(())
    }

    /// Plot a positional bar chart with explicit X and Y data
    pub fn positional_bar_plot(
        &self,
        label: &str,
        x_data: &[f64],
        y_data: &[f64],
    ) -> Result<(), PlotError> {
        let plot = PositionalBarPlot::new(label, x_data, y_data);
        plot.validate()?;
        plot.plot();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bar_plot_creation() {
        let values = [1.0, 2.0, 3.0, 4.0];
        let plot = BarPlot::new("test", &values);
        assert_eq!(plot.label(), "test");
        assert!(plot.validate().is_ok());
    }

    #[test]
    fn test_bar_plot_empty_data() {
        let values: &[f64] = &[];
        let plot = BarPlot::new("test", values);
        assert!(plot.validate().is_err());
    }

    #[test]
    fn test_positional_bar_plot() {
        let x_data = [1.0, 2.0, 3.0, 4.0];
        let y_data = [1.0, 4.0, 2.0, 3.0];

        let plot = PositionalBarPlot::new("test", &x_data, &y_data);
        assert_eq!(plot.label(), "test");
        assert!(plot.validate().is_ok());
    }

    #[test]
    fn test_positional_bar_plot_validation() {
        let x_data = [1.0, 2.0, 3.0];
        let y_data = [1.0, 4.0]; // Different length

        let plot = PositionalBarPlot::new("test", &x_data, &y_data);
        assert!(plot.validate().is_err());
    }
}
