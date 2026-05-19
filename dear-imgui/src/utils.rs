//! Miscellaneous utilities
//!
//! Helper flags and `Ui` extension methods for common queries (hovered/focused
//! checks, item rectangles, etc.). These are thin wrappers around Dear ImGui
//! functions for convenience and type safety.
//!
use crate::input::{Key, MouseButton};
use crate::{StyleColor, sys};
use bitflags::bitflags;

fn non_negative_count_from_i32(caller: &str, raw: i32) -> usize {
    usize::try_from(raw).unwrap_or_else(|_| panic!("{caller} returned a negative count"))
}

/// Auto-open depth for Dear ImGui logging helpers.
///
/// Dear ImGui uses `-1` to mean "use the configured default depth". This type keeps that sentinel
/// out of the safe Rust API while still allowing an explicit non-negative tree depth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LogAutoOpenDepth(Option<u32>);

impl LogAutoOpenDepth {
    /// Use Dear ImGui's configured default log auto-open depth.
    pub const DEFAULT: Self = Self(None);

    /// Create an explicit non-negative auto-open depth.
    ///
    /// Panics if `depth` exceeds Dear ImGui's signed `int` range.
    #[inline]
    pub const fn new(depth: u32) -> Self {
        assert!(
            depth <= i32::MAX as u32,
            "LogAutoOpenDepth::new() depth exceeded i32::MAX"
        );
        Self(Some(depth))
    }

    #[inline]
    pub(crate) const fn raw(self) -> i32 {
        match self.0 {
            Some(depth) => depth as i32,
            None => -1,
        }
    }
}

impl From<u32> for LogAutoOpenDepth {
    fn from(depth: u32) -> Self {
        Self::new(depth)
    }
}

bitflags! {
    /// Flags accepted by `Ui::is_window_hovered_with_flags()`.
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct WindowHoveredFlags: i32 {
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
        /// IsWindowHovered() only: Shortcut for `ROOT_WINDOW | CHILD_WINDOWS`.
        const ROOT_AND_CHILD_WINDOWS = sys::ImGuiHoveredFlags_RootAndChildWindows as i32;
        /// Shortcut for standard flags when using IsWindowHovered() + tooltip-style hover behavior.
        const FOR_TOOLTIP = sys::ImGuiHoveredFlags_ForTooltip as i32;
        /// Require mouse to be stationary for style.HoverStationaryDelay (~0.15 sec) at least one time.
        const STATIONARY = sys::ImGuiHoveredFlags_Stationary as i32;
    }
}

impl Default for WindowHoveredFlags {
    fn default() -> Self {
        WindowHoveredFlags::NONE
    }
}

