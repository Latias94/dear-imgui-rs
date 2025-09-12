use crate::draw::DrawListMut;
use crate::input::MouseCursor;
use crate::internal::RawWrapper;
use crate::string::UiBuffer;
use crate::sys;
use std::cell::UnsafeCell;

/// Represents the Dear ImGui user interface for one frame
#[derive(Debug)]
pub struct Ui {
    /// Internal buffer for string operations
    buffer: UnsafeCell<UiBuffer>,
}

impl Ui {
    /// Creates a new Ui instance
    ///
    /// This should only be called by Context::create()
    pub(crate) fn new() -> Self {
        Ui {
            buffer: UnsafeCell::new(UiBuffer::new(1024)),
        }
    }

    /// Returns an immutable reference to the inputs/outputs object
    #[doc(alias = "GetIO")]
    pub fn io(&self) -> &crate::io::Io {
        unsafe { &*(sys::ImGui_GetIO() as *const crate::io::Io) }
    }

    /// Internal method to push a single text to our scratch buffer.
    pub(crate) fn scratch_txt(&self, txt: impl AsRef<str>) -> *const std::os::raw::c_char {
        unsafe {
            let handle = &mut *self.buffer.get();
            handle.scratch_txt(txt)
        }
    }

    /// Internal method to push an option text to our scratch buffer.
    pub(crate) fn scratch_txt_opt(
        &self,
        txt: Option<impl AsRef<str>>,
    ) -> *const std::os::raw::c_char {
        unsafe {
            let handle = &mut *self.buffer.get();
            handle.scratch_txt_opt(txt)
        }
    }

    /// Helper method for two strings
    pub(crate) fn scratch_txt_two(
        &self,
        txt_0: impl AsRef<str>,
        txt_1: impl AsRef<str>,
    ) -> (*const std::os::raw::c_char, *const std::os::raw::c_char) {
        unsafe {
            let handle = &mut *self.buffer.get();
            handle.scratch_txt_two(txt_0, txt_1)
        }
    }

    /// Helper method with one optional value
    pub(crate) fn scratch_txt_with_opt(
        &self,
        txt_0: impl AsRef<str>,
        txt_1: Option<impl AsRef<str>>,
    ) -> (*const std::os::raw::c_char, *const std::os::raw::c_char) {
        unsafe {
            let handle = &mut *self.buffer.get();
            handle.scratch_txt_with_opt(txt_0, txt_1)
        }
    }

    /// Get access to the scratch buffer for complex string operations
    pub(crate) fn scratch_buffer(&self) -> &UnsafeCell<UiBuffer> {
        &self.buffer
    }

    /// Display text
    #[doc(alias = "TextUnformatted")]
    pub fn text<T: AsRef<str>>(&self, text: T) {
        let s = text.as_ref();
        unsafe {
            let start = s.as_ptr();
            let end = start.add(s.len());
            crate::sys::ImGui_TextUnformatted(
                start as *const std::os::raw::c_char,
                end as *const std::os::raw::c_char,
            );
        }
    }

