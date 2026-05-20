use super::*;

impl Ui {
    pub(super) fn assert_finite_f32(caller: &str, name: &str, value: f32) {
        assert!(value.is_finite(), "{caller} {name} must be finite");
    }

    pub(super) fn assert_finite_vec2(caller: &str, name: &str, value: [f32; 2]) {
        assert!(
            value[0].is_finite() && value[1].is_finite(),
            "{caller} {name} must contain finite values"
        );
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
        unsafe {
            let io = sys::igGetIO_Nil();
            if io.is_null() {
                panic!("Ui::io() requires an active ImGui context");
            }
            &*(io as *const crate::io::Io)
        }
    }

    /// Internal method to push a single text to our scratch buffer.
    pub(crate) fn scratch_txt(&self, txt: impl AsRef<str>) -> *const std::os::raw::c_char {
        unsafe {
            let handle = &mut *self.buffer.get();
            handle.scratch_txt(txt)
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

    /// Returns an ID from a string label in the current ID scope.
    ///
    /// This mirrors `ImGui::GetID(label)`. Useful for building stable IDs
    /// for widgets or dockspaces inside the current window/scope.
    #[doc(alias = "GetID")]
    pub fn get_id(&self, label: &str) -> Id {
        unsafe { Id::from(sys::igGetID_Str(self.scratch_txt(label))) }
    }
}
