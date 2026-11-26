// Style and theming for plots

use crate::context::with_cleanup;
use crate::sys;
use crate::{FloatFormat, PlotContext, PlotScope, PlotUi};
use dear_imgui_rs::{with_scratch_txt, with_scratch_txt_two};
use std::borrow::Cow;
use std::cell::Cell;

use crate::Colormap;

const DEFAULT_COLORMAP_SCALE_FORMAT: &str = "%g";
const DEFAULT_COLORMAP_SLIDER_FORMAT: &str = "";

/// Style variables that can be modified
#[repr(i32)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum StyleVar {
    PlotDefaultSize = sys::ImPlotStyleVar_PlotDefaultSize as i32,
    PlotMinSize = sys::ImPlotStyleVar_PlotMinSize as i32,
    PlotBorderSize = sys::ImPlotStyleVar_PlotBorderSize as i32,
    MinorAlpha = sys::ImPlotStyleVar_MinorAlpha as i32,
    MajorTickLen = sys::ImPlotStyleVar_MajorTickLen as i32,
    MinorTickLen = sys::ImPlotStyleVar_MinorTickLen as i32,
    MajorTickSize = sys::ImPlotStyleVar_MajorTickSize as i32,
    MinorTickSize = sys::ImPlotStyleVar_MinorTickSize as i32,
    MajorGridSize = sys::ImPlotStyleVar_MajorGridSize as i32,
    MinorGridSize = sys::ImPlotStyleVar_MinorGridSize as i32,
    PlotPadding = sys::ImPlotStyleVar_PlotPadding as i32,
    LabelPadding = sys::ImPlotStyleVar_LabelPadding as i32,
    LegendPadding = sys::ImPlotStyleVar_LegendPadding as i32,
    LegendInnerPadding = sys::ImPlotStyleVar_LegendInnerPadding as i32,
    LegendSpacing = sys::ImPlotStyleVar_LegendSpacing as i32,
    MousePosPadding = sys::ImPlotStyleVar_MousePosPadding as i32,
    AnnotationPadding = sys::ImPlotStyleVar_AnnotationPadding as i32,
    FitPadding = sys::ImPlotStyleVar_FitPadding as i32,
    DigitalPadding = sys::ImPlotStyleVar_DigitalPadding as i32,
    DigitalSpacing = sys::ImPlotStyleVar_DigitalSpacing as i32,
}

/// Runtime ImPlot colormap index.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct ColormapIndex(pub(crate) i32);

impl ColormapIndex {
    #[inline]
    pub const fn new(index: usize) -> Option<Self> {
        if index <= i32::MAX as usize {
            Some(Self(index as i32))
        } else {
            None
        }
    }

    #[inline]
    pub const fn get(self) -> usize {
        self.0 as usize
    }

    #[inline]
    pub const fn raw(self) -> i32 {
        self.0
    }

    #[inline]
    pub(crate) const fn from_raw(raw: i32) -> Option<Self> {
        if raw >= 0 { Some(Self(raw)) } else { None }
    }
}

impl From<Colormap> for ColormapIndex {
    #[inline]
    fn from(value: Colormap) -> Self {
        value.index()
    }
}

impl From<usize> for ColormapIndex {
    #[inline]
    fn from(value: usize) -> Self {
        Self::new(value).expect("colormap index exceeded ImPlot's i32 range")
    }
}

/// Selected colormap for helpers that may use either the current style colormap or an explicit one.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ColormapSelection {
    Current,
    Index(ColormapIndex),
}

impl ColormapSelection {
    #[inline]
    pub(crate) const fn raw(self) -> i32 {
        match self {
            Self::Current => crate::IMPLOT_AUTO,
            Self::Index(index) => index.raw(),
        }
    }
}

impl From<Colormap> for ColormapSelection {
    #[inline]
    fn from(value: Colormap) -> Self {
        Self::Index(value.index())
    }
}

impl From<ColormapIndex> for ColormapSelection {
    #[inline]
    fn from(value: ColormapIndex) -> Self {
        Self::Index(value)
    }
}

impl From<Option<ColormapIndex>> for ColormapSelection {
    #[inline]
    fn from(value: Option<ColormapIndex>) -> Self {
        value.map_or(Self::Current, Self::Index)
    }
}

