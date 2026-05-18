use crate::flags::Marker3D;
use crate::sys;
use std::borrow::Cow;
use std::marker::PhantomData;
use std::rc::Rc;

/// Colorable ImPlot3D style elements.
#[repr(i32)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Plot3DColorElement {
    TitleText = sys::ImPlot3DCol_TitleText as i32,
    InlayText = sys::ImPlot3DCol_InlayText as i32,
    FrameBg = sys::ImPlot3DCol_FrameBg as i32,
    PlotBg = sys::ImPlot3DCol_PlotBg as i32,
    PlotBorder = sys::ImPlot3DCol_PlotBorder as i32,
    LegendBg = sys::ImPlot3DCol_LegendBg as i32,
    LegendBorder = sys::ImPlot3DCol_LegendBorder as i32,
    LegendText = sys::ImPlot3DCol_LegendText as i32,
    AxisText = sys::ImPlot3DCol_AxisText as i32,
    AxisGrid = sys::ImPlot3DCol_AxisGrid as i32,
    AxisTick = sys::ImPlot3DCol_AxisTick as i32,
    AxisBg = sys::ImPlot3DCol_AxisBg as i32,
    AxisBgHovered = sys::ImPlot3DCol_AxisBgHovered as i32,
    AxisBgActive = sys::ImPlot3DCol_AxisBgActive as i32,
}

/// ImPlot3D style variables.
#[repr(i32)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Plot3DStyleVar {
    LineWeight = sys::ImPlot3DStyleVar_LineWeight as i32,
    Marker = sys::ImPlot3DStyleVar_Marker as i32,
    MarkerSize = sys::ImPlot3DStyleVar_MarkerSize as i32,
    FillAlpha = sys::ImPlot3DStyleVar_FillAlpha as i32,
    PlotDefaultSize = sys::ImPlot3DStyleVar_PlotDefaultSize as i32,
    PlotMinSize = sys::ImPlot3DStyleVar_PlotMinSize as i32,
    PlotPadding = sys::ImPlot3DStyleVar_PlotPadding as i32,
    LabelPadding = sys::ImPlot3DStyleVar_LabelPadding as i32,
    ViewScaleFactor = sys::ImPlot3DStyleVar_ViewScaleFactor as i32,
    LegendPadding = sys::ImPlot3DStyleVar_LegendPadding as i32,
    LegendInnerPadding = sys::ImPlot3DStyleVar_LegendInnerPadding as i32,
    LegendSpacing = sys::ImPlot3DStyleVar_LegendSpacing as i32,
}

/// Built-in ImPlot3D colormaps.
#[repr(i32)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Colormap {
    Deep = sys::ImPlot3DColormap_Deep as i32,
    Dark = sys::ImPlot3DColormap_Dark as i32,
    Pastel = sys::ImPlot3DColormap_Pastel as i32,
    Paired = sys::ImPlot3DColormap_Paired as i32,
    Viridis = sys::ImPlot3DColormap_Viridis as i32,
    Plasma = sys::ImPlot3DColormap_Plasma as i32,
    Hot = sys::ImPlot3DColormap_Hot as i32,
    Cool = sys::ImPlot3DColormap_Cool as i32,
    Pink = sys::ImPlot3DColormap_Pink as i32,
    Jet = sys::ImPlot3DColormap_Jet as i32,
    Twilight = sys::ImPlot3DColormap_Twilight as i32,
    RdBu = sys::ImPlot3DColormap_RdBu as i32,
    BrBG = sys::ImPlot3DColormap_BrBG as i32,
    PiYG = sys::ImPlot3DColormap_PiYG as i32,
    Spectral = sys::ImPlot3DColormap_Spectral as i32,
    Greys = sys::ImPlot3DColormap_Greys as i32,
}

impl Colormap {
    #[inline]
    pub const fn index(self) -> ColormapIndex {
        ColormapIndex(self as i32)
    }
}

