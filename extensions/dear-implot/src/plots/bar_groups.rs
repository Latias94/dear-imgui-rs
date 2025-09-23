//! Bar groups plot implementation

use super::{safe_cstring, PlotData, PlotError};
use crate::sys;
use crate::BarGroupsFlags;
use std::ffi::CString;

/// Builder for bar groups plots with extensive customization options
pub struct BarGroupsPlot<'a> {
    label_ids: Vec<&'a str>,
    values: &'a [f64],
    item_count: usize,
    group_count: usize,
    group_size: f64,
    shift: f64,
    flags: BarGroupsFlags,
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
            item_count,
            group_count,
            group_size: 0.67,
            shift: 0.0,
            flags: BarGroupsFlags::NONE,
        }
    }

    /// Set the group size (width of each group)
    pub fn with_group_size(mut self, group_size: f64) -> Self {
        self.group_size = group_size;
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
        if self.label_ids.len() != self.item_count {
            return Err(PlotError::InvalidData(format!(
                "Label count ({}) must match item count ({})",
                self.label_ids.len(),
                self.item_count
            )));
        }

        let expected_values = self.item_count * self.group_count;
        if self.values.len() != expected_values {
            return Err(PlotError::InvalidData(format!(
                "Values length ({}) must equal item_count * group_count ({})",
                self.values.len(),
                expected_values
            )));
        }

        if self.item_count == 0 || self.group_count == 0 {
            return Err(PlotError::EmptyData);
        }

        Ok(())
    }

    /// Plot the bar groups
    pub fn plot(self) {
        // Convert labels to CString pointers
        let label_cstrings: Vec<CString> = self
            .label_ids
            .iter()
            .map(|&label| safe_cstring(label))
            .collect();

        let label_ptrs: Vec<*const i8> = label_cstrings.iter().map(|cstr| cstr.as_ptr()).collect();

        unsafe {
            sys::ImPlot_PlotBarGroups_doublePtr(
                label_ptrs.as_ptr(),
                self.values.as_ptr(),
                self.item_count as i32,
                self.group_count as i32,
                self.group_size,
                self.shift,
                self.flags.bits() as i32,
            );
        }
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
    item_count: usize,
    group_count: usize,
    group_size: f64,
    shift: f64,
    flags: BarGroupsFlags,
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
            item_count,
            group_count,
            group_size: 0.67,
            shift: 0.0,
            flags: BarGroupsFlags::NONE,
        }
    }

    /// Set the group size
    pub fn with_group_size(mut self, group_size: f64) -> Self {
        self.group_size = group_size;
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
        if self.label_ids.len() != self.item_count {
            return Err(PlotError::InvalidData(format!(
                "Label count ({}) must match item count ({})",
                self.label_ids.len(),
                self.item_count
            )));
        }

        let expected_values = self.item_count * self.group_count;
        if self.values.len() != expected_values {
            return Err(PlotError::InvalidData(format!(
                "Values length ({}) must equal item_count * group_count ({})",
                self.values.len(),
                expected_values
            )));
        }

        if self.item_count == 0 || self.group_count == 0 {
            return Err(PlotError::EmptyData);
        }

        Ok(())
    }

    /// Plot the bar groups
    pub fn plot(self) {
        // Convert labels to CString pointers
        let label_cstrings: Vec<CString> = self
            .label_ids
            .iter()
            .map(|&label| safe_cstring(label))
            .collect();

        let label_ptrs: Vec<*const i8> = label_cstrings.iter().map(|cstr| cstr.as_ptr()).collect();

        unsafe {
            sys::ImPlot_PlotBarGroups_FloatPtr(
                label_ptrs.as_ptr(),
                self.values.as_ptr(),
                self.item_count as i32,
                self.group_count as i32,
                self.group_size,
                self.shift,
                self.flags.bits() as i32,
            );
        }
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
    group_size: f64,
    flags: BarGroupsFlags,
}

impl<'a> SimpleBarGroupsPlot<'a> {
    /// Create a simple bar groups plot from a 2D data structure
    ///
    /// # Arguments
    /// * `labels` - Labels for each series
    /// * `data` - Vector of vectors, where each inner vector is a series
    pub fn new(labels: Vec<&'a str>, data: Vec<Vec<f64>>) -> Self {
        Self {
            labels,
            data,
            group_size: 0.67,
            flags: BarGroupsFlags::NONE,
        }
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
    pub fn plot(self) {
        if self.data.is_empty() || self.labels.is_empty() {
            return;
        }

        let item_count = self.data.len();
        let group_count = self.data[0].len();

        // Flatten data into row-major order
        let mut flattened_data = Vec::with_capacity(item_count * group_count);
        for group_idx in 0..group_count {
            for item_idx in 0..item_count {
                if group_idx < self.data[item_idx].len() {
                    flattened_data.push(self.data[item_idx][group_idx]);
                } else {
                    flattened_data.push(0.0); // Fill missing data with zeros
                }
            }
        }

        let plot = BarGroupsPlot::new(self.labels, &flattened_data, item_count, group_count)
            .with_group_size(self.group_size)
            .with_flags(self.flags);

        plot.plot();
    }
}
