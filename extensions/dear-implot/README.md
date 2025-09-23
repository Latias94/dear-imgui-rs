# Dear ImPlot - Rust Bindings

High-level Rust bindings for ImPlot, the immediate mode plotting library. This crate provides safe, idiomatic Rust bindings designed to work seamlessly with `dear-imgui` (which uses cimgui C API) and `dear-implot-sys` (which uses cimplot C API).

For native build/link options (source, system/prebuilt, remote prebuilt), see `extensions/dear-implot-sys/README.md`.

## Features

- **Safe, idiomatic Rust API** - Memory-safe interfaces with proper lifetime management
- **Full compatibility with dear-imgui** - Uses the same context management patterns
- **Builder pattern for plots** - Fluent, ergonomic API for creating plots
- **Comprehensive plot types** - Support for all major plot types including:
  - Line plots and scatter plots
  - Bar charts and histograms (1D and 2D)
  - Heatmaps and pie charts
  - Error bars and shaded plots
  - Stem plots and more
- **Advanced features** - Subplots, multi-axis plots, and legend management
- **Modular design** - Each plot type is in its own module for better organization
- **Universal plot builder** - Unified API for creating any plot type

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
dear-imgui = "0.11"
dear-implot = { path = "path/to/dear-implot" }
```

Basic usage:

```rust
use dear_imgui::*;
use dear_implot::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut ctx = Context::create_or_panic();
    let plot_ctx = PlotContext::create(&ctx);

    // In your main loop:
    let ui = ctx.frame();
    let plot_ui = plot_ctx.get_plot_ui(&ui);

    ui.window("My Plots")
        .size([800.0, 600.0], Condition::FirstUseEver)
        .build(|| {
            if let Some(token) = plot_ui.begin_plot("Line Plot") {
                let x_data = [0.0, 1.0, 2.0, 3.0, 4.0];
                let y_data = [0.0, 1.0, 4.0, 9.0, 16.0];

                // New modular API
                LinePlot::new("Quadratic", &x_data, &y_data).plot();

                token.end();
            }
        });

    Ok(())
}
```

## Plot Types

### Line and Scatter Plots

```rust
// Line plot
LinePlot::new("My Line", &x_data, &y_data).plot();

// Scatter plot
ScatterPlot::new("My Points", &x_data, &y_data).plot();
```

### Bar Charts

```rust
// Simple bar chart
BarPlot::new("Bars", &values)
    .with_bar_size(0.8)
    .plot();

// Positional bar chart
PositionalBarPlot::new("Positioned Bars", &x_positions, &values)
    .with_bar_size(0.5)
    .plot();
```

### Histograms

```rust
// 1D Histogram
HistogramPlot::new("Distribution", &data)
    .with_bins(20)
    .density()
    .cumulative()
    .plot();

// 2D Histogram
Histogram2DPlot::new("2D Distribution", &x_data, &y_data)
    .with_bins(10, 10)
    .plot();
```

### Heatmaps

```rust
let data: Vec<f64> = (0..100).map(|i| (i as f64 * 0.1).sin()).collect();

HeatmapPlot::new("Heat", &data, 10, 10)
    .with_scale(-1.0, 1.0)
    .with_bounds(0.0, 0.0, 1.0, 1.0)
    .with_label_format("%.2f")
    .plot();
```

### Pie Charts

```rust
let labels = vec!["A", "B", "C", "D"];
let values = [0.25, 0.35, 0.20, 0.20];

PieChartPlot::new(labels, &values, 0.5, 0.5, 0.4)
    .normalize()
    .exploding()
    .with_angle0(90.0)
    .plot();
```

### Error Bars

```rust
let errors = [0.1, 0.2, 0.15, 0.3, 0.25];

ErrorBarsPlot::new("Measurements", &x_data, &y_data, &errors)
    .horizontal()
    .plot();
```

### Shaded Plots

```rust
// Shaded area plot
ShadedPlot::new("Area", &x_data, &y_data)
    .with_y_ref(0.0)
    .plot();