    /// Access to the current window's draw list
    #[doc(alias = "GetWindowDrawList")]
    pub fn get_window_draw_list(&self) -> DrawListMut<'_> {
        DrawListMut::window(self)
    }

    /// Access to the background draw list
    #[doc(alias = "GetBackgroundDrawList")]
    pub fn get_background_draw_list(&self) -> DrawListMut<'_> {
        DrawListMut::background(self)
    }

    /// Access to the foreground draw list
    #[doc(alias = "GetForegroundDrawList")]
    pub fn get_foreground_draw_list(&self) -> DrawListMut<'_> {
        DrawListMut::foreground(self)
    }

    /// Creates a window builder
    pub fn window(&self, name: impl Into<String>) -> crate::window::Window<'_> {
        crate::window::Window::new(self, name)
    }

    /// Renders a demo window (previously called a test window), which demonstrates most
    /// Dear ImGui features.
    #[doc(alias = "ShowDemoWindow")]
    pub fn show_demo_window(&self, opened: &mut bool) {
        unsafe {
            crate::sys::ImGui_ShowDemoWindow(opened);
        }
    }

    /// Renders an about window.
    ///
    /// Displays the Dear ImGui version/credits, and build/system information.
    #[doc(alias = "ShowAboutWindow")]
    pub fn show_about_window(&self, opened: &mut bool) {
        unsafe {
            crate::sys::ImGui_ShowAboutWindow(opened);
        }
    }

    /// Renders a metrics/debug window.
    ///
    /// Displays Dear ImGui internals: draw commands (with individual draw calls and vertices),
    /// window list, basic internal state, etc.
    #[doc(alias = "ShowMetricsWindow")]
    pub fn show_metrics_window(&self, opened: &mut bool) {
        unsafe {
            crate::sys::ImGui_ShowMetricsWindow(opened);
        }
    }

    /// Renders a style editor block (not a window) for the given `Style` structure
    #[doc(alias = "ShowStyleEditor")]
    pub fn show_style_editor(&self, style: &mut crate::style::Style) {
        unsafe {
            crate::sys::ImGui_ShowStyleEditor(style.raw_mut());
        }
    }

    /// Renders a style editor block (not a window) for the currently active style
    #[doc(alias = "ShowStyleEditor")]
    pub fn show_default_style_editor(&self) {
        unsafe {
            crate::sys::ImGui_ShowStyleEditor(std::ptr::null_mut());
        }
    }

    /// Renders a basic help/info block (not a window)
    #[doc(alias = "ShowUserGuide")]
    pub fn show_user_guide(&self) {
        unsafe {
            crate::sys::ImGui_ShowUserGuide();
        }
    }

    // Drag widgets

    /// Creates a drag float slider
    #[doc(alias = "DragFloat")]
    pub fn drag_float(&self, label: impl AsRef<str>, value: &mut f32) -> bool {
        crate::widget::drag::Drag::new(label).build(self, value)
    }

    /// Creates a drag float slider with configuration
    #[doc(alias = "DragFloat")]
    pub fn drag_float_config<L: AsRef<str>>(&self, label: L) -> crate::widget::drag::Drag<f32, L> {
        crate::widget::drag::Drag::new(label)
    }

    /// Creates a drag int slider
    #[doc(alias = "DragInt")]
    pub fn drag_int(&self, label: impl AsRef<str>, value: &mut i32) -> bool {
        crate::widget::drag::Drag::new(label).build(self, value)
    }

    /// Creates a drag int slider with configuration
    #[doc(alias = "DragInt")]
    pub fn drag_int_config<L: AsRef<str>>(&self, label: L) -> crate::widget::drag::Drag<i32, L> {
        crate::widget::drag::Drag::new(label)
    }

    /// Creates a drag float range slider
    #[doc(alias = "DragFloatRange2")]
    pub fn drag_float_range2(&self, label: impl AsRef<str>, min: &mut f32, max: &mut f32) -> bool {
        crate::widget::drag::DragRange::<f32, _>::new(label).build(self, min, max)
    }

    /// Creates a drag float range slider with configuration
    #[doc(alias = "DragFloatRange2")]
    pub fn drag_float_range2_config<L: AsRef<str>>(
        &self,
        label: L,
    ) -> crate::widget::drag::DragRange<f32, L> {
        crate::widget::drag::DragRange::new(label)
    }

    /// Creates a drag int range slider
    #[doc(alias = "DragIntRange2")]
    pub fn drag_int_range2(&self, label: impl AsRef<str>, min: &mut i32, max: &mut i32) -> bool {
        crate::widget::drag::DragRange::<i32, _>::new(label).build(self, min, max)
    }

    /// Creates a drag int range slider with configuration
    #[doc(alias = "DragIntRange2")]
    pub fn drag_int_range2_config<L: AsRef<str>>(
        &self,
        label: L,
    ) -> crate::widget::drag::DragRange<i32, L> {
        crate::widget::drag::DragRange::new(label)
    }

    /// Returns the currently desired mouse cursor type
    ///
    /// Returns `None` if no cursor should be displayed
    #[doc(alias = "GetMouseCursor")]
    pub fn mouse_cursor(&self) -> Option<MouseCursor> {
        unsafe {
            match sys::ImGui_GetMouseCursor() {
                sys::ImGuiMouseCursor_Arrow => Some(MouseCursor::Arrow),
                sys::ImGuiMouseCursor_TextInput => Some(MouseCursor::TextInput),
                sys::ImGuiMouseCursor_ResizeAll => Some(MouseCursor::ResizeAll),
                sys::ImGuiMouseCursor_ResizeNS => Some(MouseCursor::ResizeNS),
                sys::ImGuiMouseCursor_ResizeEW => Some(MouseCursor::ResizeEW),
                sys::ImGuiMouseCursor_ResizeNESW => Some(MouseCursor::ResizeNESW),
                sys::ImGuiMouseCursor_ResizeNWSE => Some(MouseCursor::ResizeNWSE),
                sys::ImGuiMouseCursor_Hand => Some(MouseCursor::Hand),
                sys::ImGuiMouseCursor_NotAllowed => Some(MouseCursor::NotAllowed),
                _ => None,
            }
        }
    }

    /// Sets the desired mouse cursor type
    ///
    /// Passing `None` hides the mouse cursor
    #[doc(alias = "SetMouseCursor")]
    pub fn set_mouse_cursor(&self, cursor_type: Option<MouseCursor>) {
        unsafe {
            sys::ImGui_SetMouseCursor(
                cursor_type
                    .map(|x| x as i32)
                    .unwrap_or(sys::ImGuiMouseCursor_None),
            );
        }
    }

    // ============================================================================
    // Focus and Navigation
    // ============================================================================

    /// Focuses keyboard on the next widget.
    ///
    /// This is the equivalent to [set_keyboard_focus_here_with_offset](Self::set_keyboard_focus_here_with_offset)
    /// with `offset` set to 0.
    #[doc(alias = "SetKeyboardFocusHere")]
    pub fn set_keyboard_focus_here(&self) {
        self.set_keyboard_focus_here_with_offset(0);
    }

    /// Focuses keyboard on a widget relative to current position.
    ///
    /// Use positive offset to focus on next widgets, negative offset to focus on previous widgets.
    #[doc(alias = "SetKeyboardFocusHere")]
    pub fn set_keyboard_focus_here_with_offset(&self, offset: i32) {
        unsafe {
            sys::ImGui_SetKeyboardFocusHere(offset);
        }
    }

    /// Set next item to be open by default.
    ///
    /// This is useful for tree nodes, collapsing headers, etc.
    #[doc(alias = "SetNextItemOpen")]
    pub fn set_next_item_open(&self, is_open: bool) {
        unsafe {
            sys::ImGui_SetNextItemOpen(is_open, 0); // 0 = ImGuiCond_Always
        }
    }

    /// Set next item to be open by default with condition.
    #[doc(alias = "SetNextItemOpen")]
    pub fn set_next_item_open_with_cond(&self, is_open: bool, cond: crate::Condition) {
        unsafe {
            sys::ImGui_SetNextItemOpen(is_open, cond as i32);
        }
    }

    /// Set next item width.
    ///
    /// Set to 0.0 for default width, >0.0 for explicit width, <0.0 for relative width.
    #[doc(alias = "SetNextItemWidth")]
    pub fn set_next_item_width(&self, item_width: f32) {
        unsafe {
            sys::ImGui_SetNextItemWidth(item_width);
        }
    }

    // ============================================================================
    // Style Access
    // ============================================================================

    /// Returns a shared reference to the current [`Style`].
    ///
    /// ## Safety
    ///
    /// This function is tagged as `unsafe` because pushing via
    /// [`push_style_color`](crate::Ui::push_style_color) or
    /// [`push_style_var`](crate::Ui::push_style_var) or popping via
    /// [`ColorStackToken::pop`](crate::ColorStackToken::pop) or
    /// [`StyleStackToken::pop`](crate::StyleStackToken::pop) will modify the values in the returned
    /// shared reference. Therefore, you should not retain this reference across calls to push and
    /// pop. The [`clone_style`](Ui::clone_style) version may instead be used to avoid `unsafe`.
    #[doc(alias = "GetStyle")]
    pub unsafe fn style(&self) -> &crate::Style {
        // safe because Style is a transparent wrapper around sys::ImGuiStyle
        &*(sys::ImGui_GetStyle() as *const crate::Style)
    }

    /// Returns a copy of the current style.
    ///
    /// This is a safe alternative to [`style`](Self::style) that avoids the lifetime issues.
    #[doc(alias = "GetStyle")]
    pub fn clone_style(&self) -> crate::Style {
        unsafe { self.style().clone() }
    }

    // ============================================================================
    // Additional Demo, Debug, Information (non-duplicate methods)
    // ============================================================================

    /// Renders a debug log window.
    ///
    /// Displays a simplified log of important dear imgui events.
    #[doc(alias = "ShowDebugLogWindow")]
    pub fn show_debug_log_window(&self, opened: &mut bool) {
        unsafe {
            sys::ImGui_ShowDebugLogWindow(opened);
        }
    }

    /// Renders an ID stack tool window.
    ///
    /// Hover items with mouse to query information about the source of their unique ID.
    #[doc(alias = "ShowIDStackToolWindow")]
    pub fn show_id_stack_tool_window(&self, opened: &mut bool) {
        unsafe {
            sys::ImGui_ShowIDStackToolWindow(opened);
        }
    }

    /// Renders a style selector combo box.
    ///
    /// Returns true when a different style was selected.
    #[doc(alias = "ShowStyleSelector")]
    pub fn show_style_selector(&self, label: impl AsRef<str>) -> bool {
        unsafe { sys::ImGui_ShowStyleSelector(self.scratch_txt(label)) }
    }

    /// Renders a font selector combo box.
    #[doc(alias = "ShowFontSelector")]
    pub fn show_font_selector(&self, label: impl AsRef<str>) {
        unsafe {
            sys::ImGui_ShowFontSelector(self.scratch_txt(label));
        }
    }

    /// Returns the Dear ImGui version string
    #[doc(alias = "GetVersion")]
    pub fn get_version(&self) -> &str {
        unsafe {
            let version_ptr = sys::ImGui_GetVersion();
            let c_str = std::ffi::CStr::from_ptr(version_ptr);
            c_str.to_str().unwrap_or("Unknown")
        }
    }
}
