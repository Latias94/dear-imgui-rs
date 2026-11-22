//! Histogram plot implementation

use super::{
    Plot, PlotDataLayout, PlotError, PlotItemStyle, plot_spec_with_style, with_plot_str_or_empty,
};
use crate::sys;
use crate::{BinMethod, HistogramBins, HistogramFlags, ItemFlags};

/// Builder for 1D histogram plots
pub struct HistogramPlot<'a> {
    label: &'a str,
    values: &'a [f64],
    style: PlotItemStyle,
    bins: HistogramBins,
    bar_scale: f64,
    range: Option<sys::ImPlotRange>,
    flags: HistogramFlags,
    item_flags: ItemFlags,
}

impl<'a> super::PlotItemStyled for HistogramPlot<'a> {
    fn style_mut(&mut self) -> &mut PlotItemStyle {
        &mut self.style
    }
}

impl<'a> HistogramPlot<'a> {
    /// Create a new histogram plot with the given label and data
    pub fn new(label: &'a str, values: &'a [f64]) -> Self {
        Self {
            label,
            values,
            style: PlotItemStyle::default(),
            bins: HistogramBins::DEFAULT,
            bar_scale: 1.0,
            range: None, // Auto-range
            flags: HistogramFlags::NONE,
            item_flags: ItemFlags::NONE,
        }
    }

    /// Set a concrete positive bin count or automatic binning method.
    pub fn with_bins(mut self, bins: impl Into<HistogramBins>) -> Self {
        self.bins = bins.into();
        self
    }

    /// Set the bar scale factor
    pub fn with_bar_scale(mut self, scale: f64) -> Self {
        self.bar_scale = scale;
        self
    }

    /// Set the data range for binning
    /// Values outside this range will be treated as outliers
    pub fn with_range(mut self, min: f64, max: f64) -> Self {
        self.range = Some(sys::ImPlotRange { Min: min, Max: max });
        self
    }

    /// Set the data range using ImPlotRange
    pub fn with_range_struct(mut self, range: sys::ImPlotRange) -> Self {
        self.range = Some(range);
        self
    }

    /// Set histogram flags for customization
    pub fn with_flags(mut self, flags: HistogramFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Set common item flags for this plot item (applies to all plot types)
    pub fn with_item_flags(mut self, flags: ItemFlags) -> Self {
        self.item_flags = flags;
        self
    }

    /// Make the histogram horizontal instead of vertical
    pub fn horizontal(mut self) -> Self {
        self.flags |= HistogramFlags::HORIZONTAL;
        self
    }

    /// Make the histogram cumulative
    pub fn cumulative(mut self) -> Self {
        self.flags |= HistogramFlags::CUMULATIVE;
        self
    }

    /// Normalize the histogram to show density (PDF)
    pub fn density(mut self) -> Self {
        self.flags |= HistogramFlags::DENSITY;
        self
    }

    /// Exclude outliers from normalization
    pub fn no_outliers(mut self) -> Self {
        self.flags |= HistogramFlags::NO_OUTLIERS;
        self
    }

    /// Validate the plot data
    pub fn validate(&self) -> Result<(), PlotError> {
        if self.values.is_empty() {
            return Err(PlotError::EmptyData);
        }
        Ok(())
    }
}

impl<'a> Plot for HistogramPlot<'a> {
    fn plot(&self, plot_ui: &crate::PlotUi<'_>) {
        if self.validate().is_err() {
            return;
        }
        let Ok(count) = i32::try_from(self.values.len()) else {
            return;
        };

        let range = if let Some(range) = &self.range {
            *range
        } else {
            sys::ImPlotRange { Min: 0.0, Max: 0.0 }
        };

        plot_ui.with_bound_context(|| {
            with_plot_str_or_empty(self.label, |label_ptr| unsafe {
                let spec = plot_spec_with_style(
                    self.style,
                    self.flags.bits() | self.item_flags.bits(),
                    PlotDataLayout::DEFAULT,
                );
                sys::ImPlot_PlotHistogram_doublePtr(
                    label_ptr,
                    self.values.as_ptr(),
                    count,
                    self.bins.raw("HistogramPlot::plot()"),
                    self.bar_scale,
                    range,
                    spec,
                );
            })
        })
    }

    fn label(&self) -> &str {
        self.label
    }
}

/// Builder for 2D histogram plots (bivariate histograms as heatmaps)
pub struct Histogram2DPlot<'a> {
    label: &'a str,
    x_values: &'a [f64],
    y_values: &'a [f64],
    style: PlotItemStyle,
    x_bins: HistogramBins,
    y_bins: HistogramBins,
    range: Option<sys::ImPlotRect>,
    flags: HistogramFlags,
    item_flags: ItemFlags,
}

