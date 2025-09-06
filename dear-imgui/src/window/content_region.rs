use crate::sys;
use crate::Ui;

impl Ui {
    /// Returns the size of the content region available for widgets
    ///
    /// This is the size of the window minus decorations (title bar, scrollbars, etc.)
    #[doc(alias = "GetContentRegionAvail")]
    pub fn content_region_avail(&self) -> [f32; 2] {
        unsafe {
            #[cfg(target_env = "msvc")]
            {
                let size_rr = sys::ImGui_GetContentRegionAvail();
                let size: sys::ImVec2 = size_rr.into();
                [size.x, size.y]
            }
            #[cfg(not(target_env = "msvc"))]
            {
                let size = sys::ImGui_GetContentRegionAvail();
                [size.x, size.y]
            }
        }
    }

    /// Returns the width of the content region available for widgets
    ///
    /// This is equivalent to `content_region_avail()[0]`
    pub fn content_region_avail_width(&self) -> f32 {
        self.content_region_avail()[0]
    }

    /// Returns the height of the content region available for widgets
    ///
    /// This is equivalent to `content_region_avail()[1]`
    pub fn content_region_avail_height(&self) -> f32 {
        self.content_region_avail()[1]
    }
}
