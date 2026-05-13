use crate::sys;

// InputText Callback System
// =========================

bitflags::bitflags! {
    /// Callback flags for InputText widgets
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct InputTextCallback: u32 {
        /// Call user function on pressing TAB (for completion handling)
        const COMPLETION = sys::ImGuiInputTextFlags_CallbackCompletion as u32;
        /// Call user function on pressing Up/Down arrows (for history handling)
        const HISTORY = sys::ImGuiInputTextFlags_CallbackHistory as u32;
        /// Call user function every time. User code may query cursor position, modify text buffer.
        const ALWAYS = sys::ImGuiInputTextFlags_CallbackAlways as u32;
        /// Call user function to filter character.
        const CHAR_FILTER = sys::ImGuiInputTextFlags_CallbackCharFilter as u32;
        /// Callback on buffer edit (note that InputText already returns true on edit, the
        /// callback is useful mainly to manipulate the underlying buffer while focus is active)
        const EDIT = sys::ImGuiInputTextFlags_CallbackEdit as u32;
    }
}

/// Direction for history navigation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryDirection {
    /// Up arrow key pressed
    Up,
    /// Down arrow key pressed
    Down,
}

/// This trait provides an interface which ImGui will call on InputText callbacks.
///
/// Each method is called *if and only if* the corresponding flag for each
/// method is passed to ImGui in the `callback` builder.
pub trait InputTextCallbackHandler {
    /// Filters a char -- returning a `None` means that the char is removed,
    /// and returning another char substitutes it out.
    ///
    /// To make ImGui run this callback, use [InputTextCallback::CHAR_FILTER].
    fn char_filter(&mut self, c: char) -> Option<char> {
        Some(c)
    }

