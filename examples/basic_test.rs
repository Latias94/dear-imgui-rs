use dear_imgui::*;

fn main() {
    println!("Testing Dear ImGui basic functionality...");

    // Test context creation
    let mut ctx = Context::create();
    println!("✓ Context created successfully");

    // Test IO access
    let io = ctx.io_mut();
    io.set_display_size([800.0, 600.0]);
    println!("✓ IO access successful");

    // Test new frame
    let ui = ctx.frame();
    println!("✓ New frame started");

    // Test basic UI
    ui.window("Test Window").build(|| {
        ui.text("Hello, Dear ImGui!");
        ui.button("Test Button");
    });
    println!("✓ Basic UI elements created");

    // Test render
    ctx.render();
    println!("✓ Render completed");

    // Test draw data access
    let draw_data = ctx.draw_data();
    if let Some(data) = draw_data {
        println!("✓ Draw data accessed, valid: {}", data.valid());
    } else {
        println!("✓ Draw data is None");
    }

    println!("All basic tests passed! ✅");
}
