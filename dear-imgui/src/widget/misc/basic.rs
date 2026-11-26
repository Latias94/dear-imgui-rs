use crate::Ui;
use crate::sys;

impl Ui {
    /// Creates a bullet point
    #[doc(alias = "Bullet")]
    pub fn bullet(&self) {
        self.run_with_bound_context(|| unsafe {
            sys::igBullet();
        });
    }

    /// Creates a bullet point with text
    #[doc(alias = "BulletText")]
    pub fn bullet_text(&self, text: impl AsRef<str>) {
        self.with_scratch_txt(text, |text_ptr| unsafe {
            // Always treat the value as unformatted user text.
            const FMT: &[u8; 3] = b"%s\0";
            sys::igBulletText(FMT.as_ptr() as *const std::os::raw::c_char, text_ptr);
        });
    }
}

impl Ui {
    /// Creates a small button
    #[doc(alias = "SmallButton")]
    pub fn small_button(&self, label: impl AsRef<str>) -> bool {
        self.with_scratch_txt(label, |label_ptr| unsafe { sys::igSmallButton(label_ptr) })
    }
}
