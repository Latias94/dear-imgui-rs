//! Bar groups plot implementation

use super::{
    PlotData, PlotDataLayout, PlotError, PlotItemStyle, plot_spec_with_style, with_plot_str_slice,
};
use crate::{BarGroupsFlags, ItemFlags, sys};

fn bar_group_flags(flags: BarGroupsFlags, item_flags: ItemFlags) -> BarGroupsFlags {
    BarGroupsFlags::from_bits_retain(flags.bits() | item_flags.bits())
}

fn bar_group_value_count(
    item_count: usize,
    group_count: usize,
    flags: BarGroupsFlags,
) -> Result<usize, PlotError> {
    if item_count == 0 || group_count == 0 {
        return Err(PlotError::EmptyData);
    }
    // PlotBarGroups indexes the data and sizes its four stacked scratch rows using int.
    if flags.contains(BarGroupsFlags::STACKED) && group_count > i32::MAX as usize / 4 {
        return Err(PlotError::InvalidData(
            "stacked bar group scratch size exceeded ImPlot's i32 range".into(),
        ));
    }
    item_count
        .checked_mul(group_count)
        .filter(|&count| count <= i32::MAX as usize)
        .ok_or_else(|| {
            PlotError::InvalidData("bar group value count exceeded ImPlot's i32 range".into())
        })
}

fn validate_bar_groups(
    label_count: usize,
    values_len: usize,
    item_count: usize,
    group_count: usize,
    flags: BarGroupsFlags,
) -> Result<(), PlotError> {
    if label_count != item_count {
        return Err(PlotError::InvalidData(format!(
            "Label count ({label_count}) must match item count ({item_count})"
        )));
    }
    let expected = bar_group_value_count(item_count, group_count, flags)?;
    if values_len != expected {
        return Err(PlotError::InvalidData(format!(
            "Values length ({values_len}) must equal item_count * group_count ({expected})"
        )));
    }
    Ok(())
}

/// Builder for bar groups plots with extensive customization options
pub struct BarGroupsPlot<'a> {
    label_ids: Vec<&'a str>,
    values: &'a [f64],
    style: PlotItemStyle,
    item_count: usize,
    group_count: usize,
    group_size: f64,
    shift: f64,
    flags: BarGroupsFlags,
    item_flags: ItemFlags,
}

impl<'a> super::PlotItemStyled for BarGroupsPlot<'a> {
    fn style_mut(&mut self) -> &mut PlotItemStyle {
        &mut self.style
    }
}

impl<'a> BarGroupsPlot<'a> {
    /// Create a new bar groups plot
    ///
    /// # Arguments
    /// * `label_ids` - Labels for each item in the group
    /// * `values` - Values in row-major order (item_count rows, group_count cols)
    /// * `item_count` - Number of items (series) in each group
    /// * `group_count` - Number of groups
    pub fn new(
        label_ids: Vec<&'a str>,
        values: &'a [f64],
        item_count: usize,
        group_count: usize,
    ) -> Self {
        Self {
            label_ids,
            values,
            style: PlotItemStyle::default(),
            item_count,
            group_count,
            group_size: 0.67,
            shift: 0.0,
            flags: BarGroupsFlags::NONE,
            item_flags: ItemFlags::NONE,
        }
    }

    /// Set the group size (width of each group)
    pub fn with_group_size(mut self, group_size: f64) -> Self {
        self.group_size = group_size;
        self
    }

    /// Set ImPlotSpec-backed style overrides for this bar groups plot.
    pub fn with_style(mut self, style: PlotItemStyle) -> Self {
        self.style = style;
        self
    }

    /// Set the shift (horizontal offset)
    pub fn with_shift(mut self, shift: f64) -> Self {
        self.shift = shift;
        self
    }

