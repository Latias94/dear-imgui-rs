//! Infinite lines plot implementation

use super::{Plot, PlotError, with_plot_str_or_empty};
use crate::{InfLinesFlags, sys};

/// Builder for infinite lines plots
pub struct InfLinesPlot<'a> {
    label: &'a str,
    positions: &'a [f64],
    flags: InfLinesFlags,
    offset: i32,
    stride: i32,
}

impl<'a> InfLinesPlot<'a> {
    /// Create a new infinite lines plot with the given label and positions (vertical by default)
    pub fn new(label: &'a str, positions: &'a [f64]) -> Self {
        Self {
            label,
            positions,
            flags: InfLinesFlags::NONE,
            offset: 0,
            stride: std::mem::size_of::<f64>() as i32,
        }
    }

    /// Make lines horizontal instead of vertical
    pub fn horizontal(mut self) -> Self {
        self.flags |= InfLinesFlags::HORIZONTAL;
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
        if self.positions.is_empty() {
            return Err(PlotError::EmptyData);
        }
        Ok(())
    }
}

impl<'a> Plot for InfLinesPlot<'a> {
    fn plot(&self) {
        if self.validate().is_err() {
            return;
        }
        with_plot_str_or_empty(self.label, |label_ptr| unsafe {
            sys::ImPlot_PlotInfLines_doublePtr(
                label_ptr,
                self.positions.as_ptr(),
                self.positions.len() as i32,
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

/// Convenience functions for quick inf-lines plotting
impl<'ui> crate::PlotUi<'ui> {
    /// Plot vertical infinite lines at given x positions
    pub fn inf_lines_vertical(&self, label: &str, xs: &[f64]) -> Result<(), PlotError> {
        let plot = InfLinesPlot::new(label, xs);
        plot.validate()?;
        plot.plot();
        Ok(())
    }

    /// Plot horizontal infinite lines at given y positions
    pub fn inf_lines_horizontal(&self, label: &str, ys: &[f64]) -> Result<(), PlotError> {
        let plot = InfLinesPlot::new(label, ys).horizontal();
        plot.validate()?;
        plot.plot();
        Ok(())
    }
}
