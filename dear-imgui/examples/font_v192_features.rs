//! Example demonstrating Dear ImGui 1.92+ dynamic font features
//!
//! This example showcases the new font system capabilities:
//! - Dynamic glyph loading (no need to pre-specify ranges)
//! - Runtime font size adjustment
//! - Custom font loaders
//! - Font loader flags
//!
//! Note: This dear-imgui crate (version 0.1.0) includes support for Dear ImGui 1.92+ features.

use dear_imgui::{FontAtlas, FontConfig, FontLoader, FontLoaderFlags, FontSource, Context};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Dear ImGui 1.92+ Font Features Demo (dear-imgui crate v0.1.0)");

    // Create context with custom font atlas
    let mut ctx = Context::create()?;

    // Example 1: Dynamic font loading without pre-specifying glyph ranges
    setup_dynamic_fonts(&mut ctx)?;

    // Example 2: Custom font loader with flags
    setup_custom_font_loader(&mut ctx)?;

    // Example 3: Runtime font size adjustment
    demo_runtime_font_sizing(&mut ctx);

    Ok(())
}

/// Example 1: Dynamic font loading
fn setup_dynamic_fonts(_ctx: &mut Context) -> Result<(), Box<dyn std::error::Error>> {
    println!("Setting up dynamic fonts...");

    let mut atlas = FontAtlas::new();

    // Load default font with dynamic sizing (Dear ImGui 1.92+ feature)
    let default_font = FontSource::DefaultFontData {
        size_pixels: None, // Dynamic sizing
        config: None
    };
    atlas.add_font(&[default_font]);

    // Load TTF font with dynamic sizing - no need to specify glyph ranges!
    // With Dear ImGui 1.92+, glyphs are loaded on-demand
    let custom_font = FontSource::TtfFile {
        path: "assets/fonts/NotoSans-Regular.ttf",
        size_pixels: None, // Dynamic sizing
        config: Some(
            FontConfig::new()
                .name("Noto Sans")
                .font_loader_flags(FontLoaderFlags::LOAD_COLOR)
        ),
    };

    atlas.add_font(&[custom_font]);

    // Load Japanese font - no glyph ranges needed with Dear ImGui 1.92+!
    let japanese_font = FontSource::TtfFile {
        path: "assets/fonts/NotoSansCJK-Regular.ttf",
        size_pixels: None, // Dynamic sizing
        config: Some(
            FontConfig::new()
                .name("Noto Sans CJK")
                .rasterizer_multiply(1.2) // Slightly brighter for better readability
        ),
    };

    atlas.add_font(&[japanese_font]);

    println!("✓ Dynamic fonts loaded successfully");
    Ok(())
}

/// Example 2: Custom font loader with flags
fn setup_custom_font_loader(_ctx: &mut Context) -> Result<(), Box<dyn std::error::Error>> {
    println!("Setting up custom font loader...");

    // Create a custom font loader (for advanced use cases)
    let loader = FontLoader::new("Custom FreeType Loader")?;

    let mut atlas = FontAtlas::with_font_loader(&loader);

    // Set global font loader flags
    atlas.set_font_loader_flags(
        FontLoaderFlags::LOAD_COLOR | FontLoaderFlags::FORCE_AUTOHINT
    );

    // Load emoji font with color support
    let emoji_font = FontSource::TtfFile {
        path: "assets/fonts/NotoColorEmoji.ttf",
        size_pixels: None, // Dynamic sizing
        config: Some(
            FontConfig::new()
                .name("Noto Color Emoji")
                .font_loader_flags(FontLoaderFlags::LOAD_COLOR)
                .glyph_min_advance_x(16.0) // Monospace emojis
        ),
    };

    atlas.add_font(&[emoji_font]);

    println!("✓ Custom font loader configured");
    Ok(())
}

