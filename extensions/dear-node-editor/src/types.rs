use crate::sys;
use bitflags::bitflags;
use std::ffi::CStr;

macro_rules! id_type {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[repr(transparent)]
        pub struct $name(pub usize);

        impl $name {
            #[inline]
            pub const fn new(value: usize) -> Self {
                Self(value)
            }

            #[inline]
            pub const fn raw(self) -> usize {
                self.0
            }

            #[inline]
            pub const fn is_null(self) -> bool {
                self.0 == 0
            }
        }

        impl From<usize> for $name {
            #[inline]
            fn from(value: usize) -> Self {
                Self(value)
            }
        }

        impl From<$name> for usize {
            #[inline]
            fn from(value: $name) -> Self {
                value.0
            }
        }
    };
}

id_type!(NodeId);
id_type!(PinId);
id_type!(LinkId);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PinKind {
    Input,
    Output,
}

impl PinKind {
    pub(crate) fn raw(self) -> sys::DnePinKind {
        match self {
            Self::Input => sys::DNE_PIN_KIND_INPUT,
            Self::Output => sys::DNE_PIN_KIND_OUTPUT,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlowDirection {
    Forward,
    Backward,
}

impl FlowDirection {
    pub(crate) fn raw(self) -> sys::DneFlowDirection {
        match self {
            Self::Forward => sys::DNE_FLOW_FORWARD,
            Self::Backward => sys::DNE_FLOW_BACKWARD,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CanvasSizeMode {
    FitVerticalView,
    FitHorizontalView,
    CenterOnly,
}

impl CanvasSizeMode {
    pub(crate) fn raw(self) -> sys::DneCanvasSizeMode {
        match self {
            Self::FitVerticalView => sys::DNE_CANVAS_SIZE_FIT_VERTICAL_VIEW,
            Self::FitHorizontalView => sys::DNE_CANVAS_SIZE_FIT_HORIZONTAL_VIEW,
            Self::CenterOnly => sys::DNE_CANVAS_SIZE_CENTER_ONLY,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StyleColor {
    Background,
    Grid,
    NodeBackground,
    NodeBorder,
    HoveredNodeBorder,
    SelectedNodeBorder,
    NodeSelectionRect,
    NodeSelectionRectBorder,
    HoveredLinkBorder,
    SelectedLinkBorder,
    HighlightLinkBorder,
    LinkSelectionRect,
    LinkSelectionRectBorder,
    PinRect,
    PinRectBorder,
    Flow,
    FlowMarker,
    GroupBackground,
    GroupBorder,
}

impl StyleColor {
    pub const COUNT: usize = sys::DNE_STYLE_COLOR_COUNT as usize;

    pub const ALL: [Self; Self::COUNT] = [
        Self::Background,
        Self::Grid,
        Self::NodeBackground,
        Self::NodeBorder,
        Self::HoveredNodeBorder,
        Self::SelectedNodeBorder,
        Self::NodeSelectionRect,
        Self::NodeSelectionRectBorder,
        Self::HoveredLinkBorder,
        Self::SelectedLinkBorder,
        Self::HighlightLinkBorder,
        Self::LinkSelectionRect,
        Self::LinkSelectionRectBorder,
        Self::PinRect,
        Self::PinRectBorder,
        Self::Flow,
        Self::FlowMarker,
        Self::GroupBackground,
        Self::GroupBorder,
    ];

    pub(crate) const fn raw(self) -> sys::DneStyleColor {
        match self {
            Self::Background => sys::DNE_STYLE_COLOR_BG,
            Self::Grid => sys::DNE_STYLE_COLOR_GRID,
            Self::NodeBackground => sys::DNE_STYLE_COLOR_NODE_BG,
            Self::NodeBorder => sys::DNE_STYLE_COLOR_NODE_BORDER,
            Self::HoveredNodeBorder => sys::DNE_STYLE_COLOR_HOVERED_NODE_BORDER,
            Self::SelectedNodeBorder => sys::DNE_STYLE_COLOR_SELECTED_NODE_BORDER,
            Self::NodeSelectionRect => sys::DNE_STYLE_COLOR_NODE_SELECTION_RECT,
            Self::NodeSelectionRectBorder => sys::DNE_STYLE_COLOR_NODE_SELECTION_RECT_BORDER,
            Self::HoveredLinkBorder => sys::DNE_STYLE_COLOR_HOVERED_LINK_BORDER,
            Self::SelectedLinkBorder => sys::DNE_STYLE_COLOR_SELECTED_LINK_BORDER,
            Self::HighlightLinkBorder => sys::DNE_STYLE_COLOR_HIGHLIGHT_LINK_BORDER,
            Self::LinkSelectionRect => sys::DNE_STYLE_COLOR_LINK_SELECTION_RECT,
            Self::LinkSelectionRectBorder => sys::DNE_STYLE_COLOR_LINK_SELECTION_RECT_BORDER,
            Self::PinRect => sys::DNE_STYLE_COLOR_PIN_RECT,
            Self::PinRectBorder => sys::DNE_STYLE_COLOR_PIN_RECT_BORDER,
            Self::Flow => sys::DNE_STYLE_COLOR_FLOW,
            Self::FlowMarker => sys::DNE_STYLE_COLOR_FLOW_MARKER,
            Self::GroupBackground => sys::DNE_STYLE_COLOR_GROUP_BG,
            Self::GroupBorder => sys::DNE_STYLE_COLOR_GROUP_BORDER,
        }
    }

    /// Returns the upstream style color name.
    #[doc(alias = "GetStyleColorName")]
    pub fn name(self) -> &'static str {
        unsafe {
            let ptr = sys::dne_get_style_color_name(self.raw());
            if ptr.is_null() {
                return "Unknown";
            }
            CStr::from_ptr(ptr).to_str().unwrap_or("Unknown")
        }
    }

    pub const fn index(self) -> usize {
        self.raw() as usize
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StyleVar {
    NodePadding,
    NodeRounding,
    NodeBorderWidth,
    HoveredNodeBorderWidth,
    SelectedNodeBorderWidth,
    PinRounding,
    PinBorderWidth,
    LinkStrength,
    SourceDirection,
    TargetDirection,
    ScrollDuration,
    FlowMarkerDistance,
    FlowSpeed,
    FlowDuration,
    PivotAlignment,
    PivotSize,
    PivotScale,
    PinCorners,
    PinRadius,
    PinArrowSize,
    PinArrowWidth,
    GroupRounding,
    GroupBorderWidth,
    HighlightConnectedLinks,
    SnapLinkToPinDir,
    HoveredNodeBorderOffset,
    SelectedNodeBorderOffset,
}

impl StyleVar {
    pub(crate) fn raw(self) -> sys::DneStyleVar {
        match self {
            Self::NodePadding => sys::DNE_STYLE_VAR_NODE_PADDING,
            Self::NodeRounding => sys::DNE_STYLE_VAR_NODE_ROUNDING,
            Self::NodeBorderWidth => sys::DNE_STYLE_VAR_NODE_BORDER_WIDTH,
            Self::HoveredNodeBorderWidth => sys::DNE_STYLE_VAR_HOVERED_NODE_BORDER_WIDTH,
            Self::SelectedNodeBorderWidth => sys::DNE_STYLE_VAR_SELECTED_NODE_BORDER_WIDTH,
            Self::PinRounding => sys::DNE_STYLE_VAR_PIN_ROUNDING,
            Self::PinBorderWidth => sys::DNE_STYLE_VAR_PIN_BORDER_WIDTH,
            Self::LinkStrength => sys::DNE_STYLE_VAR_LINK_STRENGTH,
            Self::SourceDirection => sys::DNE_STYLE_VAR_SOURCE_DIRECTION,
            Self::TargetDirection => sys::DNE_STYLE_VAR_TARGET_DIRECTION,
            Self::ScrollDuration => sys::DNE_STYLE_VAR_SCROLL_DURATION,
            Self::FlowMarkerDistance => sys::DNE_STYLE_VAR_FLOW_MARKER_DISTANCE,
            Self::FlowSpeed => sys::DNE_STYLE_VAR_FLOW_SPEED,
            Self::FlowDuration => sys::DNE_STYLE_VAR_FLOW_DURATION,
            Self::PivotAlignment => sys::DNE_STYLE_VAR_PIVOT_ALIGNMENT,
            Self::PivotSize => sys::DNE_STYLE_VAR_PIVOT_SIZE,
            Self::PivotScale => sys::DNE_STYLE_VAR_PIVOT_SCALE,
            Self::PinCorners => sys::DNE_STYLE_VAR_PIN_CORNERS,
            Self::PinRadius => sys::DNE_STYLE_VAR_PIN_RADIUS,
            Self::PinArrowSize => sys::DNE_STYLE_VAR_PIN_ARROW_SIZE,
            Self::PinArrowWidth => sys::DNE_STYLE_VAR_PIN_ARROW_WIDTH,
            Self::GroupRounding => sys::DNE_STYLE_VAR_GROUP_ROUNDING,
            Self::GroupBorderWidth => sys::DNE_STYLE_VAR_GROUP_BORDER_WIDTH,
            Self::HighlightConnectedLinks => sys::DNE_STYLE_VAR_HIGHLIGHT_CONNECTED_LINKS,
            Self::SnapLinkToPinDir => sys::DNE_STYLE_VAR_SNAP_LINK_TO_PIN_DIR,
            Self::HoveredNodeBorderOffset => sys::DNE_STYLE_VAR_HOVERED_NODE_BORDER_OFFSET,
            Self::SelectedNodeBorderOffset => sys::DNE_STYLE_VAR_SELECTED_NODE_BORDER_OFFSET,
        }
    }
}

bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct SaveReasonFlags: u32 {
        const NAVIGATION = sys::DNE_SAVE_REASON_NAVIGATION as u32;
        const POSITION = sys::DNE_SAVE_REASON_POSITION as u32;
        const SIZE = sys::DNE_SAVE_REASON_SIZE as u32;
        const SELECTION = sys::DNE_SAVE_REASON_SELECTION as u32;
        const ADD_NODE = sys::DNE_SAVE_REASON_ADD_NODE as u32;
        const REMOVE_NODE = sys::DNE_SAVE_REASON_REMOVE_NODE as u32;
        const USER = sys::DNE_SAVE_REASON_USER as u32;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn style_color_names_are_available() {
        assert_ne!(StyleColor::Background.name(), "Unknown");
        assert!(!StyleColor::Background.name().is_empty());
    }
}
