//! Drag and Drop Demo
//!
//! This example demonstrates the drag and drop functionality in dear-imgui.
//! It shows different types of payloads: empty, typed, and raw data.

use dear_imgui::*;

fn main() {
    println!("Dear ImGui Drag and Drop Demo");
    println!("This example demonstrates drag and drop functionality:");
    println!("1. Empty payload - simple event notification");
    println!("2. Typed payload - type-safe data transfer");
    println!("3. Raw payload - unsafe but flexible data transfer");
    
    // This is just a compilation test since we don't have a full window system setup
    // In a real application, you would integrate this with winit + wgpu
    
    let mut ctx = Context::create();
    let ui = ctx.frame();
    
    // Example 1: Empty payload drag and drop
    ui.text("Drag and Drop Examples:");
    ui.separator();
    
    // Source button
    ui.button("Drag me (empty payload)");
    if let Some(source) = ui.drag_drop_source_config("EMPTY_PAYLOAD").begin() {
        ui.text("Dragging empty payload...");
        source.end();
    }
    
    ui.same_line();
    
    // Target button
    ui.button("Drop here (empty)");
    if let Some(target) = ui.drag_drop_target() {
        if target.accept_payload_empty("EMPTY_PAYLOAD", DragDropFlags::NONE).is_some() {
            println!("Empty payload dropped!");
        }
        target.pop();
    }
    
    ui.separator();
    
    // Example 2: Typed payload
    let data_to_send = 42i32;
    
    ui.button("Drag me (typed payload)");
    if let Some(source) = ui.drag_drop_source_config("TYPED_PAYLOAD")
        .flags(DragDropFlags::SOURCE_NO_PREVIEW_TOOLTIP)
        .begin_payload(data_to_send) {
        ui.text(format!("Dragging integer: {}", data_to_send));
        source.end();
    }
    
    ui.same_line();
    
    ui.button("Drop here (typed)");
    if let Some(target) = ui.drag_drop_target() {
        if let Some(Ok(payload)) = target.accept_payload::<i32, _>("TYPED_PAYLOAD", DragDropFlags::NONE) {
            println!("Received typed payload: {}", payload.data);
        }
        target.pop();
    }
    
    ui.separator();
    
    // Example 3: String payload
    let string_data = "Hello, World!";
    
    ui.button("Drag me (string payload)");
    if let Some(source) = ui.drag_drop_source_config("STRING_PAYLOAD")
        .condition(Condition::Always)
        .begin_payload(string_data) {
        ui.text("Dragging string data...");
        source.end();
    }
    
    ui.same_line();
    
    ui.button("Drop here (string)");
    if let Some(target) = ui.drag_drop_target() {
        if let Some(Ok(payload)) = target.accept_payload::<&'static str, _>("STRING_PAYLOAD", DragDropFlags::NONE) {
            println!("Received string payload: {}", payload.data);
        }
        target.pop();
    }
    
    ui.separator();
    ui.text("Check console output for drop results!");
    
    println!("Drag and drop demo setup complete!");
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_drag_drop_compilation() {
        // Test that drag drop code compiles correctly
        let mut ctx = Context::create();
        let ui = ctx.frame();
        
        // Test drag source creation
        let source = ui.drag_drop_source_config("TEST");
        assert_eq!(std::mem::size_of_val(&source), std::mem::size_of::<DragDropSource<&str>>());
        
        // Test flags
        let flags = DragDropFlags::SOURCE_NO_PREVIEW_TOOLTIP | DragDropFlags::ACCEPT_BEFORE_DELIVERY;
        assert!(!flags.is_empty());
        
        // Test that we can create different payload types
        let _empty_source = ui.drag_drop_source_config("EMPTY");
        let _typed_source = ui.drag_drop_source_config("TYPED");
        
        println!("All drag drop types compile successfully!");
    }
    
    #[test]
    fn test_drag_drop_flags() {
        // Test flag combinations
        let source_flags = DragDropFlags::SOURCE_NO_PREVIEW_TOOLTIP 
            | DragDropFlags::SOURCE_NO_DISABLE_HOVER;
        
        let target_flags = DragDropFlags::ACCEPT_BEFORE_DELIVERY 
            | DragDropFlags::ACCEPT_NO_DRAW_DEFAULT_RECT;
        
        let peek_flags = DragDropFlags::ACCEPT_PEEK_ONLY;
        
        assert!(source_flags.contains(DragDropFlags::SOURCE_NO_PREVIEW_TOOLTIP));
        assert!(target_flags.contains(DragDropFlags::ACCEPT_BEFORE_DELIVERY));
        assert!(peek_flags.contains(DragDropFlags::ACCEPT_BEFORE_DELIVERY));
        assert!(peek_flags.contains(DragDropFlags::ACCEPT_NO_DRAW_DEFAULT_RECT));
        
        println!("All drag drop flags work correctly!");
    }
}