impl<'a> super::PlotItemStyled for Histogram2DPlot<'a> {
    fn style_mut(&mut self) -> &mut PlotItemStyle {
        &mut self.style
    }
}

impl<'a> Histogram2DPlot<'a> {
    /// Create a new 2D histogram plot with the given label and data
    pub fn new(label: &'a str, x_values: &'a [f64], y_values: &'a [f64]) -> Self {
        Self {
            label,
            x_values,
            y_values,
            style: PlotItemStyle::default(),
            x_bins: HistogramBins::DEFAULT,
            y_bins: HistogramBins::DEFAULT,
            range: None, // Auto-range
            flags: HistogramFlags::NONE,
            item_flags: ItemFlags::NONE,
        }
    }

    /// Set the number of bins for both X and Y axes
    pub fn with_bins(
        mut self,
        x_bins: impl Into<HistogramBins>,
        y_bins: impl Into<HistogramBins>,
    ) -> Self {
        self.x_bins = x_bins.into();
        self.y_bins = y_bins.into();
        self
    }

    /// Set the data range for binning. A `(0.0, 0.0)` pair auto-ranges that axis.
    pub fn with_range(mut self, x_min: f64, x_max: f64, y_min: f64, y_max: f64) -> Self {
        self.range = Some(sys::ImPlotRect {
            X: sys::ImPlotRange {
                Min: x_min,
                Max: x_max,
            },
            Y: sys::ImPlotRange {
                Min: y_min,
                Max: y_max,
            },
        });
        self
    }

    /// Set the data range using ImPlotRect
    pub fn with_range_struct(mut self, range: sys::ImPlotRect) -> Self {
        self.range = Some(range);
        self
    }

