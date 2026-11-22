use crate::{Plot3DScope, sys};
use std::borrow::Cow;

/// One-shot array-backed item style overrides for the next ImPlot3D submission.
/// An empty slice uses the native scalar style for that property.
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

    pub(super) fn apply_to_spec(&self, spec: &mut sys::ImPlot3DSpec_c) {
        spec.LineColors = self
            .line_colors
            .as_deref()
            .filter(|values| !values.is_empty())
            .map_or(std::ptr::null_mut(), |colors| colors.as_ptr() as *mut _);
        spec.FillColors = self
            .fill_colors
            .as_deref()
            .filter(|values| !values.is_empty())
            .map_or(std::ptr::null_mut(), |colors| colors.as_ptr() as *mut _);
        spec.MarkerSizes = self
            .marker_sizes
            .as_deref()
            .filter(|values| !values.is_empty())
            .map_or(std::ptr::null_mut(), |sizes| sizes.as_ptr() as *mut _);
        spec.MarkerLineColors = self
            .marker_line_colors
            .as_deref()
            .filter(|values| !values.is_empty())
            .map_or(std::ptr::null_mut(), |colors| colors.as_ptr() as *mut _);
        spec.MarkerFillColors = self
            .marker_fill_colors
            .as_deref()
            .filter(|values| !values.is_empty())
            .map_or(std::ptr::null_mut(), |colors| colors.as_ptr() as *mut _);
    }
}

impl<'plot> Plot3DScope<'plot> {
    /// Apply array-backed item styling to the next ImPlot3D submission executed inside `f`.
    ///
    /// This is closure-scoped so borrowed slices stay valid for the entire next
    /// plot call and are restored even if `f` panics before submitting an item.
    ///
    /// # Safety
    ///
    /// Every non-empty style array must contain enough elements for every index the submitted plot
    /// may read. Upstream accepts only pointers and carries no array lengths, so this cannot be
    /// validated by this method. In particular, a surface's wireframe can have more edges than
    /// grid points. Only submissions to this plot consume the override. The native item reads
    /// its arrays during submission and releases them in `EndItem`.
    pub unsafe fn with_next_plot3d_item_array_style<'a, R>(
        &self,
        style: Plot3DItemArrayStyle<'a>,
        f: impl FnOnce(&Plot3DScope<'plot>) -> R,
    ) -> R {
        self.next_item
            .scoped(|spec| style.apply_to_spec(spec), || f(self))
    }
}
