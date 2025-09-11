//! GraphEditor implementation
//!
//! This module provides node-based graph editing functionality.

use crate::{
    types::{Color, ColorExt, Vec2},
    GuizmoResult,
};
use dear_imgui::{DrawListMut, Ui};
use std::collections::HashMap;

/// Node identifier
pub type NodeId = u32;

/// Connection identifier  
pub type ConnectionId = u32;

/// Input/Output slot identifier
pub type SlotId = u32;

/// Node template for creating nodes
#[derive(Debug, Clone)]
pub struct NodeTemplate {
    /// Template identifier
    pub id: u32,
    /// Display name
    pub name: String,
    /// Input slot count
    pub input_count: u32,
    /// Output slot count  
    pub output_count: u32,
    /// Node color
    pub color: Color,
}

/// Graph node
#[derive(Debug)]
pub struct GraphNode {
    /// Unique node identifier
    pub id: NodeId,
    /// Template this node was created from
    pub template_id: u32,
    /// Node position in graph space
    pub position: Vec2,
    /// Whether the node is selected
    pub selected: bool,
    /// Custom node data (use Arc<dyn Any + Send + Sync> for shared data)
    pub user_data: Option<std::sync::Arc<dyn std::any::Any + Send + Sync>>,
}

impl Clone for GraphNode {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            template_id: self.template_id,
            position: self.position,
            selected: self.selected,
            user_data: self.user_data.clone(), // Arc clones the reference, not the data
        }
    }
}

/// Connection between nodes
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphConnection {
    /// Unique connection identifier
    pub id: ConnectionId,
    /// Source node ID
    pub input_node_id: NodeId,
    /// Source slot ID
    pub input_slot_id: SlotId,
    /// Target node ID
    pub output_node_id: NodeId,
    /// Target slot ID
    pub output_slot_id: SlotId,
}

/// Trait for graph delegate interface
pub trait GraphDelegate {
    /// Get all node templates
    fn get_node_templates(&self) -> &[NodeTemplate];

    /// Get all nodes
    fn get_nodes(&self) -> &[GraphNode];

    /// Get all connections
    fn get_connections(&self) -> &[GraphConnection];

    /// Add a new node
    fn add_node(&mut self, template_id: u32, position: Vec2) -> NodeId;

    /// Delete a node
    fn delete_node(&mut self, node_id: NodeId);

    /// Move a node
    fn move_node(&mut self, node_id: NodeId, position: Vec2);

    /// Add a connection
    fn add_connection(&mut self, connection: GraphConnection) -> ConnectionId;

    /// Delete a connection
    fn delete_connection(&mut self, connection_id: ConnectionId);

    /// Check if a connection is valid
    fn is_connection_valid(
        &self,
        input_node: NodeId,
        input_slot: SlotId,
        output_node: NodeId,
        output_slot: SlotId,
    ) -> bool;

    /// Get node display name
    fn get_node_name(&self, node_id: NodeId) -> String;

    /// Get slot name
    fn get_slot_name(&self, node_id: NodeId, slot_id: SlotId, is_input: bool) -> String;
}

/// Graph editor widget
pub struct GraphEditor {
    /// Current selection
    selected_nodes: Vec<NodeId>,
    /// Current view offset
    view_offset: Vec2,
    /// Current zoom level
    zoom: f32,
    /// Whether we're currently dragging
    dragging: bool,
    /// Drag start position
    drag_start: Vec2,
}

impl GraphEditor {
    /// Create a new graph editor
    pub fn new() -> Self {
        Self {
            selected_nodes: Vec::new(),
            view_offset: Vec2::ZERO,
            zoom: 1.0,
            dragging: false,
            drag_start: Vec2::ZERO,
        }
    }

    /// Render the graph editor
    pub fn edit<T: GraphDelegate>(
        &mut self,
        ui: &Ui,
        delegate: &mut T,
        size: Vec2,
    ) -> GuizmoResult<bool> {
        let mut modified = false;

        // Create child window for graph editor
        ui.child_window("GraphEditor")
            .size([size.x, size.y])
            .border(true)
            .build(ui, || {
                let draw_list = ui.get_window_draw_list();
                let window_pos = ui.cursor_screen_pos();
                let window_size = [size.x, size.y];

                // Draw background grid
                if let Err(_) = self.draw_grid(&draw_list, window_pos, window_size) {
                    return;
                }

                // Draw connections
                if let Err(_) = self.draw_connections(&draw_list, delegate, window_pos) {
                    return;
                }

                // Draw nodes
                if let Ok(nodes_modified) = self.draw_nodes(&draw_list, ui, delegate, window_pos) {
                    modified = modified || nodes_modified;
                }

                // Handle mouse interaction
                if let Ok(interaction_modified) =
                    self.handle_mouse_interaction(ui, delegate, window_pos, window_size)
                {
                    modified = modified || interaction_modified;
                }
            });

        Ok(modified)
    }