/// Zero-based color entry inside the active or selected colormap.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct ColormapColorIndex(i32);

impl ColormapColorIndex {
    #[inline]
    pub const fn new(index: usize) -> Option<Self> {
        Self::from_usize(index)
    }

    #[inline]
    pub const fn from_usize(index: usize) -> Option<Self> {
        if index <= i32::MAX as usize {
            Some(Self(index as i32))
        } else {
            None
        }
    }

    #[inline]
    pub const fn get(self) -> usize {
        self.0 as usize
    }

    #[inline]
    pub const fn raw(self) -> i32 {
        self.0
    }
}

impl From<usize> for ColormapColorIndex {
    #[inline]
    fn from(value: usize) -> Self {
        Self::new(value).expect("colormap color index exceeded ImPlot's i32 range")
    }
}

/// One-shot array-backed item style overrides for the next plot submission.
///
/// This mirrors the new per-item array fields added to `ImPlotSpec` without storing
/// borrowed pointers beyond the closure passed to [`PlotScope::with_next_plot_item_array_style`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlotItemArrayStyle<'a> {
    line_colors: Option<Cow<'a, [u32]>>,
    fill_colors: Option<Cow<'a, [u32]>>,
    marker_sizes: Option<Cow<'a, [f32]>>,
    marker_line_colors: Option<Cow<'a, [u32]>>,
    marker_fill_colors: Option<Cow<'a, [u32]>>,
}

impl<'a> PlotItemArrayStyle<'a> {
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

    fn apply_to_spec(&self, spec: &mut sys::ImPlotSpec_c) {
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

fn with_scoped_next_plot_item_array_style<R>(
    pending: &Cell<Option<sys::ImPlotSpec_c>>,
    style: PlotItemArrayStyle<'_>,
    draw: impl FnOnce() -> R,
) -> R {
    let previous = pending.get();
    let mut spec = previous.unwrap_or_else(crate::plots::default_plot_spec);
    style.apply_to_spec(&mut spec);
    pending.set(Some(spec));
    with_cleanup(
        || {
            if pending.take().is_some() {
                pending.set(previous);
            }
        },
        draw,
    )
}

impl PlotScope<'_> {
    /// Apply array-backed styling to the next item in this plot within `draw`.
    ///
    /// # Safety
    /// Every supplied array must cover every index its item may read. ImPlot's item ABI
    /// only carries pointers and does not report the required array length.
    pub unsafe fn with_next_plot_item_array_style<R>(
        &self,
        style: PlotItemArrayStyle<'_>,
        draw: impl FnOnce(&Self) -> R,
    ) -> R {
        self.with_bound_context(|| ());
        with_scoped_next_plot_item_array_style(&self.next_spec, style, || draw(self))
    }

    /// Read the next color from the active item group and advance its cursor.
    pub fn next_colormap_color(&self) -> [f32; 4] {
        self.with_bound_context(|| unsafe {
            let out = sys::ImPlot_NextColormapColor();
            [out.x, out.y, out.z, out.w]
        })
    }
}

impl StyleVar {
    fn is_scalar(self) -> bool {
        matches!(
            self,
            Self::PlotBorderSize | Self::MinorAlpha | Self::DigitalPadding | Self::DigitalSpacing
        )
    }
}

impl PlotUi<'_> {
    // ImPlot's style stacks belong to the context. Keep the scope lexical across plots and
    // subplot-owned modifiers, and bind only for the two native calls.
    fn with_style<R>(
        &self,
        push: impl FnOnce(),
        pop: impl FnOnce(),
        draw: impl FnOnce() -> R,
    ) -> R {
        self.context
            .with_bound_context("dear-implot: push style", push);
        with_cleanup(
            || {
                self.context
                    .with_bound_context("dear-implot: pop style", pop)
            },
            draw,
        )
    }

    /// Use a scalar style variable while drawing.
    pub fn with_style_var_f32<R>(&self, var: StyleVar, value: f32, draw: impl FnOnce() -> R) -> R {
        assert!(var.is_scalar(), "this style variable requires a Vec2");
        self.with_style(
            || unsafe { sys::ImPlot_PushStyleVar_Float(var as _, value) },
            || unsafe { sys::ImPlot_PopStyleVar(1) },
            draw,
        )
    }

