use crate::sys;

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

/// A context style variable and its native value type.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Plot3DStyleVar {
    LineWeight(f32),
    MarkerSize(f32),
    FillAlpha(f32),
    ViewScaleFactor(f32),
    Marker(crate::Marker3D),
    PlotDefaultSize([f32; 2]),
    PlotMinSize([f32; 2]),
    PlotPadding([f32; 2]),
    LabelPadding([f32; 2]),
    LegendPadding([f32; 2]),
    LegendInnerPadding([f32; 2]),
    LegendSpacing([f32; 2]),
}

impl Plot3DStyleVar {
    // The caller has bound the ImPlot3D context. Each variant selects the overload named by
    // its entry in the native GPlot3DStyleVarInfo table.
    pub(super) unsafe fn push(self) {
        use Plot3DStyleVar::*;
        let scalar = |idx, value| unsafe { sys::ImPlot3D_PushStyleVar_Float(idx, value) };
        let vec2 = |idx, [x, y]: [f32; 2]| unsafe {
            sys::ImPlot3D_PushStyleVar_Vec2(idx, crate::imvec2(x, y))
        };
        match self {
            LineWeight(v) => scalar(sys::ImPlot3DStyleVar_LineWeight as _, v),
            MarkerSize(v) => scalar(sys::ImPlot3DStyleVar_MarkerSize as _, v),
            FillAlpha(v) => scalar(sys::ImPlot3DStyleVar_FillAlpha as _, v),
            ViewScaleFactor(v) => scalar(sys::ImPlot3DStyleVar_ViewScaleFactor as _, v),
            Marker(v) => unsafe {
                sys::ImPlot3D_PushStyleVar_Int(sys::ImPlot3DStyleVar_Marker as _, v as _)
            },
            PlotDefaultSize(v) => vec2(sys::ImPlot3DStyleVar_PlotDefaultSize as _, v),
            PlotMinSize(v) => vec2(sys::ImPlot3DStyleVar_PlotMinSize as _, v),
            PlotPadding(v) => vec2(sys::ImPlot3DStyleVar_PlotPadding as _, v),
            LabelPadding(v) => vec2(sys::ImPlot3DStyleVar_LabelPadding as _, v),
            LegendPadding(v) => vec2(sys::ImPlot3DStyleVar_LegendPadding as _, v),
            LegendInnerPadding(v) => vec2(sys::ImPlot3DStyleVar_LegendInnerPadding as _, v),
            LegendSpacing(v) => vec2(sys::ImPlot3DStyleVar_LegendSpacing as _, v),
        }
    }
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
        match ColormapIndex::from_raw(self as i32) {
            Some(index) => index,
            None => panic!("built-in ImPlot3D colormap index must be valid"),
        }
    }
}

/// Runtime ImPlot3D colormap index.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct ColormapIndex(i32);

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
        Self::new(value).expect("colormap index exceeded ImPlot3D's i32 range")
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
        Self::new(value).expect("colormap color index exceeded ImPlot3D's i32 range")
    }
}