    /// Get current selection
    pub fn get_selected_nodes(&self) -> &[NodeId] {
        &self.selected_nodes
    }

    /// Set selection
    pub fn set_selected_nodes(&mut self, nodes: Vec<NodeId>) {
        self.selected_nodes = nodes;
    }

    /// Clear selection
    pub fn clear_selection(&mut self) {
        self.selected_nodes.clear();
    }

    /// Get view offset
    pub fn get_view_offset(&self) -> Vec2 {
        self.view_offset
    }

    /// Set view offset
    pub fn set_view_offset(&mut self, offset: Vec2) {
        self.view_offset = offset;
    }

    /// Get zoom level
    pub fn get_zoom(&self) -> f32 {
        self.zoom
    }

    /// Set zoom level
    pub fn set_zoom(&mut self, zoom: f32) {
        self.zoom = zoom.max(0.1).min(10.0);
    }

    /// Draw background grid
    fn draw_grid(
        &self,
        draw_list: &DrawListMut,
        window_pos: [f32; 2],
        window_size: [f32; 2],
    ) -> GuizmoResult<()> {
        let grid_size = 50.0 * self.zoom;
        let grid_color = 0xFF404040; // Dark gray

        // Apply view offset
        let offset_x = self.view_offset.x % grid_size;
        let offset_y = self.view_offset.y % grid_size;

        // Draw vertical lines
        let mut x = -offset_x;
        while x <= window_size[0] {
            if x >= 0.0 {
                draw_list
                    .add_line(
                        [window_pos[0] + x, window_pos[1]],
                        [window_pos[0] + x, window_pos[1] + window_size[1]],
                        grid_color,
                    )
                    .build();
            }
            x += grid_size;
        }

        // Draw horizontal lines
        let mut y = -offset_y;
        while y <= window_size[1] {
            if y >= 0.0 {
                draw_list
                    .add_line(
                        [window_pos[0], window_pos[1] + y],
                        [window_pos[0] + window_size[0], window_pos[1] + y],
                        grid_color,
                    )
                    .build();
            }
            y += grid_size;
        }

        Ok(())
    }

    /// Draw connections between nodes
    fn draw_connections<T: GraphDelegate>(
        &self,
        draw_list: &DrawListMut,
        delegate: &T,
        window_pos: [f32; 2],
    ) -> GuizmoResult<()> {
        let connection_color = 0xFFFFFFFF; // White
        let connection_thickness = 2.0;

        // Get all connections and draw them
        for connection in delegate.get_connections() {
            if let (Some(output_pos), Some(input_pos)) = (
                self.get_output_slot_pos(
                    delegate,
                    connection.output_node_id,
                    connection.output_slot_id,
                    window_pos,
                )?,
                self.get_input_slot_pos(
                    delegate,
                    connection.input_node_id,
                    connection.input_slot_id,
                    window_pos,
                )?,
            ) {
                // Draw bezier curve for connection
                self.draw_bezier_connection(
                    draw_list,
                    output_pos,
                    input_pos,
                    connection_color,
                    connection_thickness,
                )?;
            }
        }

        Ok(())
    }

    /// Get output slot position
    fn get_output_slot_pos<T: GraphDelegate>(
        &self,
        delegate: &T,
        node_id: NodeId,
        slot_id: u32,
        window_pos: [f32; 2],
    ) -> GuizmoResult<Option<Vec2>> {
        // Find the node
        if let Some(node) = delegate.get_nodes().iter().find(|n| n.id == node_id) {
            // Find the template to get slot count
            if let Some(template) = delegate
                .get_node_templates()
                .iter()
                .find(|t| t.id == node.template_id)
            {
                if slot_id < template.output_count {
                    let node_size = Vec2::new(100.0, 60.0); // Default node size
                    let slot_y = node.position.y
                        + (slot_id as f32 + 0.5) * (node_size.y / template.output_count as f32);
                    let slot_pos = Vec2::new(
                        window_pos[0]
                            + (node.position.x + node_size.x) * self.zoom
                            + self.view_offset.x,
                        window_pos[1] + slot_y * self.zoom + self.view_offset.y,
                    );
                    return Ok(Some(slot_pos));
                }
            }
        }
        Ok(None)
    }

