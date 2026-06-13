use crate::Ui;
use crate::sys;

impl Ui {
    /// Return ~ FontSize.
    #[doc(alias = "GetTextLineHeight")]
    pub fn text_line_height(&self) -> f32 {
        self.run_with_bound_context(|| unsafe { sys::igGetTextLineHeight() })
    }

    /// Return ~ FontSize + style.ItemSpacing.y.
    #[doc(alias = "GetTextLineHeightWithSpacing")]
    pub fn text_line_height_with_spacing(&self) -> f32 {
        self.run_with_bound_context(|| unsafe { sys::igGetTextLineHeightWithSpacing() })
    }

    /// Return ~ FontSize + style.FramePadding.y * 2.
    #[doc(alias = "GetFrameHeight")]
    pub fn frame_height(&self) -> f32 {
        self.run_with_bound_context(|| unsafe { sys::igGetFrameHeight() })
    }

    /// Return ~ FontSize + style.FramePadding.y * 2 + style.ItemSpacing.y.
    #[doc(alias = "GetFrameHeightWithSpacing")]
    pub fn frame_height_with_spacing(&self) -> f32 {
        self.run_with_bound_context(|| unsafe { sys::igGetFrameHeightWithSpacing() })
    }
}
