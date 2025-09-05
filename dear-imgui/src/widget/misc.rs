use crate::sys;
use crate::Ui;

impl Ui {
    /// Creates a separator (horizontal line)
    #[doc(alias = "Separator")]
    pub fn separator(&self) {
        unsafe {
            sys::ImGui_Separator();
        }
    }

    /// Creates a vertical separator
    #[doc(alias = "SeparatorEx")]
    pub fn separator_vertical(&self) {
        unsafe {
            sys::ImGui_SeparatorEx(sys::ImGuiSeparatorFlags_Vertical, 0.0);
        }
    }

    /// Creates a horizontal separator
    #[doc(alias = "SeparatorEx")]
    pub fn separator_horizontal(&self) {
        unsafe {
            sys::ImGui_SeparatorEx(sys::ImGuiSeparatorFlags_Horizontal, 0.0);
        }
    }

    /// Creates a spacing between widgets
    #[doc(alias = "Spacing")]
    pub fn spacing(&self) {
        unsafe {
            sys::ImGui_Spacing();
        }
    }

    /// Creates a dummy widget of the given size
    #[doc(alias = "Dummy")]
    pub fn dummy(&self, size: impl Into<[f32; 2]>) {
        let size_vec: sys::ImVec2 = size.into().into();
        unsafe {
            sys::ImGui_Dummy(&size_vec);
        }
    }

    /// Moves the cursor to a new line
    #[doc(alias = "NewLine")]
    pub fn new_line(&self) {
        unsafe {
            sys::ImGui_NewLine();
        }
    }

    /// Indents the following widgets
    #[doc(alias = "Indent")]
    pub fn indent(&self) {
        unsafe {
            sys::ImGui_Indent(0.0);
        }
    }

    /// Indents the following widgets by the given amount
    #[doc(alias = "Indent")]
    pub fn indent_by(&self, indent_w: f32) {
        unsafe {
            sys::ImGui_Indent(indent_w);
        }
    }

    /// Unindents the following widgets
    #[doc(alias = "Unindent")]
    pub fn unindent(&self) {
        unsafe {
            sys::ImGui_Unindent(0.0);
        }
    }

    /// Unindents the following widgets by the given amount
    #[doc(alias = "Unindent")]
    pub fn unindent_by(&self, indent_w: f32) {
        unsafe {
            sys::ImGui_Unindent(indent_w);
        }
    }

    /// Creates a bullet point
    #[doc(alias = "Bullet")]
    pub fn bullet(&self) {
        unsafe {
            sys::ImGui_Bullet();
        }
    }

    /// Creates a bullet point with text
    #[doc(alias = "BulletText")]
    pub fn bullet_text(&self, text: impl AsRef<str>) {
        let text_ptr = self.scratch_txt(text);
        unsafe {
            sys::ImGui_BulletText(text_ptr);
        }
    }
}

impl Ui {
    /// Creates a small button
    #[doc(alias = "SmallButton")]
    pub fn small_button(&self, label: impl AsRef<str>) -> bool {
        let label_ptr = self.scratch_txt(label);
        unsafe { sys::ImGui_SmallButton(label_ptr) }
    }

    /// Creates an invisible button
    #[doc(alias = "InvisibleButton")]
    pub fn invisible_button(&self, str_id: impl AsRef<str>, size: impl Into<[f32; 2]>) -> bool {
        self.invisible_button_flags(str_id, size, crate::widget::ButtonFlags::NONE)
    }

    /// Creates an invisible button with flags
    #[doc(alias = "InvisibleButton")]
    pub fn invisible_button_flags(
        &self,
        str_id: impl AsRef<str>,
        size: impl Into<[f32; 2]>,
        flags: crate::widget::ButtonFlags,
    ) -> bool {
        let id_ptr = self.scratch_txt(str_id);
        let size_vec: sys::ImVec2 = size.into().into();
        unsafe { sys::ImGui_InvisibleButton(id_ptr, &size_vec, flags.bits()) }
    }

    /// Creates an arrow button
    #[doc(alias = "ArrowButton")]
    pub fn arrow_button(&self, str_id: impl AsRef<str>, dir: ArrowDirection) -> bool {
        let id_ptr = self.scratch_txt(str_id);
        unsafe { sys::ImGui_ArrowButton(id_ptr, dir as i32) }
    }
}

/// Direction for arrow buttons
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum ArrowDirection {
    Left = sys::ImGuiDir_Left,
    Right = sys::ImGuiDir_Right,
    Up = sys::ImGuiDir_Up,
    Down = sys::ImGuiDir_Down,
}
