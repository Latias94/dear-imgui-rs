use crate::sys;

bitflags::bitflags! {
    /// Flags for ImPlot3D plot configuration
    #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
    pub struct Plot3DFlags: u32 {
        const NONE        = sys::ImPlot3DFlags_None as u32;
        const NO_TITLE    = sys::ImPlot3DFlags_NoTitle as u32;
        const NO_LEGEND   = sys::ImPlot3DFlags_NoLegend as u32;
        const NO_MOUSE_TXT= sys::ImPlot3DFlags_NoMouseText as u32;
        const NO_CLIP     = sys::ImPlot3DFlags_NoClip as u32;
        const NO_MENUS    = sys::ImPlot3DFlags_NoMenus as u32;
        const CANVAS_ONLY = sys::ImPlot3DFlags_CanvasOnly as u32;
    }
}

bitflags::bitflags! {
    /// Triangle flags
    #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
    pub struct Triangle3DFlags: u32 {
        const NONE       = sys::ImPlot3DTriangleFlags_None as u32;
        const NO_LEGEND  = sys::ImPlot3DTriangleFlags_NoLegend as u32;
        const NO_FIT     = sys::ImPlot3DTriangleFlags_NoFit as u32;
        const NO_LINES   = sys::ImPlot3DTriangleFlags_NoLines as u32;
        const NO_FILL    = sys::ImPlot3DTriangleFlags_NoFill as u32;
        const NO_MARKERS = sys::ImPlot3DTriangleFlags_NoMarkers as u32;
    }
}

bitflags::bitflags! {
    /// Quad flags
    #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
    pub struct Quad3DFlags: u32 {
        const NONE       = sys::ImPlot3DQuadFlags_None as u32;
        const NO_LEGEND  = sys::ImPlot3DQuadFlags_NoLegend as u32;
        const NO_FIT     = sys::ImPlot3DQuadFlags_NoFit as u32;
        const NO_LINES   = sys::ImPlot3DQuadFlags_NoLines as u32;
        const NO_FILL    = sys::ImPlot3DQuadFlags_NoFill as u32;
        const NO_MARKERS = sys::ImPlot3DQuadFlags_NoMarkers as u32;
    }
}

bitflags::bitflags! {
    /// Surface flags
    #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
    pub struct Surface3DFlags: u32 {
        const NONE       = sys::ImPlot3DSurfaceFlags_None as u32;
        const NO_LEGEND  = sys::ImPlot3DSurfaceFlags_NoLegend as u32;
        const NO_FIT     = sys::ImPlot3DSurfaceFlags_NoFit as u32;
        const NO_LINES   = sys::ImPlot3DSurfaceFlags_NoLines as u32;
        const NO_FILL    = sys::ImPlot3DSurfaceFlags_NoFill as u32;
        const NO_MARKERS = sys::ImPlot3DSurfaceFlags_NoMarkers as u32;
    }
}

bitflags::bitflags! {
    /// Mesh flags
    #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
    pub struct Mesh3DFlags: u32 {
        const NONE       = sys::ImPlot3DMeshFlags_None as u32;
        const NO_LEGEND  = sys::ImPlot3DMeshFlags_NoLegend as u32;
        const NO_FIT     = sys::ImPlot3DMeshFlags_NoFit as u32;
        const NO_LINES   = sys::ImPlot3DMeshFlags_NoLines as u32;
        const NO_FILL    = sys::ImPlot3DMeshFlags_NoFill as u32;
        const NO_MARKERS = sys::ImPlot3DMeshFlags_NoMarkers as u32;
    }
}

bitflags::bitflags! {
    /// Image flags
    #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
    pub struct Image3DFlags: u32 {
        const NONE      = sys::ImPlot3DImageFlags_None as u32;
        const NO_LEGEND = sys::ImPlot3DImageFlags_NoLegend as u32;
        const NO_FIT    = sys::ImPlot3DImageFlags_NoFit as u32;
    }
}

