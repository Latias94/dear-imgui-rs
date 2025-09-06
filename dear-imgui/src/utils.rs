use crate::{sys, StyleColor, Ui};
use bitflags::bitflags;

bitflags! {
    /// Flags for hovering detection
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct HoveredFlags: i32 {
        /// Return true if directly over the item/window, not obstructed by another window, not obstructed by an active popup or modal blocking inputs under them.
        const NONE = sys::ImGuiHoveredFlags_None;
        /// IsWindowHovered() only: Return true if any children of the window is hovered
        const CHILD_WINDOWS = sys::ImGuiHoveredFlags_ChildWindows;
        /// IsWindowHovered() only: Test from root window (top most parent of the current hierarchy)
        const ROOT_WINDOW = sys::ImGuiHoveredFlags_RootWindow;
        /// IsWindowHovered() only: Return true if any window is hovered
        const ANY_WINDOW = sys::ImGuiHoveredFlags_AnyWindow;
        /// IsWindowHovered() only: Do not consider popup hierarchy (do not treat popup emitter as parent of popup) (when used with _ChildWindows or _RootWindow)
        const NO_POPUP_HIERARCHY = sys::ImGuiHoveredFlags_NoPopupHierarchy;
        /// IsWindowHovered() only: Consider docking hierarchy (treat dockspace host as parent of docked window) (when used with _ChildWindows or _RootWindow)
        const DOCK_HIERARCHY = sys::ImGuiHoveredFlags_DockHierarchy;
        /// Return true even if a popup window is normally blocking access to this item/window
        const ALLOW_WHEN_BLOCKED_BY_POPUP = sys::ImGuiHoveredFlags_AllowWhenBlockedByPopup;
        /// Return true even if an active item is blocking access to this item/window. Useful for Drag and Drop patterns.
        const ALLOW_WHEN_BLOCKED_BY_ACTIVE_ITEM = sys::ImGuiHoveredFlags_AllowWhenBlockedByActiveItem;
        /// IsItemHovered() only: Return true even if the position is obstructed or overlapped by another window
        const ALLOW_WHEN_OVERLAPPED = sys::ImGuiHoveredFlags_AllowWhenOverlapped;
        /// IsItemHovered() only: Return true even if the item is disabled
        const ALLOW_WHEN_DISABLED = sys::ImGuiHoveredFlags_AllowWhenDisabled;
        /// IsItemHovered() only: Disable using gamepad/keyboard navigation state when active, always query mouse.
        const NO_NAV_OVERRIDE = sys::ImGuiHoveredFlags_NoNavOverride;
        /// Shortcut for standard flags when using IsItemHovered() + SetTooltip() sequence.
        const FOR_TOOLTIP = sys::ImGuiHoveredFlags_ForTooltip;
        /// Require mouse to be stationary for style.HoverStationaryDelay (~0.15 sec) _at least one time_. After this, can move on same item/window. Using the stationary test tends to reduces the need for a long delay.
        const STATIONARY = sys::ImGuiHoveredFlags_Stationary;
        /// IsItemHovered() only: Return true immediately (default). As opposed to IsItemHovered() returning true only after style.HoverDelayNormal elapsed (~0.30 sec)
        const DELAY_NONE = sys::ImGuiHoveredFlags_DelayNone;
        /// IsItemHovered() only: Return true after style.HoverDelayShort elapsed (~0.10 sec)
        const DELAY_SHORT = sys::ImGuiHoveredFlags_DelayShort;
        /// IsItemHovered() only: Return true after style.HoverDelayNormal elapsed (~0.30 sec)
        const DELAY_NORMAL = sys::ImGuiHoveredFlags_DelayNormal;
        /// IsItemHovered() only: Disable shared delay system where moving from one item to a neighboring item keeps the previous timer for a short time (standard for tooltips with long delays)
        const NO_SHARED_DELAY = sys::ImGuiHoveredFlags_NoSharedDelay;
    }
}

impl Default for HoveredFlags {
    fn default() -> Self {
        HoveredFlags::NONE
    }
}

/// Utility functions for Dear ImGui
impl crate::ui::Ui {
    // ============================================================================
    // Item/widget utilities (additional functions not in tooltip.rs)
    // ============================================================================

