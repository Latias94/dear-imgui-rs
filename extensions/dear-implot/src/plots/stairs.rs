//! Stairs plot implementation

use super::{safe_cstring, validate_data_lengths, PlotData, PlotError};
use crate::sys;
use crate::StairsFlags;

/// Builder for stairs plots with extensive customization options
pub struct StairsPlot<'a> {
    label: &'a str,
    x_data: &'a [f64],
    y_data: &'a [f64],
    flags: StairsFlags,
    offset: i32,
    stride: i32,
}

impl<'a> StairsPlot<'a> {
    /// Create a new stairs plot with the given label and data
    pub fn new(label: &'a str, x_data: &'a [f64], y_data: &'a [f64]) -> Self {
        Self {
            label,
            x_data,
            y_data,
            flags: StairsFlags::NONE,
            offset: 0,
            stride: std::mem::size_of::<f64>() as i32,
        }
    }

    /// Set stairs flags for customization
    pub fn with_flags(mut self, flags: StairsFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Enable pre-step mode (step before the point instead of after)
    pub fn pre_step(mut self) -> Self {
        self.flags |= StairsFlags::PRE_STEP;
        self
    }

    /// Enable shaded stairs (fill area under stairs)
    pub fn shaded(mut self) -> Self {
        self.flags |= StairsFlags::SHADED;
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

    /// Plot the stairs
    pub fn plot(self) {
        let label_cstring = safe_cstring(self.label);

        unsafe {
            sys::ImPlot_PlotStairs_double(
                label_cstring.as_ptr(),
                self.x_data.as_ptr(),
                self.y_data.as_ptr(),
                self.x_data.len() as i32,
                self.flags.bits() as i32,
            );
        }
    }
}

impl<'a> PlotData for StairsPlot<'a> {
    fn label(&self) -> &str {
        self.label
    }

    fn data_len(&self) -> usize {
        self.x_data.len().min(self.y_data.len())
    }
}

/// Simple stairs plot for f32 data
pub struct StairsPlotF32<'a> {
    label: &'a str,
    x_data: &'a [f32],
    y_data: &'a [f32],
    flags: StairsFlags,
}

impl<'a> StairsPlotF32<'a> {
    /// Create a new stairs plot with f32 data
    pub fn new(label: &'a str, x_data: &'a [f32], y_data: &'a [f32]) -> Self {
        Self {
            label,
            x_data,
            y_data,
            flags: StairsFlags::NONE,
        }
    }

    /// Set stairs flags for customization
    pub fn with_flags(mut self, flags: StairsFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Enable pre-step mode
    pub fn pre_step(mut self) -> Self {
        self.flags |= StairsFlags::PRE_STEP;
        self
    }

    /// Enable shaded stairs
    pub fn shaded(mut self) -> Self {
        self.flags |= StairsFlags::SHADED;
        self
    }

    /// Validate the plot data
    pub fn validate(&self) -> Result<(), PlotError> {
        if self.x_data.len() != self.y_data.len() {
            return Err(PlotError::DataLengthMismatch {
                x_len: self.x_data.len(),
                y_len: self.y_data.len(),
            });
        }
        if self.x_data.is_empty() {
            return Err(PlotError::EmptyData);
        }
        Ok(())
    }

    /// Plot the stairs
    pub fn plot(self) {
        let label_cstring = safe_cstring(self.label);

        unsafe {
            sys::ImPlot_PlotStairs_float(
                label_cstring.as_ptr(),
                self.x_data.as_ptr(),
                self.y_data.as_ptr(),
                self.x_data.len() as i32,
                self.flags.bits() as i32,
            );
        }
    }
}

impl<'a> PlotData for StairsPlotF32<'a> {
    fn label(&self) -> &str {
        self.label
    }

    fn data_len(&self) -> usize {
        self.x_data.len().min(self.y_data.len())
    }
}

/// Simple stairs plot for single array data (y values only, x is auto-generated)
pub struct SimpleStairsPlot<'a> {
    label: &'a str,
    y_data: &'a [f64],
    flags: StairsFlags,
    x_scale: f64,
    x_start: f64,
}

impl<'a> SimpleStairsPlot<'a> {
    /// Create a new simple stairs plot with only y data
    pub fn new(label: &'a str, y_data: &'a [f64]) -> Self {
        Self {
            label,
            y_data,
            flags: StairsFlags::NONE,
            x_scale: 1.0,
            x_start: 0.0,
        }
    }

    /// Set the x scale (spacing between points)
    pub fn with_x_scale(mut self, x_scale: f64) -> Self {
        self.x_scale = x_scale;
        self
    }

    /// Set the x start value
    pub fn with_x_start(mut self, x_start: f64) -> Self {
        self.x_start = x_start;
        self
    }

    /// Set stairs flags
    pub fn with_flags(mut self, flags: StairsFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Enable pre-step mode
    pub fn pre_step(mut self) -> Self {
        self.flags |= StairsFlags::PRE_STEP;
        self
    }

    /// Enable shaded stairs
    pub fn shaded(mut self) -> Self {
        self.flags |= StairsFlags::SHADED;
        self
    }

    /// Validate the plot data
    pub fn validate(&self) -> Result<(), PlotError> {
        if self.y_data.is_empty() {
            return Err(PlotError::EmptyData);
        }
        Ok(())
    }

    /// Plot the stairs
    pub fn plot(self) {
        // Generate x data
        let x_data: Vec<f64> = (0..self.y_data.len())
            .map(|i| self.x_start + i as f64 * self.x_scale)
            .collect();

        let label_cstring = safe_cstring(self.label);

        unsafe {
            sys::ImPlot_PlotStairs_double(
                label_cstring.as_ptr(),
                x_data.as_ptr(),
                self.y_data.as_ptr(),
                self.y_data.len() as i32,
                self.flags.bits() as i32,
            );
        }
    }
}

impl<'a> PlotData for SimpleStairsPlot<'a> {
    fn label(&self) -> &str {
        self.label
    }

    fn data_len(&self) -> usize {
        self.y_data.len()
    }
}
