use crate::sys;
use std::borrow::Cow;
use std::str::Utf8Error;

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
/// Each method requires its matching callback flag. If a callback panics, the
/// widget skips further user callbacks and resumes the panic after Dear ImGui
/// has returned and the Rust string has been restored.
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
pub struct TextCallbackData<'cb>(&'cb mut sys::ImGuiInputTextCallbackData);

impl<'cb> TextCallbackData<'cb> {
    /// `data` must describe a live native text-edit callback buffer for `'cb`.
    pub(super) unsafe fn new(data: &'cb mut sys::ImGuiInputTextCallbackData) -> Self {
        Self(data)
    }

    fn data(&self) -> &sys::ImGuiInputTextCallbackData {
        self.0
    }

    fn data_mut(&mut self) -> &mut sys::ImGuiInputTextCallbackData {
        self.0
    }

    fn valid_text_len(&self) -> usize {
        let data = self.data();
        assert!(!data.Buf.is_null(), "internal imgui error: Buf was null");
        assert!(
            data.BufTextLen >= 0,
            "internal imgui error: BufTextLen was negative"
        );
        let editable_resize = data.Flags & sys::ImGuiInputTextFlags_CallbackResize != 0
            && data.Flags & sys::ImGuiInputTextFlags_ReadOnly == 0;
        let size = if editable_resize {
            // InputTextEx passes TextA to edit callbacks before resizing the caller's buffer.
            // BufSize can still describe that smaller caller buffer after typing or pasting.
            assert!(!data.Ctx.is_null(), "missing text callback context");
            let text = unsafe { &(*data.Ctx).InputTextState.TextA };
            assert_eq!(data.Buf, text.Data, "unexpected text callback buffer");
            assert!(
                text.Size <= text.Capacity,
                "internal imgui error: callback text buffer size exceeded capacity"
            );
            text.Size
        } else {
            data.BufSize
        };
        assert!(
            data.BufTextLen < size,
            "internal imgui error: callback text must fit its buffer with a trailing NUL"
        );
        data.BufTextLen as usize
    }

    fn valid_text_len_i32(&self) -> i32 {
        self.valid_text_len() as i32
    }

    fn position(name: &str, pos: i32, len: usize) -> usize {
        let pos = usize::try_from(pos).unwrap_or_else(|_| {
            panic!("internal imgui error: {name} was negative");
        });
        assert!(
            pos <= len,
            "internal imgui error: {name} exceeded BufTextLen"
        );
        pos
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

    fn text_bytes(&self) -> &[u8] {
        let len = self.valid_text_len();
        unsafe { std::slice::from_raw_parts(self.data().Buf.cast(), len) }
    }

    fn expect_utf8(&self) -> &str {
        self.str()
            .expect("invalid UTF-8 in InputText callback buffer")
    }

    /// Borrows the callback text, excluding the trailing NUL, or returns its UTF-8 error.
    ///
    /// # Panics
    ///
    /// Panics if native callback metadata does not describe an in-bounds buffer.
    pub fn str(&self) -> Result<&str, Utf8Error> {
        std::str::from_utf8(self.text_bytes())
    }

    /// Borrows valid text or replaces malformed UTF-8 sequences with U+FFFD.
    /// Uses the same buffer bounds checks as [`Self::str`].
    pub fn str_lossy(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(self.text_bytes())
    }

    /// Get the current cursor position
    pub fn cursor_pos(&self) -> usize {
        let len = self.valid_text_len();
        Self::position("CursorPos", self.data().CursorPos, len)
    }

    /// Set the cursor position
    pub fn set_cursor_pos(&mut self, pos: usize) {
        let text = self.expect_utf8();
        assert!(pos <= text.len(), "cursor position out of bounds");
        Self::assert_byte_boundary(text, "cursor position", pos);
        self.data_mut().CursorPos = Self::position_to_i32("cursor position", pos);
    }

    /// Get the selection start position
    pub fn selection_start(&self) -> usize {
        let len = self.valid_text_len();
        Self::position("SelectionStart", self.data().SelectionStart, len)
    }

    /// Set the selection start position
    pub fn set_selection_start(&mut self, pos: usize) {
        let text = self.expect_utf8();
        assert!(pos <= text.len(), "selection start out of bounds");
        Self::assert_byte_boundary(text, "selection start", pos);
        self.data_mut().SelectionStart = Self::position_to_i32("selection start", pos);
    }

    /// Get the selection end position
    pub fn selection_end(&self) -> usize {
        let len = self.valid_text_len();
        Self::position("SelectionEnd", self.data().SelectionEnd, len)
    }

    /// Set the selection end position
    pub fn set_selection_end(&mut self, pos: usize) {
        let text = self.expect_utf8();
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
        let cursor_pos = Self::position_to_i32("cursor position", self.cursor_pos());
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
        let text = self.expect_utf8();
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

    /// Insert text at the given byte position. Insertion stops at the first NUL.
    /// A read-only widget's callback can edit only within the caller's existing buffer.
    pub fn insert_chars(&mut self, pos: usize, text: &str) {
        let current = self.expect_utf8();
        assert!(pos <= current.len(), "insert position out of bounds");
        Self::assert_byte_boundary(current, "insert position", pos);
        let text = self.insertion_prefix(text, current.len());
        if text.is_empty() {
            return;
        }
        // InsertChars grows TextA using int arithmetic (up to 4 * insertion length).
        assert!(
            text.len() <= i32::MAX as usize / 4
                && current.len() + text.len().max(256) <= i32::MAX as usize - 2,
            "inserted text exceeds Dear ImGui's i32 growth range"
        );
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

    fn insertion_prefix<'a>(&self, text: &'a str, current_len: usize) -> &'a str {
        let text = text.split_once('\0').map_or(text, |(prefix, _)| prefix);
        let data = self.data();
        if data.Flags & sys::ImGuiInputTextFlags_ReadOnly != 0 {
            let available = data.BufSize as usize - current_len - 1;
            &text[..text.floor_char_boundary(available)]
        } else {
            text
        }
    }

    /// Gives access to the underlying byte array MUTABLY.
    ///
    /// ## Safety
    ///
    /// This is very unsafe, and the following invariants must be
    /// upheld:
    /// 1. Keep the data valid UTF-8 and free of NUL bytes.
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
        unsafe { std::slice::from_raw_parts_mut(self.data_mut().Buf.cast(), len) }
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
        let text = self.expect_utf8();
        let start = self.selection_start().min(self.selection_end());
        let end = self.selection_start().max(self.selection_end());
        assert!(end <= text.len(), "selection range out of bounds");
        Self::assert_byte_boundary(text, "selection start", start);
        Self::assert_byte_boundary(text, "selection end", end);
        &text[start..end]
    }

