//! Scatter plot implementation

use super::{
    Plot, PlotError, PlotItemStyle, plot_spec_with_style, validate_data_lengths,
    with_plot_str_or_empty,
};
use crate::{ItemFlags, Marker, ScatterFlags, sys};

/// Builder for scatter plots with customization options
pub struct ScatterPlot<'a> {
    label: &'a str,
    x_data: &'a [f64],
    y_data: &'a [f64],
    style: PlotItemStyle,
    flags: ScatterFlags,
    item_flags: ItemFlags,
    offset: i32,
    stride: i32,
}

impl<'a> super::PlotItemStyled for ScatterPlot<'a> {
    fn style_mut(&mut self) -> &mut PlotItemStyle {
        &mut self.style
    }
}

impl<'a> ScatterPlot<'a> {
    /// Create a new scatter plot with the given label and data
    pub fn new(label: &'a str, x_data: &'a [f64], y_data: &'a [f64]) -> Self {
        Self {
            label,
            x_data,
            y_data,
            style: PlotItemStyle::default(),
            flags: ScatterFlags::NONE,
            item_flags: ItemFlags::NONE,
            offset: 0,
            stride: std::mem::size_of::<f64>() as i32,
        }
    }

    /// Replace the entire item style override for this scatter plot.
    pub fn with_style(mut self, style: PlotItemStyle) -> Self {
        self.style = style;
        self
    }

    /// Set the scatter line color. Use the alpha channel to control transparency.
    pub fn with_line_color(mut self, color: [f32; 4]) -> Self {
        self.style = self.style.with_line_color(color);
        self
    }

    /// Set the outline width in pixels.
    pub fn with_line_weight(mut self, weight: f32) -> Self {
        self.style = self.style.with_line_weight(weight);
        self
    }

    /// Set the marker type for the scatter plot.
    pub fn with_marker(mut self, marker: Marker) -> Self {
        self.style = self.style.with_marker(marker);
        self
    }

    /// Set the marker size in pixels.
    pub fn with_marker_size(mut self, size: f32) -> Self {
        self.style = self.style.with_marker_size(size);
        self
    }

    /// Set the marker outline color.
    pub fn with_marker_line_color(mut self, color: [f32; 4]) -> Self {
        self.style = self.style.with_marker_line_color(color);
        self
    }

    /// Set the marker fill color.
    pub fn with_marker_fill_color(mut self, color: [f32; 4]) -> Self {
        self.style = self.style.with_marker_fill_color(color);
        self
    }

