use crate::input::{Key, MouseButton};
use crate::{StyleColor, sys};
use bitflags::bitflags;

bitflags! {
    /// Flags for hovering detection
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct HoveredFlags: i32 {
        /// Return true if directly over the item/window, not obstructed by another window, not obstructed by an active popup or modal blocking inputs under them.
        const NONE = sys::ImGuiHoveredFlags_None as i32;
        /// IsWindowHovered() only: Return true if any children of the window is hovered
        const CHILD_WINDOWS = sys::ImGuiHoveredFlags_ChildWindows as i32;
        /// IsWindowHovered() only: Test from root window (top most parent of the current hierarchy)
        const ROOT_WINDOW = sys::ImGuiHoveredFlags_RootWindow as i32;
        /// IsWindowHovered() only: Return true if any window is hovered
        const ANY_WINDOW = sys::ImGuiHoveredFlags_AnyWindow as i32;
        /// IsWindowHovered() only: Do not consider popup hierarchy (do not treat popup emitter as parent of popup) (when used with _ChildWindows or _RootWindow)
        const NO_POPUP_HIERARCHY = sys::ImGuiHoveredFlags_NoPopupHierarchy as i32;
        /// IsWindowHovered() only: Consider docking hierarchy (treat dockspace host as parent of docked window) (when used with _ChildWindows or _RootWindow)
        const DOCK_HIERARCHY = sys::ImGuiHoveredFlags_DockHierarchy as i32;
        /// Return true even if a popup window is normally blocking access to this item/window
        const ALLOW_WHEN_BLOCKED_BY_POPUP = sys::ImGuiHoveredFlags_AllowWhenBlockedByPopup as i32;
        /// Return true even if an active item is blocking access to this item/window. Useful for Drag and Drop patterns.
        const ALLOW_WHEN_BLOCKED_BY_ACTIVE_ITEM = sys::ImGuiHoveredFlags_AllowWhenBlockedByActiveItem as i32;
        /// IsItemHovered() only: Return true even if the position is obstructed or overlapped by another window
        const ALLOW_WHEN_OVERLAPPED = sys::ImGuiHoveredFlags_AllowWhenOverlapped as i32;
        /// IsItemHovered() only: Return true even if the item is disabled
        const ALLOW_WHEN_DISABLED = sys::ImGuiHoveredFlags_AllowWhenDisabled as i32;
        /// IsItemHovered() only: Disable using gamepad/keyboard navigation state when active, always query mouse.
        const NO_NAV_OVERRIDE = sys::ImGuiHoveredFlags_NoNavOverride as i32;
        /// Shortcut for standard flags when using IsItemHovered() + SetTooltip() sequence.
        const FOR_TOOLTIP = sys::ImGuiHoveredFlags_ForTooltip as i32;
        /// Require mouse to be stationary for style.HoverStationaryDelay (~0.15 sec) _at least one time_. After this, can move on same item/window. Using the stationary test tends to reduces the need for a long delay.
        const STATIONARY = sys::ImGuiHoveredFlags_Stationary as i32;
        /// IsItemHovered() only: Return true immediately (default). As opposed to IsItemHovered() returning true only after style.HoverDelayNormal elapsed (~0.30 sec)
        const DELAY_NONE = sys::ImGuiHoveredFlags_DelayNone as i32;
        /// IsItemHovered() only: Return true after style.HoverDelayShort elapsed (~0.10 sec)
        const DELAY_SHORT = sys::ImGuiHoveredFlags_DelayShort as i32;
        /// IsItemHovered() only: Return true after style.HoverDelayNormal elapsed (~0.30 sec)
        const DELAY_NORMAL = sys::ImGuiHoveredFlags_DelayNormal as i32;
        /// IsItemHovered() only: Disable shared delay system where moving from one item to a neighboring item keeps the previous timer for a short time (standard for tooltips with long delays)
        const NO_SHARED_DELAY = sys::ImGuiHoveredFlags_NoSharedDelay as i32;
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
    // Item/widget utilities (non-duplicate functions)
    // ============================================================================

    /// Returns `true` if the last item open state was toggled
    #[doc(alias = "IsItemToggledOpen")]
    pub fn is_item_toggled_open(&self) -> bool {
        unsafe { sys::igIsItemToggledOpen() }
    }

    /// Returns the upper-left bounding rectangle of the last item (screen space)
    #[doc(alias = "GetItemRectMin")]
    pub fn item_rect_min(&self) -> [f32; 2] {
        unsafe {
            let mut rect = sys::ImVec2 { x: 0.0, y: 0.0 };
            sys::igGetItemRectMin(&mut rect);
            [rect.x, rect.y]
        }
    }

    /// Returns the lower-right bounding rectangle of the last item (screen space)
    #[doc(alias = "GetItemRectMax")]
    pub fn item_rect_max(&self) -> [f32; 2] {
        unsafe {
            let mut rect = sys::ImVec2 { x: 0.0, y: 0.0 };
            sys::igGetItemRectMax(&mut rect);
            [rect.x, rect.y]
        }
    }

    // ============================================================================
    // Window utilities
    // ============================================================================

    /// Returns `true` if the current window is hovered (and typically: not blocked by a popup/modal)
    #[doc(alias = "IsWindowHovered")]
    pub fn is_window_hovered(&self) -> bool {
        unsafe { sys::igIsWindowHovered(HoveredFlags::NONE.bits()) }
    }

    /// Returns `true` if the current window is hovered based on the given flags
    #[doc(alias = "IsWindowHovered")]
    pub fn is_window_hovered_with_flags(&self, flags: HoveredFlags) -> bool {
        unsafe { sys::igIsWindowHovered(flags.bits()) }
    }

    /// Returns `true` if the current window is focused (and typically: not blocked by a popup/modal)
    #[doc(alias = "IsWindowFocused")]
    pub fn is_window_focused(&self) -> bool {
        unsafe { sys::igIsWindowFocused(0) }
    }

    /// Returns `true` if the current window is appearing this frame.
    #[doc(alias = "IsWindowAppearing")]
    pub fn is_window_appearing(&self) -> bool {
        unsafe { sys::igIsWindowAppearing() }
    }

    /// Returns `true` if the current window is collapsed.
    #[doc(alias = "IsWindowCollapsed")]
    pub fn is_window_collapsed(&self) -> bool {
        unsafe { sys::igIsWindowCollapsed() }
    }

    // ============================================================================
    // Additional input utilities (non-duplicate functions)
    // ============================================================================

    /// Returns the number of times the key was pressed in the current frame
    #[doc(alias = "GetKeyPressedAmount")]
    pub fn get_key_pressed_amount(&self, key: Key, repeat_delay: f32, rate: f32) -> i32 {
        unsafe { sys::igGetKeyPressedAmount(key as i32, repeat_delay, rate) }
    }

    /// Returns the name of a key
    #[doc(alias = "GetKeyName")]
    pub fn get_key_name(&self, key: Key) -> &str {
        unsafe {
            let name_ptr = sys::igGetKeyName(key as i32);
            let c_str = std::ffi::CStr::from_ptr(name_ptr);
            c_str.to_str().unwrap_or("Unknown")
        }
    }

    /// Returns the number of times the mouse button was clicked in the current frame
    #[doc(alias = "GetMouseClickedCount")]
    pub fn get_mouse_clicked_count(&self, button: MouseButton) -> i32 {
        unsafe { sys::igGetMouseClickedCount(button as i32) }
    }

    /// Returns the mouse position in screen coordinates
    #[doc(alias = "GetMousePos")]
    pub fn get_mouse_pos(&self) -> [f32; 2] {
        unsafe {
            #[cfg(target_env = "msvc")]
            {
                let mut pos = sys::ImVec2 { x: 0.0, y: 0.0 };
                sys::igGetMousePos(&mut pos);
                [pos.x, pos.y]
            }
            #[cfg(not(target_env = "msvc"))]
            {
                let mut pos = sys::ImVec2 { x: 0.0, y: 0.0 };
                sys::igGetMousePos(&mut pos);
                [pos.x, pos.y]
            }
        }
    }

    /// Returns the mouse position when the button was clicked
    #[doc(alias = "GetMousePosOnOpeningCurrentPopup")]
    pub fn get_mouse_pos_on_opening_current_popup(&self) -> [f32; 2] {
        unsafe {
            #[cfg(target_env = "msvc")]
            {
                let mut pos = sys::ImVec2 { x: 0.0, y: 0.0 };
                sys::igGetMousePosOnOpeningCurrentPopup(&mut pos);
                [pos.x, pos.y]
            }
            #[cfg(not(target_env = "msvc"))]
            {
                let mut pos = sys::ImVec2 { x: 0.0, y: 0.0 };
                sys::igGetMousePosOnOpeningCurrentPopup(&mut pos);
                [pos.x, pos.y]
            }
        }
    }

    /// Returns the mouse drag delta
    #[doc(alias = "GetMouseDragDelta")]
    pub fn get_mouse_drag_delta(&self, button: MouseButton, lock_threshold: f32) -> [f32; 2] {
        unsafe {
            #[cfg(target_env = "msvc")]
            {
                let mut delta = sys::ImVec2 { x: 0.0, y: 0.0 };
                sys::igGetMouseDragDelta(&mut delta, button as i32, lock_threshold);
                [delta.x, delta.y]
            }
            #[cfg(not(target_env = "msvc"))]
            {
                let mut delta = sys::ImVec2 { x: 0.0, y: 0.0 };
                sys::igGetMouseDragDelta(&mut delta, button as i32, lock_threshold);
                [delta.x, delta.y]
            }
        }
    }

    /// Returns the mouse wheel delta
    #[doc(alias = "GetIO")]
    pub fn get_mouse_wheel(&self) -> f32 {
        unsafe { (*sys::igGetIO_Nil()).MouseWheel }
    }

    /// Returns the horizontal mouse wheel delta
    #[doc(alias = "GetIO")]
    pub fn get_mouse_wheel_h(&self) -> f32 {
        unsafe { (*sys::igGetIO_Nil()).MouseWheelH }
    }

    /// Returns `true` if any mouse button is down
    #[doc(alias = "IsAnyMouseDown")]
    pub fn is_any_mouse_down(&self) -> bool {
        unsafe { sys::igIsAnyMouseDown() }
    }

    // ============================================================================
    // General utilities
    // ============================================================================

    /// Get global imgui time. Incremented by io.DeltaTime every frame.
    #[doc(alias = "GetTime")]
    pub fn time(&self) -> f64 {
        unsafe { sys::igGetTime() }
    }

    /// Get global imgui frame count. Incremented by 1 every frame.
    #[doc(alias = "GetFrameCount")]
    pub fn frame_count(&self) -> i32 {
        unsafe { sys::igGetFrameCount() }
    }

    /// Returns a single style color from the user interface style.
    ///
    /// Use this function if you need to access the colors, but don't want to clone the entire
    /// style object.
    #[doc(alias = "GetStyle")]
    pub fn style_color(&self, style_color: StyleColor) -> [f32; 4] {
        unsafe {
            let style_ptr = sys::igGetStyle();
            let colors = (*style_ptr).Colors.as_ptr();
            let color = *colors.add(style_color as usize);
            [color.x, color.y, color.z, color.w]
        }
    }

    /// Returns the name of a style color.
    ///
    /// This is just a wrapper around calling [`name`] on [StyleColor].
    ///
    /// [`name`]: StyleColor::name
    #[doc(alias = "GetStyleColorName")]
    pub fn style_color_name(&self, style_color: StyleColor) -> &'static str {
        unsafe {
            let name_ptr = sys::igGetStyleColorName(style_color as sys::ImGuiCol);
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
            sys::igIsRectVisible_Nil(size)
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
            sys::igIsRectVisible_Vec2(rect_min, rect_max)
        }
    }

    // ========== Additional Geometry Functions ==========

    /// Get cursor position in screen coordinates.
    #[doc(alias = "GetCursorScreenPos")]
    pub fn get_cursor_screen_pos(&self) -> [f32; 2] {
        unsafe {
            #[cfg(target_env = "msvc")]
            {
                let mut pos = sys::ImVec2 { x: 0.0, y: 0.0 };
                sys::igGetCursorScreenPos(&mut pos);
                [pos.x, pos.y]
            }
            #[cfg(not(target_env = "msvc"))]
            {
                let mut pos = sys::ImVec2 { x: 0.0, y: 0.0 };
                sys::igGetCursorScreenPos(&mut pos);
                [pos.x, pos.y]
            }
        }
    }

    /// Get available content region size.
    #[doc(alias = "GetContentRegionAvail")]
    pub fn get_content_region_avail(&self) -> [f32; 2] {
        unsafe {
            #[cfg(target_env = "msvc")]
            {
                let mut size = sys::ImVec2 { x: 0.0, y: 0.0 };
                sys::igGetContentRegionAvail(&mut size);
                [size.x, size.y]
            }
            #[cfg(not(target_env = "msvc"))]
            {
                let mut size = sys::ImVec2 { x: 0.0, y: 0.0 };
                sys::igGetContentRegionAvail(&mut size);
                [size.x, size.y]
            }
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

    // Additional utility functions

    /// Allows the next item to be overlapped by a subsequent item.
    #[doc(alias = "SetNextItemAllowOverlap")]
    pub fn set_next_item_allow_overlap(&self) {
        unsafe { sys::igSetNextItemAllowOverlap() };
    }
}
