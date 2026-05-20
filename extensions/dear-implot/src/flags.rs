use crate::sys;

bitflags::bitflags! {
    /// Flags for ANY `PlotX` function. Used by setting `ImPlotSpec::Flags`.
    ///
    /// These flags can be composed with the plot-type-specific flags (e.g. `LineFlags`).
    #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
    pub struct ItemFlags: u32 {
        const NONE = sys::ImPlotItemFlags_None as u32;
        const NO_LEGEND = sys::ImPlotItemFlags_NoLegend as u32;
        const NO_FIT = sys::ImPlotItemFlags_NoFit as u32;
    }
}

bitflags::bitflags! {
    /// Flags for heatmap plots
    pub struct HeatmapFlags: u32 {
        const NONE = sys::ImPlotHeatmapFlags_None as u32;
        const COL_MAJOR = sys::ImPlotHeatmapFlags_ColMajor as u32;
    }
}

bitflags::bitflags! {
    /// Flags for histogram plots
    pub struct HistogramFlags: u32 {
        const NONE = sys::ImPlotHistogramFlags_None as u32;
        const HORIZONTAL = sys::ImPlotHistogramFlags_Horizontal as u32;
        const CUMULATIVE = sys::ImPlotHistogramFlags_Cumulative as u32;
        const DENSITY = sys::ImPlotHistogramFlags_Density as u32;
        const NO_OUTLIERS = sys::ImPlotHistogramFlags_NoOutliers as u32;
        const COL_MAJOR = sys::ImPlotHistogramFlags_ColMajor as u32;
    }
}

bitflags::bitflags! {
    /// Flags for pie chart plots
    pub struct PieChartFlags: u32 {
        const NONE = sys::ImPlotPieChartFlags_None as u32;
        const NORMALIZE = sys::ImPlotPieChartFlags_Normalize as u32;
        const IGNORE_HIDDEN = sys::ImPlotPieChartFlags_IgnoreHidden as u32;
        const EXPLODING = sys::ImPlotPieChartFlags_Exploding as u32;
        const NO_SLICE_BORDER = sys::ImPlotPieChartFlags_NoSliceBorder as u32;
    }
}

bitflags::bitflags! {
    /// Flags for line plots
    pub struct LineFlags: u32 {
        const NONE = sys::ImPlotLineFlags_None as u32;
        const SEGMENTS = sys::ImPlotLineFlags_Segments as u32;
        const LOOP = sys::ImPlotLineFlags_Loop as u32;
        const SKIP_NAN = sys::ImPlotLineFlags_SkipNaN as u32;
        const NO_CLIP = sys::ImPlotLineFlags_NoClip as u32;
        const SHADED = sys::ImPlotLineFlags_Shaded as u32;
    }
}

bitflags::bitflags! {
    /// Flags for polygon plots
    pub struct PolygonFlags: u32 {
        const NONE = sys::ImPlotPolygonFlags_None as u32;
        const CONCAVE = sys::ImPlotPolygonFlags_Concave as u32;
    }
}

bitflags::bitflags! {
    /// Flags for scatter plots
    pub struct ScatterFlags: u32 {
        const NONE = sys::ImPlotScatterFlags_None as u32;
        const NO_CLIP = sys::ImPlotScatterFlags_NoClip as u32;
    }
}

bitflags::bitflags! {
    /// Flags for bar plots
    pub struct BarsFlags: u32 {
        const NONE = sys::ImPlotBarsFlags_None as u32;
        const HORIZONTAL = sys::ImPlotBarsFlags_Horizontal as u32;
    }
}

bitflags::bitflags! {
    /// Flags for shaded plots
    pub struct ShadedFlags: u32 {
        const NONE = sys::ImPlotShadedFlags_None as u32;
    }
}

bitflags::bitflags! {
    /// Flags for stem plots
    pub struct StemsFlags: u32 {
        const NONE = sys::ImPlotStemsFlags_None as u32;
        const HORIZONTAL = sys::ImPlotStemsFlags_Horizontal as u32;
    }
}

bitflags::bitflags! {
    /// Flags for error bar plots
    pub struct ErrorBarsFlags: u32 {
        const NONE = sys::ImPlotErrorBarsFlags_None as u32;
        const HORIZONTAL = sys::ImPlotErrorBarsFlags_Horizontal as u32;
    }
}