    /// Returns `true` if the last item modified its underlying value this frame or was pressed
    #[doc(alias = "IsItemEdited")]
    pub fn is_item_edited(&self) -> bool {
        unsafe { sys::ImGui_IsItemEdited() }
    }

    /// Returns `true` if the last item open state was toggled
    #[doc(alias = "IsItemToggledOpen")]
    pub fn is_item_toggled_open(&self) -> bool {
        unsafe { sys::ImGui_IsItemToggledOpen() }
    }

    /// Returns the upper-left bounding rectangle of the last item (screen space)
    #[doc(alias = "GetItemRectMin")]
    pub fn item_rect_min(&self) -> [f32; 2] {
        unsafe {
            let rect = sys::ImGui_GetItemRectMin();
            [rect.x, rect.y]
        }
    }

    /// Returns the lower-right bounding rectangle of the last item (screen space)
    #[doc(alias = "GetItemRectMax")]
    pub fn item_rect_max(&self) -> [f32; 2] {
        unsafe {
            let rect = sys::ImGui_GetItemRectMax();
            [rect.x, rect.y]
        }
    }

    // ============================================================================
    // Window utilities
    // ============================================================================

    /// Returns `true` if the current window is hovered (and typically: not blocked by a popup/modal)
    #[doc(alias = "IsWindowHovered")]
    pub fn is_window_hovered(&self) -> bool {
        unsafe { sys::ImGui_IsWindowHovered(HoveredFlags::NONE.bits()) }
    }

    /// Returns `true` if the current window is hovered based on the given flags
    #[doc(alias = "IsWindowHovered")]
    pub fn is_window_hovered_with_flags(&self, flags: HoveredFlags) -> bool {
        unsafe { sys::ImGui_IsWindowHovered(flags.bits()) }
    }

    /// Returns `true` if the current window is focused (and typically: not blocked by a popup/modal)
    #[doc(alias = "IsWindowFocused")]
    pub fn is_window_focused(&self) -> bool {
        unsafe { sys::ImGui_IsWindowFocused(0) }
    }

    // ============================================================================
    // Utilities
    // ============================================================================

    /// Get global imgui time. Incremented by io.DeltaTime every frame.
    #[doc(alias = "GetTime")]
    pub fn time(&self) -> f64 {
        unsafe { sys::ImGui_GetTime() }
    }

    /// Get global imgui frame count. Incremented by 1 every frame.
    #[doc(alias = "GetFrameCount")]
    pub fn frame_count(&self) -> i32 {
        unsafe { sys::ImGui_GetFrameCount() }
    }