    /// Use a two-component style variable while drawing.
    pub fn with_style_var_vec2<R>(
        &self,
        var: StyleVar,
        value: [f32; 2],
        draw: impl FnOnce() -> R,
    ) -> R {
        assert!(!var.is_scalar(), "this style variable requires a scalar");
        let value = sys::ImVec2_c {
            x: value[0],
            y: value[1],
        };
        self.with_style(
            || unsafe { sys::ImPlot_PushStyleVar_Vec2(var as _, value) },
            || unsafe { sys::ImPlot_PopStyleVar(1) },
            draw,
        )
    }

    /// Use a style color while drawing.
    pub fn with_style_color<R>(
        &self,
        element: crate::PlotColorElement,
        color: [f32; 4],
        draw: impl FnOnce() -> R,
    ) -> R {
        let value = sys::ImVec4_c {
            x: color[0],
            y: color[1],
            z: color[2],
            w: color[3],
        };
        self.with_style(
            || unsafe { sys::ImPlot_PushStyleColor_Vec4(element as _, value) },
            || unsafe { sys::ImPlot_PopStyleColor(1) },
            draw,
        )
    }

    /// Select a registered colormap while drawing.
    pub fn with_colormap<R>(&self, cmap: impl Into<ColormapIndex>, draw: impl FnOnce() -> R) -> R {
        let cmap = cmap.into().raw();
        self.with_style(
            || unsafe { sys::ImPlot_PushColormap_PlotColormap(checked_colormap(cmap)) },
            || unsafe { sys::ImPlot_PopColormap(1) },
            draw,
        )
    }

    /// Select a registered colormap by name while drawing.
    pub fn with_colormap_name<R>(&self, name: &str, draw: impl FnOnce() -> R) -> R {
        let index = self
            .context
            .colormap_index_by_name(name)
            .expect("unknown colormap name");
        self.with_colormap(index, draw)
    }
}

// The caller has bound the owning ImPlot context. IMPLOT_AUTO selects its current style.
fn checked_colormap(raw: i32) -> i32 {
    assert!(
        raw == crate::IMPLOT_AUTO || (0..unsafe { sys::ImPlot_GetColormapCount() }).contains(&raw),
        "colormap is not registered in this context"
    );
    raw
}

fn colormap_count_from_i32(raw: i32, caller: &str) -> usize {
    assert!(raw >= 0, "{caller} returned a negative colormap count");
    usize::try_from(raw).expect("non-negative colormap count must fit usize")
}

fn assert_colormap_sample_t(t: f32) {
    assert!(
        (0.0..=1.0).contains(&t),
        "sample_colormap t must be between 0 and 1"
    );
}

// ImPlot::LerpTable computes its last lookup with f32 arithmetic before indexing an ImVector.
fn colormap_table_size(count: i32, qualitative: bool) -> i64 {
    if qualitative {
        assert!(
            (count as f32) as i64 <= i64::from(i32::MAX),
            "colormap lookup exceeds ImPlot's index range"
        );
        return i64::from(count);
    }
    let last = 255 * (i64::from(count) - 1);
    assert!(
        ((last as f32 + 0.5) as i64) <= last,
        "colormap endpoint rounds past its interpolation table"
    );
    last + 1
}

impl PlotContext {
    /// Add a custom colormap from colors. The colors are copied by ImPlot.
    pub fn add_colormap(
        &self,
        name: &str,
        colors: &[[f32; 4]],
        qualitative: bool,
    ) -> ColormapIndex {
        assert!(!name.contains('\0'), "colormap name contained NUL");
        assert!(
            colors.len() > 1,
            "colormap must contain at least two colors"
        );
        assert!(
            colors
                .iter()
                .flatten()
                .all(|component| component.is_finite()),
            "colormap colors must be finite"
        );
        let count = i32::try_from(colors.len()).expect("colormap contained too many colors");
        let index = self.with_bound_context("dear-implot: PlotContext::add_colormap()", || {
            let table_size = colormap_table_size(count, qualitative);
            let data = unsafe { &(*sys::ImPlot_GetCurrentContext()).ColormapData };
            assert!(
                i64::from(data.Tables.Size) + table_size <= i64::from(i32::MAX)
                    && data.Keys.Size.checked_add(count).is_some(),
                "colormap exceeds ImPlot's index range"
            );
            with_scratch_txt(name, |ptr| unsafe {
                assert!(
                    sys::ImPlot_GetColormapIndex(ptr) < 0,
                    "colormap name is already registered"
                );
                let colors: Vec<sys::ImVec4> = colors
                    .iter()
                    .map(|color| sys::ImVec4 {
                        x: color[0],
                        y: color[1],
                        z: color[2],
                        w: color[3],
                    })
                    .collect();
                sys::ImPlot_AddColormap_Vec4Ptr(ptr, colors.as_ptr(), count, qualitative)
            })
        });
        ColormapIndex::from_raw(index).expect("ImPlot returned a negative colormap index")
    }

