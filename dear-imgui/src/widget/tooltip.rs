use crate::input::MouseButton;
use crate::sys;
use crate::ui::Ui;

/// # Tooltip Widgets
impl Ui {
    /// Construct a tooltip window that can have any kind of content.
    ///
    /// Typically used with `Ui::is_item_hovered()` or some other conditional check.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use dear_imgui::*;
    /// # let mut ctx = Context::create_or_panic();
    /// # let ui = ctx.frame();
    /// ui.text("Hover over me");
    /// if ui.is_item_hovered() {
    ///     ui.tooltip(|| {
    ///         ui.text_colored([1.0, 0.0, 0.0, 1.0], "I'm red!");
    ///     });
    /// }
    /// ```
    #[doc(alias = "BeginTooltip", alias = "EndTooltip")]
    pub fn tooltip<F: FnOnce()>(&self, f: F) {
        if let Some(_token) = self.begin_tooltip() {
            f();
        }
    }

    /// Construct a tooltip window that can have any kind of content.
    ///
    /// Returns a `TooltipToken` that must be ended by calling `.end()` or by dropping.
    #[doc(alias = "BeginTooltip")]
    pub fn begin_tooltip(&self) -> Option<TooltipToken<'_>> {
        if unsafe { sys::igBeginTooltip() } {
            Some(TooltipToken::new(self))
        } else {
            None
        }
    }

    /// Shortcut to call [`Self::tooltip`] with simple text content.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use dear_imgui::*;
    /// # let mut ctx = Context::create_or_panic();
    /// # let ui = ctx.frame();
    /// ui.text("Hover over me");
    /// if ui.is_item_hovered() {
    ///     ui.tooltip_text("I'm a tooltip!");
    /// }
    /// ```
    #[doc(alias = "BeginTooltip", alias = "EndTooltip", alias = "SetTooltip")]
    pub fn tooltip_text(&self, text: impl AsRef<str>) {
        self.tooltip(|| self.text(text));
    }

    /// Sets a tooltip with simple text content.
    /// This is more efficient than begin_tooltip/end_tooltip for simple text.
    #[doc(alias = "SetTooltip")]
    pub fn set_tooltip(&self, text: impl AsRef<str>) {
        let text_ptr = self.scratch_txt(text);
        unsafe {
            sys::igSetTooltip(text_ptr);
        }
    }

    /// Sets a tooltip with formatted text content.
    #[doc(alias = "SetTooltip")]
    pub fn set_tooltip_formatted(&self, text: impl AsRef<str>) {
        self.set_tooltip(text);
    }

    /// Sets a tooltip for the last item with simple text content.
    /// More efficient than building a tooltip window for simple cases.
    #[doc(alias = "SetItemTooltip")]
    pub fn set_item_tooltip(&self, text: impl AsRef<str>) {
        let text_ptr = self.scratch_txt(text);
        unsafe { sys::igSetItemTooltip(text_ptr) }
    }
}

/// # Item/Widget Utilities and Query Functions
impl Ui {
    /// Returns true if the last item is being hovered by mouse (and usable).
    /// This is typically used to show tooltips.
    #[doc(alias = "IsItemHovered")]
    pub fn is_item_hovered(&self) -> bool {
        unsafe { sys::igIsItemHovered(super::HoveredFlags::NONE.bits()) }
    }

    /// Returns true if the last item is being hovered by mouse with specific flags.
    #[doc(alias = "IsItemHovered")]
    pub fn is_item_hovered_with_flags(&self, flags: HoveredFlags) -> bool {
        unsafe { sys::igIsItemHovered(flags.bits()) }
    }

    /// Returns true if the last item is active (e.g. button being held, text field being edited).
    #[doc(alias = "IsItemActive")]
    pub fn is_item_active(&self) -> bool {
        unsafe { sys::igIsItemActive() }
    }

    /// Returns true if the last item is focused (e.g. text input field).
    #[doc(alias = "IsItemFocused")]
    pub fn is_item_focused(&self) -> bool {
        unsafe { sys::igIsItemFocused() }
    }

    /// Returns true if the last item was just clicked.
    #[doc(alias = "IsItemClicked")]
    pub fn is_item_clicked(&self) -> bool {
        unsafe { sys::igIsItemClicked(crate::input::MouseButton::Left as i32) }
    }

    /// Returns true if the last item was clicked with specific mouse button.
    #[doc(alias = "IsItemClicked")]
    pub fn is_item_clicked_with_button(&self, mouse_button: MouseButton) -> bool {
        unsafe { sys::igIsItemClicked(mouse_button as i32) }
    }

    /// Returns true if the last item is visible (not clipped).
    #[doc(alias = "IsItemVisible")]
    pub fn is_item_visible(&self) -> bool {
        unsafe { sys::igIsItemVisible() }
    }

