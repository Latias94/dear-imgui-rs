//! Pie chart plot implementation

use super::{safe_cstring, Plot, PlotError};
use crate::sys;
use crate::PieChartFlags;

/// Builder for pie chart plots
pub struct PieChartPlot<'a> {
    label_ids: Vec<&'a str>,
    values: &'a [f64],
    center_x: f64,
    center_y: f64,
    radius: f64,
    label_fmt: Option<&'a str>,
    angle0: f64,
    flags: PieChartFlags,
}

impl<'a> PieChartPlot<'a> {
    /// Create a new pie chart plot
    ///
    /// # Arguments
    /// * `label_ids` - Labels for each slice of the pie
    /// * `values` - Values for each slice
    /// * `center_x` - X coordinate of the pie center in plot units
    /// * `center_y` - Y coordinate of the pie center in plot units
    /// * `radius` - Radius of the pie in plot units
    pub fn new(
        label_ids: Vec<&'a str>,
        values: &'a [f64],
        center_x: f64,
        center_y: f64,
        radius: f64,
    ) -> Self {
        Self {
            label_ids,
            values,
            center_x,
            center_y,
            radius,
            label_fmt: Some("%.1f"),
            angle0: 90.0, // Start angle in degrees
            flags: PieChartFlags::NONE,
        }
    }

    /// Set the label format for slice values (e.g., "%.1f", "%.0f%%")
    /// Set to None to disable labels
    pub fn with_label_format(mut self, fmt: Option<&'a str>) -> Self {
        self.label_fmt = fmt;
        self
    }

    /// Set the starting angle in degrees (default: 90.0)
    pub fn with_start_angle(mut self, angle: f64) -> Self {
        self.angle0 = angle;
        self
    }

    /// Set pie chart flags for customization
    pub fn with_flags(mut self, flags: PieChartFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Normalize the pie chart values (force full circle even if sum < 1.0)
    pub fn normalize(mut self) -> Self {
        self.flags |= PieChartFlags::NORMALIZE;
        self
    }

    /// Ignore hidden slices when drawing (as if they were not there)
    pub fn ignore_hidden(mut self) -> Self {
        self.flags |= PieChartFlags::IGNORE_HIDDEN;
        self
    }

    /// Enable exploding effect for legend-hovered slices
    pub fn exploding(mut self) -> Self {
        self.flags |= PieChartFlags::EXPLODING;
        self
    }

    /// Validate the plot data
    pub fn validate(&self) -> Result<(), PlotError> {
        if self.values.is_empty() {
            return Err(PlotError::EmptyData);
        }

        if self.label_ids.len() != self.values.len() {
            return Err(PlotError::DataLengthMismatch {
                x_len: self.label_ids.len(),
                y_len: self.values.len(),
            });
        }

        if self.radius <= 0.0 {
            return Err(PlotError::InvalidData(
                "Radius must be positive".to_string(),
            ));
        }

        // Check for negative values
        if self.values.iter().any(|&v| v < 0.0) {
            return Err(PlotError::InvalidData(
                "Pie chart values cannot be negative".to_string(),
            ));
        }

        Ok(())
    }
}

impl<'a> Plot for PieChartPlot<'a> {
    fn plot(&self) {
        if self.validate().is_err() {
            return;
        }

        // Convert label strings to CStrings
        let label_cstrs: Vec<_> = self
            .label_ids
            .iter()
            .map(|&label| safe_cstring(label))
            .collect();

        // Create array of pointers to C strings
        let label_ptrs: Vec<*const std::os::raw::c_char> =
            label_cstrs.iter().map(|cstr| cstr.as_ptr()).collect();

        let label_fmt_cstr = self.label_fmt.map(safe_cstring);
        let label_fmt_ptr = label_fmt_cstr
            .as_ref()
            .map(|cstr| cstr.as_ptr())
            .unwrap_or(std::ptr::null());

        unsafe {
            sys::ImPlot_PlotPieChart_doublePtrStr(
                label_ptrs.as_ptr(),
                self.values.as_ptr(),
                self.values.len() as i32,
                self.center_x,
                self.center_y,
                self.radius,
                label_fmt_ptr,
                self.angle0,
                self.flags.bits() as i32,
            );
        }
    }

    fn label(&self) -> &str {
        "PieChart" // Pie charts don't have a single label
    }
}

/// Float version of pie chart for better performance with f32 data
pub struct PieChartPlotF32<'a> {
    label_ids: Vec<&'a str>,
    values: &'a [f32],
    center_x: f64,
    center_y: f64,
    radius: f64,
    label_fmt: Option<&'a str>,
    angle0: f64,
    flags: PieChartFlags,
}