    /// Return the number of available colormaps.
    pub fn colormap_count(&self) -> usize {
        self.with_bound_context("dear-implot: PlotContext::colormap_count()", || {
            colormap_count_from_i32(
                unsafe { sys::ImPlot_GetColormapCount() },
                "PlotContext::colormap_count()",
            )
        })
    }

    /// Return a colormap name, or an empty string if the index is invalid for this context.
    pub fn colormap_name(&self, index: impl Into<ColormapIndex>) -> String {
        let index = index.into().raw();
        self.with_bound_context("dear-implot: PlotContext::colormap_name()", || unsafe {
            if !(0..sys::ImPlot_GetColormapCount()).contains(&index) {
                return String::new();
            }
            let name = sys::ImPlot_GetColormapName(index);
            if name.is_null() {
                return String::new();
            }
            std::ffi::CStr::from_ptr(name)
                .to_string_lossy()
                .into_owned()
        })
    }

    /// Look up a colormap index by its name.
    pub fn colormap_index_by_name(&self, name: &str) -> Option<ColormapIndex> {
        if name.contains('\0') {
            return None;
        }
        let index = self
            .with_bound_context("dear-implot: PlotContext::colormap_index_by_name()", || {
                with_scratch_txt(name, |ptr| unsafe { sys::ImPlot_GetColormapIndex(ptr) })
            });
        ColormapIndex::from_raw(index)
    }

    /// Return the number of color entries in a colormap.
    pub fn colormap_size(&self, index: impl Into<ColormapIndex>) -> usize {
        self.with_bound_context("dear-implot: PlotContext::colormap_size()", || {
            colormap_count_from_i32(
                unsafe { sys::ImPlot_GetColormapSize(checked_colormap(index.into().raw())) },
                "PlotContext::colormap_size()",
            )
        })
    }

    /// Return the default colormap stored in this ImPlot context's style.
    pub fn style_colormap_index(&self) -> Option<ColormapIndex> {
        self.with_bound_context(
            "dear-implot: PlotContext::style_colormap_index()",
            || unsafe {
                let style = sys::ImPlot_GetStyle();
                if style.is_null() {
                    return None;
                }
                ColormapIndex::from_raw((*style).Colormap)
            },
        )
    }

    /// Return this context's default colormap name.
    pub fn style_colormap_name(&self) -> Option<String> {
        let idx = self.style_colormap_index()?;
        let count = self.colormap_count();
        if idx.get() >= count {
            return None;
        }
        Some(self.colormap_name(idx))
    }

    /// Permanently set the default colormap used by this ImPlot context.
    pub fn set_style_colormap(&self, index: impl Into<ColormapIndex>) {
        self.with_bound_context(
            "dear-implot: PlotContext::set_style_colormap()",
            || unsafe {
                let style = sys::ImPlot_GetStyle();
                if !style.is_null() {
                    let count = colormap_count_from_i32(
                        sys::ImPlot_GetColormapCount(),
                        "PlotContext::set_style_colormap()",
                    );
                    if count > 0 {
                        let index = index.into().get();
                        let idx = index.min(count - 1);
                        (*style).Colormap = ColormapIndex::from(idx).raw();
                    }
                }
            },
        )
    }

    /// Permanently set the default colormap by name. Invalid names are ignored.
    pub fn set_style_colormap_by_name(&self, name: &str) {
        if let Some(idx) = self.colormap_index_by_name(name) {
            self.set_style_colormap(idx);
        }
    }

    /// Return a color from this context's active colormap.
    pub fn colormap_color(&self, index: ColormapColorIndex) -> [f32; 4] {
        self.with_bound_context("dear-implot: PlotContext::colormap_color()", || unsafe {
            let out = sys::ImPlot_GetColormapColor(index.raw(), crate::IMPLOT_AUTO);
            [out.x, out.y, out.z, out.w]
        })
    }

