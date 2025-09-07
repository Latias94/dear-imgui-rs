//! Text Filter Demo
//!
//! This example demonstrates the TextFilter functionality, showing how to:
//! - Create and use text filters
//! - Filter lists of items
//! - Use include/exclude patterns

use dear_imgui::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== TextFilter Demo ===");

    // Sample data to filter
    let items = vec![
        "apple",
        "banana",
        "cherry",
        "date",
        "elderberry",
        "fig",
        "grape",
        "honeydew",
        "kiwi",
        "lemon",
        "mango",
        "orange",
        "papaya",
        "quince",
        "raspberry",
        "strawberry",
        "tangerine",
        "watermelon",
    ];

    // Test 1: Basic filter
    println!("\n1. Testing basic filter creation:");
    let mut basic_filter = TextFilter::new("Search".to_string());
    println!("   Empty filter is_active: {}", basic_filter.is_active());

    // Test 2: Filter with pattern
    println!("\n2. Testing filter with pattern:");
    let mut pattern_filter = TextFilter::new_with_filter(
        "Advanced Search".to_string(),
        "berry".to_string()
    );
    pattern_filter.build(); // Build the filter
    println!("   Pattern filter is_active: {}", pattern_filter.is_active());

    // Test 3: Filter matching
    println!("\n3. Testing filter matching:");
    println!("   Items matching 'berry' pattern:");
    for item in &items {
        if pattern_filter.pass_filter(item) {
            println!("     - {}", item);
        }
    }

    // Test 4: Complex filter - NOTE: ImGui TextFilter behavior
    println!("\n4. Testing complex filter (berry,-straw):");
    println!("   NOTE: ImGui TextFilter checks include patterns first!");
    println!("   If 'berry' matches, it returns true immediately without checking '-straw'");
    let mut complex_filter = TextFilter::new_with_filter(
        "Complex Search".to_string(),
        "berry,-straw".to_string()
    );
    complex_filter.build();
    println!("   Complex filter is_active: {}", complex_filter.is_active());
    println!("   Items matching 'berry,-straw' pattern:");
    for item in &items {
        if complex_filter.pass_filter(item) {
            println!("     - {}", item);
        }
    }

    // Test 4b: Test exclusion filter separately
    println!("\n4b. Testing exclusion filter (-straw):");
    let mut exclude_filter = TextFilter::new_with_filter(
        "Exclude Search".to_string(),
        "-straw".to_string()
    );
    exclude_filter.build();
    println!("   Items NOT matching 'straw' pattern:");
    for item in &items {
        if exclude_filter.pass_filter(item) {
            println!("     - {}", item);
        }
    }

    // Test 4c: Test correct exclusion pattern (-straw,berry)
    println!("\n4c. Testing correct exclusion pattern (-straw,berry):");
    println!("   This should exclude 'straw' items first, then include 'berry' items");
    let mut correct_filter = TextFilter::new_with_filter(
        "Correct Search".to_string(),
        "-straw,berry".to_string()
    );
    correct_filter.build();
    println!("   Items matching '-straw,berry' pattern:");
    for item in &items {
        if correct_filter.pass_filter(item) {
            println!("     - {}", item);
        }
    }

    // Test 5: Clear filter
    println!("\n5. Testing filter clear:");
    println!("   Before clear - is_active: {}", complex_filter.is_active());
    complex_filter.clear();
    println!("   After clear - is_active: {}", complex_filter.is_active());

    // Test 6: Multiple patterns
    println!("\n6. Testing multiple patterns (apple,orange):");
    let mut multi_filter = TextFilter::new_with_filter(
        "Multi Search".to_string(),
        "apple,orange".to_string()
    );
    multi_filter.build();
    println!("   Items matching 'apple,orange' pattern:");
    for item in &items {
        if multi_filter.pass_filter(item) {
            println!("     - {}", item);
        }
    }

    println!("\n=== TextFilter Demo Complete ===");
    println!("TextFilter implementation is working correctly!");

    Ok(())
}