    /// Set histogram flags for customization
    pub fn with_flags(mut self, flags: HistogramFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Set common item flags for this plot item (applies to all plot types)
    pub fn with_item_flags(mut self, flags: ItemFlags) -> Self {
        self.item_flags = flags;
        self
    }

    /// Normalize the histogram to show density
    pub fn density(mut self) -> Self {
        self.flags |= HistogramFlags::DENSITY;
        self
    }

    /// Exclude outliers from normalization
    pub fn no_outliers(mut self) -> Self {
        self.flags |= HistogramFlags::NO_OUTLIERS;
        self
    }

    /// Use column-major data ordering
    pub fn column_major(mut self) -> Self {
        self.flags |= HistogramFlags::COL_MAJOR;
        self
    }

    /// Check that the samples, ranges, and resulting 2D bin grid can be passed to ImPlot.
    pub fn validate(&self) -> Result<(), PlotError> {
        self.bin_grid().map(|_| ())
    }

    fn bin_grid(&self) -> Result<(sys::ImPlotRect, i32, i32), PlotError> {
        super::validate_data_lengths(self.x_values, self.y_values)?;
        if self.x_values.len() > i32::MAX as usize {
            return Err(PlotError::InvalidData(
                "histogram sample count exceeded ImPlot's i32 range".into(),
            ));
        }
        let (x_range, x_bins) =
            histogram_axis(self.x_values, self.range.map(|r| r.X), self.x_bins)?;
        let (y_range, y_bins) =
            histogram_axis(self.y_values, self.range.map(|r| r.Y), self.y_bins)?;
        x_bins.checked_mul(y_bins).ok_or_else(|| {
            PlotError::InvalidData("2D histogram bin count exceeded ImPlot's i32 range".into())
        })?;
        Ok((
            sys::ImPlotRect {
                X: x_range,
                Y: y_range,
            },
            x_bins,
            y_bins,
        ))
    }

    fn plot_checked(&self, plot_ui: &crate::PlotUi<'_>) -> Result<(), PlotError> {
        let (range, x_bins, y_bins) = self.bin_grid()?;
        plot_ui.with_bound_context(|| {
            with_plot_str_or_empty(self.label, |label_ptr| unsafe {
                let spec = plot_spec_with_style(
                    self.style,
                    self.flags.bits() | self.item_flags.bits(),
                    PlotDataLayout::DEFAULT,
                );
                sys::ImPlot_PlotHistogram2D_doublePtr(
                    label_ptr,
                    self.x_values.as_ptr(),
                    self.y_values.as_ptr(),
                    self.x_values.len() as i32,
                    x_bins,
                    y_bins,
                    range,
                    spec,
                );
            })
        });
        Ok(())
    }
}

// Resolve both axes before FFI. Native PlotHistogram2D computes its flat bin count in int
// and the bundled version writes both automatic X bounds to range.X.Min.
fn histogram_axis(
    values: &[f64],
    range: Option<sys::ImPlotRange>,
    bins: HistogramBins,
) -> Result<(sys::ImPlotRange, i32), PlotError> {
    let range = range
        .filter(|r| r.Min != 0.0 || r.Max != 0.0)
        .unwrap_or_else(|| automatic_range(values));
    let span = range.Max - range.Min;
    if !span.is_finite() || span <= 0.0 {
        return Err(PlotError::InvalidData(
            "histogram range must have a positive finite width".into(),
        ));
    }
    let n = values.len() as f64;
    let count = match bins {
        HistogramBins::Count(count) => count as f64,
        HistogramBins::Method(BinMethod::Sqrt) => n.sqrt().ceil(),
        HistogramBins::Method(BinMethod::Sturges) => (1.0 + n.log2()).ceil(),
        HistogramBins::Method(BinMethod::Rice) => (2.0 * n.cbrt()).ceil(),
        HistogramBins::Method(BinMethod::Scott) => scott_bins(values, span)?,
    };
    // ImPlot casts the upper endpoint's quotient to int before clamping it to the last bin.
    let width = span / count;
    if !(1.0..=f64::from(i32::MAX)).contains(&count) || span / width >= f64::from(i32::MAX) + 1.0 {
        return Err(PlotError::InvalidData(
            "histogram bin count or width is outside ImPlot's range".into(),
        ));
    }
    Ok((range, count as i32))
}

fn automatic_range(values: &[f64]) -> sys::ImPlotRange {
    let (mut min, mut max) = (f64::INFINITY, f64::NEG_INFINITY);
    for &value in values.iter().filter(|v| v.is_finite()) {
        min = min.min(value);
        max = max.max(value);
    }
    if min == max {
        // A constant axis still needs a nonzero width, even at zero or f64::MAX.
        let pad = (min.abs() * f64::EPSILON).max(0.5);
        min = (min - pad).max(-f64::MAX);
        max = (max + pad).min(f64::MAX);
    }
    sys::ImPlotRange { Min: min, Max: max }
}

fn scott_bins(values: &[f64], span: f64) -> Result<f64, PlotError> {
    if values[0].is_finite() && values.iter().all(|&v| v == values[0]) {
        return Ok(1.0);
    }
    // Match the operation order of ImPlot's ImMean and ImStdDev. At large offsets a
    // different mean or variance algorithm can move Scott's result across a rounding boundary.
    let n = values.len() as f64;
    let weight = n.recip();
    let mean = values.iter().map(|v| v * weight).sum::<f64>();
    let weight = (n - 1.0).recip();
    let variance = values
        .iter()
        .map(|v| {
            let delta = v - mean;
            delta * delta * weight
        })
        .sum::<f64>();
    if !variance.is_finite() {
        return Err(PlotError::InvalidData(
            "Scott histogram binning requires finite sample variance".into(),
        ));
    }
    if variance == 0.0 {
        return Ok(1.0);
    }
    let width = 3.49 * variance.sqrt() / n.cbrt();
    Ok((span / width).round().max(1.0))
}

impl<'a> Plot for Histogram2DPlot<'a> {
    fn plot(&self, plot_ui: &crate::PlotUi<'_>) {
        let _ = self.plot_checked(plot_ui);
    }

    fn label(&self) -> &str {
        self.label
    }
}

/// Convenience functions for quick histogram plotting
impl<'ui> crate::PlotUi<'ui> {
    /// Plot a 1D histogram with default settings
    pub fn histogram_plot(&self, label: &str, values: &[f64]) -> Result<(), PlotError> {
        let plot = HistogramPlot::new(label, values);
        plot.validate()?;
        plot.plot(self);
        Ok(())
    }

    /// Plot a 1D histogram with custom bin count
    pub fn histogram_plot_with_bins(
        &self,
        label: &str,
        values: &[f64],
        bins: impl Into<HistogramBins>,
    ) -> Result<(), PlotError> {
        let plot = HistogramPlot::new(label, values).with_bins(bins);
        plot.validate()?;
        plot.plot(self);
        Ok(())
    }

