//! Node-related functionality for ImNodeFlow

use crate::*;
use std::ffi::{CStr, CString};
use std::marker::PhantomData;

/// Represents a node in the node editor
pub struct Node {
    pub(crate) ptr: sys::BaseNodePtr,
    pub(crate) _phantom: PhantomData<*mut ()>, // Ensure !Send + !Sync for raw pointer
}

impl Node {
    /// Create a new node
    pub fn new() -> Result<Self> {
        let ptr = unsafe { sys::BaseNode_Create() };

        if ptr.is_null() {
            return Err(NodeFlowError::NodeCreationFailed);
        }

        Ok(Self {
            ptr,
            _phantom: PhantomData,
        })
    }

    /// Set the node's title
    pub fn set_title(&mut self, title: &str) -> Result<()> {
        let title_cstr = CString::new(title)?;
        unsafe {
            sys::BaseNode_SetTitle(self.ptr, title_cstr.as_ptr());
        }
        Ok(())
    }

    /// Set the node's position in grid coordinates
    pub fn set_position(&mut self, pos: ImVec2) {
        unsafe {
            sys::BaseNode_SetPos(self.ptr, pos.x, pos.y);
        }
    }

    /// Set the node editor handler for this node
    pub fn set_handler(&mut self, editor: &NodeEditor) {
        unsafe {
            sys::BaseNode_SetHandler(self.ptr, editor.as_ptr());
        }
    }

    /// Set the node's style
    pub fn set_style(&mut self, style: &NodeStyle) {
        unsafe {
            sys::BaseNode_SetStyle(self.ptr, style.as_ptr());
        }
    }

    /// Set the node's selected state
    pub fn set_selected(&mut self, selected: bool) {
        unsafe {
            sys::BaseNode_Selected(self.ptr, selected);
        }
    }

    /// Update the node's public status
    pub fn update_public_status(&mut self) {
        unsafe {
            sys::BaseNode_UpdatePublicStatus(self.ptr);
        }
    }

    /// Mark the node for destruction
    pub fn destroy(&mut self) {
        unsafe {
            sys::BaseNode_Destroy_Node(self.ptr);
        }
    }

    /// Check if the node should be destroyed
    pub fn should_destroy(&self) -> bool {
        unsafe { sys::BaseNode_ToDestroy(self.ptr) }
    }

    /// Check if the node is hovered
    pub fn is_hovered(&self) -> bool {
        unsafe { sys::BaseNode_IsHovered(self.ptr) }
    }

    /// Check if the node is selected
    pub fn is_selected(&self) -> bool {
        unsafe { sys::BaseNode_IsSelected(self.ptr) }
    }

    /// Check if the node is being dragged
    pub fn is_dragged(&self) -> bool {
        unsafe { sys::BaseNode_IsDragged(self.ptr) }
    }

    /// Get the node's unique identifier
    pub fn uid(&self) -> sys::NodeUID {
        unsafe { sys::BaseNode_GetUID(self.ptr) }
    }

    /// Get the node's name
    pub fn name(&self) -> Result<String> {
        unsafe {
            let name_ptr = sys::BaseNode_GetName(self.ptr);
            if name_ptr.is_null() {
                return Ok(String::new());
            }
            let name_cstr = CStr::from_ptr(name_ptr);
            Ok(name_cstr.to_str()?.to_string())
        }
    }

    /// Get the node's size
    pub fn size(&self) -> ImVec2 {
        unsafe { sys::BaseNode_GetSize(self.ptr) }
    }

    /// Get the node's position
    pub fn position(&self) -> ImVec2 {
        unsafe { sys::BaseNode_GetPos(self.ptr) }
    }

    /// Get the node editor handler
    pub fn handler(&self) -> Option<NodeEditor> {
        unsafe {
            let handler_ptr = sys::BaseNode_GetHandler(self.ptr);
            if handler_ptr.is_null() {
                None
            } else {
                // Note: This creates a new NodeEditor wrapper around the existing pointer
                // This is potentially unsafe if the original NodeEditor is dropped
                // In practice, the handler should outlive the node
                Some(NodeEditor {
                    ptr: handler_ptr,
                    name: CString::new("").unwrap(), // We don't have access to the original name
                })
            }
        }
    }

    /// Get the node's style
    pub fn style(&self) -> Option<NodeStyle> {
        unsafe {
            let style_ptr = sys::BaseNode_GetStyle(self.ptr);
            if style_ptr.is_null() {
                None
            } else {
                Some(NodeStyle::from_ptr(style_ptr))
            }
        }
    }

    /// Update the node (should be called every frame)
    pub fn update(&mut self) {
        unsafe {
            sys::BaseNode_Update(self.ptr);
        }
    }

    /// Get the raw pointer (for advanced usage)
    pub fn as_ptr(&self) -> sys::BaseNodePtr {
        self.ptr
    }
}

impl Drop for Node {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe {
                sys::BaseNode_Destroy(self.ptr);
            }
        }
    }
}

/// Builder for creating nodes with a fluent API
pub struct NodeBuilder {
    title: Option<String>,
    position: Option<ImVec2>,
    style: Option<NodeStyle>,
    selected: bool,
}

impl NodeBuilder {
    /// Create a new node builder
    pub fn new() -> Self {
        Self {
            title: None,
            position: None,
            style: None,
            selected: false,
        }
    }

    /// Set the node's title
    pub fn title<S: Into<String>>(mut self, title: S) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set the node's position
    pub fn position(mut self, pos: ImVec2) -> Self {
        self.position = Some(pos);
        self
    }

    /// Set the node's style
    pub fn style(mut self, style: NodeStyle) -> Self {
        self.style = Some(style);
        self
    }

    /// Set the node as selected
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Build the node
    pub fn build(self) -> Result<Node> {
        let mut node = Node::new()?;

        if let Some(title) = self.title {
            node.set_title(&title)?;
        }

        if let Some(position) = self.position {
            node.set_position(position);
        }

        if let Some(style) = self.style {
            node.set_style(&style);
        }

        node.set_selected(self.selected);

        Ok(node)
    }
}

impl Default for NodeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_node() {
        let node = Node::new().unwrap();
        assert!(!node.as_ptr().is_null());
        assert_eq!(node.uid(), 0); // Default UID
    }

    #[test]
    fn test_node_builder() {
        let node = NodeBuilder::new()
            .title("Test Node")
            .position(ImVec2 { x: 100.0, y: 200.0 })
            .selected(true)
            .build()
            .unwrap();

        assert_eq!(node.name().unwrap(), "Test Node");
        assert_eq!(node.position().x, 100.0);
        assert_eq!(node.position().y, 200.0);
        assert!(node.is_selected());
    }

    #[test]
    fn test_node_properties() {
        let mut node = Node::new().unwrap();

        node.set_title("Test").unwrap();
        assert_eq!(node.name().unwrap(), "Test");

        let pos = ImVec2 { x: 50.0, y: 75.0 };
        node.set_position(pos);
        assert_eq!(node.position().x, pos.x);
        assert_eq!(node.position().y, pos.y);

        node.set_selected(true);
        node.update_public_status();
        assert!(node.is_selected());
    }
}
