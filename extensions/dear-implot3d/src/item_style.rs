use crate::plots::Plot3D;
use crate::sys;
use dear_imgui_rs::texture::TextureRef;

fn color4(rgba: [f32; 4]) -> sys::ImVec4_c {
    sys::ImVec4_c {
        x: rgba[0],
        y: rgba[1],
        z: rgba[2],
        w: rgba[3],
    }
}

/// Common style overrides for plot items backed by `ImPlot3DSpec`.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Plot3DItemStyle {
    pub(crate) line_color: Option<sys::ImVec4_c>,
    pub(crate) line_weight: Option<f32>,
    pub(crate) fill_color: Option<sys::ImVec4_c>,
    pub(crate) fill_alpha: Option<f32>,
    pub(crate) marker: Option<sys::ImPlot3DMarker>,
    pub(crate) marker_size: Option<f32>,
    pub(crate) marker_line_color: Option<sys::ImVec4_c>,
    pub(crate) marker_fill_color: Option<sys::ImVec4_c>,
}

impl Plot3DItemStyle {
    /// Create an empty style override that keeps ImPlot3D defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Override the plot item's line color.
    pub fn with_line_color(mut self, color: [f32; 4]) -> Self {
        self.line_color = Some(color4(color));
        self
    }

    /// Override the plot item's line width in pixels.
    pub fn with_line_weight(mut self, weight: f32) -> Self {
        self.line_weight = Some(weight);
        self
    }

    /// Override the plot item's fill color.
    pub fn with_fill_color(mut self, color: [f32; 4]) -> Self {
        self.fill_color = Some(color4(color));
        self
    }

    /// Override the fill alpha multiplier used by filled regions and marker faces.
    pub fn with_fill_alpha(mut self, alpha: f32) -> Self {
        self.fill_alpha = Some(alpha);
        self
    }

    /// Override the marker type.
    pub fn with_marker(mut self, marker: crate::Marker3D) -> Self {
        self.marker = Some(marker as sys::ImPlot3DMarker);
        self
    }

    /// Override the marker size in pixels.
    pub fn with_marker_size(mut self, size: f32) -> Self {
        self.marker_size = Some(size);
        self
    }

    /// Override the marker outline color.
    pub fn with_marker_line_color(mut self, color: [f32; 4]) -> Self {
        self.marker_line_color = Some(color4(color));
        self
    }

    /// Override the marker fill color.
    pub fn with_marker_fill_color(mut self, color: [f32; 4]) -> Self {
        self.marker_fill_color = Some(color4(color));
        self
    }

    pub(crate) fn apply_to_spec(self, spec: &mut sys::ImPlot3DSpec_c) {
        if let Some(line_color) = self.line_color {
            spec.LineColor = line_color;
        }
        if let Some(line_weight) = self.line_weight {
            spec.LineWeight = line_weight;
        }
        if let Some(fill_color) = self.fill_color {
            spec.FillColor = fill_color;
        }
        if let Some(fill_alpha) = self.fill_alpha {
            spec.FillAlpha = fill_alpha;
        }
        if let Some(marker) = self.marker {
            spec.Marker = marker;
        }
        if let Some(marker_size) = self.marker_size {
            spec.MarkerSize = marker_size;
        }
        if let Some(marker_line_color) = self.marker_line_color {
            spec.MarkerLineColor = marker_line_color;
        }
        if let Some(marker_fill_color) = self.marker_fill_color {
            spec.MarkerFillColor = marker_fill_color;
        }
    }
}

pub(crate) fn plot3d_spec_with_style(
    style: Plot3DItemStyle,
    flags: u32,
    offset: i32,
    stride: i32,
) -> sys::ImPlot3DSpec_c {
    let mut spec = crate::plot3d_spec_from(flags, offset, stride);
    style.apply_to_spec(&mut spec);
    spec
}

fn with_scoped_next_plot3d_spec<R>(
    style: Plot3DItemStyle,
    item_flags: crate::Item3DFlags,
    f: impl FnOnce() -> R,
) -> R {
    let previous = crate::take_next_plot3d_spec();
    let mut spec = previous.unwrap_or_else(crate::default_plot3d_spec);
    style.apply_to_spec(&mut spec);
    spec.Flags = ((spec.Flags as u32) | item_flags.bits()) as sys::ImPlot3DItemFlags;
    crate::set_next_plot3d_spec(Some(spec));

    let out = f();

    if crate::take_next_plot3d_spec().is_some() {
        crate::set_next_plot3d_spec(previous);
    }

    out
}

/// Shared ImPlot3D item-style builder methods for plot builders backed by `ImPlot3DSpec`.
pub trait Plot3DItemStyled: Sized {
    /// The output type returned by style-building methods.
    type Output;

