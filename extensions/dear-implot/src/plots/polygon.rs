//! Polygon plot implementation

use super::{
    Plot, PlotError, PlotItemStyle, plot_spec_with_style, validate_data_lengths,
    with_plot_str_or_empty,
};
use crate::{ItemFlags, PolygonFlags, sys};

/// Builder for polygon plots.
pub struct PolygonPlot<'a> {
    label: &'a str,
    x_data: &'a [f64],
    y_data: &'a [f64],
    style: PlotItemStyle,
    flags: PolygonFlags,
    item_flags: ItemFlags,
    offset: i32,
    stride: i32,
}

impl<'a> super::PlotItemStyled for PolygonPlot<'a> {
    fn style_mut(&mut self) -> &mut PlotItemStyle {
        &mut self.style
    }
}

impl<'a> PolygonPlot<'a> {
    /// Create a new polygon plot with the given label and vertices.
    pub fn new(label: &'a str, x_data: &'a [f64], y_data: &'a [f64]) -> Self {
        Self {
            label,
            x_data,
            y_data,
            style: PlotItemStyle::default(),
            flags: PolygonFlags::NONE,
            item_flags: ItemFlags::NONE,
            offset: 0,
            stride: std::mem::size_of::<f64>() as i32,
        }
    }

    /// Replace the entire item style override for this polygon plot.
    pub fn with_style(mut self, style: PlotItemStyle) -> Self {
        self.style = style;
        self
    }

    /// Set the line color. Use the alpha channel to control polygon outline transparency.
    pub fn with_line_color(mut self, color: [f32; 4]) -> Self {
        self.style = self.style.with_line_color(color);
        self
    }

    /// Set the line width in pixels.
    pub fn with_line_weight(mut self, weight: f32) -> Self {
        self.style = self.style.with_line_weight(weight);
        self
    }

    /// Set the fill color.
    pub fn with_fill_color(mut self, color: [f32; 4]) -> Self {
        self.style = self.style.with_fill_color(color);
        self
    }

    /// Set the fill alpha multiplier.
    pub fn with_fill_alpha(mut self, alpha: f32) -> Self {
        self.style = self.style.with_fill_alpha(alpha);
        self
    }

    /// Set polygon-specific flags.
    pub fn with_flags(mut self, flags: PolygonFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Set common item flags for this plot item.
    pub fn with_item_flags(mut self, flags: ItemFlags) -> Self {
        self.item_flags = flags;
        self
    }

    /// Set data offset for partial plotting.
    pub fn with_offset(mut self, offset: i32) -> Self {
        self.offset = offset;
        self
    }

    /// Set data stride for non-contiguous data.
    pub fn with_stride(mut self, stride: i32) -> Self {
        self.stride = stride;
        self
    }

    /// Validate the polygon data.
    pub fn validate(&self) -> Result<(), PlotError> {
        validate_data_lengths(self.x_data, self.y_data)
    }
}

impl<'a> Plot for PolygonPlot<'a> {
    fn plot(&self) {
        if self.validate().is_err() {
            return;
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
            sys::ImPlot_PlotPolygon_doublePtr(
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

/// Convenience functions for quick polygon plotting.
impl<'ui> crate::PlotUi<'ui> {
    /// Plot a polygon with X and Y vertex data.
    pub fn polygon_plot(
        &self,
        label: &str,
        x_data: &[f64],
        y_data: &[f64],
    ) -> Result<(), PlotError> {
        let plot = PolygonPlot::new(label, x_data, y_data);
        plot.validate()?;
        plot.plot();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polygon_plot_creation() {
        let x_data = [0.0, 1.0, 1.0, 0.0];
        let y_data = [0.0, 0.0, 1.0, 1.0];

        let plot = PolygonPlot::new("poly", &x_data, &y_data);
        assert_eq!(plot.label(), "poly");
        assert!(plot.validate().is_ok());
    }

    #[test]
    fn polygon_plot_validation() {
        let x_data = [0.0, 1.0, 1.0];
        let y_data = [0.0, 0.0];

        let plot = PolygonPlot::new("poly", &x_data, &y_data);
        assert!(plot.validate().is_err());
    }
}