/// Example 3: Runtime font size adjustment
fn demo_runtime_font_sizing(ctx: &mut Context) {
    println!("Demonstrating runtime font sizing...");
    
    let ui = ctx.frame();
    
    // Traditional approach: different font sizes need to be pre-loaded
    // With Dear ImGui 1.92+: we can adjust size dynamically!
    
    ui.text("Normal size text");
    
    // Use the same font with different sizes dynamically
    ui.with_font_and_size(None, 24.0, || {
        ui.text("Large text (24px)");
    });
    
    ui.with_font_and_size(None, 12.0, || {
        ui.text("Small text (12px)");
    });
    
    ui.with_font_and_size(None, 32.0, || {
        ui.text("Extra large text (32px)");
    });
    
    // You can also push/pop manually for more control
    ui.push_font_with_size(None, 18.0);
    ui.text("Medium text (18px)");
    ui.text("Still medium text");
    // Font will be popped automatically when ui goes out of scope
    
    println!("✓ Runtime font sizing demonstrated");
}

/// Example 4: Advanced font configuration
#[allow(dead_code)]
fn advanced_font_config_example() -> FontConfig {
    FontConfig::new()
        .name("Advanced Font")
        .size_pixels(16.0)
        .font_loader_flags(FontLoaderFlags::LOAD_COLOR | FontLoaderFlags::NO_HINTING)
        .glyph_offset([0.0, 1.0]) // Slight vertical offset
        .glyph_min_advance_x(8.0)
        .glyph_max_advance_x(32.0)
        .glyph_extra_advance_x(1.0) // Extra spacing between characters
        .rasterizer_multiply(1.1) // Slightly brighter
        .rasterizer_density(1.0) // Normal DPI
        .pixel_snap_h(true) // Snap to pixel boundaries horizontally
        .pixel_snap_v(false)
        .oversample_h(2) // 2x horizontal oversampling
        .oversample_v(1) // 1x vertical oversampling
}

/// Example 5: Font merging with exclude ranges (Dear ImGui 1.92+ feature)
#[allow(dead_code)]
fn font_merging_example() -> Result<(), Box<dyn std::error::Error>> {
    let mut atlas = FontAtlas::new();

    // Load base font
    let base_font = FontSource::DefaultFontData {
        size_pixels: None,
        config: None
    };
    atlas.add_font(&[base_font]);

    // Merge icon font, excluding overlapping ranges
    let icon_ranges_to_exclude = [0x0020, 0x007F, 0]; // Exclude ASCII range
    let icon_font = FontSource::TtfFile {
        path: "assets/fonts/FontAwesome.ttf",
        size_pixels: None,
        config: Some(
            FontConfig::new()
                .name("FontAwesome Icons")
                .merge_mode(true) // Merge with previous font
                .glyph_exclude_ranges(&icon_ranges_to_exclude) // Dear ImGui 1.92+ feature
                .glyph_min_advance_x(16.0) // Monospace icons
        ),
    };

    atlas.add_font(&[icon_font]);

    // Merge CJK font for Asian characters
    let cjk_font = FontSource::TtfFile {
        path: "assets/fonts/NotoSansCJK.ttf",
        size_pixels: None,
        config: Some(
            FontConfig::new()
                .name("CJK Support")
                .merge_mode(true)
                // No glyph ranges needed with Dear ImGui 1.92+ - loaded on demand!
        ),
    };

    atlas.add_font(&[cjk_font]);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_font_loader_creation() {
        let _loader = FontLoader::new("Test Loader").unwrap();
        // FontLoader created successfully
    }

    #[test]
    fn test_font_config_builder() {
        let _config = FontConfig::new()
            .size_pixels(16.0)
            .font_loader_flags(FontLoaderFlags::LOAD_COLOR)
            .name("Test Font");

        // Config created successfully
    }

    #[test]
    fn test_font_source_creation() {
        let source = FontSource::DefaultFontData {
            size_pixels: None,
            config: None
        };
        match source {
            FontSource::DefaultFontData { size_pixels, .. } => {
                assert_eq!(size_pixels, None); // Dynamic sizing
            }
            _ => panic!("Expected DefaultFontData"),
        }
    }
}