    fn map_style<F>(self, f: F) -> Self::Output
    where
        F: FnOnce(&mut Plot3DItemStyle);

    /// Replace the entire item style override for this plot.
    fn with_style(self, style: Plot3DItemStyle) -> Self::Output {
        self.map_style(|current| *current = style)
    }

    /// Set the line color.
    fn with_line_color(self, color: [f32; 4]) -> Self::Output {
        self.map_style(|style| style.line_color = Some(color4(color)))
    }

    /// Set the line width in pixels.
    fn with_line_weight(self, weight: f32) -> Self::Output {
        self.map_style(|style| style.line_weight = Some(weight))
    }

    /// Set the fill color.
    fn with_fill_color(self, color: [f32; 4]) -> Self::Output {
        self.map_style(|style| style.fill_color = Some(color4(color)))
    }

    /// Set the fill alpha multiplier used for fills and marker faces.
    fn with_fill_alpha(self, alpha: f32) -> Self::Output {
        self.map_style(|style| style.fill_alpha = Some(alpha))
    }

    /// Set the marker type.
    fn with_marker(self, marker: crate::Marker3D) -> Self::Output {
        self.map_style(|style| style.marker = Some(marker as sys::ImPlot3DMarker))
    }

    /// Set the marker size in pixels.
    fn with_marker_size(self, size: f32) -> Self::Output {
        self.map_style(|style| style.marker_size = Some(size))
    }

    /// Set the marker outline color.
    fn with_marker_line_color(self, color: [f32; 4]) -> Self::Output {
        self.map_style(|style| style.marker_line_color = Some(color4(color)))
    }

    /// Set the marker fill color.
    fn with_marker_fill_color(self, color: [f32; 4]) -> Self::Output {
        self.map_style(|style| style.marker_fill_color = Some(color4(color)))
    }
}

/// Shared ImPlot3D item-flag builder methods for plot builders backed by `ImPlot3DSpec`.
pub trait Plot3DItemFlagged: Sized {
    /// The output type returned by item-flag building methods.
    type Output;

    fn map_item_flags<F>(self, f: F) -> Self::Output
    where
        F: FnOnce(&mut crate::Item3DFlags);

    /// Set common item flags such as `NO_LEGEND` / `NO_FIT`.
    fn with_item_flags(self, flags: crate::Item3DFlags) -> Self::Output {
        self.map_item_flags(|current| *current = flags)
    }
}

/// Styled wrapper for plot types that do not store item-style state directly.
pub struct StyledPlot3D<T> {
    inner: T,
    style: Plot3DItemStyle,
    item_flags: crate::Item3DFlags,
}

impl<T> StyledPlot3D<T> {
    /// Consume the wrapper and return the wrapped plot item.
    pub fn into_inner(self) -> T {
        self.inner
    }

    /// Borrow the wrapped plot item.
    pub fn inner(&self) -> &T {
        &self.inner
    }
}

impl<T> Plot3DItemStyled for StyledPlot3D<T> {
    type Output = Self;

    fn map_style<F>(mut self, f: F) -> Self::Output
    where
        F: FnOnce(&mut Plot3DItemStyle),
    {
        f(&mut self.style);
        self
    }
}

impl<T> Plot3DItemFlagged for StyledPlot3D<T> {
    type Output = Self;

    fn map_item_flags<F>(mut self, f: F) -> Self::Output
    where
        F: FnOnce(&mut crate::Item3DFlags),
    {
        f(&mut self.item_flags);
        self
    }
}

impl<T> Plot3D for StyledPlot3D<T>
where
    T: Plot3D,
{
    fn label(&self) -> &str {
        self.inner.label()
    }

    fn try_plot(&self, ui: &crate::Plot3DUi<'_>) -> Result<(), crate::plots::Plot3DError> {
        with_scoped_next_plot3d_spec(self.style, self.item_flags, || self.inner.try_plot(ui))
    }
}

macro_rules! impl_wrapped_plot3d_item_styled {
    ($ty:ty) => {
        impl Plot3DItemStyled for $ty {
            type Output = StyledPlot3D<Self>;

            fn map_style<F>(self, f: F) -> Self::Output
            where
                F: FnOnce(&mut Plot3DItemStyle),
            {
                let mut style = Plot3DItemStyle::default();
                f(&mut style);
                StyledPlot3D {
                    inner: self,
                    style,
                    item_flags: crate::Item3DFlags::NONE,
                }
            }
        }
    };
}

macro_rules! impl_wrapped_plot3d_item_flagged {
    ($ty:ty) => {
        impl Plot3DItemFlagged for $ty {
            type Output = StyledPlot3D<Self>;

            fn map_item_flags<F>(self, f: F) -> Self::Output
            where
                F: FnOnce(&mut crate::Item3DFlags),
            {
                let mut item_flags = crate::Item3DFlags::NONE;
                f(&mut item_flags);
                StyledPlot3D {
                    inner: self,
                    style: Plot3DItemStyle::default(),
                    item_flags,
                }
            }
        }
    };
}

