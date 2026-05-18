//! Basic plots
//!
//! Simple line/histogram plot helpers built on top of Dear ImGui. For more
//! advanced charts, consider using the optional implot bindings.
//!
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::as_conversions
)]
use crate::internal::plot_value_count_i32;
use crate::sys;
use crate::ui::Ui;
use std::borrow::Cow;

/// # Plot Widgets
impl Ui {
    /// Creates a plot lines widget
    #[doc(alias = "PlotLines")]
    pub fn plot_lines(&self, label: impl AsRef<str>, values: &[f32]) {
        self.plot_lines_config(label.as_ref(), values).build()
    }

    /// Creates a plot histogram widget
    #[doc(alias = "PlotHistogram")]
    pub fn plot_histogram(&self, label: impl AsRef<str>, values: &[f32]) {
        self.plot_histogram_config(label.as_ref(), values).build()
    }

    /// Creates a plot lines builder
    pub fn plot_lines_config<'ui, 'p>(
        &'ui self,
        label: impl Into<Cow<'ui, str>>,
        values: &'p [f32],
    ) -> PlotLines<'ui, 'p> {
        PlotLines::new(self, label, values)
    }

    /// Creates a plot histogram builder
    pub fn plot_histogram_config<'ui, 'p>(
        &'ui self,
        label: impl Into<Cow<'ui, str>>,
        values: &'p [f32],
    ) -> PlotHistogram<'ui, 'p> {
        PlotHistogram::new(self, label, values)
    }
}

/// Builder for a plot lines widget
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PlotValueOffset(usize);

impl PlotValueOffset {
    /// First value in the slice.
    pub const ZERO: Self = Self(0);

    /// Create a value offset from a Rust slice index.
    #[inline]
    pub const fn new(offset: usize) -> Self {
        Self(offset)
    }

    #[inline]
    pub(crate) fn into_i32(self, caller: &str, value_count: i32) -> i32 {
        let offset = i32::try_from(self.0).unwrap_or_else(|_| {
            panic!("{caller} values_offset supports at most i32::MAX");
        });
        assert!(
            value_count == 0 || offset < value_count,
            "{caller} values_offset must be less than values.len()"
        );
        offset
    }
}

impl From<usize> for PlotValueOffset {
    fn from(offset: usize) -> Self {
        Self::new(offset)
    }
}

/// Builder for a plot lines widget
#[derive(Debug)]
#[must_use]
pub struct PlotLines<'ui, 'p> {
    ui: &'ui Ui,
    label: Cow<'ui, str>,
    values: &'p [f32],
    values_offset: PlotValueOffset,
    overlay_text: Option<Cow<'ui, str>>,
    scale_min: f32,
    scale_max: f32,
    graph_size: [f32; 2],
}

impl<'ui, 'p> PlotLines<'ui, 'p> {
    /// Creates a new plot lines builder
    pub fn new(ui: &'ui Ui, label: impl Into<Cow<'ui, str>>, values: &'p [f32]) -> Self {
        Self {
            ui,
            label: label.into(),
            values,
            values_offset: PlotValueOffset::ZERO,
            overlay_text: None,
            scale_min: f32::MAX,
            scale_max: f32::MAX,
            graph_size: [0.0, 0.0],
        }
    }

    /// Sets the offset for the values array
    pub fn values_offset(mut self, offset: impl Into<PlotValueOffset>) -> Self {
        self.values_offset = offset.into();
        self
    }

    /// Sets the overlay text
    pub fn overlay_text(mut self, text: impl Into<Cow<'ui, str>>) -> Self {
        self.overlay_text = Some(text.into());
        self
    }

    /// Sets the scale minimum value
    pub fn scale_min(mut self, scale_min: f32) -> Self {
        self.scale_min = scale_min;
        self
    }

    /// Sets the scale maximum value
    pub fn scale_max(mut self, scale_max: f32) -> Self {
        self.scale_max = scale_max;
        self
    }

    /// Sets the graph size
    pub fn graph_size(mut self, size: [f32; 2]) -> Self {
        self.graph_size = size;
        self
    }

    /// Builds the plot lines widget
    pub fn build(self) {
        let count = plot_value_count_i32("PlotLines::build()", self.values.len());
        let values_offset = self.values_offset.into_i32("PlotLines::build()", count);
        let (label_ptr, overlay_ptr) = self
            .ui
            .scratch_txt_with_opt(self.label.as_ref(), self.overlay_text.as_deref());
        let graph_size_vec: sys::ImVec2 = self.graph_size.into();

        unsafe {
            sys::igPlotLines_FloatPtr(
                label_ptr,
                self.values.as_ptr(),
                count,
                values_offset,
                overlay_ptr,
                self.scale_min,
                self.scale_max,
                graph_size_vec,
                std::mem::size_of::<f32>() as i32,
            );
        }
    }
}

/// Builder for a plot histogram widget
#[derive(Debug)]
#[must_use]
pub struct PlotHistogram<'ui, 'p> {
    ui: &'ui Ui,
    label: Cow<'ui, str>,
    values: &'p [f32],
    values_offset: PlotValueOffset,
    overlay_text: Option<Cow<'ui, str>>,
    scale_min: f32,
    scale_max: f32,
    graph_size: [f32; 2],
}

impl<'ui, 'p> PlotHistogram<'ui, 'p> {
    /// Creates a new plot histogram builder
    pub fn new(ui: &'ui Ui, label: impl Into<Cow<'ui, str>>, values: &'p [f32]) -> Self {
        Self {
            ui,
            label: label.into(),
            values,
            values_offset: PlotValueOffset::ZERO,
            overlay_text: None,
            scale_min: f32::MAX,
            scale_max: f32::MAX,
            graph_size: [0.0, 0.0],
        }
    }

    /// Sets the offset for the values array
    pub fn values_offset(mut self, offset: impl Into<PlotValueOffset>) -> Self {
        self.values_offset = offset.into();
        self
    }

    /// Sets the overlay text
    pub fn overlay_text(mut self, text: impl Into<Cow<'ui, str>>) -> Self {
        self.overlay_text = Some(text.into());
        self
    }

    /// Sets the scale minimum value
    pub fn scale_min(mut self, scale_min: f32) -> Self {
        self.scale_min = scale_min;
        self
    }

    /// Sets the scale maximum value
    pub fn scale_max(mut self, scale_max: f32) -> Self {
        self.scale_max = scale_max;
        self
    }

    /// Sets the graph size
    pub fn graph_size(mut self, size: [f32; 2]) -> Self {
        self.graph_size = size;
        self
    }

    /// Builds the plot histogram widget
    pub fn build(self) {
        let count = plot_value_count_i32("PlotHistogram::build()", self.values.len());
        let values_offset = self.values_offset.into_i32("PlotHistogram::build()", count);
        let (label_ptr, overlay_ptr) = self
            .ui
            .scratch_txt_with_opt(self.label.as_ref(), self.overlay_text.as_deref());
        let graph_size_vec: sys::ImVec2 = self.graph_size.into();

        unsafe {
            sys::igPlotHistogram_FloatPtr(
                label_ptr,
                self.values.as_ptr(),
                count,
                values_offset,
                overlay_ptr,
                self.scale_min,
                self.scale_max,
                graph_size_vec,
                std::mem::size_of::<f32>() as i32,
            );
        }
    }
}