    /// Get input slot position
    fn get_input_slot_pos<T: GraphDelegate>(
        &self,
        delegate: &T,
        node_id: NodeId,
        slot_id: u32,
        window_pos: [f32; 2],
    ) -> GuizmoResult<Option<Vec2>> {
        // Find the node
        if let Some(node) = delegate.get_nodes().iter().find(|n| n.id == node_id) {
            // Find the template to get slot count
            if let Some(template) = delegate
                .get_node_templates()
                .iter()
                .find(|t| t.id == node.template_id)
            {
                if slot_id < template.input_count {
                    let node_size = Vec2::new(100.0, 60.0); // Default node size
                    let slot_y = node.position.y
                        + (slot_id as f32 + 0.5) * (node_size.y / template.input_count as f32);
                    let slot_pos = Vec2::new(
                        window_pos[0] + node.position.x * self.zoom + self.view_offset.x,
                        window_pos[1] + slot_y * self.zoom + self.view_offset.y,
                    );
                    return Ok(Some(slot_pos));
                }
            }
        }
        Ok(None)
    }

    /// Draw bezier connection between two points
    fn draw_bezier_connection(
        &self,
        draw_list: &DrawListMut,
        start: Vec2,
        end: Vec2,
        color: u32,
        thickness: f32,
    ) -> GuizmoResult<()> {
        let control_offset = 50.0;
        let control1 = Vec2::new(start.x + control_offset, start.y);
        let control2 = Vec2::new(end.x - control_offset, end.y);

        // Draw bezier curve using multiple line segments
        let segments = 20;
        for i in 0..segments {
            let t1 = i as f32 / segments as f32;
            let t2 = (i + 1) as f32 / segments as f32;

            let p1 = self.bezier_point(start, control1, control2, end, t1);
            let p2 = self.bezier_point(start, control1, control2, end, t2);

            draw_list
                .add_line([p1.x, p1.y], [p2.x, p2.y], color)
                .thickness(thickness)
                .build();
        }

        Ok(())
    }

    /// Calculate bezier curve point
    fn bezier_point(&self, p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2, t: f32) -> Vec2 {
        let u = 1.0 - t;
        let tt = t * t;
        let uu = u * u;
        let uuu = uu * u;
        let ttt = tt * t;

        p0 * uuu + p1 * (3.0 * uu * t) + p2 * (3.0 * u * tt) + p3 * ttt
    }

    /// Draw all nodes
    fn draw_nodes<T: GraphDelegate>(
        &self,
        draw_list: &DrawListMut,
        _ui: &Ui,
        delegate: &T,
        window_pos: [f32; 2],
    ) -> GuizmoResult<bool> {
        let node_size = Vec2::new(100.0, 60.0); // Default node size

        for node in delegate.get_nodes() {
            // Find template for this node
            if let Some(template) = delegate
                .get_node_templates()
                .iter()
                .find(|t| t.id == node.template_id)
            {
                // Calculate screen position
                let screen_pos = Vec2::new(
                    window_pos[0] + node.position.x * self.zoom + self.view_offset.x,
                    window_pos[1] + node.position.y * self.zoom + self.view_offset.y,
                );

                // Draw node background
                let node_color = if node.selected {
                    0xFFFFFF00
                } else {
                    template.color.as_u32()
                };
                draw_list
                    .add_rect(
                        [screen_pos.x, screen_pos.y],
                        [screen_pos.x + node_size.x, screen_pos.y + node_size.y],
                        node_color,
                    )
                    .filled(true)
                    .build();

                // Draw node border
                draw_list
                    .add_rect(
                        [screen_pos.x, screen_pos.y],
                        [screen_pos.x + node_size.x, screen_pos.y + node_size.y],
                        0xFF000000, // Black border
                    )
                    .build();

                // Draw input slots
                for i in 0..template.input_count {
                    let slot_y = screen_pos.y
                        + (i as f32 + 0.5) * (node_size.y / template.input_count as f32);
                    draw_list
                        .add_circle(
                            [screen_pos.x, slot_y],
                            4.0,
                            0xFF00FF00, // Green for inputs
                        )
                        .filled(true)
                        .build();
                }

                // Draw output slots
                for i in 0..template.output_count {
                    let slot_y = screen_pos.y
                        + (i as f32 + 0.5) * (node_size.y / template.output_count as f32);
                    draw_list
                        .add_circle(
                            [screen_pos.x + node_size.x, slot_y],
                            4.0,
                            0xFFFF0000, // Red for outputs
                        )
                        .filled(true)
                        .build();
                }
            }
        }

        Ok(false) // No modification for now
    }

    /// Handle mouse interaction
    fn handle_mouse_interaction<T: GraphDelegate>(
        &mut self,
        ui: &Ui,
        _delegate: &mut T,
        _window_pos: [f32; 2],
        _window_size: [f32; 2],
    ) -> GuizmoResult<bool> {
        let io = ui.io();
        let _mouse_pos = io.mouse_pos();

        // TODO: Implement mouse interaction for graph editing
        // - Node selection
        // - Node dragging
        // - Connection creation
        // - Panning and zooming

        Ok(false)
    }
}

