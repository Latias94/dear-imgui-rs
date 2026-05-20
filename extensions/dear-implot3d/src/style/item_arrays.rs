use crate::sys;
use std::borrow::Cow;

/// One-shot array-backed item style overrides for the next ImPlot3D submission.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Plot3DItemArrayStyle<'a> {
    line_colors: Option<Cow<'a, [u32]>>,
    fill_colors: Option<Cow<'a, [u32]>>,
    marker_sizes: Option<Cow<'a, [f32]>>,
    marker_line_colors: Option<Cow<'a, [u32]>>,
    marker_fill_colors: Option<Cow<'a, [u32]>>,
}

impl<'a> Plot3DItemArrayStyle<'a> {
    /// Create an empty array-style override.
    pub fn new() -> Self {
        Self::default()
    }

    /// Override per-index line colors using Dear ImGui packed colors (`ImU32` / ABGR).
    pub fn with_line_colors(mut self, colors: &'a [u32]) -> Self {
        self.line_colors = Some(Cow::Borrowed(colors));
        self
    }

    /// Override per-index fill colors using Dear ImGui packed colors (`ImU32` / ABGR).
    pub fn with_fill_colors(mut self, colors: &'a [u32]) -> Self {
        self.fill_colors = Some(Cow::Borrowed(colors));
        self
    }

    /// Override per-index marker sizes in pixels.
    pub fn with_marker_sizes(mut self, sizes: &'a [f32]) -> Self {
        self.marker_sizes = Some(Cow::Borrowed(sizes));
        self
    }

    /// Override per-index marker outline colors using Dear ImGui packed colors (`ImU32` / ABGR).
    pub fn with_marker_line_colors(mut self, colors: &'a [u32]) -> Self {
        self.marker_line_colors = Some(Cow::Borrowed(colors));
        self
    }

    /// Override per-index marker fill colors using Dear ImGui packed colors (`ImU32` / ABGR).
    pub fn with_marker_fill_colors(mut self, colors: &'a [u32]) -> Self {
        self.marker_fill_colors = Some(Cow::Borrowed(colors));
        self
    }

    fn apply_to_spec(&self, spec: &mut sys::ImPlot3DSpec_c) {
        spec.LineColors = self
            .line_colors
            .as_ref()
            .map_or(std::ptr::null_mut(), |colors| colors.as_ptr() as *mut _);
        spec.FillColors = self
            .fill_colors
            .as_ref()
            .map_or(std::ptr::null_mut(), |colors| colors.as_ptr() as *mut _);
        spec.MarkerSizes = self
            .marker_sizes
            .as_ref()
            .map_or(std::ptr::null_mut(), |sizes| sizes.as_ptr() as *mut _);
        spec.MarkerLineColors = self
            .marker_line_colors
            .as_ref()
            .map_or(std::ptr::null_mut(), |colors| colors.as_ptr() as *mut _);
        spec.MarkerFillColors = self
            .marker_fill_colors
            .as_ref()
            .map_or(std::ptr::null_mut(), |colors| colors.as_ptr() as *mut _);
    }
}

/// Apply array-backed item styling to the next ImPlot3D submission executed inside `f`.
pub fn with_next_plot3d_item_array_style<'a, R>(
    style: Plot3DItemArrayStyle<'a>,
    f: impl FnOnce() -> R,
) -> R {
    let previous = crate::take_next_plot3d_spec();
    let mut spec = previous.unwrap_or_else(crate::default_plot3d_spec);
    style.apply_to_spec(&mut spec);
    crate::set_next_plot3d_spec(Some(spec));

    let out = f();

    if crate::take_next_plot3d_spec().is_some() {
        crate::set_next_plot3d_spec(previous);
    }

    out
}
