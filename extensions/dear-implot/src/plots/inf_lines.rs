//! Infinite lines plot implementation

use super::{
    Plot, PlotDataLayout, PlotDataOffset, PlotDataStride, PlotError, PlotItemStyle,
    plot_spec_with_style, with_plot_str_or_empty,
};
use crate::{InfLinesFlags, ItemFlags, sys};

/// Builder for infinite lines plots
pub struct InfLinesPlot<'a> {
    label: &'a str,
    positions: &'a [f64],
    style: PlotItemStyle,
    flags: InfLinesFlags,
    item_flags: ItemFlags,
    layout: PlotDataLayout,
}

impl<'a> super::PlotItemStyled for InfLinesPlot<'a> {
    fn style_mut(&mut self) -> &mut PlotItemStyle {
        &mut self.style
    }
}

impl<'a> InfLinesPlot<'a> {
    /// Create a new infinite lines plot with the given label and positions (vertical by default)
    pub fn new(label: &'a str, positions: &'a [f64]) -> Self {
        Self {
            label,
            positions,
            style: PlotItemStyle::default(),
            flags: InfLinesFlags::NONE,
            item_flags: ItemFlags::NONE,
            layout: PlotDataLayout::DEFAULT,
        }
    }

    /// Make lines horizontal instead of vertical
    pub fn horizontal(mut self) -> Self {
        self.flags |= InfLinesFlags::HORIZONTAL;
        self
    }

    /// Set common item flags for this plot item (applies to all plot types)
    pub fn with_item_flags(mut self, flags: ItemFlags) -> Self {
        self.item_flags = flags;
        self
    }

    /// Set the data layout used to read positions.
    pub fn with_data_layout(mut self, layout: PlotDataLayout) -> Self {
        self.layout = layout;
        self
    }

    /// Set the sample-index offset used to read positions.
    pub fn with_offset(mut self, offset: PlotDataOffset) -> Self {
        self.layout = self.layout.with_offset(offset);
        self
    }

    /// Set the byte stride used to read positions.
    pub fn with_stride(mut self, stride: PlotDataStride) -> Self {
        self.layout = self.layout.with_stride(stride);
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
    fn plot(&self, plot_ui: &crate::PlotUi<'_>) {
        if self.validate().is_err() {
            return;
        }
        let Ok(count) = i32::try_from(self.positions.len()) else {
            return;
        };
        let _guard = plot_ui.bind();
        with_plot_str_or_empty(self.label, |label_ptr| unsafe {
            let spec = plot_spec_with_style(
                self.style,
                self.flags.bits() | self.item_flags.bits(),
                self.layout,
            );
            sys::ImPlot_PlotInfLines_doublePtr(label_ptr, self.positions.as_ptr(), count, spec);
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
        plot.plot(self);
        Ok(())
    }

    /// Plot horizontal infinite lines at given y positions
    pub fn inf_lines_horizontal(&self, label: &str, ys: &[f64]) -> Result<(), PlotError> {
        let plot = InfLinesPlot::new(label, ys).horizontal();
        plot.validate()?;
        plot.plot(self);
        Ok(())
    }
}
