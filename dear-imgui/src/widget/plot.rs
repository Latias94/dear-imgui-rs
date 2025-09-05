use crate::sys;
use crate::ui::Ui;

/// # Plot Widgets
impl Ui {
    /// Creates a plot lines widget
    #[doc(alias = "PlotLines")]
    pub fn plot_lines(&self, label: impl AsRef<str>, values: &[f32]) {
        self.plot_lines_config(label, values).build()
    }

    /// Creates a plot histogram widget
    #[doc(alias = "PlotHistogram")]
    pub fn plot_histogram(&self, label: impl AsRef<str>, values: &[f32]) {
        self.plot_histogram_config(label, values).build()
    }

    /// Creates a plot lines builder
    pub fn plot_lines_config<'p>(
        &self,
        label: impl AsRef<str>,
        values: &'p [f32],
    ) -> PlotLines<'_, 'p> {
        PlotLines::new(self, label, values)
    }

    /// Creates a plot histogram builder
    pub fn plot_histogram_config<'p>(
        &self,
        label: impl AsRef<str>,
        values: &'p [f32],
    ) -> PlotHistogram<'_, 'p> {
        PlotHistogram::new(self, label, values)
    }
}

/// Builder for a plot lines widget
#[derive(Debug)]
#[must_use]
pub struct PlotLines<'ui, 'p> {
    ui: &'ui Ui,
    label: String,
    values: &'p [f32],
    values_offset: i32,
    overlay_text: Option<String>,
    scale_min: f32,
    scale_max: f32,
    graph_size: [f32; 2],
}

impl<'ui, 'p> PlotLines<'ui, 'p> {
    /// Creates a new plot lines builder
    pub fn new(ui: &'ui Ui, label: impl AsRef<str>, values: &'p [f32]) -> Self {
        Self {
            ui,
            label: label.as_ref().to_string(),
            values,
            values_offset: 0,
            overlay_text: None,
            scale_min: f32::MAX,
            scale_max: f32::MAX,
            graph_size: [0.0, 0.0],
        }
    }

    /// Sets the offset for the values array
    pub fn values_offset(mut self, offset: i32) -> Self {
        self.values_offset = offset;
        self
    }

    /// Sets the overlay text
    pub fn overlay_text(mut self, text: impl Into<String>) -> Self {
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
        let label_ptr = self.ui.scratch_txt(&self.label);
        let overlay_ptr = self.ui.scratch_txt_opt(self.overlay_text.as_ref());
        let graph_size_vec: sys::ImVec2 = self.graph_size.into();

        unsafe {
            sys::ImGui_PlotLines(
                label_ptr,
                self.values.as_ptr(),
                self.values.len() as i32,
                self.values_offset,
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
    label: String,
    values: &'p [f32],
    values_offset: i32,
    overlay_text: Option<String>,
    scale_min: f32,
    scale_max: f32,
    graph_size: [f32; 2],
}

impl<'ui, 'p> PlotHistogram<'ui, 'p> {
    /// Creates a new plot histogram builder
    pub fn new(ui: &'ui Ui, label: impl AsRef<str>, values: &'p [f32]) -> Self {
        Self {
            ui,
            label: label.as_ref().to_string(),
            values,
            values_offset: 0,
            overlay_text: None,
            scale_min: f32::MAX,
            scale_max: f32::MAX,
            graph_size: [0.0, 0.0],
        }
    }

    /// Sets the offset for the values array
    pub fn values_offset(mut self, offset: i32) -> Self {
        self.values_offset = offset;
        self
    }

    /// Sets the overlay text
    pub fn overlay_text(mut self, text: impl Into<String>) -> Self {
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
        let label_ptr = self.ui.scratch_txt(&self.label);
        let overlay_ptr = self.ui.scratch_txt_opt(self.overlay_text.as_ref());
        let graph_size_vec: sys::ImVec2 = self.graph_size.into();

        unsafe {
            sys::ImGui_PlotHistogram(
                label_ptr,
                self.values.as_ptr(),
                self.values.len() as i32,
                self.values_offset,
                overlay_ptr,
                self.scale_min,
                self.scale_max,
                graph_size_vec,
                std::mem::size_of::<f32>() as i32,
            );
        }
    }
}
