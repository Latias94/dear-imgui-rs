//! Per-frame UI entry point
//!
//! The `Ui` type exposes most user-facing Dear ImGui APIs for a single frame:
//! creating windows, drawing widgets, accessing draw lists, showing built-in
//! tools and more. Obtain it from [`Context::frame`].
//!
//! Example:
//! ```no_run
//! # use dear_imgui_rs::*;
//! let mut ctx = Context::create();
//! let ui = ctx.frame();
//! ui.text("Hello, world!");
//! ```
//!
use crate::Id;
use crate::draw::DrawListMut;
use crate::input::MouseCursor;
use crate::internal::RawWrapper;
use crate::string::UiBuffer;
use crate::sys;
use crate::texture::TextureRef;
use std::cell::UnsafeCell;

/// Represents the Dear ImGui user interface for one frame
#[derive(Debug)]
pub struct Ui {
    /// Internal buffer for string operations
    buffer: UnsafeCell<UiBuffer>,
}

impl Ui {
    /// Returns a reference to the main Dear ImGui viewport (safe wrapper)
    ///
    /// Same viewport used by `dockspace_over_main_viewport()`.
    ///
    /// The returned reference is owned by the currently active ImGui context and
    /// must not be used after the context is destroyed.
    #[doc(alias = "GetMainViewport")]
    pub fn main_viewport(&self) -> &crate::platform_io::Viewport {
        unsafe {
            let ptr = sys::igGetMainViewport();
            if ptr.is_null() {
                panic!("Ui::main_viewport() requires an active ImGui context");
            }
            crate::platform_io::Viewport::from_raw(ptr as *const sys::ImGuiViewport)
        }
    }
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
        unsafe { &*(sys::igGetIO_Nil() as *const crate::io::Io) }
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
            crate::sys::igTextUnformatted(
                start as *const std::os::raw::c_char,
                end as *const std::os::raw::c_char,
            );
        }
    }

    /// Set the viewport for the next window.
    ///
    /// This is a convenience wrapper over `ImGui::SetNextWindowViewport`.
    /// Useful when hosting a fullscreen DockSpace window inside the main viewport.
    #[doc(alias = "SetNextWindowViewport")]
    pub fn set_next_window_viewport(&self, viewport_id: Id) {
        unsafe { sys::igSetNextWindowViewport(viewport_id.into()) }
    }

    /// Returns an ID from a string label in the current ID scope.
    ///
    /// This mirrors `ImGui::GetID(label)`. Useful for building stable IDs
    /// for widgets or dockspaces inside the current window/scope.
    #[doc(alias = "GetID")]
    pub fn get_id(&self, label: &str) -> Id {
        unsafe { Id::from(sys::igGetID_Str(self.scratch_txt(label))) }
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
    pub fn window<'ui>(
        &'ui self,
        name: impl Into<std::borrow::Cow<'ui, str>>,
    ) -> crate::window::Window<'ui> {
        crate::window::Window::new(self, name)
    }

    /// Renders a demo window (previously called a test window), which demonstrates most
    /// Dear ImGui features.
    #[doc(alias = "ShowDemoWindow")]
    pub fn show_demo_window(&self, opened: &mut bool) {
        unsafe {
            crate::sys::igShowDemoWindow(opened);
        }
    }

    /// Convenience: draw an image with background and tint (ImGui 1.92+)
    ///
    /// Equivalent to using `image_config(...).build_with_bg(bg, tint)` but in one call.
    #[doc(alias = "ImageWithBg")]
    pub fn image_with_bg(
        &self,
        texture: impl Into<TextureRef>,
        size: [f32; 2],
        bg_color: [f32; 4],
        tint_color: [f32; 4],
    ) {
        crate::widget::image::Image::new(self, texture, size).build_with_bg(bg_color, tint_color)
    }

    /// Renders an about window.
    ///
    /// Displays the Dear ImGui version/credits, and build/system information.
    #[doc(alias = "ShowAboutWindow")]
    pub fn show_about_window(&self, opened: &mut bool) {
        unsafe {
            crate::sys::igShowAboutWindow(opened);
        }
    }

    /// Renders a metrics/debug window.
    ///
    /// Displays Dear ImGui internals: draw commands (with individual draw calls and vertices),
    /// window list, basic internal state, etc.
    #[doc(alias = "ShowMetricsWindow")]
    pub fn show_metrics_window(&self, opened: &mut bool) {
        unsafe {
            crate::sys::igShowMetricsWindow(opened);
        }
    }

    /// Renders a style editor block (not a window) for the given `Style` structure
    #[doc(alias = "ShowStyleEditor")]
    pub fn show_style_editor(&self, style: &mut crate::style::Style) {
        unsafe {
            crate::sys::igShowStyleEditor(style.raw_mut());
        }
    }

    /// Renders a style editor block (not a window) for the currently active style
    #[doc(alias = "ShowStyleEditor")]
    pub fn show_default_style_editor(&self) {
        unsafe {
            crate::sys::igShowStyleEditor(std::ptr::null_mut());
        }
    }

    /// Renders a basic help/info block (not a window)
    #[doc(alias = "ShowUserGuide")]
    pub fn show_user_guide(&self) {
        unsafe {
            crate::sys::igShowUserGuide();
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
            match sys::igGetMouseCursor() {
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
            let val: sys::ImGuiMouseCursor = cursor_type
                .map(|x| x as sys::ImGuiMouseCursor)
                .unwrap_or(sys::ImGuiMouseCursor_None);
            sys::igSetMouseCursor(val);
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
            sys::igSetKeyboardFocusHere(offset);
        }
    }

    /// Set next item to be open by default.
    ///
    /// This is useful for tree nodes, collapsing headers, etc.
    #[doc(alias = "SetNextItemOpen")]
    pub fn set_next_item_open(&self, is_open: bool) {
        unsafe {
            sys::igSetNextItemOpen(is_open, 0); // 0 = ImGuiCond_Always
        }
    }

    /// Set next item to be open by default with condition.
    #[doc(alias = "SetNextItemOpen")]
    pub fn set_next_item_open_with_cond(&self, is_open: bool, cond: crate::Condition) {
        unsafe { sys::igSetNextItemOpen(is_open, cond as sys::ImGuiCond) }
    }

    /// Set next item width.
    ///
    /// Set to 0.0 for default width, >0.0 for explicit width, <0.0 for relative width.
    #[doc(alias = "SetNextItemWidth")]
    pub fn set_next_item_width(&self, item_width: f32) {
        unsafe {
            sys::igSetNextItemWidth(item_width);
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
        unsafe {
            // safe because Style is a transparent wrapper around sys::ImGuiStyle
            &*(sys::igGetStyle() as *const crate::Style)
        }
    }

    /// Returns a copy of the current style.
    ///
    /// This is a safe alternative to [`style`](Self::style) that avoids the lifetime issues.
    #[doc(alias = "GetStyle")]
    pub fn clone_style(&self) -> crate::Style {
        unsafe { self.style().clone() }
    }

    /// Apply the built-in Dark style to the current style.
    #[doc(alias = "StyleColorsDark")]
    pub fn style_colors_dark(&self) {
        unsafe { sys::igStyleColorsDark(std::ptr::null_mut()) }
    }

    /// Apply the built-in Light style to the current style.
    #[doc(alias = "StyleColorsLight")]
    pub fn style_colors_light(&self) {
        unsafe { sys::igStyleColorsLight(std::ptr::null_mut()) }
    }

    /// Apply the built-in Classic style to the current style.
    #[doc(alias = "StyleColorsClassic")]
    pub fn style_colors_classic(&self) {
        unsafe { sys::igStyleColorsClassic(std::ptr::null_mut()) }
    }

    /// Write the Dark style values into the provided [`Style`] object.
    #[doc(alias = "StyleColorsDark")]
    pub fn style_colors_dark_into(&self, dst: &mut crate::Style) {
        unsafe { sys::igStyleColorsDark(dst as *mut _ as *mut sys::ImGuiStyle) }
    }

    /// Write the Light style values into the provided [`Style`] object.
    #[doc(alias = "StyleColorsLight")]
    pub fn style_colors_light_into(&self, dst: &mut crate::Style) {
        unsafe { sys::igStyleColorsLight(dst as *mut _ as *mut sys::ImGuiStyle) }
    }

    /// Write the Classic style values into the provided [`Style`] object.
    #[doc(alias = "StyleColorsClassic")]
    pub fn style_colors_classic_into(&self, dst: &mut crate::Style) {
        unsafe { sys::igStyleColorsClassic(dst as *mut _ as *mut sys::ImGuiStyle) }
    }

    /// Returns DPI scale currently associated to the current window's viewport.
    #[doc(alias = "GetWindowDpiScale")]
    pub fn window_dpi_scale(&self) -> f32 {
        unsafe { sys::igGetWindowDpiScale() }
    }

    /// Display a text label with a boolean value (for quick debug UIs).
    #[doc(alias = "Value")]
    pub fn value_bool(&self, prefix: impl AsRef<str>, v: bool) {
        unsafe { sys::igValue_Bool(self.scratch_txt(prefix), v) }
    }

    /// Get current window width (shortcut for `GetWindowSize().x`).
    #[doc(alias = "GetWindowWidth")]
    pub fn window_width(&self) -> f32 {
        unsafe { sys::igGetWindowWidth() }
    }

    /// Get current window height (shortcut for `GetWindowSize().y`).
    #[doc(alias = "GetWindowHeight")]
    pub fn window_height(&self) -> f32 {
        unsafe { sys::igGetWindowHeight() }
    }

    /// Get current window position in screen space.
    #[doc(alias = "GetWindowPos")]
    pub fn window_pos(&self) -> [f32; 2] {
        let v = unsafe { sys::igGetWindowPos() };
        [v.x, v.y]
    }

    /// Get current window size.
    #[doc(alias = "GetWindowSize")]
    pub fn window_size(&self) -> [f32; 2] {
        let v = unsafe { sys::igGetWindowSize() };
        [v.x, v.y]
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
            sys::igShowDebugLogWindow(opened);
        }
    }

    /// Renders an ID stack tool window.
    ///
    /// Hover items with mouse to query information about the source of their unique ID.
    #[doc(alias = "ShowIDStackToolWindow")]
    pub fn show_id_stack_tool_window(&self, opened: &mut bool) {
        unsafe {
            sys::igShowIDStackToolWindow(opened);
        }
    }

    /// Renders a style selector combo box.
    ///
    /// Returns true when a different style was selected.
    #[doc(alias = "ShowStyleSelector")]
    pub fn show_style_selector(&self, label: impl AsRef<str>) -> bool {
        unsafe { sys::igShowStyleSelector(self.scratch_txt(label)) }
    }

    /// Renders a font selector combo box.
    #[doc(alias = "ShowFontSelector")]
    pub fn show_font_selector(&self, label: impl AsRef<str>) {
        unsafe {
            sys::igShowFontSelector(self.scratch_txt(label));
        }
    }

    /// Returns the Dear ImGui version string
    #[doc(alias = "GetVersion")]
    pub fn get_version(&self) -> &str {
        unsafe {
            let version_ptr = sys::igGetVersion();
            if version_ptr.is_null() {
                return "Unknown";
            }
            let c_str = std::ffi::CStr::from_ptr(version_ptr);
            c_str.to_str().unwrap_or("Unknown")
        }
    }
}