bitflags! {
    /// Flags accepted by `Ui::is_item_hovered_with_flags()`.
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct ItemHoveredFlags: i32 {
        /// Return true if directly over the item, not obstructed by another window, not obstructed by an active popup or modal blocking inputs under it.
        const NONE = sys::ImGuiHoveredFlags_None as i32;
        /// Return true even if a popup window is normally blocking access to this item/window
        const ALLOW_WHEN_BLOCKED_BY_POPUP = sys::ImGuiHoveredFlags_AllowWhenBlockedByPopup as i32;
        /// Return true even if an active item is blocking access to this item/window. Useful for Drag and Drop patterns.
        const ALLOW_WHEN_BLOCKED_BY_ACTIVE_ITEM = sys::ImGuiHoveredFlags_AllowWhenBlockedByActiveItem as i32;
        /// IsItemHovered() only: Return true even if the item uses AllowOverlap mode and is overlapped by another hoverable item.
        const ALLOW_WHEN_OVERLAPPED_BY_ITEM = sys::ImGuiHoveredFlags_AllowWhenOverlappedByItem as i32;
        /// IsItemHovered() only: Return true even if the item position is overlapped by another window.
        const ALLOW_WHEN_OVERLAPPED_BY_WINDOW = sys::ImGuiHoveredFlags_AllowWhenOverlappedByWindow as i32;
        /// IsItemHovered() only: Return true even if the position is obstructed or overlapped by another window
        const ALLOW_WHEN_OVERLAPPED = sys::ImGuiHoveredFlags_AllowWhenOverlapped as i32;
        /// IsItemHovered() only: Return true even if the item is disabled
        const ALLOW_WHEN_DISABLED = sys::ImGuiHoveredFlags_AllowWhenDisabled as i32;
        /// IsItemHovered() only: Disable using gamepad/keyboard navigation state when active, always query mouse.
        const NO_NAV_OVERRIDE = sys::ImGuiHoveredFlags_NoNavOverride as i32;
        /// IsItemHovered() only: test rectangle visibility with popup/active-item/overlap bypasses.
        const RECT_ONLY = sys::ImGuiHoveredFlags_RectOnly as i32;
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

impl Default for ItemHoveredFlags {
    fn default() -> Self {
        ItemHoveredFlags::NONE
    }
}

bitflags! {
    /// Flags stored in style tooltip hover defaults.
    ///
    /// This is the item-hover subset that can be expanded by
    /// `ItemHoveredFlags::FOR_TOOLTIP`; it intentionally excludes `FOR_TOOLTIP`
    /// itself to avoid recursive tooltip-style storage.
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct TooltipHoveredFlags: i32 {
        /// No flags
        const NONE = sys::ImGuiHoveredFlags_None as i32;
        /// Return true even if a popup window is normally blocking access to this item/window
        const ALLOW_WHEN_BLOCKED_BY_POPUP = sys::ImGuiHoveredFlags_AllowWhenBlockedByPopup as i32;
        /// Return true even if an active item is blocking access to this item/window. Useful for Drag and Drop patterns.
        const ALLOW_WHEN_BLOCKED_BY_ACTIVE_ITEM = sys::ImGuiHoveredFlags_AllowWhenBlockedByActiveItem as i32;
        /// Return true even if the item uses AllowOverlap mode and is overlapped by another hoverable item.
        const ALLOW_WHEN_OVERLAPPED_BY_ITEM = sys::ImGuiHoveredFlags_AllowWhenOverlappedByItem as i32;
        /// Return true even if the item position is overlapped by another window.
        const ALLOW_WHEN_OVERLAPPED_BY_WINDOW = sys::ImGuiHoveredFlags_AllowWhenOverlappedByWindow as i32;
        /// Return true even if the position is obstructed or overlapped by another window.
        const ALLOW_WHEN_OVERLAPPED = sys::ImGuiHoveredFlags_AllowWhenOverlapped as i32;
        /// Return true even if the item is disabled.
        const ALLOW_WHEN_DISABLED = sys::ImGuiHoveredFlags_AllowWhenDisabled as i32;
        /// Disable using gamepad/keyboard navigation state when active, always query mouse.
        const NO_NAV_OVERRIDE = sys::ImGuiHoveredFlags_NoNavOverride as i32;
        /// Test rectangle visibility with popup/active-item/overlap bypasses.
        const RECT_ONLY = sys::ImGuiHoveredFlags_RectOnly as i32;
        /// Require mouse to be stationary for style.HoverStationaryDelay at least one time.
        const STATIONARY = sys::ImGuiHoveredFlags_Stationary as i32;
        /// Return true immediately.
        const DELAY_NONE = sys::ImGuiHoveredFlags_DelayNone as i32;
        /// Return true after style.HoverDelayShort elapsed.
        const DELAY_SHORT = sys::ImGuiHoveredFlags_DelayShort as i32;
        /// Return true after style.HoverDelayNormal elapsed.
        const DELAY_NORMAL = sys::ImGuiHoveredFlags_DelayNormal as i32;
        /// Disable shared delay system where moving between items preserves the previous timer.
        const NO_SHARED_DELAY = sys::ImGuiHoveredFlags_NoSharedDelay as i32;
    }
}

impl Default for TooltipHoveredFlags {
    fn default() -> Self {
        TooltipHoveredFlags::NONE
    }
}

bitflags! {
    /// Flags for focus detection
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct FocusedFlags: i32 {
        /// Return true if window is focused
        const NONE = sys::ImGuiFocusedFlags_None as i32;
        /// IsWindowFocused() only: Return true if any children of the window is focused
        const CHILD_WINDOWS = sys::ImGuiFocusedFlags_ChildWindows as i32;
        /// IsWindowFocused() only: Test from root window (top most parent of the current hierarchy)
        const ROOT_WINDOW = sys::ImGuiFocusedFlags_RootWindow as i32;
        /// IsWindowFocused() only: Return true if any window is focused
        const ANY_WINDOW = sys::ImGuiFocusedFlags_AnyWindow as i32;
        /// IsWindowFocused() only: Do not consider popup hierarchy
        const NO_POPUP_HIERARCHY = sys::ImGuiFocusedFlags_NoPopupHierarchy as i32;
        /// IsWindowFocused() only: Consider docking hierarchy
        const DOCK_HIERARCHY = sys::ImGuiFocusedFlags_DockHierarchy as i32;
        /// IsWindowFocused() only: Shortcut for `ROOT_WINDOW | CHILD_WINDOWS`.
        const ROOT_AND_CHILD_WINDOWS = sys::ImGuiFocusedFlags_RootAndChildWindows as i32;
    }
}

impl Default for FocusedFlags {
    fn default() -> Self {
        FocusedFlags::NONE
    }
}

pub(crate) fn validate_window_hovered_flags(caller: &str, flags: WindowHoveredFlags) {
    let unsupported = flags.bits() & !WindowHoveredFlags::all().bits();
    assert!(
        unsupported == 0,
        "{caller} received unsupported ImGuiHoveredFlags window bits: 0x{unsupported:X}"
    );
}

pub(crate) fn validate_item_hovered_flags(caller: &str, flags: ItemHoveredFlags) {
    let unsupported = flags.bits() & !ItemHoveredFlags::all().bits();
    assert!(
        unsupported == 0,
        "{caller} received unsupported ImGuiHoveredFlags item bits: 0x{unsupported:X}"
    );
}

pub(crate) fn validate_tooltip_hovered_flags(caller: &str, flags: TooltipHoveredFlags) {
    let unsupported = flags.bits() & !TooltipHoveredFlags::all().bits();
    assert!(
        unsupported == 0,
        "{caller} received unsupported ImGuiHoveredFlags tooltip bits: 0x{unsupported:X}"
    );
}

fn validate_focused_flags(caller: &str, flags: FocusedFlags) {
    let unsupported = flags.bits() & !FocusedFlags::all().bits();
    assert!(
        unsupported == 0,
        "{caller} received unsupported ImGuiFocusedFlags bits: 0x{unsupported:X}"
    );
}

fn assert_finite_f32(caller: &str, name: &str, value: f32) {
    assert!(value.is_finite(), "{caller} {name} must be finite");
}

fn assert_finite_vec2(caller: &str, name: &str, value: [f32; 2]) {
    assert!(
        value[0].is_finite() && value[1].is_finite(),
        "{caller} {name} must contain finite values"
    );
}

fn assert_finite_vec4(caller: &str, name: &str, value: [f32; 4]) {
    assert!(
        value.iter().all(|component| component.is_finite()),
        "{caller} {name} must contain finite values"
    );
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
        let rect = unsafe { sys::igGetItemRectMin() };
        [rect.x, rect.y]
    }

    /// Returns the lower-right bounding rectangle of the last item (screen space)
    #[doc(alias = "GetItemRectMax")]
    pub fn item_rect_max(&self) -> [f32; 2] {
        let rect = unsafe { sys::igGetItemRectMax() };
        [rect.x, rect.y]
    }

    // ============================================================================
    // Window utilities
    // ============================================================================

    /// Returns `true` if the current window is hovered (and typically: not blocked by a popup/modal)
    #[doc(alias = "IsWindowHovered")]
    pub fn is_window_hovered(&self) -> bool {
        unsafe { sys::igIsWindowHovered(WindowHoveredFlags::NONE.bits()) }
    }

    /// Returns `true` if the current window is hovered based on the given flags
    #[doc(alias = "IsWindowHovered")]
    pub fn is_window_hovered_with_flags(&self, flags: WindowHoveredFlags) -> bool {
        validate_window_hovered_flags("Ui::is_window_hovered_with_flags()", flags);
        unsafe { sys::igIsWindowHovered(flags.bits()) }
    }

    /// Returns `true` if the current window is focused (and typically: not blocked by a popup/modal)
    #[doc(alias = "IsWindowFocused")]
    pub fn is_window_focused(&self) -> bool {
        self.is_window_focused_with_flags(FocusedFlags::NONE)
    }

    /// Returns `true` if the current window is focused based on the given flags
    #[doc(alias = "IsWindowFocused")]
    pub fn is_window_focused_with_flags(&self, flags: FocusedFlags) -> bool {
        validate_focused_flags("Ui::is_window_focused_with_flags()", flags);
        unsafe { sys::igIsWindowFocused(flags.bits()) }
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
    pub fn get_key_pressed_amount(&self, key: Key, repeat_delay: f32, rate: f32) -> usize {
        assert_finite_f32("Ui::get_key_pressed_amount()", "repeat_delay", repeat_delay);
        assert_finite_f32("Ui::get_key_pressed_amount()", "rate", rate);
        non_negative_count_from_i32("Ui::get_key_pressed_amount()", unsafe {
            sys::igGetKeyPressedAmount(key.into(), repeat_delay, rate)
        })
    }

    /// Returns the name of a key
    #[doc(alias = "GetKeyName")]
    pub fn get_key_name(&self, key: Key) -> &str {
        unsafe {
            let name_ptr = sys::igGetKeyName(key.into());
            if name_ptr.is_null() {
                return "Unknown";
            }
            let c_str = std::ffi::CStr::from_ptr(name_ptr);
            c_str.to_str().unwrap_or("Unknown")
        }
    }

    /// Returns the number of times the mouse button was clicked in the current frame
    #[doc(alias = "GetMouseClickedCount")]
    pub fn get_mouse_clicked_count(&self, button: MouseButton) -> usize {
        non_negative_count_from_i32("Ui::get_mouse_clicked_count()", unsafe {
            sys::igGetMouseClickedCount(button.into())
        })
    }

    /// Returns the mouse position in screen coordinates
    #[doc(alias = "GetMousePos")]
    pub fn get_mouse_pos(&self) -> [f32; 2] {
        let pos = unsafe { sys::igGetMousePos() };
        [pos.x, pos.y]
    }

    /// Returns the mouse position when the button was clicked
    #[doc(alias = "GetMousePosOnOpeningCurrentPopup")]
    pub fn get_mouse_pos_on_opening_current_popup(&self) -> [f32; 2] {
        let pos = unsafe { sys::igGetMousePosOnOpeningCurrentPopup() };
        [pos.x, pos.y]
    }

    /// Returns the mouse drag delta
    #[doc(alias = "GetMouseDragDelta")]
    pub fn get_mouse_drag_delta(&self, button: MouseButton, lock_threshold: f32) -> [f32; 2] {
        assert_finite_f32(
            "Ui::get_mouse_drag_delta()",
            "lock_threshold",
            lock_threshold,
        );
        let delta = unsafe { sys::igGetMouseDragDelta(button.into(), lock_threshold) };
        [delta.x, delta.y]
    }

    /// Returns the mouse wheel delta
    #[doc(alias = "GetIO")]
    pub fn get_mouse_wheel(&self) -> f32 {
        self.io().mouse_wheel()
    }

    /// Returns the horizontal mouse wheel delta
    #[doc(alias = "GetIO")]
    pub fn get_mouse_wheel_h(&self) -> f32 {
        self.io().mouse_wheel_h()
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
    pub fn frame_count(&self) -> usize {
        non_negative_count_from_i32("Ui::frame_count()", unsafe { sys::igGetFrameCount() })
    }

    /// Returns the width of an item based on the current layout state.
    #[doc(alias = "CalcItemWidth")]
    pub fn calc_item_width(&self) -> f32 {
        unsafe { sys::igCalcItemWidth() }
    }

    /// Start logging to TTY.
    #[doc(alias = "LogToTTY")]
    pub fn log_to_tty(&self, auto_open_depth: impl Into<LogAutoOpenDepth>) {
        unsafe { sys::igLogToTTY(auto_open_depth.into().raw()) }
    }

    /// Start logging to file with the default filename.
    #[doc(alias = "LogToFile")]
    pub fn log_to_file_default(&self, auto_open_depth: impl Into<LogAutoOpenDepth>) {
        unsafe { sys::igLogToFile(auto_open_depth.into().raw(), std::ptr::null()) }
    }

    /// Start logging to file.
    ///
    /// # Errors
    ///
    /// Returns an error if `filename` contains NUL bytes.
    #[doc(alias = "LogToFile")]
    pub fn log_to_file(
        &self,
        auto_open_depth: impl Into<LogAutoOpenDepth>,
        filename: &std::path::Path,
    ) -> crate::error::ImGuiResult<()> {
        use crate::error::SafeStringConversion;
        let cstr = filename.to_string_lossy().into_owned().to_cstring_safe()?;
        unsafe { sys::igLogToFile(auto_open_depth.into().raw(), cstr.as_ptr()) }
        Ok(())
    }

    /// Start logging to clipboard.
    #[doc(alias = "LogToClipboard")]
    pub fn log_to_clipboard(&self, auto_open_depth: impl Into<LogAutoOpenDepth>) {
        unsafe { sys::igLogToClipboard(auto_open_depth.into().raw()) }
    }

    /// Show ImGui's logging buttons (TTY/File/Clipboard).
    #[doc(alias = "LogButtons")]
    pub fn log_buttons(&self) {
        unsafe { sys::igLogButtons() }
    }

    /// Finish logging (close file / copy to clipboard as needed).
    #[doc(alias = "LogFinish")]
    pub fn log_finish(&self) {
        unsafe { sys::igLogFinish() }
    }

    /// Returns a single style color from the user interface style.
    ///
    /// Use this function if you need to access the colors, but don't want to clone the entire
    /// style object.
    #[doc(alias = "GetStyle", alias = "GetStyleColorVec4")]
    pub fn style_color(&self, style_color: StyleColor) -> [f32; 4] {
        unsafe {
            let color = sys::igGetStyleColorVec4(style_color as sys::ImGuiCol);
            let color = &*color;
            [color.x, color.y, color.z, color.w]
        }
    }

    /// Returns an ImGui-packed ABGR color (`ImU32`) from a style color.
    ///
    /// This is a convenience wrapper over `ImGui::GetColorU32(ImGuiCol, alpha_mul)`.
    #[doc(alias = "GetColorU32")]
    pub fn get_color_u32(&self, style_color: StyleColor) -> u32 {
        self.get_color_u32_with_alpha(style_color, 1.0)
    }

    /// Returns an ImGui-packed ABGR color (`ImU32`) from a style color, with alpha multiplier.
    #[doc(alias = "GetColorU32")]
    pub fn get_color_u32_with_alpha(&self, style_color: StyleColor, alpha_mul: f32) -> u32 {
        assert_finite_f32("Ui::get_color_u32_with_alpha()", "alpha_mul", alpha_mul);
        unsafe { sys::igGetColorU32_Col(style_color as sys::ImGuiCol, alpha_mul) }
    }

    /// Returns an ImGui-packed ABGR color (`ImU32`) from an RGBA float color.
    ///
    /// Note: Dear ImGui applies the global style alpha when converting colors for rendering.
    #[doc(alias = "GetColorU32")]
    pub fn get_color_u32_from_rgba(&self, rgba: [f32; 4]) -> u32 {
        assert_finite_vec4("Ui::get_color_u32_from_rgba()", "rgba", rgba);
        unsafe {
            sys::igGetColorU32_Vec4(sys::ImVec4_c {
                x: rgba[0],
                y: rgba[1],
                z: rgba[2],
                w: rgba[3],
            })
        }
    }

    /// Returns an ImGui-packed ABGR color (`ImU32`) from an existing packed color, with alpha multiplier.
    #[doc(alias = "GetColorU32")]
    pub fn get_color_u32_from_packed(&self, abgr: u32, alpha_mul: f32) -> u32 {
        assert_finite_f32("Ui::get_color_u32_from_packed()", "alpha_mul", alpha_mul);
        unsafe { sys::igGetColorU32_U32(abgr, alpha_mul) }
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
            if name_ptr.is_null() {
                return "Unknown";
            }
            let c_str = std::ffi::CStr::from_ptr(name_ptr);
            c_str.to_str().unwrap_or("Unknown")
        }
    }

    /// Test if rectangle (of given size, starting from cursor position) is visible / not clipped.
    #[doc(alias = "IsRectVisible")]
    pub fn is_rect_visible(&self, size: [f32; 2]) -> bool {
        assert_finite_vec2("Ui::is_rect_visible()", "size", size);
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
        assert_finite_vec2("Ui::is_rect_visible_ex()", "rect_min", rect_min);
        assert_finite_vec2("Ui::is_rect_visible_ex()", "rect_max", rect_max);
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
        let pos = unsafe { sys::igGetCursorScreenPos() };
        [pos.x, pos.y]
    }

    /// Get available content region size.
    #[doc(alias = "GetContentRegionAvail")]
    pub fn get_content_region_avail(&self) -> [f32; 2] {
        let size = unsafe { sys::igGetContentRegionAvail() };
        [size.x, size.y]
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

#[cfg(test)]
mod tests {
    #[test]
    fn non_negative_count_conversion_rejects_negative_values() {
        assert_eq!(super::non_negative_count_from_i32("test", 7), 7);
        assert!(
            std::panic::catch_unwind(|| {
                let _ = super::non_negative_count_from_i32("test", -1);
            })
            .is_err()
        );
    }

    #[test]
    fn log_auto_open_depth_preserves_default_sentinel_and_rejects_overflow() {
        assert_eq!(super::LogAutoOpenDepth::DEFAULT.raw(), -1);
        assert_eq!(super::LogAutoOpenDepth::new(0).raw(), 0);
        assert_eq!(super::LogAutoOpenDepth::new(3).raw(), 3);
        assert!(
            std::panic::catch_unwind(|| super::LogAutoOpenDepth::new(i32::MAX as u32 + 1)).is_err()
        );
    }
}
