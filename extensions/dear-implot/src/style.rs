// Style and theming for plots

use crate::sys;
use dear_imgui_rs::{with_scratch_txt, with_scratch_txt_two};
use std::borrow::Cow;
use std::marker::PhantomData;
use std::os::raw::c_char;
use std::rc::Rc;

use crate::Colormap;

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

/// Token for managing style variable changes
pub struct StyleVarToken {
    was_popped: bool,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl StyleVarToken {
    /// Pop this style variable from the stack
    pub fn pop(mut self) {
        if self.was_popped {
            panic!("Attempted to pop a style var token twice.");
        }
        self.was_popped = true;
        unsafe {
            sys::ImPlot_PopStyleVar(1);
        }
    }
}

impl Drop for StyleVarToken {
    fn drop(&mut self) {
        if !self.was_popped {
            unsafe {
                sys::ImPlot_PopStyleVar(1);
            }
        }
    }
}

/// Token for managing style color changes
pub struct StyleColorToken {
    was_popped: bool,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl StyleColorToken {
    /// Pop this style color from the stack
    pub fn pop(mut self) {
        if self.was_popped {
            panic!("Attempted to pop a style color token twice.");
        }
        self.was_popped = true;
        unsafe {
            sys::ImPlot_PopStyleColor(1);
        }
    }
}

impl Drop for StyleColorToken {
    fn drop(&mut self) {
        if !self.was_popped {
            unsafe {
                sys::ImPlot_PopStyleColor(1);
            }
        }
    }
}

/// Runtime ImPlot colormap index.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct ColormapIndex(pub(crate) i32);

impl ColormapIndex {
    #[inline]
    pub const fn new(index: i32) -> Option<Self> {
        if index >= 0 { Some(Self(index)) } else { None }
    }