bitflags::bitflags! {
    /// Flags for stairs plots
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct StairsFlags: u32 {
        const NONE = sys::ImPlotStairsFlags_None as u32;
        const PRE_STEP = sys::ImPlotStairsFlags_PreStep as u32;
        const SHADED = sys::ImPlotStairsFlags_Shaded as u32;
    }
}

bitflags::bitflags! {
    /// Flags for bar groups plots
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct BarGroupsFlags: u32 {
        const NONE = sys::ImPlotBarGroupsFlags_None as u32;
        const HORIZONTAL = sys::ImPlotBarGroupsFlags_Horizontal as u32;
        const STACKED = sys::ImPlotBarGroupsFlags_Stacked as u32;
    }
}

bitflags::bitflags! {
    /// Flags for digital plots
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct DigitalFlags: u32 {
        const NONE = sys::ImPlotDigitalFlags_None as u32;
    }
}

bitflags::bitflags! {
    /// Flags for text plots
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct TextFlags: u32 {
        const NONE = sys::ImPlotTextFlags_None as u32;
        const VERTICAL = sys::ImPlotTextFlags_Vertical as u32;
    }
}

bitflags::bitflags! {
    /// Flags for dummy plots
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct DummyFlags: u32 {
        const NONE = sys::ImPlotDummyFlags_None as u32;
    }
}

bitflags::bitflags! {
    /// Flags for drag tools (points/lines)
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct DragToolFlags: u32 {
        const NONE = sys::ImPlotDragToolFlags_None as u32;
        const NO_CURSORS = sys::ImPlotDragToolFlags_NoCursors as u32;
        const NO_FIT = sys::ImPlotDragToolFlags_NoFit as u32;
        const NO_INPUTS = sys::ImPlotDragToolFlags_NoInputs as u32;
        const DELAYED = sys::ImPlotDragToolFlags_Delayed as u32;
    }
}

bitflags::bitflags! {
    /// Flags for infinite lines plots
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct InfLinesFlags: u32 {
        const NONE = sys::ImPlotInfLinesFlags_None as u32;
        const HORIZONTAL = sys::ImPlotInfLinesFlags_Horizontal as u32;
    }
}

bitflags::bitflags! {
    /// Flags for image plots
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ImageFlags: u32 {
        const NONE = sys::ImPlotImageFlags_None as u32;
    }
}

bitflags::bitflags! {
    /// Axis flags matching ImPlotAxisFlags_ (see cimplot.h)
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct AxisFlags: u32 {
        const NONE           = sys::ImPlotAxisFlags_None as u32;
        const NO_LABEL       = sys::ImPlotAxisFlags_NoLabel as u32;
        const NO_GRID_LINES  = sys::ImPlotAxisFlags_NoGridLines as u32;
        const NO_TICK_MARKS  = sys::ImPlotAxisFlags_NoTickMarks as u32;
        const NO_TICK_LABELS = sys::ImPlotAxisFlags_NoTickLabels as u32;
        const NO_INITIAL_FIT = sys::ImPlotAxisFlags_NoInitialFit as u32;
        const NO_MENUS       = sys::ImPlotAxisFlags_NoMenus as u32;
        const NO_SIDE_SWITCH = sys::ImPlotAxisFlags_NoSideSwitch as u32;
        const NO_HIGHLIGHT   = sys::ImPlotAxisFlags_NoHighlight as u32;
        const OPPOSITE       = sys::ImPlotAxisFlags_Opposite as u32;
        const FOREGROUND     = sys::ImPlotAxisFlags_Foreground as u32;
        const INVERT         = sys::ImPlotAxisFlags_Invert as u32;
        const AUTO_FIT       = sys::ImPlotAxisFlags_AutoFit as u32;
        const RANGE_FIT      = sys::ImPlotAxisFlags_RangeFit as u32;
        const PAN_STRETCH    = sys::ImPlotAxisFlags_PanStretch as u32;
        const LOCK_MIN       = sys::ImPlotAxisFlags_LockMin as u32;
        const LOCK_MAX       = sys::ImPlotAxisFlags_LockMax as u32;
    }
}
