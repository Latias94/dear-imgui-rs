# Dear ImPlot - Rust Bindings

[![Crates.io](https://img.shields.io/crates/v/dear-implot.svg)](https://crates.io/crates/dear-implot)
[![Documentation](https://docs.rs/dear-implot/badge.svg)](https://docs.rs/dear-implot)

High-level Rust bindings for ImPlot, the immediate mode plotting library. This crate provides safe, idiomatic Rust bindings designed to work seamlessly with `dear-imgui-rs` (which uses cimgui C API) and `dear-implot-sys` (which uses cimplot C API).

For native build/link options (source, system/prebuilt, remote prebuilt), see `extensions/dear-implot-sys/README.md`.

<p align="center">
  <img src="https://raw.githubusercontent.com/Latias94/dear-imgui-rs/main/screenshots/implot-basic.png" alt="ImPlot" width="75%"/>
  <br/>
</p>

## Links

- Upstream: https://github.com/epezent/implot
- C API: https://github.com/cimgui/cimplot

## Compatibility

The `0.22` release train requires Rust 1.95 or newer. Upgrade all workspace crates together. See the [0.22 migration notes](../../docs/COMPATIBILITY.md#022-release-train).

| Item              | Version |
|-------------------|---------|
| Crate             | 0.22.0  |
| dear-imgui-rs     | 0.22.0  |
| dear-implot-sys   | 0.22.0  |

### WASM (WebAssembly) support

This crate has **experimental** support for `wasm32-unknown-unknown` targets via the same import-style design used by the core ImGui bindings:

- `dear-implot` + `dear-implot-sys` expose a `wasm` feature which:
  - Enables import-style FFI that links against the shared `imgui-sys-v1` provider module.
  - Avoids compiling C/C++ during the Rust build for wasm.
- The provider module (`imgui-sys-v1`) is built once using Emscripten and contains:
  - Dear ImGui + cimgui (from `dear-imgui-sys`)
  - ImPlot + cimplot (from `dear-implot-sys`)

To try the web demo with ImPlot enabled:

```bash
# 1) Generate pregenerated wasm bindings (Dear ImGui core + ImPlot)
cargo run -p xtask -- wasm-bindgen
cargo run -p xtask -- wasm-bindgen-implot

# 2) Build the main wasm + JS (includes ImPlot demo window)
cargo run -p xtask -- web-demo implot

# 3) Build the provider (Emscripten imgui-sys-v1 with ImGui + ImPlot)
cargo run -p xtask -- build-cimgui-provider

# 4) Serve and open in a browser
python -m http.server -d target/web-demo 8080
```

Notes:
- The `dear-imgui-web-demo` crate in `examples-wasm` enables `implot` when passed to `xtask web-demo implot`; it is not a default feature.
- This is an early, experimental path; API and build steps may evolve in future releases. For production use, pin to a specific compatible `0.22` release and follow changes in `docs/WASM.md`.

See also: [docs/COMPATIBILITY.md](https://github.com/Latias94/dear-imgui-rs/blob/main/docs/COMPATIBILITY.md) for the full workspace matrix.


## Integration Quickstart

This crate integrates with `dear-imgui-rs` directly — add both crates, then build plots inside an ImGui window using a `PlotContext` bound to the current ImGui context.

Use compatible stable requirements:

```toml
[dependencies]
dear-imgui-rs = "0.22"
dear-implot = "0.22"
```

```rust
use dear_imgui_rs as imgui;
use dear_implot::*;

fn main() {
    let mut ctx = imgui::Context::create();
    ctx.io_mut().set_display_size([800.0, 600.0]);
    ctx.io_mut().set_delta_time(1.0 / 60.0);
    ctx.font_atlas().try_claim_legacy_renderer()
        .expect("CPU-only example requires the legacy atlas").build();
    let plot_ctx = PlotContext::create(&ctx);

    // Build one CPU-only frame without a platform or GPU renderer.
    let ui = ctx.frame();
    // Ui extension: obtain PlotUi from Ui + PlotContext
    let plot_ui = ui.implot(&plot_ctx);
    ui.window("Plot Example").build(|| {
        if let Some(token) = plot_ui.begin_plot("Line") {
            let x = [0.0, 1.0, 2.0, 3.0];
            let y = [0.0, 1.0, 4.0, 9.0];
            LinePlot::new("y=x^2", &x, &y).plot(&plot_ui);
            token.end();
        }
    });
    drop(plot_ui);
    drop(ctx.render_legacy());
}
```

This backend-free sample builds CPU draw data only. A visible application needs
a platform backend and renderer; use `implot_minimal` below for that integration.
Managed renderers must consume their normal frame/texture protocol instead of
this CPU-only legacy path.

Notes:
- Base ImGui static library linking is handled by `dear-imgui-sys`; you do not need to link it here.
- Refer to the `-sys` README for `IMPLOT_SYS_*` env vars when using prebuilt libraries.

### Migrating live axis links

`PlotUi::setup_axis_links` and `set_next_axis_links` now require `unsafe`, like
their raw-axis variants. ImPlot retains the supplied `f64` pointers and writes
back at `EndPlot`, so a short Rust reference cannot prove their full lifetime.
Keep storage at a stable address without other access until `PlotContext` is
destroyed, or explicitly clear all stored/pending links. To clear an applied
link, begin the same plot ID, enable the axis, call `setup_axis_links` with
`None`, then end that plot. Clear pending next-plot links separately.
`EndPlot` alone does not unlink every axis: subsequent plots can still write
through a retained secondary-axis pointer. Rejected begin calls and frame
changes do not generally discard pending links. `set_next_axis_links` targets
the next plot, not the next frame. Follow each method's setup-phase requirements.
Prefer `setup_*_axis_limits` / `set_next_*_axis_limits` when live writeback is
unnecessary; these copy values and remain safe.

### Validated numeric formats

ImPlot value labels use validated `FloatFormat` values instead of arbitrary C
format strings. Axis formats additionally use `AxisFormat`, which enforces
ImPlot's fixed 15-byte storage limit. These formats must be ASCII because
ImPlot forwards the complete string to locale-sensitive native formatting
paths. Use an axis formatter closure when labels need localized or arbitrary
UTF-8 text.

```rust
let value_format = FloatFormat::new("%.2f ms")?;
let axis_format = AxisFormat::new("%.1f")?;
```

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
dear-imgui-rs = "0.22"
dear-implot = "0.22"
```

Basic usage:

```rust
use dear_imgui_rs::*;
use dear_implot::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut ctx = Context::create();
    ctx.io_mut().set_display_size([800.0, 600.0]);
    ctx.io_mut().set_delta_time(1.0 / 60.0);
    ctx.font_atlas().try_claim_legacy_renderer()
        .expect("CPU-only example requires the legacy atlas").build();
    let plot_ctx = PlotContext::create(&ctx);

    // One CPU-only frame; use a renderer-driven loop in a visible application.
    let ui = ctx.frame();
    // Or via Ui extension:
    let plot_ui = ui.implot(&plot_ctx);

    ui.window("My Plots")
        .size([800.0, 600.0], Condition::FirstUseEver)
        .build(|| {
            if let Some(token) = plot_ui.begin_plot("Line Plot") {
                let x_data = [0.0, 1.0, 2.0, 3.0, 4.0];
                let y_data = [0.0, 1.0, 4.0, 9.0, 16.0];

                // New modular API
                LinePlot::new("Quadratic", &x_data, &y_data).plot(&plot_ui);

                token.end();
            }
        });
    drop(plot_ui);
    drop(ctx.render_legacy());
    Ok(())
}
```

The following plot snippets assume an initialized renderer-driven frame, an
active plot, and application-owned input arrays. They illustrate API calls;
`implot_minimal` and `implot_showcase` are complete runnable applications.

## Plot Types

### Line and Scatter Plots

```rust
// Line plot
LinePlot::new("My Line", &x_data, &y_data).plot(&plot_ui);

// Scatter plot
ScatterPlot::new("My Points", &x_data, &y_data).plot(&plot_ui);

// Item styling via ImPlotSpec-backed builder helpers
LinePlot::new("Styled Line", &x_data, &y_data)
    .with_line_color([0.20, 0.70, 1.00, 0.35]) // alpha comes from the color itself
    .with_line_weight(2.0)
    .with_marker(Marker::Circle)
    .with_marker_size(6.0)
    .with_marker_fill_color([0.20, 0.70, 1.00, 0.85])
    .plot(&plot_ui);

// The same styling helpers are available on other ImPlotSpec-backed builders
BarPlot::new("Styled Bars", &values)
    .with_fill_color([0.90, 0.45, 0.20, 0.80])
    .with_line_color([0.45, 0.15, 0.05, 1.00])
    .with_line_weight(1.5)
    .plot(&plot_ui);
```

### Bar Charts

```rust
// Simple bar chart
BarPlot::new("Bars", &values)
    .with_bar_size(0.8)
    .plot(&plot_ui);

// Positional bar chart
PositionalBarPlot::new("Positioned Bars", &x_positions, &values)
    .with_bar_size(0.5)
    .plot(&plot_ui);
```

### Histograms

```rust
// 1D Histogram
HistogramPlot::new("Distribution", &data)
    .with_bins(20usize)
    .density()
    .cumulative()
    .plot(&plot_ui);

// Automatic binning method
HistogramPlot::new("Auto Distribution", &data)
    .with_bins(BinMethod::Scott)
    .plot(&plot_ui);

// 2D Histogram
Histogram2DPlot::new("2D Distribution", &x_data, &y_data)
    .with_bins(10usize, 10usize)
    .plot(&plot_ui);
```

### Heatmaps

```rust
let data: Vec<f64> = (0..100).map(|i| (i as f64 * 0.1).sin()).collect();

HeatmapPlot::new("Heat", &data, 10, 10)
    .with_scale(-1.0, 1.0)
    .with_bounds(0.0, 0.0, 1.0, 1.0)
    .try_label_format("%.2f")
    .expect("static numeric format is valid")
    .plot(&plot_ui);
```

### Pie Charts

```rust
let labels = vec!["A", "B", "C", "D"];
let values = [0.25, 0.35, 0.20, 0.20];

PieChartPlot::new(labels, &values, 0.5, 0.5, 0.4)
    .normalize()
    .exploding()
    .with_start_angle(90.0)
    .plot(&plot_ui);
```

### Error Bars

```rust
let errors = [0.1, 0.2, 0.15, 0.3, 0.25];

ErrorBarsPlot::new("Measurements", &x_data, &y_data, &errors)
    .horizontal()
    .plot(&plot_ui);
```

### Shaded Plots

```rust
// Shaded area plot
ShadedPlot::new("Area", &x_data, &y_data)
    .with_y_ref(0.0)
    .plot(&plot_ui);

// Shaded between two curves
ShadedBetweenPlot::new("Between", &x_data, &y1_data, &y2_data)
    .plot(&plot_ui);
```

## Advanced Features

### Subplots

```rust
if let Ok(token) = SubplotGrid::new("My Subplots", 2usize, 2usize)
    .with_size([800.0, 600.0])
    .with_flags(SubplotFlags::NONE)
    .begin(&plot_ui) {

    // First subplot
    if let Some(plot_token) = plot_ui.begin_plot("") {
        LinePlot::new("Line", &x_data, &y_data).plot(&plot_ui);
        plot_token.end();
    }

    // Second subplot
    if let Some(plot_token) = plot_ui.begin_plot("") {
        BarPlot::new("Bars", &values).plot(&plot_ui);
        plot_token.end();
    }

    // ... more subplots

    token.end();
}
```

### Multi-Axis Plots

```rust
let multi_plot = MultiAxisPlot::new("Multi-Axis")
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

if let Ok(token) = multi_plot.begin(&plot_ui) {
    // Plot on first Y-axis
    token.set_y_axis(YAxis::Y1);
    LinePlot::new("Temperature", &time, &temp).plot(&plot_ui);

    // Plot on second Y-axis
    token.set_y_axis(YAxis::Y2);
    LinePlot::new("Pressure", &time, &pressure).plot(&plot_ui);

    token.end();
}
```

### Axis Setup & Selection

```rust
// Inside an active plot (between begin_plot/end)
// Configure axes labels/flags
plot_ui.setup_axes(Some("Time (s)"), Some("Value"), AxisFlags::NO_GRID_LINES, AxisFlags::NONE);

// Set explicit ticks on X1
let tick_pos = [0.0, 1.0, 2.0, 3.0];
let tick_lbl = ["0s", "1s", "2s", "3s"];
plot_ui.setup_x_axis_ticks_positions(XAxis::X1, &tick_pos, Some(&tick_lbl), true);

// Format Y1 ticks
let y_axis_format = AxisFormat::new("%.2f").expect("static axis format is valid");
plot_ui.setup_y_axis_format(YAxis::Y1, &y_axis_format);

// Apply limits
plot_ui.setup_axes_limits(0.0, 3.0, -1.0, 1.0, PlotCond::Once);

// Selection query
if plot_ui.is_plot_selected() {
    if let Some(rect) = plot_ui.plot_selection_axes(XAxis::X1, YAxis::Y1) {
        // rect.X.Min/Max, rect.Y.Min/Max
    }
}
```

### Infinite Lines & Image

```rust
// Infinite lines
plot_ui.inf_lines_vertical("vlines", &[1.0, 2.0, 3.0])?;
plot_ui.inf_lines_horizontal("hlines", &[0.0])?;

// Image plot (using ImTextureID)
let bounds_min = ImPlotPoint { x: 0.0, y: 0.0 };
let bounds_max = ImPlotPoint { x: 1.0, y: 1.0 };
plot_ui.plot_image("image", my_tex_id, bounds_min, bounds_max)?;
```

### Universal Plot Builder

For a unified fallible API, use `PlotBuilder`:

```rust
// Line plot
PlotBuilder::line("My Line", &x_data, &y_data).build(&plot_ui)?;

// Bar plot
PlotBuilder::bar("My Bars", &values).build(&plot_ui)?;

// Histogram
PlotBuilder::histogram("Distribution", &data).build(&plot_ui)?;

// Heatmap
PlotBuilder::heatmap("Heat", &data, 10, 10).build(&plot_ui)?;
```

## Error Handling

`PlotBuilder::build` and the fallible `PlotUi` helpers return `Result<(), PlotError>`.
The `Plot::plot` trait method returns `()` and skips invalid data; call `validate()`
first when the application needs an error:

```rust
match LinePlot::new("My Plot", &x_data, &y_data).validate() {
    Ok(_) => {
        // Plot is valid, proceed
        LinePlot::new("My Plot", &x_data, &y_data).plot(&plot_ui);
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

- `examples/03-features/implot_minimal.rs` - Minimal plot integration.
- `examples/03-features/implot_showcase.rs` - Plot gallery, subplots, and dynamic data.

```text
cargo run -p dear-imgui-examples --bin implot_minimal --features implot
cargo run -p dear-imgui-examples --bin implot_showcase --features implot
```

## Building

This crate requires:

- Rust 1.95 or later (the workspace minimum).
- A C++17 compiler for default native source builds.
- Initialized recursive submodules when building from a Git checkout.

CMake is optional: `IMPLOT_SYS_USE_CMAKE` requests its alternate source route,
which can fall back to the maintained `cc` route when an upstream wrapper needs
a repository patch. Ordinary builds use validated checked-in Rust bindings and
do not require libclang. Explicit regeneration requires `bindgen` and libclang.

The build process automatically handles:

- Compiling the pinned ImPlot and cimplot sources.
- Validating checked-in binding provenance.
- Linking to the shared Dear ImGui core supplied by `dear-imgui-sys`.

## Architecture

This crate follows the same architectural patterns as `dear-imgui-rs`:

- **Context Management**: Separate ImPlot context that works alongside Dear ImGui
- **Lifetime Safety**: Plot tokens ensure proper begin/end pairing
- **RAII**: Automatic cleanup of resources and style stacks
- **Builder Patterns**: Fluent APIs for configuration
- **Modular Design**: Each plot type is in its own module

## License

The Rust crate is licensed under MIT OR Apache-2.0, matching the workspace.
The pinned ImPlot/cimplot sources retain their upstream license notices.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