    #[inline]
    pub const fn raw(self) -> i32 {
        self.0
    }
}

impl From<Colormap> for ColormapIndex {
    #[inline]
    fn from(value: Colormap) -> Self {
        value.index()
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
    pub const fn new(index: i32) -> Option<Self> {
        if index >= 0 { Some(Self(index)) } else { None }
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
    pub const fn raw(self) -> i32 {
        self.0
    }
}

/// Token for managing colormap changes.
#[must_use]
pub struct ColormapToken {
    was_popped: bool,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl ColormapToken {
    /// Pop this colormap from the stack.
    pub fn pop(mut self) {
        if self.was_popped {
            panic!("Attempted to pop an ImPlot colormap token twice.");
        }
        self.was_popped = true;
        unsafe {
            sys::ImPlot_PopColormap(1);
        }
    }
}

impl Drop for ColormapToken {
    fn drop(&mut self) {
        if !self.was_popped {
            unsafe {
                sys::ImPlot_PopColormap(1);
            }
        }
    }
}

/// One-shot array-backed item style overrides for the next plot submission.
///
/// This mirrors the new per-item array fields added to `ImPlotSpec` without storing
/// borrowed pointers beyond the closure passed to [`with_next_plot_item_array_style`].
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

/// Apply array-backed item styling to the next plot submission executed inside `f`.
///
/// This is intentionally closure-scoped so borrowed slices stay valid for the entire
/// duration of the next plot call and cannot leak into later frames accidentally.
pub fn with_next_plot_item_array_style<'a, R>(
    style: PlotItemArrayStyle<'a>,
    f: impl FnOnce() -> R,
) -> R {
    let previous = crate::plots::take_next_plot_spec();
    let mut spec = previous.unwrap_or_else(crate::plots::default_plot_spec);
    style.apply_to_spec(&mut spec);
    crate::plots::set_next_plot_spec(Some(spec));

    let out = f();

    if crate::plots::take_next_plot_spec().is_some() {
        crate::plots::set_next_plot_spec(previous);
    }

    out
}

/// Push a float style variable to the stack
pub fn push_style_var_f32(var: StyleVar, value: f32) -> StyleVarToken {
    unsafe {
        sys::ImPlot_PushStyleVar_Float(var as sys::ImPlotStyleVar, value);
    }
    StyleVarToken {
        was_popped: false,
        _not_send_or_sync: PhantomData,
    }
}

/// Push an integer style variable to the stack (converted to float)
pub fn push_style_var_i32(var: StyleVar, value: i32) -> StyleVarToken {
    unsafe {
        sys::ImPlot_PushStyleVar_Int(var as sys::ImPlotStyleVar, value);
    }
    StyleVarToken {
        was_popped: false,
        _not_send_or_sync: PhantomData,
    }
}

/// Push a Vec2 style variable to the stack
pub fn push_style_var_vec2(var: StyleVar, value: [f32; 2]) -> StyleVarToken {
    unsafe {
        sys::ImPlot_PushStyleVar_Vec2(
            var as sys::ImPlotStyleVar,
            sys::ImVec2_c {
                x: value[0],
                y: value[1],
            },
        );
    }
    StyleVarToken {
        was_popped: false,
        _not_send_or_sync: PhantomData,
    }
}

/// Push a style color to the stack
pub fn push_style_color(element: crate::PlotColorElement, color: [f32; 4]) -> StyleColorToken {
    unsafe {
        // Convert color to ImU32 format (RGBA)
        let r = (color[0] * 255.0) as u32;
        let g = (color[1] * 255.0) as u32;
        let b = (color[2] * 255.0) as u32;
        let a = (color[3] * 255.0) as u32;
        let color_u32 = (a << 24) | (b << 16) | (g << 8) | r;

        sys::ImPlot_PushStyleColor_U32(element as sys::ImPlotCol, color_u32);
    }
    StyleColorToken {
        was_popped: false,
        _not_send_or_sync: PhantomData,
    }
}

/// Push a colormap to the stack
pub fn push_colormap(cmap: impl Into<ColormapIndex>) -> ColormapToken {
    unsafe {
        sys::ImPlot_PushColormap_PlotColormap(cmap.into().raw());
    }
    ColormapToken {
        was_popped: false,
        _not_send_or_sync: PhantomData,
    }
}

/// Push a colormap by name to the stack.
pub fn push_colormap_name(name: &str) -> ColormapToken {
    assert!(!name.contains('\0'), "colormap name contained NUL");
    with_scratch_txt(name, |ptr| unsafe { sys::ImPlot_PushColormap_Str(ptr) });
    ColormapToken {
        was_popped: false,
        _not_send_or_sync: PhantomData,
    }
}

/// Add a custom colormap from colors. The colors are copied by ImPlot.
pub fn add_colormap(name: &str, colors: &[[f32; 4]], qualitative: bool) -> ColormapIndex {
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
    let colors: Vec<sys::ImVec4> = colors
        .iter()
        .map(|color| sys::ImVec4 {
            x: color[0],
            y: color[1],
            z: color[2],
            w: color[3],
        })
        .collect();
    let index = with_scratch_txt(name, |ptr| unsafe {
        sys::ImPlot_AddColormap_Vec4Ptr(ptr, colors.as_ptr(), count, qualitative)
    });
    ColormapIndex::new(index).expect("ImPlot returned a negative colormap index")
}

/// Return the number of available colormaps.
pub fn colormap_count() -> i32 {
    unsafe { sys::ImPlot_GetColormapCount() }
}

/// Return a colormap name, or an empty string if the index is invalid for the current context.
pub fn colormap_name(index: impl Into<ColormapIndex>) -> String {
    unsafe {
        let p = sys::ImPlot_GetColormapName(index.into().raw());
        if p.is_null() {
            return String::new();
        }
        std::ffi::CStr::from_ptr(p).to_string_lossy().into_owned()
    }
}

/// Look up a colormap index by its name.
pub fn colormap_index_by_name(name: &str) -> Option<ColormapIndex> {
    if name.contains('\0') {
        return None;
    }
    let index = with_scratch_txt(name, |ptr| unsafe { sys::ImPlot_GetColormapIndex(ptr) });
    ColormapIndex::new(index)
}

/// Return the number of color entries in a colormap.
pub fn colormap_size(index: impl Into<ColormapIndex>) -> i32 {
    unsafe { sys::ImPlot_GetColormapSize(index.into().raw()) }
}

/// Return the current default colormap stored in the ImPlot style.
pub fn get_style_colormap_index() -> Option<ColormapIndex> {
    unsafe {
        let style = sys::ImPlot_GetStyle();
        if style.is_null() {
            return None;
        }
        ColormapIndex::new((*style).Colormap)
    }
}

/// Return the current default colormap name.
pub fn get_style_colormap_name() -> Option<String> {
    let idx = get_style_colormap_index()?;
    let count = colormap_count();
    if idx.raw() >= count {
        return None;
    }
    Some(colormap_name(idx))
}

/// Permanently set the default colormap used by ImPlot.
pub fn set_style_colormap(index: impl Into<ColormapIndex>) {
    unsafe {
        let style = sys::ImPlot_GetStyle();
        if !style.is_null() {
            let count = sys::ImPlot_GetColormapCount();
            if count > 0 {
                let index = index.into().raw();
                let idx = if index >= count { count - 1 } else { index };
                (*style).Colormap = idx;
            }
        }
    }
}

/// Permanently set the default colormap by name. Invalid names are ignored.
pub fn set_style_colormap_by_name(name: &str) {
    if let Some(idx) = colormap_index_by_name(name) {
        set_style_colormap(idx);
    }
}

/// Return a color from the active colormap.
pub fn get_colormap_color(index: ColormapColorIndex) -> [f32; 4] {
    unsafe {
        let out = sys::ImPlot_GetColormapColor(index.raw(), crate::IMPLOT_AUTO);
        [out.x, out.y, out.z, out.w]
    }
}

/// Return a color from a selected colormap.
pub fn get_colormap_color_from(
    index: ColormapColorIndex,
    cmap: impl Into<ColormapIndex>,
) -> [f32; 4] {
    unsafe {
        let out = sys::ImPlot_GetColormapColor(index.raw(), cmap.into().raw());
        [out.x, out.y, out.z, out.w]
    }
}

/// Sample the active colormap at `t` in `[0, 1]`.
pub fn sample_colormap(t: f32) -> [f32; 4] {
    assert!(
        (0.0..=1.0).contains(&t),
        "sample_colormap t must be between 0 and 1"
    );
    unsafe {
        let out = sys::ImPlot_SampleColormap(t, crate::IMPLOT_AUTO);
        [out.x, out.y, out.z, out.w]
    }
}

/// Sample a selected colormap at `t` in `[0, 1]`.
pub fn sample_colormap_from(t: f32, cmap: impl Into<ColormapSelection>) -> [f32; 4] {
    assert!(
        (0.0..=1.0).contains(&t),
        "sample_colormap t must be between 0 and 1"
    );
    unsafe {
        let out = sys::ImPlot_SampleColormap(t, cmap.into().raw());
        [out.x, out.y, out.z, out.w]
    }
}

/// Return the next color from the current colormap and advance the plot color cursor.
pub fn next_colormap_color() -> [f32; 4] {
    unsafe {
        let out = sys::ImPlot_NextColormapColor();
        [out.x, out.y, out.z, out.w]
    }
}

// Style editor / selectors and input-map helpers

/// Show the ImPlot style editor window
pub fn show_style_editor() {
    unsafe { sys::ImPlot_ShowStyleEditor(std::ptr::null_mut()) }
}

/// Show the ImPlot style selector combo; returns true if selection changed
pub fn show_style_selector(label: &str) -> bool {
    let label = if label.contains('\0') { "" } else { label };
    with_scratch_txt(label, |ptr| unsafe { sys::ImPlot_ShowStyleSelector(ptr) })
}

/// Show the ImPlot colormap selector combo; returns true if selection changed
pub fn show_colormap_selector(label: &str) -> bool {
    let label = if label.contains('\0') { "" } else { label };
    with_scratch_txt(label, |ptr| unsafe {
        sys::ImPlot_ShowColormapSelector(ptr)
    })
}

/// Show the ImPlot input-map selector combo; returns true if selection changed
pub fn show_input_map_selector(label: &str) -> bool {
    let label = if label.contains('\0') { "" } else { label };
    with_scratch_txt(label, |ptr| unsafe {
        sys::ImPlot_ShowInputMapSelector(ptr)
    })
}

/// Map input to defaults
pub fn map_input_default() {
    unsafe { sys::ImPlot_MapInputDefault(sys::ImPlot_GetInputMap()) }
}

/// Map input to reversed scheme
pub fn map_input_reverse() {
    unsafe { sys::ImPlot_MapInputReverse(sys::ImPlot_GetInputMap()) }
}

// Colormap widgets

/// Draw a colormap scale widget
pub fn colormap_scale(
    label: &str,
    scale_min: f64,
    scale_max: f64,
    height: f32,
    cmap: impl Into<ColormapSelection>,
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
    let fmt_ptr: *const c_char = std::ptr::null();
    let flags = sys::ImPlotColormapScaleFlags_None;
    let cmap = cmap.into().raw();
    with_scratch_txt(label, |ptr| unsafe {
        sys::ImPlot_ColormapScale(ptr, scale_min, scale_max, size, fmt_ptr, flags, cmap)
    })
}

/// Draw a colormap slider; returns true if selection changed
pub fn colormap_slider(
    label: &str,
    t: &mut f32,
    out_color: Option<&mut [f32; 4]>,
    format: Option<&str>,
    cmap: impl Into<ColormapSelection>,
) -> bool {
    assert!(t.is_finite(), "colormap_slider t must be finite");
    let label = if label.contains('\0') { "" } else { label };
    let format = format.filter(|s| !s.contains('\0'));
    let cmap = cmap.into().raw();
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

    let changed = match format {
        Some(fmt) => with_scratch_txt_two(label, fmt, |label_ptr, fmt_ptr| unsafe {
            sys::ImPlot_ColormapSlider(label_ptr, t as *mut f32, out_ptr, fmt_ptr, cmap)
        }),
        None => with_scratch_txt(label, |label_ptr| unsafe {
            sys::ImPlot_ColormapSlider(label_ptr, t as *mut f32, out_ptr, std::ptr::null(), cmap)
        }),
    };

    if let Some(out_color) = out_color {
        *out_color = [out.x, out.y, out.z, out.w];
    }
    changed
}

/// Draw a colormap picker button; returns true if clicked
pub fn colormap_button(label: &str, size: [f32; 2], cmap: impl Into<ColormapSelection>) -> bool {
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
    with_scratch_txt(label, |ptr| unsafe {
        sys::ImPlot_ColormapButton(ptr, sz, cmap)
    })
}

#[cfg(test)]
mod tests {
    use super::{
        Colormap, ColormapColorIndex, ColormapIndex, ColormapSelection, PlotItemArrayStyle,
        with_next_plot_item_array_style,
    };
    use crate::plots::{PlotDataLayout, PlotDataOffset, PlotDataStride};

    #[test]
    fn colormap_indices_reject_negative_values() {
        assert_eq!(ColormapIndex::new(-1), None);
        assert_eq!(ColormapIndex::new(0).map(ColormapIndex::raw), Some(0));
        assert_eq!(
            ColormapIndex::from(Colormap::Viridis).raw(),
            crate::sys::ImPlotColormap_Viridis
        );
        assert_eq!(ColormapSelection::Current.raw(), crate::IMPLOT_AUTO);
        assert_eq!(
            ColormapSelection::from(Colormap::Viridis).raw(),
            crate::sys::ImPlotColormap_Viridis
        );

        assert_eq!(ColormapColorIndex::new(-1), None);
        assert_eq!(
            ColormapColorIndex::from_usize(i32::MAX as usize).map(ColormapColorIndex::raw),
            Some(i32::MAX)
        );
        assert_eq!(ColormapColorIndex::from_usize(i32::MAX as usize + 1), None);
    }

    #[test]
    #[should_panic(expected = "sample_colormap t must be between 0 and 1")]
    fn sample_colormap_rejects_out_of_range_t_before_ffi() {
        let _ = super::sample_colormap(-0.1);
    }

    #[test]
    fn next_plot_item_array_style_is_consumed_by_next_spec() {
        let line_colors = [0x01020304u32, 0x05060708];
        let fill_colors = [0x11121314u32];
        let marker_sizes = [2.0f32, 4.0, 8.0];

        with_next_plot_item_array_style(
            PlotItemArrayStyle::new()
                .with_line_colors(&line_colors)
                .with_fill_colors(&fill_colors)
                .with_marker_sizes(&marker_sizes),
            || {
                let layout =
                    PlotDataLayout::new(PlotDataOffset::samples(3), PlotDataStride::bytes(16));
                let spec = crate::plots::plot_spec_from(7, layout);
                assert_eq!(spec.Flags, 7);
                assert_eq!(spec.Offset, 3);
                assert_eq!(spec.Stride, 16);
                assert_eq!(spec.LineColors, line_colors.as_ptr() as *mut _);
                assert_eq!(spec.FillColors, fill_colors.as_ptr() as *mut _);
                assert_eq!(spec.MarkerSizes, marker_sizes.as_ptr() as *mut _);
            },
        );

        let spec = crate::plots::plot_spec_from(0, PlotDataLayout::DEFAULT);
        assert!(spec.LineColors.is_null());
        assert!(spec.FillColors.is_null());
        assert!(spec.MarkerSizes.is_null());
    }

    #[test]
    fn next_plot_item_array_style_is_restored_if_unused() {
        let line_colors = [0xAABBCCDDu32];

        with_next_plot_item_array_style(
            PlotItemArrayStyle::new().with_line_colors(&line_colors),
            || {},
        );

        let spec = crate::plots::plot_spec_from(0, PlotDataLayout::DEFAULT);
        assert!(spec.LineColors.is_null());
    }
}
