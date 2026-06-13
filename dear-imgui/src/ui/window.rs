use super::*;

impl Ui {
    /// Creates a window builder
    pub fn window<'ui>(
        &'ui self,
        name: impl Into<std::borrow::Cow<'ui, str>>,
    ) -> crate::window::Window<'ui> {
        crate::window::Window::new(self, name)
    }

    /// Focus a window by name, or clear focus from all windows.
    ///
    /// Passing `None` is equivalent to `ImGui::SetWindowFocus(NULL)` in the C++ API.
    /// This can be used to "unfocus" the entire UI (e.g. on Escape, to behave like
    /// clicking outside of the UI).
    #[doc(alias = "SetWindowFocus")]
    pub fn set_window_focus(&self, name: Option<&str>) {
        let name = name.map(|name| self.scratch_txt(name));
        self.run_with_bound_context(|| unsafe {
            match name {
                Some(name) => sys::igSetWindowFocus_Str(name),
                None => sys::igSetWindowFocus_Nil(),
            }
        });
    }

    /// Sets the position of the current window.
    #[doc(alias = "SetWindowPos")]
    pub fn set_window_pos(&self, pos: [f32; 2]) {
        self.set_window_pos_with_cond(pos, crate::Condition::Always);
    }

    /// Sets the position of the current window with a condition.
    #[doc(alias = "SetWindowPos")]
    pub fn set_window_pos_with_cond(&self, pos: [f32; 2], cond: crate::Condition) {
        Self::assert_finite_vec2("Ui::set_window_pos_with_cond()", "position", pos);
        let pos_vec = sys::ImVec2_c {
            x: pos[0],
            y: pos[1],
        };
        self.run_with_bound_context(|| unsafe {
            sys::igSetWindowPos_Vec2(pos_vec, cond as sys::ImGuiCond)
        });
    }

    /// Sets the position of a named window.
    #[doc(alias = "SetWindowPos")]
    pub fn set_window_pos_by_name(&self, name: impl AsRef<str>, pos: [f32; 2]) {
        self.set_window_pos_by_name_with_cond(name, pos, crate::Condition::Always);
    }

    /// Sets the position of a named window with a condition.
    #[doc(alias = "SetWindowPos")]
    pub fn set_window_pos_by_name_with_cond(
        &self,
        name: impl AsRef<str>,
        pos: [f32; 2],
        cond: crate::Condition,
    ) {
        Self::assert_finite_vec2("Ui::set_window_pos_by_name_with_cond()", "position", pos);
        let pos_vec = sys::ImVec2_c {
            x: pos[0],
            y: pos[1],
        };
        let name = self.scratch_txt(name);
        self.run_with_bound_context(|| unsafe {
            sys::igSetWindowPos_Str(name, pos_vec, cond as sys::ImGuiCond)
        });
    }

    /// Sets the size of the current window.
    #[doc(alias = "SetWindowSize")]
    pub fn set_window_size(&self, size: [f32; 2]) {
        self.set_window_size_with_cond(size, crate::Condition::Always);
    }

    /// Sets the size of the current window with a condition.
    #[doc(alias = "SetWindowSize")]
    pub fn set_window_size_with_cond(&self, size: [f32; 2], cond: crate::Condition) {
        Self::assert_finite_vec2("Ui::set_window_size_with_cond()", "size", size);
        let size_vec = sys::ImVec2_c {
            x: size[0],
            y: size[1],
        };
        self.run_with_bound_context(|| unsafe {
            sys::igSetWindowSize_Vec2(size_vec, cond as sys::ImGuiCond)
        });
    }

    /// Sets the size of a named window.
    #[doc(alias = "SetWindowSize")]
    pub fn set_window_size_by_name(&self, name: impl AsRef<str>, size: [f32; 2]) {
        self.set_window_size_by_name_with_cond(name, size, crate::Condition::Always);
    }

    /// Sets the size of a named window with a condition.
    #[doc(alias = "SetWindowSize")]
    pub fn set_window_size_by_name_with_cond(
        &self,
        name: impl AsRef<str>,
        size: [f32; 2],
        cond: crate::Condition,
    ) {
        Self::assert_finite_vec2("Ui::set_window_size_by_name_with_cond()", "size", size);
        let size_vec = sys::ImVec2_c {
            x: size[0],
            y: size[1],
        };
        let name = self.scratch_txt(name);
        self.run_with_bound_context(|| unsafe {
            sys::igSetWindowSize_Str(name, size_vec, cond as sys::ImGuiCond);
        });
    }

    /// Collapses or expands the current window.
    #[doc(alias = "SetWindowCollapsed")]
    pub fn set_window_collapsed(&self, collapsed: bool) {
        self.set_window_collapsed_with_cond(collapsed, crate::Condition::Always);
    }

    /// Collapses or expands the current window with a condition.
    #[doc(alias = "SetWindowCollapsed")]
    pub fn set_window_collapsed_with_cond(&self, collapsed: bool, cond: crate::Condition) {
        self.run_with_bound_context(|| unsafe {
            sys::igSetWindowCollapsed_Bool(collapsed, cond as sys::ImGuiCond)
        });
    }

    /// Collapses or expands a named window.
    #[doc(alias = "SetWindowCollapsed")]
    pub fn set_window_collapsed_by_name(&self, name: impl AsRef<str>, collapsed: bool) {
        self.set_window_collapsed_by_name_with_cond(name, collapsed, crate::Condition::Always);
    }

    /// Collapses or expands a named window with a condition.
    #[doc(alias = "SetWindowCollapsed")]
    pub fn set_window_collapsed_by_name_with_cond(
        &self,
        name: impl AsRef<str>,
        collapsed: bool,
        cond: crate::Condition,
    ) {
        let name = self.scratch_txt(name);
        self.run_with_bound_context(|| unsafe {
            sys::igSetWindowCollapsed_Str(name, collapsed, cond as sys::ImGuiCond);
        });
    }

    /// Returns DPI scale currently associated to the current window's viewport.
    #[doc(alias = "GetWindowDpiScale")]
    pub fn window_dpi_scale(&self) -> f32 {
        self.run_with_bound_context(|| unsafe { sys::igGetWindowDpiScale() })
    }

    /// Get current window width (shortcut for `GetWindowSize().x`).
    #[doc(alias = "GetWindowWidth")]
    pub fn window_width(&self) -> f32 {
        self.run_with_bound_context(|| unsafe { sys::igGetWindowWidth() })
    }

    /// Get current window height (shortcut for `GetWindowSize().y`).
    #[doc(alias = "GetWindowHeight")]
    pub fn window_height(&self) -> f32 {
        self.run_with_bound_context(|| unsafe { sys::igGetWindowHeight() })
    }

    /// Get current window position in screen space.
    #[doc(alias = "GetWindowPos")]
    pub fn window_pos(&self) -> [f32; 2] {
        let v = self.run_with_bound_context(|| unsafe { sys::igGetWindowPos() });
        [v.x, v.y]
    }

    /// Get current window size.
    #[doc(alias = "GetWindowSize")]
    pub fn window_size(&self) -> [f32; 2] {
        let v = self.run_with_bound_context(|| unsafe { sys::igGetWindowSize() });
        [v.x, v.y]
    }
}
