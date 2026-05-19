use crate::io::Io;
use crate::sys;

impl Io {
    /// Returns the mouse delta from the previous frame, in pixels.
    #[doc(alias = "MouseDelta")]
    pub fn mouse_delta(&self) -> [f32; 2] {
        let delta = self.inner().MouseDelta;
        [delta.x, delta.y]
    }

    /// Returns whether Ctrl modifier is held.
    #[doc(alias = "KeyCtrl")]
    pub fn key_ctrl(&self) -> bool {
        self.inner().KeyCtrl
    }

    /// Returns whether Shift modifier is held.
    #[doc(alias = "KeyShift")]
    pub fn key_shift(&self) -> bool {
        self.inner().KeyShift
    }

    /// Returns whether Alt modifier is held.
    #[doc(alias = "KeyAlt")]
    pub fn key_alt(&self) -> bool {
        self.inner().KeyAlt
    }

    /// Returns whether Super/Command modifier is held.
    #[doc(alias = "KeySuper")]
    pub fn key_super(&self) -> bool {
        self.inner().KeySuper
    }

    /// Returns the current mouse input source.
    #[doc(alias = "MouseSource")]
    pub fn mouse_source(&self) -> crate::input::MouseSource {
        match self.inner().MouseSource {
            sys::ImGuiMouseSource_Mouse => crate::input::MouseSource::Mouse,
            sys::ImGuiMouseSource_TouchScreen => crate::input::MouseSource::TouchScreen,
            sys::ImGuiMouseSource_Pen => crate::input::MouseSource::Pen,
            _ => crate::input::MouseSource::Mouse,
        }
    }

    /// Returns the viewport id hovered by the OS mouse (if supported by the backend).
    #[doc(alias = "MouseHoveredViewport")]
    pub fn mouse_hovered_viewport(&self) -> crate::Id {
        crate::Id::from(self.inner().MouseHoveredViewport)
    }

    /// Returns whether Ctrl+LeftClick should be treated as RightClick.
    #[doc(alias = "MouseCtrlLeftAsRightClick")]
    pub fn mouse_ctrl_left_as_right_click(&self) -> bool {
        self.inner().MouseCtrlLeftAsRightClick
    }

    /// Set whether Ctrl+LeftClick should be treated as RightClick.
    #[doc(alias = "MouseCtrlLeftAsRightClick")]
    pub fn set_mouse_ctrl_left_as_right_click(&mut self, enabled: bool) {
        self.inner_mut().MouseCtrlLeftAsRightClick = enabled;
    }
}