bitflags::bitflags! {
    /// Item flags (common to plot items)
    #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
    pub struct Item3DFlags: u32 {
        const NONE      = sys::ImPlot3DItemFlags_None as u32;
        const NO_LEGEND = sys::ImPlot3DItemFlags_NoLegend as u32;
        const NO_FIT    = sys::ImPlot3DItemFlags_NoFit as u32;
    }
}

bitflags::bitflags! {
    /// Scatter flags
    #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
    pub struct Scatter3DFlags: u32 {
        const NONE      = sys::ImPlot3DScatterFlags_None as u32;
        const NO_LEGEND = sys::ImPlot3DScatterFlags_NoLegend as u32;
        const NO_FIT    = sys::ImPlot3DScatterFlags_NoFit as u32;
    }
}

bitflags::bitflags! {
    /// Line flags
    #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
    pub struct Line3DFlags: u32 {
        const NONE      = sys::ImPlot3DLineFlags_None as u32;
        const NO_LEGEND = sys::ImPlot3DLineFlags_NoLegend as u32;
        const NO_FIT    = sys::ImPlot3DLineFlags_NoFit as u32;
        const SEGMENTS  = sys::ImPlot3DLineFlags_Segments as u32;
        const LOOP      = sys::ImPlot3DLineFlags_Loop as u32;
        const SKIP_NAN  = sys::ImPlot3DLineFlags_SkipNaN as u32;
    }
}

bitflags::bitflags! {
    /// Axis flags (per-axis)
    #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
    pub struct Axis3DFlags: u32 {
        const NONE           = sys::ImPlot3DAxisFlags_None as u32;
        const NO_LABEL       = sys::ImPlot3DAxisFlags_NoLabel as u32;
        const NO_GRID_LINES  = sys::ImPlot3DAxisFlags_NoGridLines as u32;
        const NO_TICK_MARKS  = sys::ImPlot3DAxisFlags_NoTickMarks as u32;
        const NO_TICK_LABELS = sys::ImPlot3DAxisFlags_NoTickLabels as u32;
        const LOCK_MIN       = sys::ImPlot3DAxisFlags_LockMin as u32;
        const LOCK_MAX       = sys::ImPlot3DAxisFlags_LockMax as u32;
        const AUTO_FIT       = sys::ImPlot3DAxisFlags_AutoFit as u32;
        const INVERT         = sys::ImPlot3DAxisFlags_Invert as u32;
        const PAN_STRETCH    = sys::ImPlot3DAxisFlags_PanStretch as u32;
        const LOCK           = sys::ImPlot3DAxisFlags_Lock as u32;
        const NO_DECORATIONS = sys::ImPlot3DAxisFlags_NoDecorations as u32;
    }
}

#[repr(i32)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Marker3D {
    None = sys::ImPlot3DMarker_None,
    Circle = sys::ImPlot3DMarker_Circle,
    Square = sys::ImPlot3DMarker_Square,
    Diamond = sys::ImPlot3DMarker_Diamond,
    Up = sys::ImPlot3DMarker_Up,
    Down = sys::ImPlot3DMarker_Down,
    Left = sys::ImPlot3DMarker_Left,
    Right = sys::ImPlot3DMarker_Right,
    Cross = sys::ImPlot3DMarker_Cross,
    Plus = sys::ImPlot3DMarker_Plus,
    Asterisk = sys::ImPlot3DMarker_Asterisk,
}
/// 3D axis selector (X/Y/Z)
#[repr(i32)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Axis3D {
    // Bindgen does not expose ImAxis3D_X/Y/Z enumerators; they map to 0/1/2.
    X = 0,
    Y = 1,
    Z = 2,
}

/// Condition for setup calls (match ImGuiCond)
#[repr(i32)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Plot3DCond {
    None = sys::ImPlot3DCond_None,
    Always = sys::ImPlot3DCond_Always,
    Once = sys::ImPlot3DCond_Once,
}