    /// Returns the name of a style color.
    ///
    /// This is just a wrapper around calling [`name`] on [StyleColor].
    ///
    /// [`name`]: StyleColor::name
    #[doc(alias = "GetStyleColorName")]
    pub fn style_color_name(&self, style_color: StyleColor) -> &'static str {
        unsafe {
            let name_ptr = sys::ImGui_GetStyleColorName(style_color as sys::ImGuiCol);
            let c_str = std::ffi::CStr::from_ptr(name_ptr);
            c_str.to_str().unwrap_or("Unknown")
        }
    }

    /// Test if rectangle (of given size, starting from cursor position) is visible / not clipped.
    #[doc(alias = "IsRectVisible")]
    pub fn is_rect_visible(&self, size: [f32; 2]) -> bool {
        unsafe {
            let size = sys::ImVec2 {
                x: size[0],
                y: size[1],
            };
            sys::ImGui_IsRectVisible(&size)
        }
    }

    /// Test if rectangle (in screen space) is visible / not clipped.
    #[doc(alias = "IsRectVisible")]
    pub fn is_rect_visible_ex(&self, rect_min: [f32; 2], rect_max: [f32; 2]) -> bool {
        unsafe {
            let rect_min = sys::ImVec2 {
                x: rect_min[0],
                y: rect_min[1],
            };
            let rect_max = sys::ImVec2 {
                x: rect_max[0],
                y: rect_max[1],
            };
            sys::ImGui_IsRectVisible1(&rect_min, &rect_max)
        }
    }

    // ========== Additional Geometry Functions ==========

    /// Get cursor position in screen coordinates.
    #[doc(alias = "GetCursorScreenPos")]
    pub fn get_cursor_screen_pos(&self) -> [f32; 2] {
        unsafe {
            let pos = sys::ImGui_GetCursorScreenPos();
            [pos.x, pos.y]
        }
    }

    /// Get available content region size.
    #[doc(alias = "GetContentRegionAvail")]
    pub fn get_content_region_avail(&self) -> [f32; 2] {
        unsafe {
            let size = sys::ImGui_GetContentRegionAvail();
            [size.x, size.y]
        }
    }

    /// Check if a point is inside a rectangle.
    pub fn is_point_in_rect(
        &self,
        point: [f32; 2],
        rect_min: [f32; 2],
        rect_max: [f32; 2],
    ) -> bool {
        point[0] >= rect_min[0]
            && point[0] <= rect_max[0]
            && point[1] >= rect_min[1]
            && point[1] <= rect_max[1]
    }

    /// Calculate distance between two points.
    pub fn distance(&self, p1: [f32; 2], p2: [f32; 2]) -> f32 {
        let dx = p2[0] - p1[0];
        let dy = p2[1] - p1[1];
        (dx * dx + dy * dy).sqrt()
    }

    /// Calculate squared distance between two points (faster than distance).
    pub fn distance_squared(&self, p1: [f32; 2], p2: [f32; 2]) -> f32 {
        let dx = p2[0] - p1[0];
        let dy = p2[1] - p1[1];
        dx * dx + dy * dy
    }

    /// Check if two line segments intersect.
    pub fn line_segments_intersect(
        &self,
        p1: [f32; 2],
        p2: [f32; 2],
        p3: [f32; 2],
        p4: [f32; 2],
    ) -> bool {
        let d1 = self.cross_product(
            [p4[0] - p3[0], p4[1] - p3[1]],
            [p1[0] - p3[0], p1[1] - p3[1]],
        );
        let d2 = self.cross_product(
            [p4[0] - p3[0], p4[1] - p3[1]],
            [p2[0] - p3[0], p2[1] - p3[1]],
        );
        let d3 = self.cross_product(
            [p2[0] - p1[0], p2[1] - p1[1]],
            [p3[0] - p1[0], p3[1] - p1[1]],
        );
        let d4 = self.cross_product(
            [p2[0] - p1[0], p2[1] - p1[1]],
            [p4[0] - p1[0], p4[1] - p1[1]],
        );

        (d1 > 0.0) != (d2 > 0.0) && (d3 > 0.0) != (d4 > 0.0)
    }

    /// Calculate cross product of two 2D vectors.
    fn cross_product(&self, v1: [f32; 2], v2: [f32; 2]) -> f32 {
        v1[0] * v2[1] - v1[1] * v2[0]
    }

    /// Normalize a 2D vector.
    pub fn normalize(&self, v: [f32; 2]) -> [f32; 2] {
        let len = (v[0] * v[0] + v[1] * v[1]).sqrt();
        if len > f32::EPSILON {
            [v[0] / len, v[1] / len]
        } else {
            [0.0, 0.0]
        }
    }

    /// Calculate dot product of two 2D vectors.
    pub fn dot_product(&self, v1: [f32; 2], v2: [f32; 2]) -> f32 {
        v1[0] * v2[0] + v1[1] * v2[1]
    }

    /// Calculate the angle between two vectors in radians.
    pub fn angle_between_vectors(&self, v1: [f32; 2], v2: [f32; 2]) -> f32 {
        let dot = self.dot_product(v1, v2);
        let len1 = (v1[0] * v1[0] + v1[1] * v1[1]).sqrt();
        let len2 = (v2[0] * v2[0] + v2[1] * v2[1]).sqrt();

        if len1 > f32::EPSILON && len2 > f32::EPSILON {
            (dot / (len1 * len2)).acos()
        } else {
            0.0
        }
    }

    /// Check if a point is inside a circle.
    pub fn is_point_in_circle(&self, point: [f32; 2], center: [f32; 2], radius: f32) -> bool {
        self.distance_squared(point, center) <= radius * radius
    }

    /// Calculate the area of a triangle given three points.
    pub fn triangle_area(&self, p1: [f32; 2], p2: [f32; 2], p3: [f32; 2]) -> f32 {
        let cross = self.cross_product(
            [p2[0] - p1[0], p2[1] - p1[1]],
            [p3[0] - p1[0], p3[1] - p1[1]],
        );
        cross.abs() * 0.5
    }
}