/// Runtime ImPlot3D colormap index.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct ColormapIndex(i32);

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

/// Token for managing style variable changes.
#[must_use]
pub struct StyleVarToken {
    was_popped: bool,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl StyleVarToken {
    /// Pop this style variable from the stack.
    pub fn pop(mut self) {
        if self.was_popped {
            panic!("Attempted to pop an ImPlot3D style var token twice.");
        }
        self.was_popped = true;
        unsafe {
            sys::ImPlot3D_PopStyleVar(1);
        }
    }
}

impl Drop for StyleVarToken {
    fn drop(&mut self) {
        if !self.was_popped {
            unsafe {
                sys::ImPlot3D_PopStyleVar(1);
            }
        }
    }
}

/// Token for managing style color changes.
#[must_use]
pub struct StyleColorToken {
    was_popped: bool,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl StyleColorToken {
    /// Pop this style color from the stack.
    pub fn pop(mut self) {
        if self.was_popped {
            panic!("Attempted to pop an ImPlot3D style color token twice.");
        }
        self.was_popped = true;
        unsafe {
            sys::ImPlot3D_PopStyleColor(1);
        }
    }
}

impl Drop for StyleColorToken {
    fn drop(&mut self) {
        if !self.was_popped {
            unsafe {
                sys::ImPlot3D_PopStyleColor(1);
            }
        }
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
            panic!("Attempted to pop an ImPlot3D colormap token twice.");
        }
        self.was_popped = true;
        unsafe {
            sys::ImPlot3D_PopColormap(1);
        }
    }
}

impl Drop for ColormapToken {
    fn drop(&mut self) {
        if !self.was_popped {
            unsafe {
                sys::ImPlot3D_PopColormap(1);
            }
        }
    }
}

#[inline]
pub fn style_colors_dark() {
    unsafe { sys::ImPlot3D_StyleColorsDark(std::ptr::null_mut()) }
}
#[inline]
pub fn style_colors_light() {
    unsafe { sys::ImPlot3D_StyleColorsLight(std::ptr::null_mut()) }
}
#[inline]
pub fn style_colors_classic() {
    unsafe { sys::ImPlot3D_StyleColorsClassic(std::ptr::null_mut()) }
}
#[inline]
pub fn style_colors_auto() {
    unsafe { sys::ImPlot3D_StyleColorsAuto(std::ptr::null_mut()) }
}

#[inline]
pub fn push_style_color(element: Plot3DColorElement, col: [f32; 4]) -> StyleColorToken {
    unsafe {
        sys::ImPlot3D_PushStyleColor_Vec4(
            element as sys::ImPlot3DCol_,
            crate::imvec4(col[0], col[1], col[2], col[3]),
        );
    }
    StyleColorToken {
        was_popped: false,
        _not_send_or_sync: PhantomData,
    }
}

/// Push a typed style color override.
#[inline]
pub fn push_style_color_element(element: Plot3DColorElement, col: [f32; 4]) -> StyleColorToken {
    push_style_color(element, col)
}

/// Push a style variable (float variant)
#[inline]
pub fn push_style_var_f32(var: Plot3DStyleVar, val: f32) -> StyleVarToken {
    unsafe { sys::ImPlot3D_PushStyleVar_Float(var as sys::ImPlot3DStyleVar, val) }
    StyleVarToken {
        was_popped: false,
        _not_send_or_sync: PhantomData,
    }
}

/// Push a style variable (int variant)
#[inline]
pub fn push_style_var_i32(var: Plot3DStyleVar, val: i32) -> StyleVarToken {
    unsafe { sys::ImPlot3D_PushStyleVar_Int(var as sys::ImPlot3DStyleVar, val) }
    StyleVarToken {
        was_popped: false,
        _not_send_or_sync: PhantomData,
    }
}

/// Push a style variable (Vec2 variant)
#[inline]
pub fn push_style_var_vec2(var: Plot3DStyleVar, val: [f32; 2]) -> StyleVarToken {
    unsafe {
        sys::ImPlot3D_PushStyleVar_Vec2(var as sys::ImPlot3DStyleVar, crate::imvec2(val[0], val[1]))
    }
    StyleVarToken {
        was_popped: false,
        _not_send_or_sync: PhantomData,
    }
}

#[inline]
pub fn set_next_line_style(col: [f32; 4], weight: f32) {
    crate::update_next_plot3d_spec(|spec| {
        spec.LineColor = crate::imvec4(col[0], col[1], col[2], col[3]);
        spec.LineWeight = weight;
    })
}

#[inline]
pub fn set_next_fill_style(col: [f32; 4], alpha_mod: f32) {
    crate::update_next_plot3d_spec(|spec| {
        spec.FillColor = crate::imvec4(col[0], col[1], col[2], col[3]);
        spec.FillAlpha = alpha_mod;
    })
}

#[inline]
pub fn set_next_marker_style(
    marker: Marker3D,
    size: f32,
    fill: [f32; 4],
    weight: f32,
    outline: [f32; 4],
) {
    crate::update_next_plot3d_spec(|spec| {
        spec.Marker = marker as sys::ImPlot3DMarker;
        spec.MarkerSize = size;
        spec.MarkerFillColor = crate::imvec4(fill[0], fill[1], fill[2], fill[3]);
        spec.MarkerLineColor = crate::imvec4(outline[0], outline[1], outline[2], outline[3]);
        spec.LineWeight = weight;
    })
}

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

#[inline]
pub fn push_colormap(cmap: impl Into<ColormapIndex>) -> ColormapToken {
    unsafe { sys::ImPlot3D_PushColormap_Plot3DColormap(cmap.into().raw()) }
    ColormapToken {
        was_popped: false,
        _not_send_or_sync: PhantomData,
    }
}

#[inline]
pub fn push_colormap_name(name: &str) -> ColormapToken {
    dear_imgui_rs::with_scratch_txt(name, |ptr| unsafe { sys::ImPlot3D_PushColormap_Str(ptr) });
    ColormapToken {
        was_popped: false,
        _not_send_or_sync: PhantomData,
    }
}
#[inline]
pub fn colormap_count() -> i32 {
    unsafe { sys::ImPlot3D_GetColormapCount() }
}
#[inline]
pub fn colormap_name(index: impl Into<ColormapIndex>) -> String {
    unsafe {
        let p = sys::ImPlot3D_GetColormapName(index.into().raw());
        if p.is_null() {
            return String::new();
        }
        std::ffi::CStr::from_ptr(p).to_string_lossy().into_owned()
    }
}

/// Get number of keys (colors) in a given colormap index
#[inline]
pub fn colormap_size(index: impl Into<ColormapIndex>) -> i32 {
    unsafe { sys::ImPlot3D_GetColormapSize(index.into().raw()) }
}

/// Get current default colormap index set in ImPlot3D style
#[inline]
pub fn get_style_colormap_index() -> Option<ColormapIndex> {
    unsafe {
        let style = sys::ImPlot3D_GetStyle();
        if style.is_null() {
            return None;
        }
        ColormapIndex::new((*style).Colormap)
    }
}

/// Get current default colormap name (if index valid)
#[inline]
pub fn get_style_colormap_name() -> Option<String> {
    let idx = get_style_colormap_index()?;
    let count = colormap_count();
    if idx.raw() >= count {
        return None;
    }
    Some(colormap_name(idx))
}

/// Permanently set the default colormap used by ImPlot3D (persists across plots/frames)
#[inline]
pub fn set_style_colormap(index: impl Into<ColormapIndex>) {
    unsafe {
        let style = sys::ImPlot3D_GetStyle();
        if !style.is_null() {
            let count = sys::ImPlot3D_GetColormapCount();
            if count > 0 {
                let index = index.into().raw();
                let idx = if index >= count { count - 1 } else { index };
                (*style).Colormap = idx;
            }
        }
    }
}

/// Look up a colormap index by its name.
#[inline]
pub fn colormap_index_by_name(name: &str) -> Option<ColormapIndex> {
    if name.contains('\0') {
        return None;
    }
    let index =
        dear_imgui_rs::with_scratch_txt(name, |ptr| unsafe { sys::ImPlot3D_GetColormapIndex(ptr) });
    ColormapIndex::new(index)
}

/// Convenience: set default colormap by name (no-op if name is invalid)
#[inline]
pub fn set_style_colormap_by_name(name: &str) {
    if let Some(idx) = colormap_index_by_name(name) {
        set_style_colormap(idx);
    }
}

/// Get a color from the current colormap at index
pub fn get_colormap_color(index: ColormapColorIndex) -> [f32; 4] {
    unsafe {
        // Pass -1 for "current" colormap (upstream convention)
        let out = crate::compat_ffi::ImPlot3D_GetColormapColor(
            index.raw(),
            (-1) as sys::ImPlot3DColormap,
        );
        [out.x, out.y, out.z, out.w]
    }
}

/// Get next colormap color (advances internal counter)
pub fn next_colormap_color() -> [f32; 4] {
    unsafe {
        let out = crate::compat_ffi::ImPlot3D_NextColormapColor();
        [out.x, out.y, out.z, out.w]
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Colormap, ColormapColorIndex, ColormapIndex, Plot3DItemArrayStyle,
        with_next_plot3d_item_array_style,
    };

    #[test]
    fn colormap_indices_reject_negative_values() {
        assert_eq!(ColormapIndex::new(-1), None);
        assert_eq!(ColormapIndex::new(0).map(ColormapIndex::raw), Some(0));
        assert_eq!(
            ColormapIndex::from(Colormap::Viridis).raw(),
            crate::sys::ImPlot3DColormap_Viridis
        );

        assert_eq!(ColormapColorIndex::new(-1), None);
        assert_eq!(
            ColormapColorIndex::from_usize(i32::MAX as usize).map(ColormapColorIndex::raw),
            Some(i32::MAX)
        );
        assert_eq!(ColormapColorIndex::from_usize(i32::MAX as usize + 1), None);
    }

    #[test]
    fn next_plot3d_item_array_style_is_consumed_by_next_spec() {
        let line_colors = [0x01020304u32, 0x05060708];
        let marker_sizes = [1.5f32, 2.5];
        let marker_fill_colors = [0x11223344u32];

        with_next_plot3d_item_array_style(
            Plot3DItemArrayStyle::new()
                .with_line_colors(&line_colors)
                .with_marker_sizes(&marker_sizes)
                .with_marker_fill_colors(&marker_fill_colors),
            || {
                let spec = crate::plot3d_spec_from(9, 2, 24);
                assert_eq!(spec.Flags, 9);
                assert_eq!(spec.Offset, 2);
                assert_eq!(spec.Stride, 24);
                assert_eq!(spec.LineColors, line_colors.as_ptr() as *mut _);
                assert_eq!(spec.MarkerSizes, marker_sizes.as_ptr() as *mut _);
                assert_eq!(spec.MarkerFillColors, marker_fill_colors.as_ptr() as *mut _);
            },
        );

        let spec = crate::plot3d_spec_from(0, 0, -1);
        assert!(spec.LineColors.is_null());
        assert!(spec.MarkerSizes.is_null());
        assert!(spec.MarkerFillColors.is_null());
    }

    #[test]
    fn next_plot3d_item_array_style_is_restored_if_unused() {
        let fill_colors = [0xAABBCCDDu32];

        with_next_plot3d_item_array_style(
            Plot3DItemArrayStyle::new().with_fill_colors(&fill_colors),
            || {},
        );

        let spec = crate::plot3d_spec_from(0, 0, -1);
        assert!(spec.FillColors.is_null());
    }
}