    /// Appends text using the same NUL and read-only capacity rules as [`Self::insert_chars`].
    /// Editable buffers grow as needed.
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

    fn callback_data_for_text(text: &mut [u8]) -> sys::ImGuiInputTextCallbackData {
        let len = text.len().saturating_sub(1);
        let mut data = sys::ImGuiInputTextCallbackData::default();
        data.Buf = text.as_mut_ptr().cast();
        data.BufTextLen = len as i32;
        data.BufSize = text.len() as i32;
        data.CursorPos = len as i32;
        data.SelectionStart = 0;
        data.SelectionEnd = len as i32;
        data
    }

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

    #[test]
    fn editable_callback_reads_native_text_before_caller_resize() {
        let mut text = *b"edited text\0";
        let mut data = callback_data_for_text(&mut text);
        let mut ctx = sys::ImGuiContext::default();
        ctx.InputTextState.TextA.Data = data.Buf;
        ctx.InputTextState.TextA.Size = data.BufSize;
        ctx.InputTextState.TextA.Capacity = data.BufSize;
        data.Ctx = &raw mut ctx;
        data.Flags = sys::ImGuiInputTextFlags_CallbackResize;
        data.BufSize = 4;

        let info = unsafe { TextCallbackData::new(&mut data) };
        assert_eq!(info.str(), Ok("edited text"));
        assert!(matches!(info.str_lossy(), Cow::Borrowed("edited text")));
    }

    #[test]
    fn callback_text_exposes_invalid_utf8_without_panicking() {
        let mut text = [b'a', 0xff, b'b', 0];
        let mut data = callback_data_for_text(&mut text);
        let info = unsafe { TextCallbackData::new(&mut data) };

        assert_eq!(info.str().unwrap_err().valid_up_to(), 1);
        assert_eq!(info.str_lossy(), "a\u{fffd}b");
    }

    #[test]
    #[should_panic(expected = "callback text must fit its buffer with a trailing NUL")]
    fn callback_text_rejects_a_length_outside_its_buffer() {
        let mut text = *b"abc\0";
        let mut data = callback_data_for_text(&mut text);
        data.BufTextLen = data.BufSize;
        let info = unsafe { TextCallbackData::new(&mut data) };

        let _ = info.str();
    }

    #[test]
    fn read_only_callback_limits_insertions_to_complete_chars_and_its_terminator() {
        let mut text = *b"ab\0\0\0\0";
        let mut data = callback_data_for_text(&mut text);
        data.BufTextLen = 2;
        data.Flags =
            (sys::ImGuiInputTextFlags_CallbackResize | sys::ImGuiInputTextFlags_ReadOnly) as i32;
        let info = unsafe { TextCallbackData::new(&mut data) };
        let len = info.str().unwrap().len();
        assert_eq!(info.insertion_prefix("éé", len), "é");
        assert_eq!(info.insertion_prefix("a\0b", len), "a");
    }

    #[test]
    fn text_callback_positions_accept_bounds() {
        let mut text = *b"abcd\0";
        let mut data = callback_data_for_text(&mut text);
        data.CursorPos = 4;
        data.SelectionStart = 1;
        data.SelectionEnd = 3;

        let info = unsafe { TextCallbackData::new(&mut data) };

        assert_eq!(info.cursor_pos(), 4);
        assert_eq!(info.selection_start(), 1);
        assert_eq!(info.selection_end(), 3);
        assert_eq!(info.selected(), "bc");
    }

    #[test]
    #[should_panic(expected = "CursorPos exceeded BufTextLen")]
    fn text_callback_cursor_pos_rejects_out_of_bounds() {
        let mut text = *b"abcd\0";
        let mut data = callback_data_for_text(&mut text);
        data.CursorPos = 5;

        let info = unsafe { TextCallbackData::new(&mut data) };
        let _ = info.cursor_pos();
    }

    #[test]
    #[should_panic(expected = "CursorPos exceeded BufTextLen")]
    fn text_callback_clear_selection_rejects_out_of_bounds_cursor() {
        let mut text = *b"abcd\0";
        let mut data = callback_data_for_text(&mut text);
        data.CursorPos = 5;

        let mut info = unsafe { TextCallbackData::new(&mut data) };
        info.clear_selection();
    }
}