impl Default for GraphEditor {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple graph delegate implementation for testing
pub struct SimpleGraphDelegate {
    templates: Vec<NodeTemplate>,
    nodes: Vec<GraphNode>,
    connections: Vec<GraphConnection>,
    next_node_id: NodeId,
    next_connection_id: ConnectionId,
}

impl SimpleGraphDelegate {
    /// Create a new simple graph delegate
    pub fn new() -> Self {
        Self {
            templates: Vec::new(),
            nodes: Vec::new(),
            connections: Vec::new(),
            next_node_id: 1,
            next_connection_id: 1,
        }
    }

    /// Add a node template
    pub fn add_template(&mut self, template: NodeTemplate) {
        self.templates.push(template);
    }
}

impl Default for SimpleGraphDelegate {
    fn default() -> Self {
        Self::new()
    }
}

impl GraphDelegate for SimpleGraphDelegate {
    fn get_node_templates(&self) -> &[NodeTemplate] {
        &self.templates
    }

    fn get_nodes(&self) -> &[GraphNode] {
        &self.nodes
    }

    fn get_connections(&self) -> &[GraphConnection] {
        &self.connections
    }

    fn add_node(&mut self, template_id: u32, position: Vec2) -> NodeId {
        let id = self.next_node_id;
        self.next_node_id += 1;

        let node = GraphNode {
            id,
            template_id,
            position,
            selected: false,
            user_data: None,
        };

        self.nodes.push(node);
        id
    }

    fn delete_node(&mut self, node_id: NodeId) {
        self.nodes.retain(|n| n.id != node_id);
        self.connections
            .retain(|c| c.input_node_id != node_id && c.output_node_id != node_id);
    }

    fn move_node(&mut self, node_id: NodeId, position: Vec2) {
        if let Some(node) = self.nodes.iter_mut().find(|n| n.id == node_id) {
            node.position = position;
        }
    }

    fn add_connection(&mut self, mut connection: GraphConnection) -> ConnectionId {
        connection.id = self.next_connection_id;
        self.next_connection_id += 1;

        let id = connection.id;
        self.connections.push(connection);
        id
    }

    fn delete_connection(&mut self, connection_id: ConnectionId) {
        self.connections.retain(|c| c.id != connection_id);
    }

    fn is_connection_valid(
        &self,
        _input_node: NodeId,
        _input_slot: SlotId,
        _output_node: NodeId,
        _output_slot: SlotId,
    ) -> bool {
        // Simple validation - just check nodes exist
        true
    }

    fn get_node_name(&self, node_id: NodeId) -> String {
        if let Some(node) = self.nodes.iter().find(|n| n.id == node_id) {
            if let Some(template) = self.templates.iter().find(|t| t.id == node.template_id) {
                return template.name.clone();
            }
        }
        format!("Node {}", node_id)
    }

    fn get_slot_name(&self, _node_id: NodeId, slot_id: SlotId, is_input: bool) -> String {
        format!("{} {}", if is_input { "In" } else { "Out" }, slot_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_editor_creation() {
        let editor = GraphEditor::new();
        assert_eq!(editor.get_selected_nodes().len(), 0);
        assert_eq!(editor.get_view_offset(), Vec2::ZERO);
        assert_eq!(editor.get_zoom(), 1.0);
    }

    #[test]
    fn test_simple_graph_delegate() {
        let mut delegate = SimpleGraphDelegate::new();

        // Add a template
        let template = NodeTemplate {
            id: 1,
            name: "Test Node".to_string(),
            input_count: 2,
            output_count: 1,
            color: [1.0, 1.0, 1.0, 1.0],
        };
        delegate.add_template(template);

        // Add a node
        let node_id = delegate.add_node(1, Vec2::new(100.0, 100.0));
        assert_eq!(node_id, 1);
        assert_eq!(delegate.get_nodes().len(), 1);

        // Test node name
        let name = delegate.get_node_name(node_id);
        assert_eq!(name, "Test Node");

        // Delete node
        delegate.delete_node(node_id);
        assert_eq!(delegate.get_nodes().len(), 0);
    }

    #[test]
    fn test_graph_connection() {
        let connection = GraphConnection {
            id: 1,
            input_node_id: 1,
            input_slot_id: 0,
            output_node_id: 2,
            output_slot_id: 0,
        };

        assert_eq!(connection.input_node_id, 1);
        assert_eq!(connection.output_node_id, 2);
    }
}