impl_wrapped_plot3d_item_styled!(crate::plots::Line3D<'_>);
impl_wrapped_plot3d_item_styled!(crate::plots::Scatter3D<'_>);
impl_wrapped_plot3d_item_styled!(crate::plots::Surface3D<'_>);
impl_wrapped_plot3d_item_styled!(crate::plots::Triangles3D<'_>);
impl_wrapped_plot3d_item_styled!(crate::plots::Quads3D<'_>);
impl_wrapped_plot3d_item_styled!(crate::plots::Mesh3D<'_>);
impl_wrapped_plot3d_item_flagged!(crate::plots::Line3D<'_>);
impl_wrapped_plot3d_item_flagged!(crate::plots::Scatter3D<'_>);
impl_wrapped_plot3d_item_flagged!(crate::plots::Surface3D<'_>);
impl_wrapped_plot3d_item_flagged!(crate::plots::Triangles3D<'_>);
impl_wrapped_plot3d_item_flagged!(crate::plots::Quads3D<'_>);
impl_wrapped_plot3d_item_flagged!(crate::plots::Mesh3D<'_>);

impl<'a, T> Plot3DItemStyled for crate::plots::Image3DByAxes<'a, T>
where
    T: Into<TextureRef> + Copy,
{
    type Output = StyledPlot3D<Self>;

    fn map_style<F>(self, f: F) -> Self::Output
    where
        F: FnOnce(&mut Plot3DItemStyle),
    {
        let mut style = Plot3DItemStyle::default();
        f(&mut style);
        StyledPlot3D {
            inner: self,
            style,
            item_flags: crate::Item3DFlags::NONE,
        }
    }
}

impl<'a, T> Plot3DItemFlagged for crate::plots::Image3DByAxes<'a, T>
where
    T: Into<TextureRef> + Copy,
{
    type Output = StyledPlot3D<Self>;

    fn map_item_flags<F>(self, f: F) -> Self::Output
    where
        F: FnOnce(&mut crate::Item3DFlags),
    {
        let mut item_flags = crate::Item3DFlags::NONE;
        f(&mut item_flags);
        StyledPlot3D {
            inner: self,
            style: Plot3DItemStyle::default(),
            item_flags,
        }
    }
}

impl<'a, T> Plot3DItemStyled for crate::plots::Image3DByCorners<'a, T>
where
    T: Into<TextureRef> + Copy,
{
    type Output = StyledPlot3D<Self>;

    fn map_style<F>(self, f: F) -> Self::Output
    where
        F: FnOnce(&mut Plot3DItemStyle),
    {
        let mut style = Plot3DItemStyle::default();
        f(&mut style);
        StyledPlot3D {
            inner: self,
            style,
            item_flags: crate::Item3DFlags::NONE,
        }
    }
}

impl<'a, T> Plot3DItemFlagged for crate::plots::Image3DByCorners<'a, T>
where
    T: Into<TextureRef> + Copy,
{
    type Output = StyledPlot3D<Self>;

    fn map_item_flags<F>(self, f: F) -> Self::Output
    where
        F: FnOnce(&mut crate::Item3DFlags),
    {
        let mut item_flags = crate::Item3DFlags::NONE;
        f(&mut item_flags);
        StyledPlot3D {
            inner: self,
            style: Plot3DItemStyle::default(),
            item_flags,
        }
    }
}

macro_rules! impl_builder_plot3d_item_styled {
    ($ty:ty) => {
        impl Plot3DItemStyled for $ty {
            type Output = Self;

            fn map_style<F>(mut self, f: F) -> Self::Output
            where
                F: FnOnce(&mut Plot3DItemStyle),
            {
                f(&mut self.style);
                self
            }
        }
    };
}

macro_rules! impl_builder_plot3d_item_flagged {
    ($ty:ty) => {
        impl Plot3DItemFlagged for $ty {
            type Output = Self;

            fn map_item_flags<F>(mut self, f: F) -> Self::Output
            where
                F: FnOnce(&mut crate::Item3DFlags),
            {
                f(&mut self.item_flags);
                self
            }
        }
    };
}

impl_builder_plot3d_item_styled!(crate::Surface3DBuilder<'_>);
impl_builder_plot3d_item_styled!(crate::Image3DByAxesBuilder<'_>);
impl_builder_plot3d_item_styled!(crate::Image3DByCornersBuilder<'_>);
impl_builder_plot3d_item_styled!(crate::Mesh3DBuilder<'_>);
impl_builder_plot3d_item_flagged!(crate::Surface3DBuilder<'_>);
impl_builder_plot3d_item_flagged!(crate::Image3DByAxesBuilder<'_>);
impl_builder_plot3d_item_flagged!(crate::Image3DByCornersBuilder<'_>);
impl_builder_plot3d_item_flagged!(crate::Mesh3DBuilder<'_>);

#[cfg(test)]
mod tests {
    use super::{Plot3DItemStyle, plot3d_spec_with_style, with_scoped_next_plot3d_spec};
    use crate::{
        Item3DFlags, Marker3D, default_plot3d_spec, set_next_plot3d_spec, take_next_plot3d_spec,
    };

    #[test]
    fn plot3d_item_style_applies_fields() {
        let style = Plot3DItemStyle::new()
            .with_line_color([0.1, 0.2, 0.3, 0.4])
            .with_line_weight(2.5)
            .with_fill_color([0.5, 0.6, 0.7, 0.8])
            .with_fill_alpha(0.35)
            .with_marker(Marker3D::Auto)
            .with_marker_size(6.0)
            .with_marker_line_color([0.9, 0.1, 0.2, 1.0])
            .with_marker_fill_color([0.3, 0.4, 0.5, 0.6]);

        let spec = plot3d_spec_with_style(style, 0, 7, 16);

        assert_eq!(spec.LineColor.x, 0.1);
        assert_eq!(spec.LineColor.y, 0.2);
        assert_eq!(spec.LineColor.z, 0.3);
        assert_eq!(spec.LineColor.w, 0.4);
        assert_eq!(spec.LineWeight, 2.5);
        assert_eq!(spec.FillColor.x, 0.5);
        assert_eq!(spec.FillColor.y, 0.6);
        assert_eq!(spec.FillColor.z, 0.7);
        assert_eq!(spec.FillColor.w, 0.8);
        assert_eq!(spec.FillAlpha, 0.35);
        assert_eq!(spec.Marker, crate::sys::ImPlot3DMarker_Auto as _);
        assert_eq!(spec.MarkerSize, 6.0);
        assert_eq!(spec.MarkerLineColor.x, 0.9);
        assert_eq!(spec.MarkerFillColor.z, 0.5);
        assert_eq!(spec.Offset, 7);
        assert_eq!(spec.Stride, 16);
    }

    #[test]
    fn scoped_next_spec_restores_previous_when_not_consumed() {
        set_next_plot3d_spec(None);

        let mut previous = default_plot3d_spec();
        previous.FillAlpha = 0.25;
        set_next_plot3d_spec(Some(previous));

        let out = with_scoped_next_plot3d_spec(
            Plot3DItemStyle::new().with_line_weight(2.0),
            Item3DFlags::NONE,
            || "no-plot",
        );

        assert_eq!(out, "no-plot");

        let restored = take_next_plot3d_spec().expect("previous next spec should be restored");
        assert_eq!(restored.FillAlpha, 0.25);
        assert_eq!(restored.LineWeight, 1.0);

        set_next_plot3d_spec(None);
    }

    #[test]
    fn scoped_next_spec_merges_with_previous_when_consumed() {
        set_next_plot3d_spec(None);

        let mut previous = default_plot3d_spec();
        previous.FillAlpha = 0.25;
        set_next_plot3d_spec(Some(previous));

        let consumed = with_scoped_next_plot3d_spec(
            Plot3DItemStyle::new()
                .with_line_weight(3.0)
                .with_marker(Marker3D::Auto),
            Item3DFlags::NO_LEGEND,
            || crate::plot3d_spec_from(0, 5, 12),
        );

        assert_eq!(consumed.FillAlpha, 0.25);
        assert_eq!(consumed.LineWeight, 3.0);
        assert_eq!(consumed.Marker, crate::sys::ImPlot3DMarker_Auto as _);
        assert_eq!(consumed.Flags as u32, Item3DFlags::NO_LEGEND.bits(),);
        assert_eq!(consumed.Offset, 5);
        assert_eq!(consumed.Stride, 12);
        assert!(take_next_plot3d_spec().is_none());
    }

    #[test]
    fn scoped_next_spec_item_flags_merge_with_plot_flags() {
        set_next_plot3d_spec(None);

        let consumed = with_scoped_next_plot3d_spec(
            Plot3DItemStyle::default(),
            Item3DFlags::NO_LEGEND,
            || crate::plot3d_spec_from(crate::Line3DFlags::SEGMENTS.bits(), 0, 4),
        );

        assert_eq!(
            consumed.Flags as u32,
            Item3DFlags::NO_LEGEND.bits() | crate::Line3DFlags::SEGMENTS.bits(),
        );
        assert!(take_next_plot3d_spec().is_none());
    }
}