impl<'a> PieChartPlotF32<'a> {
    /// Create a new f32 pie chart plot
    pub fn new(
        label_ids: Vec<&'a str>,
        values: &'a [f32],
        center_x: f64,
        center_y: f64,
        radius: f64,
    ) -> Self {
        Self {
            label_ids,
            values,
            center_x,
            center_y,
            radius,
            label_fmt: Some("%.1f"),
            angle0: 90.0,
            flags: PieChartFlags::NONE,
        }
    }

    /// Set the label format for slice values
    pub fn with_label_format(mut self, fmt: Option<&'a str>) -> Self {
        self.label_fmt = fmt;
        self
    }

    /// Set the starting angle in degrees
    pub fn with_start_angle(mut self, angle: f64) -> Self {
        self.angle0 = angle;
        self
    }

    /// Set pie chart flags for customization
    pub fn with_flags(mut self, flags: PieChartFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Normalize the pie chart values
    pub fn normalize(mut self) -> Self {
        self.flags |= PieChartFlags::NORMALIZE;
        self
    }

    /// Ignore hidden slices when drawing
    pub fn ignore_hidden(mut self) -> Self {
        self.flags |= PieChartFlags::IGNORE_HIDDEN;
        self
    }

    /// Enable exploding effect for legend-hovered slices
    pub fn exploding(mut self) -> Self {
        self.flags |= PieChartFlags::EXPLODING;
        self
    }

    /// Validate the plot data
    pub fn validate(&self) -> Result<(), PlotError> {
        if self.values.is_empty() {
            return Err(PlotError::EmptyData);
        }

        if self.label_ids.len() != self.values.len() {
            return Err(PlotError::DataLengthMismatch {
                x_len: self.label_ids.len(),
                y_len: self.values.len(),
            });
        }

        if self.radius <= 0.0 {
            return Err(PlotError::InvalidData(
                "Radius must be positive".to_string(),
            ));
        }

        if self.values.iter().any(|&v| v < 0.0) {
            return Err(PlotError::InvalidData(
                "Pie chart values cannot be negative".to_string(),
            ));
        }

        Ok(())
    }
}

impl<'a> Plot for PieChartPlotF32<'a> {
    fn plot(&self) {
        if self.validate().is_err() {
            return;
        }

        let label_cstrs: Vec<_> = self
            .label_ids
            .iter()
            .map(|&label| safe_cstring(label))
            .collect();

        let label_ptrs: Vec<*const std::os::raw::c_char> =
            label_cstrs.iter().map(|cstr| cstr.as_ptr()).collect();

        let label_fmt_cstr = self.label_fmt.map(safe_cstring);
        let label_fmt_ptr = label_fmt_cstr
            .as_ref()
            .map(|cstr| cstr.as_ptr())
            .unwrap_or(std::ptr::null());

        unsafe {
            sys::ImPlot_PlotPieChart_FloatPtrStr(
                label_ptrs.as_ptr(),
                self.values.as_ptr(),
                self.values.len() as i32,
                self.center_x,
                self.center_y,
                self.radius,
                label_fmt_ptr,
                self.angle0,
                self.flags.bits() as i32,
            );
        }
    }

    fn label(&self) -> &str {
        "PieChart"
    }
}

/// Convenience functions for quick pie chart plotting
impl<'ui> crate::PlotUi<'ui> {
    /// Plot a pie chart with f64 data
    pub fn pie_chart_plot(
        &self,
        label_ids: Vec<&str>,
        values: &[f64],
        center_x: f64,
        center_y: f64,
        radius: f64,
    ) -> Result<(), PlotError> {
        let plot = PieChartPlot::new(label_ids, values, center_x, center_y, radius);
        plot.validate()?;
        plot.plot();
        Ok(())
    }

    /// Plot a pie chart with f32 data
    pub fn pie_chart_plot_f32(
        &self,
        label_ids: Vec<&str>,
        values: &[f32],
        center_x: f64,
        center_y: f64,
        radius: f64,
    ) -> Result<(), PlotError> {
        let plot = PieChartPlotF32::new(label_ids, values, center_x, center_y, radius);
        plot.validate()?;
        plot.plot();
        Ok(())
    }

    /// Plot a centered pie chart (center at 0.5, 0.5 with radius 0.4)
    pub fn centered_pie_chart(
        &self,
        label_ids: Vec<&str>,
        values: &[f64],
    ) -> Result<(), PlotError> {
        let plot = PieChartPlot::new(label_ids, values, 0.5, 0.5, 0.4);
        plot.validate()?;
        plot.plot();
        Ok(())
    }
}
