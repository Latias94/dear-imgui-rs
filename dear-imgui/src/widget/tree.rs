use crate::ui::Ui;
use dear_imgui_sys as sys;
/// Tree widgets
///
/// This module contains all tree-related UI components like tree nodes, collapsing headers, etc.
use std::ffi::CString;

/// # Widgets: Tree
impl<'frame> Ui<'frame> {
    /// Create a tree node
    ///
    /// Returns `true` if the tree node is open (expanded).
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// if ui.tree_node("Root Node") {
    ///     ui.text("Child item 1");
    ///     ui.text("Child item 2");
    ///     if ui.tree_node("Sub Node") {
    ///         ui.text("Nested item");
    ///         ui.tree_pop();
    ///     }
    ///     ui.tree_pop();
    /// }
    /// # });
    /// ```
    pub fn tree_node(&mut self, label: impl AsRef<str>) -> bool {
        let label = label.as_ref();
        let c_label = CString::new(label).unwrap_or_default();
        unsafe { sys::ImGui_TreeNode(c_label.as_ptr()) }
    }

    /// Create a tree node with flags
    ///
    /// Returns `true` if the tree node is open (expanded).
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// // Create a leaf node (no arrow, cannot be opened)
    /// if ui.tree_node_ex("Leaf Node", 1 << 9) { // ImGuiTreeNodeFlags_Leaf
    ///     ui.text("This is a leaf");
    ///     ui.tree_pop();
    /// }
    /// # });
    /// ```
    pub fn tree_node_ex(&mut self, label: impl AsRef<str>, flags: i32) -> bool {
        let label = label.as_ref();
        let c_label = CString::new(label).unwrap_or_default();
        unsafe { sys::ImGui_TreeNodeEx(c_label.as_ptr(), flags) }
    }

    /// Pop tree node (must be called after tree_node returns true)
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// if ui.tree_node("Node") {
    ///     ui.text("Content");
    ///     ui.tree_pop(); // Always call this!
    /// }
    /// # });
    /// ```
    pub fn tree_pop(&mut self) {
        unsafe {
            sys::ImGui_TreePop();
        }
    }

    /// Create a collapsing header
    ///
    /// Returns `true` if the header is open (expanded).
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// if ui.collapsing_header("Settings") {
    ///     ui.text("Setting 1");
    ///     ui.text("Setting 2");
    /// }
    /// # });
    /// ```
    pub fn collapsing_header(&mut self, label: impl AsRef<str>) -> bool {
        let label = label.as_ref();
        let c_label = CString::new(label).unwrap_or_default();
        unsafe { sys::ImGui_CollapsingHeader(c_label.as_ptr(), 0) }
    }
}
