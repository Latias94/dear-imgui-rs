//! Digital plot implementation

use super::{PlotData, PlotError, safe_cstring, validate_data_lengths};
use crate::DigitalFlags;
use crate::sys;

/// Builder for digital plots with extensive customization options
///
/// Digital plots are used to display digital signals (0/1, high/low, etc.)
/// They do not respond to y drag or zoom, and are always referenced to the bottom of the plot.
pub struct DigitalPlot<'a> {
    label: &'a str,
    x_data: &'a [f64],
    y_data: &'a [f64],
    flags: DigitalFlags,
    offset: i32,
    stride: i32,
}

impl<'a> DigitalPlot<'a> {
    /// Create a new digital plot with the given label and data
    pub fn new(label: &'a str, x_data: &'a [f64], y_data: &'a [f64]) -> Self {
        Self {
            label,
            x_data,
            y_data,
            flags: DigitalFlags::NONE,
            offset: 0,
            stride: std::mem::size_of::<f64>() as i32,
        }
    }

    /// Set digital flags for customization
    pub fn with_flags(mut self, flags: DigitalFlags) -> Self {
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
        validate_data_lengths(self.x_data, self.y_data)?;

        // Digital plots should have binary-like data (0/1, but we allow any values)
        // The validation is mainly for data length consistency
        Ok(())
    }

    /// Plot the digital signal
    pub fn plot(self) {
        let label_cstring = safe_cstring(self.label);

        unsafe {
            sys::ImPlot_PlotDigital_doublePtr(
                label_cstring.as_ptr(),
                self.x_data.as_ptr(),
                self.y_data.as_ptr(),
                self.x_data.len() as i32,
                self.flags.bits() as i32,
                self.offset,
                self.stride,
            );
        }
    }
}

impl<'a> PlotData for DigitalPlot<'a> {
    fn label(&self) -> &str {
        self.label
    }

    fn data_len(&self) -> usize {
        self.x_data.len().min(self.y_data.len())
    }
}

/// Digital plot for f32 data
pub struct DigitalPlotF32<'a> {
    label: &'a str,
    x_data: &'a [f32],
    y_data: &'a [f32],
    flags: DigitalFlags,
}

impl<'a> DigitalPlotF32<'a> {
    /// Create a new digital plot with f32 data
    pub fn new(label: &'a str, x_data: &'a [f32], y_data: &'a [f32]) -> Self {
        Self {
            label,
            x_data,
            y_data,
            flags: DigitalFlags::NONE,
        }
    }

    /// Set digital flags for customization
    pub fn with_flags(mut self, flags: DigitalFlags) -> Self {
        self.flags = flags;
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

    /// Plot the digital signal
    pub fn plot(self) {
        let label_cstring = safe_cstring(self.label);

        unsafe {
            sys::ImPlot_PlotDigital_FloatPtr(
                label_cstring.as_ptr(),
                self.x_data.as_ptr(),
                self.y_data.as_ptr(),
                self.x_data.len() as i32,
                self.flags.bits() as i32,
                0,
                std::mem::size_of::<f32>() as i32,
            );
        }
    }
}

impl<'a> PlotData for DigitalPlotF32<'a> {
    fn label(&self) -> &str {
        self.label
    }

    fn data_len(&self) -> usize {
        self.x_data.len().min(self.y_data.len())
    }
}

/// Simple digital plot for single array data (y values only, x is auto-generated)
pub struct SimpleDigitalPlot<'a> {
    label: &'a str,
    y_data: &'a [f64],
    flags: DigitalFlags,
    x_scale: f64,
    x_start: f64,
}

impl<'a> SimpleDigitalPlot<'a> {
    /// Create a new simple digital plot with only y data
    pub fn new(label: &'a str, y_data: &'a [f64]) -> Self {
        Self {
            label,
            y_data,
            flags: DigitalFlags::NONE,
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

    /// Set digital flags
    pub fn with_flags(mut self, flags: DigitalFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Validate the plot data
    pub fn validate(&self) -> Result<(), PlotError> {
        if self.y_data.is_empty() {
            return Err(PlotError::EmptyData);
        }
        Ok(())
    }

    /// Plot the digital signal
    pub fn plot(self) {
        // Generate x data
        let x_data: Vec<f64> = (0..self.y_data.len())
            .map(|i| self.x_start + i as f64 * self.x_scale)
            .collect();

        let label_cstring = safe_cstring(self.label);

        unsafe {
            sys::ImPlot_PlotDigital_doublePtr(
                label_cstring.as_ptr(),
                x_data.as_ptr(),
                self.y_data.as_ptr(),
                self.y_data.len() as i32,
                self.flags.bits() as i32,
                0,
                std::mem::size_of::<f64>() as i32,
            );
        }
    }
}

impl<'a> PlotData for SimpleDigitalPlot<'a> {
    fn label(&self) -> &str {
        self.label
    }

    fn data_len(&self) -> usize {
        self.y_data.len()
    }
}

/// Digital plot for boolean data (true/false converted to 1.0/0.0)
pub struct BooleanDigitalPlot<'a> {
    label: &'a str,
    x_data: &'a [f64],
    y_data: &'a [bool],
    flags: DigitalFlags,
}

impl<'a> BooleanDigitalPlot<'a> {
    /// Create a new digital plot with boolean data
    pub fn new(label: &'a str, x_data: &'a [f64], y_data: &'a [bool]) -> Self {
        Self {
            label,
            x_data,
            y_data,
            flags: DigitalFlags::NONE,
        }
    }

    /// Set digital flags for customization
    pub fn with_flags(mut self, flags: DigitalFlags) -> Self {
        self.flags = flags;
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

    /// Plot the digital signal
    pub fn plot(self) {
        // Convert boolean data to f64
        let y_data_f64: Vec<f64> = self
            .y_data
            .iter()
            .map(|&b| if b { 1.0 } else { 0.0 })
            .collect();

        let label_cstring = safe_cstring(self.label);

        unsafe {
            sys::ImPlot_PlotDigital_doublePtr(
                label_cstring.as_ptr(),
                self.x_data.as_ptr(),
                y_data_f64.as_ptr(),
                self.x_data.len() as i32,
                self.flags.bits() as i32,
                0,
                std::mem::size_of::<f64>() as i32,
            );
        }
    }
}

impl<'a> PlotData for BooleanDigitalPlot<'a> {
    fn label(&self) -> &str {
        self.label
    }

    fn data_len(&self) -> usize {
        self.x_data.len().min(self.y_data.len())
    }
}
