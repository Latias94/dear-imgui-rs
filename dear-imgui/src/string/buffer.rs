use std::os::raw::c_char;

/// Internal buffer for UI string operations
#[derive(Debug)]
pub struct UiBuffer {
    pub buffer: Vec<u8>,
    pub max_len: usize,
}

impl UiBuffer {
    /// Creates a new buffer with the specified capacity
    pub const fn new(max_len: usize) -> Self {
        Self {
            buffer: Vec::new(),
            max_len,
        }
    }

    /// Internal method to push a single text to our scratch buffer.
    ///
    /// Note: any interior NUL bytes (`'\0'`) will be replaced with `?` to preserve C string semantics.
    pub fn scratch_txt(&mut self, txt: impl AsRef<str>) -> *const c_char {
        self.refresh_buffer();

        let start_of_substr = self.push(txt);
        unsafe { self.offset(start_of_substr) }
    }

    /// Internal method to push an option text to our scratch buffer.
    pub fn scratch_txt_opt(&mut self, txt: Option<impl AsRef<str>>) -> *const c_char {
        match txt {
            Some(v) => self.scratch_txt(v),
            None => std::ptr::null(),
        }
    }

    /// Helper method, same as [`Self::scratch_txt`] but for two strings
    pub fn scratch_txt_two(
        &mut self,
        txt_0: impl AsRef<str>,
        txt_1: impl AsRef<str>,
    ) -> (*const c_char, *const c_char) {
        self.refresh_buffer();

        let first_offset = self.push(txt_0);
        let second_offset = self.push(txt_1);

        unsafe { (self.offset(first_offset), self.offset(second_offset)) }
    }

    /// Helper method, same as [`Self::scratch_txt`] but with one optional value
    pub fn scratch_txt_with_opt(
        &mut self,
        txt_0: impl AsRef<str>,
        txt_1: Option<impl AsRef<str>>,
    ) -> (*const c_char, *const c_char) {
        match txt_1 {
            Some(value) => self.scratch_txt_two(txt_0, value),
            None => (self.scratch_txt(txt_0), std::ptr::null()),
        }
    }

    /// Attempts to clear the buffer if it's over the maximum length allowed.
    /// This is to prevent us from making a giant vec over time.
    pub fn refresh_buffer(&mut self) {
        if self.buffer.len() > self.max_len {
            self.buffer.clear();
        }
    }

    /// Given a position, gives an offset from the start of the scratch buffer.
    ///
    /// # Safety
    /// This can return a pointer to undefined data if given a `pos >= self.buffer.len()`.
    /// This is marked as unsafe to reflect that.
    pub unsafe fn offset(&self, pos: usize) -> *const c_char {
        unsafe { self.buffer.as_ptr().add(pos) as *const _ }
    }

    /// Pushes a new scratch sheet text and return the byte index where the sub-string
    /// starts.
    pub fn push(&mut self, txt: impl AsRef<str>) -> usize {
        let txt = txt.as_ref();
        let len = self.buffer.len();
        let bytes = txt.as_bytes();
        if bytes.contains(&0) {
            self.buffer
                .extend(bytes.iter().map(|&b| if b == 0 { b'?' } else { b }));
        } else {
            self.buffer.extend(bytes);
        }
        self.buffer.push(b'\0');

        len
    }
}