    /// Set bar groups flags for customization
    pub fn with_flags(mut self, flags: BarGroupsFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Set common item flags for this plot item (applies to all plot types)
    pub fn with_item_flags(mut self, flags: ItemFlags) -> Self {
        self.item_flags = flags;
        self
    }

    /// Make bars horizontal instead of vertical
    pub fn horizontal(mut self) -> Self {
        self.flags |= BarGroupsFlags::HORIZONTAL;
        self
    }

    /// Stack bars instead of grouping them side by side
    pub fn stacked(mut self) -> Self {
        self.flags |= BarGroupsFlags::STACKED;
        self
    }

    /// Validate the plot data
    pub fn validate(&self) -> Result<(), PlotError> {
        validate_bar_groups(
            self.label_ids.len(),
            self.values.len(),
            self.item_count,
            self.group_count,
            bar_group_flags(self.flags, self.item_flags),
        )
    }

    /// Plot the bar groups
    pub fn plot(self, plot_ui: &crate::PlotUi<'_>) {
        if self.validate().is_err() {
            return;
        }
        let Ok(item_count) = i32::try_from(self.item_count) else {
            return;
        };
        let Ok(group_count) = i32::try_from(self.group_count) else {
            return;
        };
        plot_ui.with_bound_context(|| {
            with_plot_str_slice(&self.label_ids, |label_ptrs| unsafe {
                let spec = plot_spec_with_style(
                    self.style,
                    bar_group_flags(self.flags, self.item_flags).bits(),
                    PlotDataLayout::DEFAULT,
                );
                sys::ImPlot_PlotBarGroups_doublePtr(
                    label_ptrs.as_ptr(),
                    self.values.as_ptr(),
                    item_count,
                    group_count,
                    self.group_size,
                    self.shift,
                    spec,
                );
            })
        })
    }
}

impl<'a> PlotData for BarGroupsPlot<'a> {
    fn label(&self) -> &str {
        "BarGroups" // Generic label for groups
    }

    fn data_len(&self) -> usize {
        self.values.len()
    }
}

/// Bar groups plot for f32 data
pub struct BarGroupsPlotF32<'a> {
    label_ids: Vec<&'a str>,
    values: &'a [f32],
    style: PlotItemStyle,
    item_count: usize,
    group_count: usize,
    group_size: f64,
    shift: f64,
    flags: BarGroupsFlags,
    item_flags: ItemFlags,
}

impl<'a> super::PlotItemStyled for BarGroupsPlotF32<'a> {
    fn style_mut(&mut self) -> &mut PlotItemStyle {
        &mut self.style
    }
}

impl<'a> BarGroupsPlotF32<'a> {
    /// Create a new bar groups plot with f32 data
    pub fn new(
        label_ids: Vec<&'a str>,
        values: &'a [f32],
        item_count: usize,
        group_count: usize,
    ) -> Self {
        Self {
            label_ids,
            values,
            style: PlotItemStyle::default(),
            item_count,
            group_count,
            group_size: 0.67,
            shift: 0.0,
            flags: BarGroupsFlags::NONE,
            item_flags: ItemFlags::NONE,
        }
    }

    /// Set the group size
    pub fn with_group_size(mut self, group_size: f64) -> Self {
        self.group_size = group_size;
        self
    }

    /// Set ImPlotSpec-backed style overrides for this bar groups plot.
    pub fn with_style(mut self, style: PlotItemStyle) -> Self {
        self.style = style;
        self
    }

    /// Set the shift
    pub fn with_shift(mut self, shift: f64) -> Self {
        self.shift = shift;
        self
    }

    /// Set flags
    pub fn with_flags(mut self, flags: BarGroupsFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Set common item flags for this plot item (applies to all plot types)
    pub fn with_item_flags(mut self, flags: ItemFlags) -> Self {
        self.item_flags = flags;
        self
    }

    /// Make bars horizontal
    pub fn horizontal(mut self) -> Self {
        self.flags |= BarGroupsFlags::HORIZONTAL;
        self
    }

    /// Stack bars
    pub fn stacked(mut self) -> Self {
        self.flags |= BarGroupsFlags::STACKED;
        self
    }

    /// Validate the plot data
    pub fn validate(&self) -> Result<(), PlotError> {
        validate_bar_groups(
            self.label_ids.len(),
            self.values.len(),
            self.item_count,
            self.group_count,
            bar_group_flags(self.flags, self.item_flags),
        )
    }

    /// Plot the bar groups
    pub fn plot(self, plot_ui: &crate::PlotUi<'_>) {
        if self.validate().is_err() {
            return;
        }
        let Ok(item_count) = i32::try_from(self.item_count) else {
            return;
        };
        let Ok(group_count) = i32::try_from(self.group_count) else {
            return;
        };
        plot_ui.with_bound_context(|| {
            with_plot_str_slice(&self.label_ids, |label_ptrs| unsafe {
                let spec = plot_spec_with_style(
                    self.style,
                    bar_group_flags(self.flags, self.item_flags).bits(),
                    PlotDataLayout::DEFAULT,
                );
                sys::ImPlot_PlotBarGroups_FloatPtr(
                    label_ptrs.as_ptr(),
                    self.values.as_ptr(),
                    item_count,
                    group_count,
                    self.group_size,
                    self.shift,
                    spec,
                );
            })
        })
    }
}

impl<'a> PlotData for BarGroupsPlotF32<'a> {
    fn label(&self) -> &str {
        "BarGroups" // Generic label for groups
    }

