use crate::sys;

pub(crate) const IMPLOT_AUTO: i32 = -1;

/// Choice of Y axis for multi-axis plots
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum YAxisChoice {
    First = 0,
    Second = 1,
    Third = 2,
}

/// Convert an Option<YAxisChoice> into an i32. Picks IMPLOT_AUTO for None.
pub(crate) fn y_axis_choice_option_to_i32(y_axis_choice: Option<YAxisChoice>) -> i32 {
    match y_axis_choice {
        Some(choice) => choice as i32,
        None => IMPLOT_AUTO,
    }
}

/// X axis selector matching ImPlot's ImAxis values
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum XAxis {
    X1 = 0,
    X2 = 1,
    X3 = 2,
}

/// Y axis selector matching ImPlot's ImAxis values
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum YAxis {
    Y1 = 3,
    Y2 = 4,
    Y3 = 5,
}

impl YAxis {
    /// Convert a Y axis (Y1..Y3) to the 0-based index used by ImPlotPlot_YAxis_Nil
    pub(crate) fn to_index(self) -> i32 {
        (self as i32) - 3
    }
}

/// Any ImPlot axis selector matching ImPlot's ImAxis values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum Axis {
    X1 = 0,
    X2 = 1,
    X3 = 2,
    Y1 = 3,
    Y2 = 4,
    Y3 = 5,
}

impl Axis {
    pub(crate) fn to_sys(self) -> sys::ImAxis {
        self as sys::ImAxis
    }
}

impl From<XAxis> for Axis {
    fn from(axis: XAxis) -> Self {
        match axis {
            XAxis::X1 => Self::X1,
            XAxis::X2 => Self::X2,
            XAxis::X3 => Self::X3,
        }
    }
}

impl From<YAxis> for Axis {
    fn from(axis: YAxis) -> Self {
        match axis {
            YAxis::Y1 => Self::Y1,
            YAxis::Y2 => Self::Y2,
            YAxis::Y3 => Self::Y3,
        }
    }
}
