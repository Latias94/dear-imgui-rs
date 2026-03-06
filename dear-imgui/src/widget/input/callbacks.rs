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
    fn char_filter(&mut self, _c: char) -> Option<char> {
        None
    }

    /// Called when the user presses the completion key (TAB by default).
    ///
    /// To make ImGui run this callback, use [InputTextCallback::COMPLETION].
    fn on_completion(&mut self, _data: TextCallbackData) {}

    /// Called when the user presses Up/Down arrow keys for history navigation.
    ///
    /// To make ImGui run this callback, use [InputTextCallback::HISTORY].
    fn on_history(&mut self, _direction: HistoryDirection, _data: TextCallbackData) {}

    /// Called every frame when the input text is active.
    ///
    /// To make ImGui run this callback, use [InputTextCallback::ALWAYS].
    fn on_always(&mut self, _data: TextCallbackData) {}

    /// Called when the text buffer is edited.
    ///
    /// To make ImGui run this callback, use [InputTextCallback::EDIT].
    fn on_edit(&mut self, _data: TextCallbackData) {}
}

/// This struct provides methods to edit the underlying text buffer that
/// Dear ImGui manipulates. Primarily, it gives [remove_chars](Self::remove_chars),
/// [insert_chars](Self::insert_chars), and mutable access to what text is selected.
pub struct TextCallbackData(*mut sys::ImGuiInputTextCallbackData);

impl TextCallbackData {
    /// Creates the buffer.
    pub(super) unsafe fn new(data: *mut sys::ImGuiInputTextCallbackData) -> Self {
        Self(data)
    }

    /// Get a reference to the text callback buffer's str.
    pub fn str(&self) -> &str {
        unsafe {
            std::str::from_utf8(std::slice::from_raw_parts(
                (*(self.0)).Buf as *const _,
                (*(self.0)).BufTextLen as usize,
            ))
            .expect("internal imgui error -- it boofed a utf8")
        }
    }

    /// Get the current cursor position
    pub fn cursor_pos(&self) -> usize {
        unsafe { (*(self.0)).CursorPos as usize }
    }

    /// Set the cursor position
    pub fn set_cursor_pos(&mut self, pos: usize) {
        unsafe {
            (*(self.0)).CursorPos = pos as i32;
        }
    }

    /// Get the selection start position
    pub fn selection_start(&self) -> usize {
        unsafe { (*(self.0)).SelectionStart as usize }
    }

    /// Set the selection start position
    pub fn set_selection_start(&mut self, pos: usize) {
        unsafe {
            (*(self.0)).SelectionStart = pos as i32;
        }
    }

    /// Get the selection end position
    pub fn selection_end(&self) -> usize {
        unsafe { (*(self.0)).SelectionEnd as usize }
    }

    /// Set the selection end position
    pub fn set_selection_end(&mut self, pos: usize) {
        unsafe {
            (*(self.0)).SelectionEnd = pos as i32;
        }
    }

    /// Select all text
    pub fn select_all(&mut self) {
        unsafe {
            (*(self.0)).SelectionStart = 0;
            (*(self.0)).SelectionEnd = (*(self.0)).BufTextLen;
        }
    }

    /// Clear selection
    pub fn clear_selection(&mut self) {
        unsafe {
            (*(self.0)).SelectionStart = (*(self.0)).CursorPos;
            (*(self.0)).SelectionEnd = (*(self.0)).CursorPos;
        }
    }

    /// Returns true if there is a selection
    pub fn has_selection(&self) -> bool {
        unsafe { (*(self.0)).SelectionStart != (*(self.0)).SelectionEnd }
    }

    /// Delete characters in the range [pos, pos+bytes_count)
    pub fn remove_chars(&mut self, pos: usize, bytes_count: usize) {
        unsafe {
            sys::ImGuiInputTextCallbackData_DeleteChars(self.0, pos as i32, bytes_count as i32);
        }
    }

    /// Insert text at the given position
    pub fn insert_chars(&mut self, pos: usize, text: &str) {
        let text_ptr = text.as_ptr() as *const std::os::raw::c_char;
        unsafe {
            sys::ImGuiInputTextCallbackData_InsertChars(
                self.0,
                pos as i32,
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
        unsafe {
            assert!(
                !(*(self.0)).Buf.is_null(),
                "internal imgui error: Buf was null"
            );
            assert!(
                (*(self.0)).BufTextLen >= 0,
                "internal imgui error: BufTextLen was negative"
            );
            assert!(
                (*(self.0)).BufSize >= 0,
                "internal imgui error: BufSize was negative"
            );
            assert!(
                (*(self.0)).BufTextLen <= (*(self.0)).BufSize,
                "internal imgui error: BufTextLen exceeded BufSize"
            );

            let str = std::str::from_utf8_mut(std::slice::from_raw_parts_mut(
                (*(self.0)).Buf as *mut u8,
                (*(self.0)).BufTextLen as usize,
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
        unsafe {
            (*(self.0)).BufDirty = true;
        }
    }

    /// Returns the selected text directly. Note that if no text is selected,
    /// an empty str slice will be returned.
    pub fn selected(&self) -> &str {
        let start = self.selection_start().min(self.selection_end());
        let end = self.selection_start().max(self.selection_end());
        &self.str()[start..end]
    }

    /// Pushes the given str to the end of this buffer. If this
    /// would require the String to resize, it will be resized.
    /// This is automatically handled.
    pub fn push_str(&mut self, text: &str) {
        let current_len = unsafe { (*(self.0)).BufTextLen as usize };
        self.insert_chars(current_len, text);
    }
}

/// This is a ZST which implements InputTextCallbackHandler as a passthrough.
///
/// If you do not set a callback handler, this will be used (but will never
/// actually run, since you will not have passed imgui any flags).
pub struct PassthroughCallback;
impl InputTextCallbackHandler for PassthroughCallback {}