    fn data_len(&self) -> usize {
        self.values.len()
    }
}

/// Simple bar groups plot with automatic layout
pub struct SimpleBarGroupsPlot<'a> {
    labels: Vec<&'a str>,
    data: Vec<Vec<f64>>,
    style: PlotItemStyle,
    group_size: f64,
    flags: BarGroupsFlags,
    item_flags: ItemFlags,
}

impl<'a> super::PlotItemStyled for SimpleBarGroupsPlot<'a> {
    fn style_mut(&mut self) -> &mut PlotItemStyle {
        &mut self.style
    }
}

impl<'a> SimpleBarGroupsPlot<'a> {
    /// Create a simple bar groups plot from a 2D data structure
    ///
    /// # Arguments
    /// * `labels` - Labels for each series
    /// * `data` - One row per series. The first row determines the number of groups.
    ///   Shorter rows are padded with zeros and longer rows are truncated.
    pub fn new(labels: Vec<&'a str>, data: Vec<Vec<f64>>) -> Self {
        Self {
            labels,
            data,
            style: PlotItemStyle::default(),
            group_size: 0.67,
            flags: BarGroupsFlags::NONE,
            item_flags: ItemFlags::NONE,
        }
    }

    /// Set ImPlotSpec-backed style overrides for this bar groups plot.
    pub fn with_style(mut self, style: PlotItemStyle) -> Self {
        self.style = style;
        self
    }

    /// Set the group size
    pub fn with_group_size(mut self, group_size: f64) -> Self {
        self.group_size = group_size;
        self
    }

    /// Set flags
    pub fn with_flags(mut self, flags: BarGroupsFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Set common item flags for this plot item (applies to all plot types)
    pub fn with_item_flags(mut self, flags: ItemFlags) -> Self {
        self.item_flags = flags;
        self
    }

    /// Make bars horizontal
    pub fn horizontal(mut self) -> Self {
        self.flags |= BarGroupsFlags::HORIZONTAL;
        self
    }

    /// Stack bars
    pub fn stacked(mut self) -> Self {
        self.flags |= BarGroupsFlags::STACKED;
        self
    }

    /// Plot the bar groups
    pub fn plot(self, plot_ui: &crate::PlotUi<'_>) {
        if self.labels.len() != self.data.len() {
            return;
        }
        let item_count = self.data.len();
        let group_count = self.data.first().map_or(0, Vec::len);
        let Ok(flattened_data) = self.flattened_values(group_count) else {
            return;
        };

        let plot = BarGroupsPlot::new(self.labels, &flattened_data, item_count, group_count)
            .with_style(self.style)
            .with_group_size(self.group_size)
            .with_flags(self.flags)
            .with_item_flags(self.item_flags);

        plot.plot(plot_ui);
    }

    fn flattened_values(&self, group_count: usize) -> Result<Vec<f64>, PlotError> {
        let count = bar_group_value_count(
            self.data.len(),
            group_count,
            bar_group_flags(self.flags, self.item_flags),
        )?;
        let mut values = vec![0.0; count];
        for (row, series) in values.chunks_exact_mut(group_count).zip(&self.data) {
            let len = series.len().min(group_count);
            row[..len].copy_from_slice(&series[..len]);
        }
        Ok(values)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_bar_groups_keep_each_series_in_one_row() {
        let plot = SimpleBarGroupsPlot::new(
            vec!["A", "B", "C"],
            vec![vec![1.0, 2.0], vec![3.0], vec![4.0, 5.0, 6.0]],
        );
        assert_eq!(
            plot.flattened_values(2).unwrap(),
            [1.0, 2.0, 3.0, 0.0, 4.0, 5.0]
        );
    }

    #[test]
    fn bar_group_counts_fit_native_indices_and_stacked_scratch_rows() {
        let max = i32::MAX as usize;
        assert!(bar_group_value_count(2, max / 2 + 1, BarGroupsFlags::NONE).is_err());
        assert!(bar_group_value_count(1, max / 4 + 1, BarGroupsFlags::STACKED).is_err());
        assert_eq!(bar_group_value_count(1, max, BarGroupsFlags::NONE), Ok(max));
        assert_eq!(
            bar_group_value_count(1, 0, BarGroupsFlags::NONE),
            Err(PlotError::EmptyData)
        );
    }
}
