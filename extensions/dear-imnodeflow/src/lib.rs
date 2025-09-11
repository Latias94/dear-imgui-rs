//! # Dear ImNodeFlow - Rust Bindings with Dear ImGui Compatibility
//!
//! High-level Rust bindings for ImNodeFlow, the immediate mode node editor library.
//! This crate provides safe, idiomatic Rust bindings designed to work seamlessly
//! with dear-imgui (C++ bindgen) rather than imgui-rs (cimgui).
//!
//! ## Features
//!
//! - Safe, idiomatic Rust API
//! - Full compatibility with dear-imgui
//! - Builder pattern for nodes and pins
//! - Memory-safe string handling
//! - Support for all node editor features
//!
//! ## Quick Start
//!
//! ```no_run
//! use dear_imgui::*;
//! use dear_imnodeflow::*;
//!
//! let mut ctx = Context::create_or_panic();
//! let mut node_editor = NodeEditor::new("My Editor");
//!
//! let ui = ctx.frame();
//! node_editor.update(&ui);
//! ```
//!
//! ## Integration with Dear ImGui
//!
//! This crate is designed to work with the `dear-imgui` ecosystem:
//! - Uses the same context management patterns
//! - Compatible with dear-imgui's UI tokens and lifetime management
//! - Shares the same underlying Dear ImGui context

use dear_imnodeflow_sys as sys;
use std::ffi::{CStr, CString};
use std::marker::PhantomData;
use std::ptr;

// Re-export essential types
pub use dear_imgui::{Context, Ui};
pub use sys::{ImU32, ImVec2, ImVec4};

mod node;
mod pin;
mod style;
mod utils;

pub use node::*;
pub use pin::*;
pub use style::*;
pub use utils::*;

// Node types for the example
mod simple_nodes;
pub use simple_nodes::*;

/// Main node editor context
pub struct NodeEditor {
    ptr: sys::ImNodeFlowPtr,
    name: CString,
}

impl NodeEditor {
    /// Create a new node editor with the given name
    pub fn new(name: &str) -> std::result::Result<Self, std::ffi::NulError> {
        let name_cstr = CString::new(name)?;
        let ptr = unsafe { sys::ImNodeFlow_Create(name_cstr.as_ptr()) };

        if ptr.is_null() {
            panic!("Failed to create ImNodeFlow instance");
        }

        Ok(Self {
            ptr,
            name: name_cstr,
        })
    }

    /// Create a new node editor with default name
    pub fn new_default() -> Self {
        let ptr = unsafe { sys::ImNodeFlow_CreateDefault() };

        if ptr.is_null() {
            panic!("Failed to create ImNodeFlow instance");
        }

        Self {
            ptr,
            name: CString::new("FlowGrid").unwrap(),
        }
    }

    /// Update the node editor (must be called every frame)
    pub fn update(&mut self, _ui: &Ui) {
        unsafe {
            sys::ImNodeFlow_Update(self.ptr);
        }
    }

    /// Set the size of the editor
    pub fn set_size(&mut self, size: ImVec2) {
        unsafe {
            sys::ImNodeFlow_SetSize(self.ptr, size.x, size.y);
        }
    }

    /// Get the editor's name
    pub fn name(&self) -> &str {
        self.name.to_str().unwrap()
    }

    /// Get the editor's position
    pub fn position(&self) -> ImVec2 {
        unsafe { sys::ImNodeFlow_GetPos(self.ptr) }
    }

    /// Get the editor's scroll offset
    pub fn scroll(&self) -> ImVec2 {
        unsafe { sys::ImNodeFlow_GetScroll(self.ptr) }
    }

    /// Get the number of nodes in the editor
    pub fn nodes_count(&self) -> u32 {
        unsafe { sys::ImNodeFlow_GetNodesCount(self.ptr) }
    }

    /// Check if a node is being dragged
    pub fn is_node_dragged(&self) -> bool {
        unsafe { sys::ImNodeFlow_IsNodeDragged(self.ptr) }
    }

    /// Get single use click status
    pub fn get_single_use_click(&self) -> bool {
        unsafe { sys::ImNodeFlow_GetSingleUseClick(self.ptr) }
    }

    /// Consume the single use click
    pub fn consume_single_use_click(&mut self) {
        unsafe { sys::ImNodeFlow_ConsumeSingleUseClick(self.ptr) }
    }

    /// Convert screen coordinates to grid coordinates
    pub fn screen_to_grid(&self, pos: ImVec2) -> ImVec2 {
        unsafe { sys::ImNodeFlow_Screen2Grid(self.ptr, pos) }
    }

    /// Convert grid coordinates to screen coordinates
    pub fn grid_to_screen(&self, pos: ImVec2) -> ImVec2 {
        unsafe { sys::ImNodeFlow_Grid2Screen(self.ptr, pos) }
    }

    /// Get the raw pointer (for internal use)
    pub(crate) fn as_ptr(&self) -> sys::ImNodeFlowPtr {
        self.ptr
    }

    /// Check if mouse is on selected node
    pub fn on_selected_node(&self) -> bool {
        unsafe { sys::ImNodeFlow_OnSelectedNode(self.ptr) }
    }