// Shaded between two curves
ShadedBetweenPlot::new("Between", &x_data, &y1_data, &y2_data)
    .plot();
```

## Advanced Features

### Subplots

```rust
if let Ok(token) = SubplotGrid::new("My Subplots", 2, 2)
    .with_size([800.0, 600.0])
    .with_flags(SubplotFlags::NONE)
    .begin() {

    // First subplot
    if let Some(plot_token) = plot_ui.begin_plot("") {
        LinePlot::new("Line", &x_data, &y_data).plot();
        plot_token.end();
    }

    // Second subplot
    if let Some(plot_token) = plot_ui.begin_plot("") {
        BarPlot::new("Bars", &values).plot();
        plot_token.end();
    }

    // ... more subplots

    token.end();
}
```

### Multi-Axis Plots

```rust
let mut multi_plot = MultiAxisPlot::new("Multi-Axis")
    .add_y_axis(YAxisConfig {
        label: Some("Temperature (°C)"),
        flags: AxisFlags::NONE,
        range: Some((-10.0, 40.0)),
    })
    .add_y_axis(YAxisConfig {
        label: Some("Pressure (hPa)"),
        flags: AxisFlags::NONE,
        range: Some((900.0, 1100.0)),
    });

if let Ok(token) = multi_plot.begin() {
    // Plot on first Y-axis
    token.set_y_axis(0);
    LinePlot::new("Temperature", &time, &temp).plot();

    // Plot on second Y-axis
    token.set_y_axis(1);
    LinePlot::new("Pressure", &time, &pressure).plot();

    token.end();
}
```

### Universal Plot Builder

For a more unified API, you can use the `PlotBuilder`:

```rust
// Line plot
PlotBuilder::line("My Line", &x_data, &y_data).build()?;

// Bar plot
PlotBuilder::bar("My Bars", &values).build()?;

// Histogram
PlotBuilder::histogram("Distribution", &data).build()?;

// Heatmap
PlotBuilder::heatmap("Heat", &data, 10, 10).build()?;
```

## Error Handling

All plot functions return `Result<(), PlotError>` for proper error handling:

```rust
match LinePlot::new("My Plot", &x_data, &y_data).validate() {
    Ok(_) => {
        // Plot is valid, proceed
        LinePlot::new("My Plot", &x_data, &y_data).plot();
    }
    Err(PlotError::DataLengthMismatch { expected, actual }) => {
        eprintln!("Data length mismatch: expected {}, got {}", expected, actual);
    }
    Err(PlotError::EmptyData) => {
        eprintln!("Cannot plot empty data");
    }
    Err(e) => {
        eprintln!("Plot error: {}", e);
    }
}
```

## Integration with Dear ImGui

This crate is designed to work seamlessly with the dear-imgui ecosystem:

- Uses the same context management patterns as dear-imgui
- Compatible with dear-imgui's UI tokens and lifetime management
- Shares the same underlying Dear ImGui context
- Follows the same safety and ergonomics principles

## Examples

See the `examples/` directory for complete working examples:

- `plot_gallery.rs` - Comprehensive showcase of all plot types
- `advanced_features.rs` - Subplots and multi-axis examples
- `real_time_plotting.rs` - Dynamic data visualization

## Building

This crate requires:

- Rust 1.70 or later
- C++ compiler (for building ImPlot)
- CMake (for building dependencies)

The build process automatically handles:

- Building ImPlot from source
- Generating C++ wrapper functions
- Linking with dear-imgui

## Architecture

This crate follows the same architectural patterns as `dear-imgui`:

- **Context Management**: Separate ImPlot context that works alongside Dear ImGui
- **Lifetime Safety**: Plot tokens ensure proper begin/end pairing
- **RAII**: Automatic cleanup of resources and style stacks
- **Builder Patterns**: Fluent APIs for configuration
- **Modular Design**: Each plot type is in its own module

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
