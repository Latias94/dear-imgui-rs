//! Simple node implementations for the ImNodeFlow example
//!
//! This module provides concrete implementations of the node types
//! shown in the C++ example: SimpleSum, CollapsingNode, and ResultNode.

use crate::*;
use dear_imgui::Ui;
use std::ffi::CString;

/// A simple sum node that adds an input value to a configurable value
pub struct SimpleSumNode {
    node: Node,
    value_b: i32,
}

impl SimpleSumNode {
    pub fn new(editor: &mut NodeEditor, pos: ImVec2, title: &str) -> Result<Self> {
        let node = editor.add_simple_node(pos, title)?;

        // Add input and output pins
        let input_name = CString::new("In")?;
        let output_name = CString::new("Out")?;

        unsafe {
            // Add input pin for integer type (data_type = 0)
            sys::BaseNode_AddInputPin(node.ptr, input_name.as_ptr(), 0);
            // Add output pin for integer type
            sys::BaseNode_AddOutputPin(node.ptr, output_name.as_ptr(), 0);
        }

        Ok(Self { node, value_b: 0 })
    }

    pub fn draw(&mut self, ui: &Ui) {
        // Draw the node's content
        let _width_token = ui.push_item_width(100.0);
        ui.input_int("##ValB", &mut self.value_b);
        // Token automatically pops when it goes out of scope
    }

    pub fn get_node(&self) -> &Node {
        &self.node
    }
}

/// A collapsing node that shows content only when selected
pub struct CollapsingNode {
    node: Node,
}

impl CollapsingNode {
    pub fn new(editor: &mut NodeEditor, pos: ImVec2, title: &str) -> Result<Self> {
        let node = editor.add_simple_node(pos, title)?;

        // Add input pins
        let input_a = CString::new("A")?;
        let input_b = CString::new("B")?;
        let output_name = CString::new("Out")?;

        unsafe {
            sys::BaseNode_AddInputPin(node.ptr, input_a.as_ptr(), 0);
            sys::BaseNode_AddInputPin(node.ptr, input_b.as_ptr(), 0);
            sys::BaseNode_AddOutputPin(node.ptr, output_name.as_ptr(), 0);
        }

        Ok(Self { node })
    }

    pub fn draw(&mut self, ui: &Ui) {
        // Check if node is selected
        let is_selected = unsafe { sys::BaseNode_IsSelected(self.node.ptr) };

        if is_selected {
            let _width_token = ui.push_item_width(100.0);
            ui.text("You can only see me when the node is selected!");
        }
    }

    pub fn get_node(&self) -> &Node {
        &self.node
    }
}

/// A result node that displays the sum of two inputs
pub struct ResultNode {
    node: Node,
}

impl ResultNode {
    pub fn new(editor: &mut NodeEditor, pos: ImVec2, title: &str) -> Result<Self> {
        let node = editor.add_simple_node(pos, title)?;

        // Add input pins
        let input_a = CString::new("A")?;
        let input_b = CString::new("B")?;

        unsafe {
            sys::BaseNode_AddInputPin(node.ptr, input_a.as_ptr(), 0);
            sys::BaseNode_AddInputPin(node.ptr, input_b.as_ptr(), 0);
        }

        Ok(Self { node })
    }

    pub fn draw(&mut self, ui: &Ui) {
        // For now, just display a placeholder result
        // In a full implementation, this would get the actual input values
        ui.text("Result: 0");
    }

    pub fn get_node(&self) -> &Node {
        &self.node
    }
}

/// Node editor wrapper that manages multiple nodes
pub struct NodeEditorWrapper {
    editor: NodeEditor,
    simple_nodes: Vec<SimpleSumNode>,
    collapsing_nodes: Vec<CollapsingNode>,
    result_nodes: Vec<ResultNode>,
}

// Removed Drop implementation to test if it's causing the crash
// Let Rust handle cleanup automatically

impl NodeEditorWrapper {
    pub fn new(name: &str) -> Result<Self> {
        let editor = NodeEditor::new(name)?;

        Ok(Self {
            editor,
            simple_nodes: Vec::new(),
            collapsing_nodes: Vec::new(),
            result_nodes: Vec::new(),
        })
    }

    pub fn set_size(&mut self, size: ImVec2) {
        self.editor.set_size(size);
    }

    pub fn add_simple_sum_node(&mut self, pos: ImVec2) -> Result<()> {
        let node = SimpleSumNode::new(&mut self.editor, pos, "Simple sum")?;
        self.simple_nodes.push(node);
        Ok(())
    }

    pub fn add_collapsing_node(&mut self, pos: ImVec2) -> Result<()> {
        let node = CollapsingNode::new(&mut self.editor, pos, "Collapsing node")?;
        self.collapsing_nodes.push(node);
        Ok(())
    }

    pub fn add_result_node(&mut self, pos: ImVec2) -> Result<()> {
        let node = ResultNode::new(&mut self.editor, pos, "Result node")?;
        self.result_nodes.push(node);
        Ok(())
    }

    pub fn update(&mut self, ui: &Ui) {
        // Update the main editor
        self.editor.update(ui);

        // Draw all nodes
        for node in &mut self.simple_nodes {
            node.draw(ui);
        }

        for node in &mut self.collapsing_nodes {
            node.draw(ui);
        }

        for node in &mut self.result_nodes {
            node.draw(ui);
        }
    }

    pub fn get_nodes_count(&self) -> u32 {
        self.editor.nodes_count()
    }
}
