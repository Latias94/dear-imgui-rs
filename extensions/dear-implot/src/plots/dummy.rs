//! Dummy plot implementation

use super::{PlotData, PlotError, plot_spec_from, with_plot_str_or_empty};
use crate::DummyFlags;
use crate::sys;

/// Builder for dummy plots
///
/// Dummy plots add a legend entry without plotting any actual data.
/// This is useful for creating custom legend entries or placeholders.
pub struct DummyPlot<'a> {
    label: &'a str,
    flags: DummyFlags,
}

impl<'a> DummyPlot<'a> {
    /// Create a new dummy plot with the given label
    pub fn new(label: &'a str) -> Self {
        Self {
            label,
            flags: DummyFlags::NONE,
        }
    }

    /// Set dummy flags for customization
    pub fn with_flags(mut self, flags: DummyFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Validate the plot data
    pub fn validate(&self) -> Result<(), PlotError> {
        if self.label.is_empty() {
            return Err(PlotError::InvalidData("Label cannot be empty".to_string()));
        }
        Ok(())
    }

    /// Plot the dummy entry
    pub fn plot(self) {
        with_plot_str_or_empty(self.label, |label_ptr| unsafe {
            let spec = plot_spec_from(self.flags.bits(), 0, crate::IMPLOT_AUTO);
            sys::ImPlot_PlotDummy(label_ptr, spec);
        })
    }
}

impl<'a> PlotData for DummyPlot<'a> {
    fn label(&self) -> &str {
        self.label
    }

    fn data_len(&self) -> usize {
        0 // Dummy plots have no actual data
    }
}

/// Multiple dummy plots for creating legend sections
pub struct MultiDummyPlot<'a> {
    labels: Vec<&'a str>,
    flags: DummyFlags,
}

impl<'a> MultiDummyPlot<'a> {
    /// Create multiple dummy plots
    pub fn new(labels: Vec<&'a str>) -> Self {
        Self {
            labels,
            flags: DummyFlags::NONE,
        }
    }

    /// Set dummy flags for all entries
    pub fn with_flags(mut self, flags: DummyFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Validate the plot data
    pub fn validate(&self) -> Result<(), PlotError> {
        if self.labels.is_empty() {
            return Err(PlotError::EmptyData);
        }

        for (i, &label) in self.labels.iter().enumerate() {
            if label.is_empty() {
                return Err(PlotError::InvalidData(format!(
                    "Label at index {} cannot be empty",
                    i
                )));
            }
        }

        Ok(())
    }

    /// Plot all dummy entries
    pub fn plot(self) {
        for &label in &self.labels {
            let dummy_plot = DummyPlot::new(label).with_flags(self.flags);
            dummy_plot.plot();
        }
    }
}

impl<'a> PlotData for MultiDummyPlot<'a> {
    fn label(&self) -> &str {
        "MultiDummy"
    }

    fn data_len(&self) -> usize {
        self.labels.len()
    }
}

/// Legend separator using dummy plots
pub struct LegendSeparator<'a> {
    label: &'a str,
}

impl<'a> LegendSeparator<'a> {
    /// Create a legend separator with optional label
    pub fn new(label: &'a str) -> Self {
        Self { label }
    }

    /// Create an empty separator
    pub fn empty() -> Self {
        Self { label: "---" }
    }

    /// Plot the separator
    pub fn plot(self) {
        let dummy_plot = DummyPlot::new(self.label);
        dummy_plot.plot();
    }
}

/// Legend header using dummy plots
pub struct LegendHeader<'a> {
    title: &'a str,
}

impl<'a> LegendHeader<'a> {
    /// Create a legend header
    pub fn new(title: &'a str) -> Self {
        Self { title }
    }

    /// Plot the header
    pub fn plot(self) {
        let dummy_plot = DummyPlot::new(self.title);
        dummy_plot.plot();
    }
}

/// Custom legend entry builder
pub struct CustomLegendEntry<'a> {
    label: &'a str,
    flags: DummyFlags,
}

impl<'a> CustomLegendEntry<'a> {
    /// Create a custom legend entry
    pub fn new(label: &'a str) -> Self {
        Self {
            label,
            flags: DummyFlags::NONE,
        }
    }

    /// Set flags for the entry
    pub fn with_flags(mut self, flags: DummyFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Plot the custom entry
    pub fn plot(self) {
        let dummy_plot = DummyPlot::new(self.label).with_flags(self.flags);
        dummy_plot.plot();
    }
}

/// Legend group for organizing related entries
pub struct LegendGroup<'a> {
    title: &'a str,
    entries: Vec<&'a str>,
    add_separator: bool,
}

impl<'a> LegendGroup<'a> {
    /// Create a new legend group
    pub fn new(title: &'a str, entries: Vec<&'a str>) -> Self {
        Self {
            title,
            entries,
            add_separator: true,
        }
    }

    /// Disable separator after the group
    pub fn no_separator(mut self) -> Self {
        self.add_separator = false;
        self
    }

    /// Plot the legend group
    pub fn plot(self) {
        // Plot the header
        LegendHeader::new(self.title).plot();

        // Plot all entries
        for &entry in &self.entries {
            DummyPlot::new(entry).plot();
        }

        // Add separator if requested
        if self.add_separator && !self.entries.is_empty() {
            LegendSeparator::empty().plot();
        }
    }
}

/// Convenience functions for common dummy plot patterns
impl<'a> DummyPlot<'a> {
    /// Create a separator dummy plot
    pub fn separator() -> DummyPlot<'static> {
        DummyPlot::new("---")
    }

    /// Create a spacer dummy plot
    pub fn spacer() -> DummyPlot<'static> {
        DummyPlot::new(" ")
    }

    /// Create a header dummy plot
    pub fn header(title: &'a str) -> DummyPlot<'a> {
        DummyPlot::new(title)
    }
}

/// Macro for creating multiple dummy plots easily
#[macro_export]
macro_rules! dummy_plots {
    ($($label:expr),* $(,)?) => {
        {
            let labels = vec![$($label),*];
            $crate::plots::dummy::MultiDummyPlot::new(labels)
        }
    };
}

/// Macro for creating a legend group
#[macro_export]
macro_rules! legend_group {
    ($title:expr, $($entry:expr),* $(,)?) => {
        {
            let entries = vec![$($entry),*];
            $crate::plots::dummy::LegendGroup::new($title, entries)
        }
    };
}