    /// Called when the user presses the completion key (TAB by default).
    ///
    /// To make ImGui run this callback, use [InputTextCallback::COMPLETION].
    fn on_completion(&mut self, _data: TextCallbackData<'_>) {}

    /// Called when the user presses Up/Down arrow keys for history navigation.
    ///
    /// To make ImGui run this callback, use [InputTextCallback::HISTORY].
    fn on_history(&mut self, _direction: HistoryDirection, _data: TextCallbackData<'_>) {}

    /// Called every frame when the input text is active.
    ///
    /// To make ImGui run this callback, use [InputTextCallback::ALWAYS].
    fn on_always(&mut self, _data: TextCallbackData<'_>) {}

    /// Called when the text buffer is edited.
    ///
    /// To make ImGui run this callback, use [InputTextCallback::EDIT].
    fn on_edit(&mut self, _data: TextCallbackData<'_>) {}
}

/// This struct provides methods to edit the underlying text buffer that
/// Dear ImGui manipulates. Primarily, it gives [remove_chars](Self::remove_chars),
/// [insert_chars](Self::insert_chars), and mutable access to what text is selected.
pub struct TextCallbackData<'cb>(
    *mut sys::ImGuiInputTextCallbackData,
    std::marker::PhantomData<&'cb mut sys::ImGuiInputTextCallbackData>,
);

impl<'cb> TextCallbackData<'cb> {
    /// Creates the buffer.
    pub(super) unsafe fn new(data: *mut sys::ImGuiInputTextCallbackData) -> Self {
        Self(data, std::marker::PhantomData)
    }

    fn data(&self) -> &sys::ImGuiInputTextCallbackData {
        unsafe {
            self.0
                .as_ref()
                .expect("internal imgui error: InputText callback data was null")
        }
    }

    fn data_mut(&mut self) -> &mut sys::ImGuiInputTextCallbackData {
        unsafe {
            self.0
                .as_mut()
                .expect("internal imgui error: InputText callback data was null")
        }
    }

    fn valid_text_len(&self) -> usize {
        let data = self.data();
        assert!(!data.Buf.is_null(), "internal imgui error: Buf was null");
        assert!(
            data.BufTextLen >= 0,
            "internal imgui error: BufTextLen was negative"
        );
        assert!(
            data.BufSize >= 0,
            "internal imgui error: BufSize was negative"
        );
        assert!(
            data.BufTextLen <= data.BufSize,
            "internal imgui error: BufTextLen exceeded BufSize"
        );
        data.BufTextLen as usize
    }

    fn valid_text_len_i32(&self) -> i32 {
        self.valid_text_len() as i32
    }

    fn position(name: &str, pos: i32) -> usize {
        usize::try_from(pos).unwrap_or_else(|_| {
            panic!("internal imgui error: {name} was negative");
        })
    }

    fn position_to_i32(name: &str, pos: usize) -> i32 {
        i32::try_from(pos).unwrap_or_else(|_| {
            panic!("{name} exceeded ImGui's i32 position range");
        })
    }

    fn assert_byte_boundary(text: &str, name: &str, pos: usize) {
        assert!(
            text.is_char_boundary(pos),
            "{name} must lie on a UTF-8 character boundary"
        );
    }

    /// Get a reference to the text callback buffer's str.
    pub fn str(&self) -> &str {
        let len = self.valid_text_len();
        unsafe {
            std::str::from_utf8(std::slice::from_raw_parts(self.data().Buf as *const _, len))
                .expect("internal imgui error -- it boofed a utf8")
        }
    }

    /// Get the current cursor position
    pub fn cursor_pos(&self) -> usize {
        Self::position("CursorPos", self.data().CursorPos)
    }

    /// Set the cursor position
    pub fn set_cursor_pos(&mut self, pos: usize) {
        let text = self.str();
        assert!(pos <= text.len(), "cursor position out of bounds");
        Self::assert_byte_boundary(text, "cursor position", pos);
        self.data_mut().CursorPos = Self::position_to_i32("cursor position", pos);
    }

    /// Get the selection start position
    pub fn selection_start(&self) -> usize {
        Self::position("SelectionStart", self.data().SelectionStart)
    }

    /// Set the selection start position
    pub fn set_selection_start(&mut self, pos: usize) {
        let text = self.str();
        assert!(pos <= text.len(), "selection start out of bounds");
        Self::assert_byte_boundary(text, "selection start", pos);
        self.data_mut().SelectionStart = Self::position_to_i32("selection start", pos);
    }

    /// Get the selection end position
    pub fn selection_end(&self) -> usize {
        Self::position("SelectionEnd", self.data().SelectionEnd)
    }

    /// Set the selection end position
    pub fn set_selection_end(&mut self, pos: usize) {
        let text = self.str();
        assert!(pos <= text.len(), "selection end out of bounds");
        Self::assert_byte_boundary(text, "selection end", pos);
        self.data_mut().SelectionEnd = Self::position_to_i32("selection end", pos);
    }

    /// Select all text
    pub fn select_all(&mut self) {
        let len = self.valid_text_len_i32();
        let data = self.data_mut();
        data.SelectionStart = 0;
        data.SelectionEnd = len;
    }

    /// Clear selection
    pub fn clear_selection(&mut self) {
        let cursor_pos = self.data().CursorPos;
        let data = self.data_mut();
        data.SelectionStart = cursor_pos;
        data.SelectionEnd = cursor_pos;
    }

    /// Returns true if there is a selection
    pub fn has_selection(&self) -> bool {
        self.data().SelectionStart != self.data().SelectionEnd
    }

    /// Delete characters in the range [pos, pos+bytes_count)
    pub fn remove_chars(&mut self, pos: usize, bytes_count: usize) {
        let text = self.str();
        let end = pos
            .checked_add(bytes_count)
            .expect("delete range overflowed usize");
        assert!(end <= text.len(), "delete range out of bounds");
        Self::assert_byte_boundary(text, "delete start", pos);
        Self::assert_byte_boundary(text, "delete end", end);
        let pos = Self::position_to_i32("delete start", pos);
        let bytes_count = Self::position_to_i32("delete byte count", bytes_count);
        unsafe {
            sys::ImGuiInputTextCallbackData_DeleteChars(self.0, pos, bytes_count);
        }
    }

    /// Insert text at the given position
    pub fn insert_chars(&mut self, pos: usize, text: &str) {
        let current = self.str();
        assert!(pos <= current.len(), "insert position out of bounds");
        Self::assert_byte_boundary(current, "insert position", pos);
        let pos = Self::position_to_i32("insert position", pos);
        let text_ptr = text.as_ptr() as *const std::os::raw::c_char;
        unsafe {
            sys::ImGuiInputTextCallbackData_InsertChars(
                self.0,
                pos,
                text_ptr,
                text_ptr.add(text.len()),
            );
        }
    }

    /// Gives access to the underlying byte array MUTABLY.
    ///
    /// ## Safety
    ///
    /// This is very unsafe, and the following invariants must be
    /// upheld:
    /// 1. Keep the data utf8 valid.
    /// 2. After editing the string, call [set_dirty].
    ///
    /// To truncate the string, please use [remove_chars]. To extend
    /// the string, please use [insert_chars] and [push_str].
    ///
    /// This function should have highly limited usage, but could be for
    /// editing certain characters in the buffer based on some external condition.
    ///
    /// [remove_chars]: Self::remove_chars
    /// [set_dirty]: Self::set_dirty
    /// [insert_chars]: Self::insert_chars
    /// [push_str]: Self::push_str
    pub unsafe fn str_as_bytes_mut(&mut self) -> &mut [u8] {
        let len = self.valid_text_len();
        unsafe {
            let str = std::str::from_utf8_mut(std::slice::from_raw_parts_mut(
                self.data_mut().Buf as *mut u8,
                len,
            ))
            .expect("internal imgui error -- it boofed a utf8");

            str.as_bytes_mut()
        }
    }

    /// Sets the dirty flag on the text to imgui, indicating that
    /// it should reapply this string to its internal state.
    ///
    /// **NB:** You only need to use this method if you're using `[str_as_bytes_mut]`.
    /// If you use the helper methods [remove_chars] and [insert_chars],
    /// this will be set for you. However, this is no downside to setting
    /// the dirty flag spuriously except the minor CPU time imgui will spend.
    ///
    /// [str_as_bytes_mut]: Self::str_as_bytes_mut
    /// [remove_chars]: Self::remove_chars
    /// [insert_chars]: Self::insert_chars
    pub fn set_dirty(&mut self) {
        self.data_mut().BufDirty = true;
    }

    /// Returns the selected text directly. Note that if no text is selected,
    /// an empty str slice will be returned.
    pub fn selected(&self) -> &str {
        let text = self.str();
        let start = self.selection_start().min(self.selection_end());
        let end = self.selection_start().max(self.selection_end());
        assert!(end <= text.len(), "selection range out of bounds");
        Self::assert_byte_boundary(text, "selection start", start);
        Self::assert_byte_boundary(text, "selection end", end);
        &text[start..end]
    }

    /// Pushes the given str to the end of this buffer. If this
    /// would require the String to resize, it will be resized.
    /// This is automatically handled.
    pub fn push_str(&mut self, text: &str) {
        let current_len = self.valid_text_len();
        self.insert_chars(current_len, text);
    }
}

/// This is a ZST which implements InputTextCallbackHandler as a passthrough.
///
/// If you do not set a callback handler, this will be used (but will never
/// actually run, since you will not have passed imgui any flags).
pub struct PassthroughCallback;
impl InputTextCallbackHandler for PassthroughCallback {}

#[cfg(test)]
mod tests {
    use super::*;

    struct DefaultHandler;
    impl InputTextCallbackHandler for DefaultHandler {}

    #[test]
    fn default_char_filter_keeps_character() {
        let mut handler = DefaultHandler;
        assert_eq!(handler.char_filter('x'), Some('x'));
    }

    #[test]
    fn passthrough_char_filter_keeps_character() {
        let mut handler = PassthroughCallback;
        assert_eq!(handler.char_filter('x'), Some('x'));
    }
}