    /// Return a color from a selected colormap.
    pub fn colormap_color_from(
        &self,
        index: ColormapColorIndex,
        cmap: impl Into<ColormapIndex>,
    ) -> [f32; 4] {
        self.with_bound_context(
            "dear-implot: PlotContext::colormap_color_from()",
            || unsafe {
                let out =
                    sys::ImPlot_GetColormapColor(index.raw(), checked_colormap(cmap.into().raw()));
                [out.x, out.y, out.z, out.w]
            },
        )
    }

    /// Sample this context's active colormap at `t` in `[0, 1]`.
    pub fn sample_colormap(&self, t: f32) -> [f32; 4] {
        assert_colormap_sample_t(t);
        self.with_bound_context("dear-implot: PlotContext::sample_colormap()", || unsafe {
            let out = sys::ImPlot_SampleColormap(t, crate::IMPLOT_AUTO);
            [out.x, out.y, out.z, out.w]
        })
    }

    /// Sample a selected colormap at `t` in `[0, 1]`.
    pub fn sample_colormap_from(&self, t: f32, cmap: impl Into<ColormapSelection>) -> [f32; 4] {
        assert_colormap_sample_t(t);
        self.with_bound_context(
            "dear-implot: PlotContext::sample_colormap_from()",
            || unsafe {
                let out = sys::ImPlot_SampleColormap(t, checked_colormap(cmap.into().raw()));
                [out.x, out.y, out.z, out.w]
            },
        )
    }

    /// Map this context's input scheme to ImPlot defaults.
    pub fn map_input_default(&self) {
        self.with_bound_context("dear-implot: PlotContext::map_input_default()", || unsafe {
            sys::ImPlot_MapInputDefault(sys::ImPlot_GetInputMap())
        })
    }

    /// Map this context's input scheme to ImPlot's reversed scheme.
    pub fn map_input_reverse(&self) {
        self.with_bound_context("dear-implot: PlotContext::map_input_reverse()", || unsafe {
            sys::ImPlot_MapInputReverse(sys::ImPlot_GetInputMap())
        })
    }
}