    /// Plot a 2D histogram (bivariate histogram as heatmap)
    pub fn histogram_2d_plot(
        &self,
        label: &str,
        x_values: &[f64],
        y_values: &[f64],
    ) -> Result<(), PlotError> {
        Histogram2DPlot::new(label, x_values, y_values).plot_checked(self)
    }

    /// Plot a 2D histogram with custom bin counts
    pub fn histogram_2d_plot_with_bins(
        &self,
        label: &str,
        x_values: &[f64],
        y_values: &[f64],
        x_bins: impl Into<HistogramBins>,
        y_bins: impl Into<HistogramBins>,
    ) -> Result<(), PlotError> {
        Histogram2DPlot::new(label, x_values, y_values)
            .with_bins(x_bins, y_bins)
            .plot_checked(self)
    }
}

#[cfg(test)]
mod tests {
    use super::{Histogram2DPlot, HistogramPlot};
    use crate::{BinMethod, HistogramBins};

    #[test]
    fn histogram_bins_distinguish_counts_from_methods() {
        assert_eq!(HistogramBins::from(8usize).raw("test"), 8);
        assert_eq!(
            HistogramBins::from(BinMethod::Rice).raw("test"),
            BinMethod::Rice as i32
        );
        assert_eq!(
            HistogramBins::DEFAULT.raw("test"),
            BinMethod::Sturges as i32
        );
    }

    #[test]
    #[should_panic(expected = "test bin count must be positive")]
    fn histogram_bins_reject_zero_counts_before_ffi() {
        let _ = HistogramBins::from(0usize).raw("test");
    }

    #[test]
    #[should_panic(expected = "test bin count exceeded ImPlot's i32 range")]
    fn histogram_bins_reject_oversized_counts_before_ffi() {
        let _ = HistogramBins::from(i32::MAX as usize + 1).raw("test");
    }

    #[test]
    fn histogram_builders_accept_typed_bins() {
        let values = [1.0, 2.0, 3.0];
        let _ = HistogramPlot::new("hist", &values).with_bins(8usize);
        let _ = HistogramPlot::new("hist", &values).with_bins(BinMethod::Scott);
        let _ = Histogram2DPlot::new("hist2d", &values, &values).with_bins(4usize, BinMethod::Rice);
    }

    #[test]
    fn histogram_2d_resolves_each_range_sentinel_independently() {
        let xs = [f64::NAN, 14.0, 10.0, f64::INFINITY];
        let ys = [3.0, -5.0, -1.0, 0.0];
        for (input, expected) in [
            ([0.0, 0.0, -8.0, 0.0], [10.0, 14.0, -8.0, 0.0]),
            ([0.0, 30.0, 0.0, 0.0], [0.0, 30.0, -5.0, 3.0]),
        ] {
            let [x_min, x_max, y_min, y_max] = input;
            let (range, _, _) = Histogram2DPlot::new("mixed", &xs, &ys)
                .with_range(x_min, x_max, y_min, y_max)
                .bin_grid()
                .unwrap();
            assert_eq!(
                [range.X.Min, range.X.Max, range.Y.Min, range.Y.Max],
                expected
            );
        }
    }

    #[test]
    fn histogram_2d_scott_gives_a_constant_axis_one_finite_bin() {
        for value in [0.0, -f64::MAX, f64::MAX] {
            let values = [value; 3];
            let (range, x_bins, y_bins) = Histogram2DPlot::new("constant", &values, &values)
                .with_bins(BinMethod::Scott, BinMethod::Scott)
                .bin_grid()
                .unwrap();
            assert_eq!((x_bins, y_bins), (1, 1));
            assert!(range.X.Min <= value && value <= range.X.Max);
            let width = range.X.Max - range.X.Min;
            assert!(width.is_finite() && width > 0.0);
        }
    }

    #[test]
    fn histogram_2d_checks_the_resolved_bin_product_before_allocation() {
        let values = [0.0, 1.0];
        let grid = |y| {
            Histogram2DPlot::new("capacity", &values, &values)
                .with_bins(BinMethod::Sqrt, y)
                .validate()
        };
        assert!(grid(2usize).is_ok());
        assert!(grid(0usize).is_err());
        assert!(grid(1usize << 30).is_err()); // Two X bins put the product one past i32::MAX.
    }

    #[test]
    fn histogram_2d_rejects_widths_that_round_the_last_bin_index_past_i32() {
        let values = [0.0, f64::from_bits(1 << 31)];
        let plot = Histogram2DPlot::new("subnormal", &values, &values)
            .with_bins(i32::MAX as usize, 1usize);
        assert!(plot.validate().is_err());
    }
}
