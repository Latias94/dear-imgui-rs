#![allow(clippy::unnecessary_cast)]
use crate::sys;
use std::fmt;

macro_rules! id_type {
    ($name:ident) => {
        #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[repr(transparent)]
        pub struct $name(i32);

        impl $name {
            #[inline]
            pub const fn new(value: i32) -> Self {
                Self(value)
            }

            #[inline]
            pub const fn raw(self) -> i32 {
                self.0
            }

            #[inline]
            pub const fn is_null(self) -> bool {
                self.0 == 0
            }
        }

        impl From<i32> for $name {
            #[inline]
            fn from(value: i32) -> Self {
                Self(value)
            }
        }

        impl From<$name> for i32 {
            #[inline]
            fn from(value: $name) -> Self {
                value.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }
    };
}

id_type!(NodeId);
id_type!(PinId);
id_type!(LinkId);

/// ImNodes calls input/output/static endpoints "attributes"; in this crate they share `PinId`.
pub type AttributeId = PinId;

#[repr(i32)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum PinShape {
    Circle = sys::ImNodesPinShape_Circle as i32,
    CircleFilled = sys::ImNodesPinShape_CircleFilled as i32,
    Triangle = sys::ImNodesPinShape_Triangle as i32,
    TriangleFilled = sys::ImNodesPinShape_TriangleFilled as i32,
    Quad = sys::ImNodesPinShape_Quad as i32,
    QuadFilled = sys::ImNodesPinShape_QuadFilled as i32,
}

bitflags::bitflags! {
    #[derive(Default)]
    pub struct AttributeFlags: i32 {
        const NONE = sys::ImNodesAttributeFlags_None as i32;
        const ENABLE_LINK_DETACH_WITH_DRAG_CLICK = sys::ImNodesAttributeFlags_EnableLinkDetachWithDragClick as i32;
        const ENABLE_LINK_CREATION_ON_SNAP = sys::ImNodesAttributeFlags_EnableLinkCreationOnSnap as i32;
    }
}

bitflags::bitflags! {
    #[derive(Default)]
    pub struct StyleFlags: i32 {
        const NONE = sys::ImNodesStyleFlags_None as i32;
        const NODE_OUTLINE = sys::ImNodesStyleFlags_NodeOutline as i32;
        const GRID_LINES = sys::ImNodesStyleFlags_GridLines as i32;
        const GRID_LINES_PRIMARY = sys::ImNodesStyleFlags_GridLinesPrimary as i32;
        const GRID_SNAPPING = sys::ImNodesStyleFlags_GridSnapping as i32;
    }
}

#[repr(u32)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum MiniMapLocation {
    BottomLeft = sys::ImNodesMiniMapLocation_BottomLeft as u32,
    BottomRight = sys::ImNodesMiniMapLocation_BottomRight as u32,
    TopLeft = sys::ImNodesMiniMapLocation_TopLeft as u32,
    TopRight = sys::ImNodesMiniMapLocation_TopRight as u32,
}

/// Result of a link creation interaction
#[derive(Copy, Clone, Debug)]
pub struct LinkCreated {
    pub start_attr: PinId,
    pub end_attr: PinId,
    pub from_snap: bool,
}

#[derive(Copy, Clone, Debug)]
pub struct LinkCreatedEx {
    pub start_node: NodeId,
    pub start_attr: PinId,
    pub end_node: NodeId,
    pub end_attr: PinId,
    pub from_snap: bool,
}
#[repr(i32)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum StyleVar {
    GridSpacing = sys::ImNodesStyleVar_GridSpacing as i32,
    NodeCornerRounding = sys::ImNodesStyleVar_NodeCornerRounding as i32,
    NodePadding = sys::ImNodesStyleVar_NodePadding as i32,
    NodeBorderThickness = sys::ImNodesStyleVar_NodeBorderThickness as i32,
    LinkThickness = sys::ImNodesStyleVar_LinkThickness as i32,
    LinkLineSegmentsPerLength = sys::ImNodesStyleVar_LinkLineSegmentsPerLength as i32,
    LinkHoverDistance = sys::ImNodesStyleVar_LinkHoverDistance as i32,
    PinCircleRadius = sys::ImNodesStyleVar_PinCircleRadius as i32,
    PinQuadSideLength = sys::ImNodesStyleVar_PinQuadSideLength as i32,
    PinTriangleSideLength = sys::ImNodesStyleVar_PinTriangleSideLength as i32,
    PinLineThickness = sys::ImNodesStyleVar_PinLineThickness as i32,
    PinHoverRadius = sys::ImNodesStyleVar_PinHoverRadius as i32,
    PinOffset = sys::ImNodesStyleVar_PinOffset as i32,
    MiniMapPadding = sys::ImNodesStyleVar_MiniMapPadding as i32,
    MiniMapOffset = sys::ImNodesStyleVar_MiniMapOffset as i32,
}
