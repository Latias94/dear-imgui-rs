// Plot flags and configuration
// This module contains plot and axis flags for configuration

// Plot flags for configuration
bitflags::bitflags! {
    /// Flags for plot configuration
    pub struct PlotFlags: u32 {
        const NONE = 0;
        const NO_TITLE = 1 << 0;
        const NO_LEGEND = 1 << 1;
        const NO_MOUSE_POS = 1 << 2;
        const NO_HIGHLIGHT = 1 << 3;
        const NO_CHILD = 1 << 4;
        const EQUAL = 1 << 5;
        const Y_AXIS_2 = 1 << 6;
        const Y_AXIS_3 = 1 << 7;
        const QUERY = 1 << 8;
        const CROSSHAIRS = 1 << 9;
        const ANTI_ALIASED = 1 << 10;
        const CANVAS_ONLY = 1 << 11;
    }
}

// Axis flags
bitflags::bitflags! {
    /// Flags for axis configuration
    pub struct AxisFlags: u32 {
        const NONE = 0;
        const NO_LABEL = 1 << 0;
        const NO_GRID_LINES = 1 << 1;
        const NO_TICK_MARKS = 1 << 2;
        const NO_TICK_LABELS = 1 << 3;
        const LOG_SCALE = 1 << 4;
        const TIME = 1 << 5;
        const INVERT = 1 << 6;
        const LOCK_MIN = 1 << 7;
        const LOCK_MAX = 1 << 8;
        const LOCK = Self::LOCK_MIN.bits() | Self::LOCK_MAX.bits();
    }
}