    /// Returns true if the last item was just made active (e.g. button was pressed).
    #[doc(alias = "IsItemActivated")]
    pub fn is_item_activated(&self) -> bool {
        unsafe { sys::igIsItemActivated() }
    }

    /// Returns true if the last item was just made inactive (e.g. button was released).
    #[doc(alias = "IsItemDeactivated")]
    pub fn is_item_deactivated(&self) -> bool {
        unsafe { sys::igIsItemDeactivated() }
    }

    /// Returns true if the last item was just made inactive and was edited.
    #[doc(alias = "IsItemDeactivatedAfterEdit")]
    pub fn is_item_deactivated_after_edit(&self) -> bool {
        unsafe { sys::igIsItemDeactivatedAfterEdit() }
    }

    /// Returns true if any item is active.
    #[doc(alias = "IsAnyItemActive")]
    pub fn is_any_item_active(&self) -> bool {
        unsafe { sys::igIsAnyItemActive() }
    }

    /// Returns true if any item is focused.
    #[doc(alias = "IsAnyItemFocused")]
    pub fn is_any_item_focused(&self) -> bool {
        unsafe { sys::igIsAnyItemFocused() }
    }

    /// Returns true if any item is hovered.
    #[doc(alias = "IsAnyItemHovered")]
    pub fn is_any_item_hovered(&self) -> bool {
        unsafe { sys::igIsAnyItemHovered() }
    }

    /// Gets the bounding rectangle of the last item in screen space.
    #[doc(alias = "GetItemRectMin", alias = "GetItemRectMax")]
    pub fn item_rect(&self) -> ([f32; 2], [f32; 2]) {
        unsafe {
            let mut min = sys::ImVec2 { x: 0.0, y: 0.0 };
            let mut max = sys::ImVec2 { x: 0.0, y: 0.0 };
            sys::igGetItemRectMin(&mut min);
            sys::igGetItemRectMax(&mut max);
            ([min.x, min.y], [max.x, max.y])
        }
    }

    /// Gets the size of the last item.
    #[doc(alias = "GetItemRectSize")]
    pub fn item_rect_size(&self) -> [f32; 2] {
        unsafe {
            let mut size = sys::ImVec2 { x: 0.0, y: 0.0 };
            sys::igGetItemRectSize(&mut size);
            [size.x, size.y]
        }
    }
}

bitflags::bitflags! {
    /// Flags for IsItemHovered()
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct HoveredFlags: i32 {
        /// No flags
        const NONE = sys::ImGuiHoveredFlags_None as i32;
        /// Return true if directly over the item/window, not obstructed by another window
        const CHILD_WINDOWS = sys::ImGuiHoveredFlags_ChildWindows as i32;
        /// Return true if any window is hovered
        const ROOT_WINDOW = sys::ImGuiHoveredFlags_RootWindow as i32;
        /// Return true if any child of the window is hovered
        const ANY_WINDOW = sys::ImGuiHoveredFlags_AnyWindow as i32;
        /// Return true even if a popup window is normally blocking access to this item/window
        const ALLOW_WHEN_BLOCKED_BY_POPUP = sys::ImGuiHoveredFlags_AllowWhenBlockedByPopup as i32;
        /// Return true even if an active item is blocking access to this item/window
        const ALLOW_WHEN_BLOCKED_BY_ACTIVE_ITEM = sys::ImGuiHoveredFlags_AllowWhenBlockedByActiveItem as i32;
        /// Return true even if the position is obstructed or overlapped by another window
        const ALLOW_WHEN_OVERLAPPED = sys::ImGuiHoveredFlags_AllowWhenOverlapped as i32;
        /// Return true even if the item is disabled
        const ALLOW_WHEN_DISABLED = sys::ImGuiHoveredFlags_AllowWhenDisabled as i32;
        /// Disable using gamepad/keyboard navigation state when active, always query mouse
        const NO_NAV_OVERRIDE = sys::ImGuiHoveredFlags_NoNavOverride as i32;
    }
}

/// Tracks a tooltip that can be ended by calling `.end()` or by dropping
#[must_use]
pub struct TooltipToken<'ui> {
    ui: &'ui Ui,
}

impl<'ui> TooltipToken<'ui> {
    /// Creates a new tooltip token
    fn new(ui: &'ui Ui) -> Self {
        TooltipToken { ui }
    }

    /// Ends the tooltip
    pub fn end(self) {
        // The drop implementation will handle the actual ending
    }
}

impl<'ui> Drop for TooltipToken<'ui> {
    fn drop(&mut self) {
        unsafe {
            sys::igEndTooltip();
        }
    }
}