impl PlotUi<'_> {
    /// Show the style editor with this plot context's saved reference, shared with its demo.
    pub fn show_style_editor(&self) {
        self.with_idle_context(|| unsafe { sys::ImPlot_ShowStyleEditor(std::ptr::null_mut()) })
    }

    /// Show the ImPlot style selector combo; returns true if selection changed.
    pub fn show_style_selector(&self, label: &str) -> bool {
        let label = if label.contains('\0') { "" } else { label };
        self.with_bound_context(|| {
            with_scratch_txt(label, |ptr| unsafe { sys::ImPlot_ShowStyleSelector(ptr) })
        })
    }

    /// Show the ImPlot colormap selector combo; returns true if selection changed.
    pub fn show_colormap_selector(&self, label: &str) -> bool {
        let label = if label.contains('\0') { "" } else { label };
        self.with_idle_context(|| {
            with_scratch_txt(label, |ptr| unsafe {
                sys::ImPlot_ShowColormapSelector(ptr)
            })
        })
    }

    /// Show the ImPlot input-map selector combo; returns true if selection changed.
    pub fn show_input_map_selector(&self, label: &str) -> bool {
        let label = if label.contains('\0') { "" } else { label };
        self.with_bound_context(|| {
            with_scratch_txt(label, |ptr| unsafe {
                sys::ImPlot_ShowInputMapSelector(ptr)
            })
        })
    }

    /// Draw a colormap scale widget.
    pub fn colormap_scale(
        &self,
        label: &str,
        scale_min: f64,
        scale_max: f64,
        height: f32,
        cmap: impl Into<ColormapSelection>,
    ) {
        self.colormap_scale_impl(
            label,
            scale_min,
            scale_max,
            height,
            DEFAULT_COLORMAP_SCALE_FORMAT,
            cmap.into(),
        );
    }

    /// Draw a colormap scale with a validated tick format.
    pub fn colormap_scale_with_format(
        &self,
        label: &str,
        scale_min: f64,
        scale_max: f64,
        height: f32,
        format: &FloatFormat<'_>,
        cmap: impl Into<ColormapSelection>,
    ) {
        self.colormap_scale_impl(
            label,
            scale_min,
            scale_max,
            height,
            format.as_str(),
            cmap.into(),
        );
    }

    fn colormap_scale_impl(
        &self,
        label: &str,
        scale_min: f64,
        scale_max: f64,
        height: f32,
        format: &str,
        cmap: ColormapSelection,
    ) {
        assert!(
            scale_min.is_finite(),
            "colormap_scale scale_min must be finite"
        );
        assert!(
            scale_max.is_finite(),
            "colormap_scale scale_max must be finite"
        );
        assert!(height.is_finite(), "colormap_scale height must be finite");
        let label = if label.contains('\0') { "" } else { label };
        let size = sys::ImVec2_c { x: 0.0, y: height };
        let flags = sys::ImPlotColormapScaleFlags_None as sys::ImPlotColormapScaleFlags;
        let cmap = cmap.raw();
        self.with_bound_context(|| {
            let cmap = checked_colormap(cmap);
            with_scratch_txt_two(label, format, |label_ptr, format_ptr| unsafe {
                sys::ImPlot_ColormapScale(
                    label_ptr, scale_min, scale_max, size, format_ptr, flags, cmap,
                )
            })
        })
    }

    /// Draw a colormap slider with ImPlot's default value format.
    pub fn colormap_slider(
        &self,
        label: &str,
        t: &mut f32,
        out_color: Option<&mut [f32; 4]>,
        cmap: impl Into<ColormapSelection>,
    ) -> bool {
        self.colormap_slider_impl(
            label,
            t,
            out_color,
            DEFAULT_COLORMAP_SLIDER_FORMAT,
            cmap.into(),
        )
    }

    /// Draw a colormap slider with a validated value format.
    pub fn colormap_slider_with_format(
        &self,
        label: &str,
        t: &mut f32,
        out_color: Option<&mut [f32; 4]>,
        format: &FloatFormat<'_>,
        cmap: impl Into<ColormapSelection>,
    ) -> bool {
        self.colormap_slider_impl(label, t, out_color, format.as_str(), cmap.into())
    }

    fn colormap_slider_impl(
        &self,
        label: &str,
        t: &mut f32,
        out_color: Option<&mut [f32; 4]>,
        format: &str,
        cmap: ColormapSelection,
    ) -> bool {
        assert!(t.is_finite(), "colormap_slider t must be finite");
        let label = if label.contains('\0') { "" } else { label };
        let cmap = cmap.raw();
        let mut out = sys::ImVec4 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            w: 0.0,
        };
        let out_ptr = if out_color.is_some() {
            &mut out as *mut sys::ImVec4
        } else {
            std::ptr::null_mut()
        };

        self.with_bound_context(|| {
            let cmap = checked_colormap(cmap);
            let changed = with_scratch_txt_two(label, format, |label_ptr, format_ptr| unsafe {
                sys::ImPlot_ColormapSlider(label_ptr, t as *mut f32, out_ptr, format_ptr, cmap)
            });

            if let Some(out_color) = out_color {
                *out_color = [out.x, out.y, out.z, out.w];
            }
            changed
        })
    }

    /// Draw a colormap picker button; returns true if clicked.
    pub fn colormap_button(
        &self,
        label: &str,
        size: [f32; 2],
        cmap: impl Into<ColormapSelection>,
    ) -> bool {
        assert!(
            size[0].is_finite() && size[1].is_finite(),
            "colormap_button size must be finite"
        );
        let label = if label.contains('\0') { "" } else { label };
        let sz = sys::ImVec2_c {
            x: size[0],
            y: size[1],
        };
        let cmap = cmap.into().raw();
        self.with_bound_context(|| {
            with_scratch_txt(label, |ptr| unsafe {
                sys::ImPlot_ColormapButton(ptr, sz, checked_colormap(cmap))
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Colormap, ColormapColorIndex, ColormapIndex, ColormapSelection, PlotItemArrayStyle,
        with_scoped_next_plot_item_array_style,
    };
    use crate::plots::{PlotDataLayout, PlotDataOffset, PlotDataStride};

    #[test]
    fn colormap_table_endpoints_fit_the_native_lookup() {
        assert_eq!(super::colormap_table_size(3, false), 511);
        assert_eq!(super::colormap_table_size(32_900, true), 32_900);
        assert!(std::panic::catch_unwind(|| super::colormap_table_size(32_900, false)).is_err());
    }

    #[test]
    fn colormap_indices_reject_negative_values() {
        assert_eq!(ColormapIndex::from_raw(-1), None);
        assert_eq!(ColormapIndex::new(0).map(ColormapIndex::raw), Some(0));
        assert_eq!(ColormapIndex::new(0).map(ColormapIndex::get), Some(0));
        assert_eq!(ColormapIndex::new(i32::MAX as usize + 1), None);
        assert_eq!(
            ColormapIndex::from(Colormap::Viridis).raw(),
            crate::sys::ImPlotColormap_Viridis
        );
        assert_eq!(ColormapSelection::Current.raw(), crate::IMPLOT_AUTO);
        assert_eq!(
            ColormapSelection::from(Colormap::Viridis).raw(),
            crate::sys::ImPlotColormap_Viridis
        );

        assert_eq!(
            ColormapColorIndex::new(0).map(ColormapColorIndex::get),
            Some(0)
        );
        assert_eq!(
            ColormapColorIndex::from_usize(i32::MAX as usize).map(ColormapColorIndex::raw),
            Some(i32::MAX)
        );
        assert_eq!(ColormapColorIndex::from_usize(i32::MAX as usize + 1), None);
    }

    #[test]
    #[should_panic(expected = "test returned a negative colormap count")]
    fn colormap_count_conversion_rejects_negative_ffi_values() {
        let _ = super::colormap_count_from_i32(-1, "test");
    }

    #[test]
    #[should_panic(expected = "sample_colormap t must be between 0 and 1")]
    fn sample_colormap_rejects_out_of_range_t_before_ffi() {
        super::assert_colormap_sample_t(-0.1);
    }

    #[test]
    fn next_plot_item_array_style_is_consumed_by_next_spec() {
        let pending = std::cell::Cell::new(None);
        let line_colors = [0x01020304u32, 0x05060708];
        let fill_colors = [0x11121314u32];
        let marker_sizes = [2.0f32, 4.0, 8.0];

        with_scoped_next_plot_item_array_style(
            &pending,
            PlotItemArrayStyle::new()
                .with_line_colors(&line_colors)
                .with_fill_colors(&fill_colors)
                .with_marker_sizes(&marker_sizes),
            || {
                let layout =
                    PlotDataLayout::new(PlotDataOffset::samples(3), PlotDataStride::bytes(16));
                let spec = crate::plots::plot_spec_from(pending.take(), 7, layout);
                assert_eq!(spec.Flags, 7);
                assert_eq!(spec.Offset, 3);
                assert_eq!(spec.Stride, 16);
                assert_eq!(spec.LineColors, line_colors.as_ptr() as *mut _);
                assert_eq!(spec.FillColors, fill_colors.as_ptr() as *mut _);
                assert_eq!(spec.MarkerSizes, marker_sizes.as_ptr() as *mut _);
            },
        );

        let spec = crate::plots::plot_spec_from(pending.take(), 0, PlotDataLayout::DEFAULT);
        assert!(spec.LineColors.is_null());
        assert!(spec.FillColors.is_null());
        assert!(spec.MarkerSizes.is_null());
    }

    #[test]
    fn next_plot_item_array_style_is_restored_if_unused() {
        let pending = std::cell::Cell::new(None);
        let line_colors = [0xAABBCCDDu32];

        with_scoped_next_plot_item_array_style(
            &pending,
            PlotItemArrayStyle::new().with_line_colors(&line_colors),
            || {},
        );

        let spec = crate::plots::plot_spec_from(pending.take(), 0, PlotDataLayout::DEFAULT);
        assert!(spec.LineColors.is_null());
    }

    #[test]
    fn next_plot_item_array_style_is_restored_if_closure_panics() {
        let pending = std::cell::Cell::new(None);
        let line_colors = [0xAABBCCDDu32];

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            with_scoped_next_plot_item_array_style(
                &pending,
                PlotItemArrayStyle::new().with_line_colors(&line_colors),
                || panic!("boom"),
            );
        }));

        assert!(result.is_err());
        assert!(pending.take().is_none());
    }
}