    /// Check if mouse is on free space
    pub fn on_free_space(&self) -> bool {
        unsafe { sys::ImNodeFlow_OnFreeSpace(self.ptr) }
    }

    /// Set dragging node state
    pub fn set_dragging_node(&mut self, state: bool) {
        unsafe { sys::ImNodeFlow_DraggingNode(self.ptr, state) }
    }

    /// Set hovering pin
    pub fn set_hovering_pin(&mut self, pin: Option<&Pin>) {
        let pin_ptr = pin.map_or(ptr::null_mut(), |p| p.ptr);
        unsafe { sys::ImNodeFlow_Hovering(self.ptr, pin_ptr) }
    }

    /// Set hovered node
    pub fn set_hovered_node(&mut self, node: Option<&Node>) {
        let node_ptr = node.map_or(ptr::null_mut(), |n| n.ptr);
        unsafe { sys::ImNodeFlow_HoveredNode(self.ptr, node_ptr) }
    }

    /// Add a simple node to the editor
    pub fn add_simple_node(&mut self, pos: ImVec2, title: &str) -> Result<Node> {
        let title_cstr = CString::new(title)?;
        let node_ptr =
            unsafe { sys::ImNodeFlow_AddSimpleNode(self.ptr, pos.x, pos.y, title_cstr.as_ptr()) };

        if node_ptr.is_null() {
            return Err(NodeFlowError::NodeCreationFailed);
        }

        Ok(Node {
            ptr: node_ptr,
            _phantom: PhantomData,
        })
    }
}

// Temporarily disabled Drop implementation to test crash location
// impl Drop for NodeEditor {
//     fn drop(&mut self) {
//         log::info!("NodeEditor::drop() called");
//         if !self.ptr.is_null() {
//             log::info!("Calling ImNodeFlow_Destroy...");
//             unsafe {
//                 sys::ImNodeFlow_Destroy(self.ptr);
//             }
//             log::info!("ImNodeFlow_Destroy completed");
//         } else {
//             log::info!("NodeEditor ptr is null, skipping destruction");
//         }
//         log::info!("NodeEditor::drop() finished");
//     }
// }

unsafe impl Send for NodeEditor {}
unsafe impl Sync for NodeEditor {}

/// Pin type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum PinType {
    Input = sys::PIN_TYPE_INPUT,
    Output = sys::PIN_TYPE_OUTPUT,
}

impl From<i32> for PinType {
    fn from(value: i32) -> Self {
        match value {
            sys::PIN_TYPE_INPUT => PinType::Input,
            sys::PIN_TYPE_OUTPUT => PinType::Output,
            _ => PinType::Input, // Default fallback
        }
    }
}

/// Helper functions for drawing bezier curves
pub mod bezier {
    use super::*;

    /// Draw a smart bezier curve between two points
    pub fn smart_bezier(p1: ImVec2, p2: ImVec2, color: ImU32, thickness: f32) {
        unsafe {
            sys::ImNodeFlow_SmartBezier(p1, p2, color, thickness);
        }
    }

    /// Check if a point collides with a bezier curve
    pub fn smart_bezier_collider(p: ImVec2, p1: ImVec2, p2: ImVec2, radius: f32) -> bool {
        unsafe { sys::ImNodeFlow_SmartBezierCollider(p, p1, p2, radius) }
    }
}

/// Error types for the node editor
#[derive(Debug, thiserror::Error)]
pub enum NodeFlowError {
    #[error("Null string error: {0}")]
    NulError(#[from] std::ffi::NulError),
    #[error("Invalid UTF-8 in string")]
    Utf8Error(#[from] std::str::Utf8Error),
    #[error("Node creation failed")]
    NodeCreationFailed,
    #[error("Pin creation failed")]
    PinCreationFailed,
}

pub type Result<T> = std::result::Result<T, NodeFlowError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_node_editor() {
        let editor = NodeEditor::new("Test Editor").unwrap();
        assert_eq!(editor.name(), "Test Editor");
        assert_eq!(editor.nodes_count(), 0);
    }

    #[test]
    fn test_create_default_node_editor() {
        let editor = NodeEditor::new_default();
        assert_eq!(editor.nodes_count(), 0);
    }

    #[test]
    fn test_coordinate_conversion() {
        let editor = NodeEditor::new_default();
        let grid_pos = ImVec2 { x: 100.0, y: 200.0 };
        let screen_pos = editor.grid_to_screen(grid_pos);
        let back_to_grid = editor.screen_to_grid(screen_pos);

        // Due to floating point precision, we check approximate equality
        assert!((back_to_grid.x - grid_pos.x).abs() < 0.001);
        assert!((back_to_grid.y - grid_pos.y).abs() < 0.001);
    }

    #[test]
    fn test_pin_type_conversion() {
        assert_eq!(PinType::from(sys::PIN_TYPE_INPUT), PinType::Input);
        assert_eq!(PinType::from(sys::PIN_TYPE_OUTPUT), PinType::Output);
        assert_eq!(PinType::from(999), PinType::Input); // Default fallback
    }
}