    /// Set scatter flags for customization
    pub fn with_flags(mut self, flags: ScatterFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Set common item flags for this plot item (applies to all plot types)
    pub fn with_item_flags(mut self, flags: ItemFlags) -> Self {
        self.item_flags = flags;
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

impl<'a> Plot for ScatterPlot<'a> {
    fn plot(&self) {
        if self.validate().is_err() {
            return; // Skip plotting if data is invalid
        }
        let Ok(count) = i32::try_from(self.x_data.len()) else {
            return;
        };

        with_plot_str_or_empty(self.label, |label_ptr| unsafe {
            let spec = plot_spec_with_style(
                self.style,
                self.flags.bits() | self.item_flags.bits(),
                self.offset,
                self.stride,
            );
            sys::ImPlot_PlotScatter_doublePtrdoublePtr(
                label_ptr,
                self.x_data.as_ptr(),
                self.y_data.as_ptr(),
                count,
                spec,
            );
        })
    }

    fn label(&self) -> &str {
        self.label
    }
}

/// Simple scatter plot for quick plotting without builder pattern
pub struct SimpleScatterPlot<'a> {
    label: &'a str,
    values: &'a [f64],
    style: PlotItemStyle,
    flags: ScatterFlags,
    item_flags: ItemFlags,
    x_scale: f64,
    x_start: f64,
}

impl<'a> super::PlotItemStyled for SimpleScatterPlot<'a> {
    fn style_mut(&mut self) -> &mut PlotItemStyle {
        &mut self.style
    }
}

impl<'a> SimpleScatterPlot<'a> {
    /// Create a simple scatter plot with Y values only (X will be indices)
    pub fn new(label: &'a str, values: &'a [f64]) -> Self {
        Self {
            label,
            values,
            style: PlotItemStyle::default(),
            flags: ScatterFlags::NONE,
            item_flags: ItemFlags::NONE,
            x_scale: 1.0,
            x_start: 0.0,
        }
    }

    /// Replace the entire item style override for this scatter plot.
    pub fn with_style(mut self, style: PlotItemStyle) -> Self {
        self.style = style;
        self
    }

    /// Set the scatter line color. Use the alpha channel to control transparency.
    pub fn with_line_color(mut self, color: [f32; 4]) -> Self {
        self.style = self.style.with_line_color(color);
        self
    }

    /// Set the outline width in pixels.
    pub fn with_line_weight(mut self, weight: f32) -> Self {
        self.style = self.style.with_line_weight(weight);
        self
    }

    /// Set the marker type for the scatter plot.
    pub fn with_marker(mut self, marker: Marker) -> Self {
        self.style = self.style.with_marker(marker);
        self
    }

    /// Set the marker size in pixels.
    pub fn with_marker_size(mut self, size: f32) -> Self {
        self.style = self.style.with_marker_size(size);
        self
    }

    /// Set the marker outline color.
    pub fn with_marker_line_color(mut self, color: [f32; 4]) -> Self {
        self.style = self.style.with_marker_line_color(color);
        self
    }

    /// Set the marker fill color.
    pub fn with_marker_fill_color(mut self, color: [f32; 4]) -> Self {
        self.style = self.style.with_marker_fill_color(color);
        self
    }

    /// Set scatter flags for customization
    pub fn with_flags(mut self, flags: ScatterFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Set common item flags for this plot item (applies to all plot types)
    pub fn with_item_flags(mut self, flags: ItemFlags) -> Self {
        self.item_flags = flags;
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

impl<'a> Plot for SimpleScatterPlot<'a> {
    fn plot(&self) {
        if self.values.is_empty() {
            return;
        }
        let Ok(count) = i32::try_from(self.values.len()) else {
            return;
        };

        with_plot_str_or_empty(self.label, |label_ptr| unsafe {
            let spec = plot_spec_with_style(
                self.style,
                self.flags.bits() | self.item_flags.bits(),
                0,
                std::mem::size_of::<f64>() as i32,
            );
            sys::ImPlot_PlotScatter_doublePtrInt(
                label_ptr,
                self.values.as_ptr(),
                count,
                self.x_scale,
                self.x_start,
                spec,
            );
        })
    }

    fn label(&self) -> &str {
        self.label
    }
}

/// Convenience functions for quick scatter plotting
impl<'ui> crate::PlotUi<'ui> {
    /// Plot a scatter plot with X and Y data
    pub fn scatter_plot(
        &self,
        label: &str,
        x_data: &[f64],
        y_data: &[f64],
    ) -> Result<(), PlotError> {
        let plot = ScatterPlot::new(label, x_data, y_data);
        plot.validate()?;
        plot.plot();
        Ok(())
    }

    /// Plot a simple scatter plot with Y values only (X will be indices)
    pub fn simple_scatter_plot(&self, label: &str, values: &[f64]) -> Result<(), PlotError> {
        if values.is_empty() {
            return Err(PlotError::EmptyData);
        }
        let plot = SimpleScatterPlot::new(label, values);
        plot.plot();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scatter_plot_creation() {
        let x_data = [1.0, 2.0, 3.0, 4.0];
        let y_data = [1.0, 4.0, 2.0, 3.0];

        let plot = ScatterPlot::new("test", &x_data, &y_data);
        assert_eq!(plot.label(), "test");
        assert!(plot.validate().is_ok());
    }

    #[test]
    fn test_scatter_plot_validation() {
        let x_data = [1.0, 2.0, 3.0];
        let y_data = [1.0, 4.0]; // Different length

        let plot = ScatterPlot::new("test", &x_data, &y_data);
        assert!(plot.validate().is_err());
    }

    #[test]
    fn test_simple_scatter_plot() {
        let values = [1.0, 2.0, 3.0, 4.0];
        let plot = SimpleScatterPlot::new("test", &values)
            .with_flags(ScatterFlags::NO_CLIP)
            .with_item_flags(ItemFlags::NO_FIT);
        assert_eq!(plot.label(), "test");
        assert_eq!(plot.flags.bits(), ScatterFlags::NO_CLIP.bits());
        assert_eq!(plot.item_flags, ItemFlags::NO_FIT);
    }

    #[test]
    fn test_scatter_plot_style_builders() {
        let x_data = [1.0, 2.0, 3.0, 4.0];
        let y_data = [1.0, 4.0, 2.0, 3.0];

        let plot = ScatterPlot::new("styled", &x_data, &y_data)
            .with_line_color([0.1, 0.2, 0.3, 0.4])
            .with_line_weight(1.5)
            .with_marker(Marker::Square)
            .with_marker_size(8.0)
            .with_marker_line_color([0.9, 0.8, 0.7, 0.6])
            .with_marker_fill_color([0.6, 0.7, 0.8, 0.9]);

        assert_eq!(
            plot.style.line_color,
            Some(sys::ImVec4_c {
                x: 0.1,
                y: 0.2,
                z: 0.3,
                w: 0.4,
            })
        );
        assert_eq!(plot.style.line_weight, Some(1.5));
        assert_eq!(plot.style.marker, Some(Marker::Square as sys::ImPlotMarker));
        assert_eq!(plot.style.marker_size, Some(8.0));
        assert_eq!(
            plot.style.marker_line_color,
            Some(sys::ImVec4_c {
                x: 0.9,
                y: 0.8,
                z: 0.7,
                w: 0.6,
            })
        );
        assert_eq!(
            plot.style.marker_fill_color,
            Some(sys::ImVec4_c {
                x: 0.6,
                y: 0.7,
                z: 0.8,
                w: 0.9,
            })
        );
    }
}
