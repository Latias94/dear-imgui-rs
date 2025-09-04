use crate::types::Vec2;
/// Layout widgets
///
/// This module contains all layout-related UI components like separators, spacing, etc.
use crate::ui::Ui;
use dear_imgui_sys as sys;

/// # Widgets: Layout
impl<'frame> Ui<'frame> {
    /// Display a horizontal separator line
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// ui.text("Above separator");
    /// ui.separator();
    /// ui.text("Below separator");
    /// # });
    /// ```
    pub fn separator(&mut self) {
        unsafe {
            sys::ImGui_Separator();
        }
    }

    /// Add spacing between widgets
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// ui.text("First line");
    /// ui.spacing();
    /// ui.text("Second line with extra space above");
    /// # });
    /// ```
    pub fn spacing(&mut self) {
        unsafe {
            sys::ImGui_Spacing();
        }
    }

    /// Move to the same line as the previous widget
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// ui.text("First");
    /// ui.same_line();
    /// ui.text("Second (on same line)");
    /// # });
    /// ```
    pub fn same_line(&mut self) {
        unsafe {
            sys::ImGui_SameLine(0.0, -1.0);
        }
    }

    /// Move to the same line with custom spacing
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// ui.text("First");
    /// ui.same_line_with_spacing(100.0, 20.0);
    /// ui.text("Second (with custom spacing)");
    /// # });
    /// ```
    pub fn same_line_with_spacing(&mut self, offset_from_start_x: f32, spacing: f32) {
        unsafe {
            sys::ImGui_SameLine(offset_from_start_x, spacing);
        }
    }

    /// Add a new line
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// ui.text("First line");
    /// ui.new_line();
    /// ui.text("Second line");
    /// # });
    /// ```
    pub fn new_line(&mut self) {
        unsafe {
            sys::ImGui_NewLine();
        }
    }

    /// Indent the next widgets
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// ui.text("Normal text");
    /// ui.indent(20.0);
    /// ui.text("Indented text");
    /// ui.unindent(20.0);
    /// ui.text("Normal text again");
    /// # });
    /// ```
    pub fn indent(&mut self, indent_w: f32) {
        unsafe {
            sys::ImGui_Indent(indent_w);
        }
    }

    /// Unindent the next widgets
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// ui.indent(20.0);
    /// ui.text("Indented text");
    /// ui.unindent(20.0);
    /// ui.text("Normal text");
    /// # });
    /// ```
    pub fn unindent(&mut self, indent_w: f32) {
        unsafe {
            sys::ImGui_Unindent(indent_w);
        }
    }

    /// Display a progress bar
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// ui.progress_bar(0.7, "70%");
    /// # });
    /// ```
    pub fn progress_bar(&mut self, fraction: f32, overlay: Option<&str>) {
        let overlay_ptr = self.scratch_txt_opt(overlay);

        unsafe {
            sys::ImGui_ProgressBar(
                fraction,
                &sys::ImVec2 { x: -1.0, y: 0.0 } as *const _, // Default size
                overlay_ptr,
            );
        }
    }

    /// Display a bullet point
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// ui.bullet();
    /// ui.same_line();
    /// ui.text("Bullet point item");
    /// # });
    /// ```
    pub fn bullet(&mut self) {
        unsafe {
            sys::ImGui_Bullet();
        }
    }

    /// Add a dummy item of given size
    ///
    /// This is useful for custom positioning or spacing.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::{Context, Vec2};
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// ui.text("Before dummy");
    /// ui.dummy(Vec2::new(100.0, 50.0)); // 100x50 empty space
    /// ui.text("After dummy");
    /// # });
    /// ```
    pub fn dummy(&mut self, size: Vec2) {
        let size_vec = sys::ImVec2 {
            x: size.x,
            y: size.y,
        };
        unsafe {
            sys::ImGui_Dummy(&size_vec as *const _);
        }
    }

    /// Begin a group (lock horizontal starting position)
    ///
    /// Must call `end_group()` after adding widgets.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// ui.begin_group();
    /// ui.text("Item 1");
    /// ui.text("Item 2");
    /// ui.end_group();
    /// ui.same_line();
    /// ui.text("Next to group");
    /// # });
    /// ```
    pub fn begin_group(&mut self) {
        unsafe {
            sys::ImGui_BeginGroup();
        }
    }

    /// End group (must be called after begin_group)
    pub fn end_group(&mut self) {
        unsafe {
            sys::ImGui_EndGroup();
        }
    }

    /// Begin a child window
    ///
    /// Returns `true` if the child window is visible and should be populated.
    /// Must call `end_child()` if this returns true.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::{Context, Vec2};
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// if ui.begin_child("child1", Vec2::new(200.0, 100.0), true) {
    ///     ui.text("Inside child window");
    ///     ui.button("Child button");
    ///     ui.end_child();
    /// }
    /// # });
    /// ```
    pub fn begin_child(
        &mut self,
        str_id: impl AsRef<str>,
        size: crate::types::Vec2,
        border: bool,
    ) -> bool {
        let size_vec = sys::ImVec2 {
            x: size.x,
            y: size.y,
        };
        let child_flags = if border { 1 } else { 0 }; // ImGuiChildFlags_Border = 1
        unsafe {
            sys::ImGui_BeginChild(
                self.scratch_txt(str_id),
                &size_vec as *const _,
                child_flags,
                0, // Default window flags
            )
        }
    }

    /// End child window (must be called after begin_child returns true)
    pub fn end_child(&mut self) {
        unsafe {
            sys::ImGui_EndChild();
        }
    }
}
