//! Example showcasing the new modular plot system
//!
//! This example demonstrates how to use the new plot types with
//! a consistent API and builder pattern.

use dear_imgui::*;
use dear_implot::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create ImGui context
    let mut ctx = Context::create_or_panic();

    // Create ImPlot context
    let plot_ctx = PlotContext::create(&ctx);

    // Sample data
    let x_data = [0.0, 1.0, 2.0, 3.0, 4.0, 5.0];
    let y_data = [0.0, 1.0, 4.0, 9.0, 16.0, 25.0];
    let histogram_data = [1.0, 2.0, 1.0, 3.0, 2.0, 4.0, 3.0, 2.0, 1.0, 2.0];
    let heatmap_data = (0..100).map(|i| (i as f64 * 0.1).sin()).collect::<Vec<_>>();

    // Main loop (simplified for example)
    loop {
        let ui = ctx.frame();
        let plot_ui = plot_ctx.get_plot_ui(&ui);

        // Create a window for our plots
        ui.window("Plot Gallery")
            .size([800.0, 600.0], Condition::FirstUseEver)
            .build(|| {
                // Line Plot Example
                ui.text("Line Plot:");
                if let Some(token) = plot_ui.begin_plot("Line Plot Example") {
                    // Using the new modular line plot
                    LinePlot::new("Quadratic", &x_data, &y_data).plot();

                    // Or using the convenience method
                    plot_ui.line_plot("Simple Line", &x_data, &y_data).ok();

                    token.end();
                }

                ui.separator();

                // Scatter Plot Example
                ui.text("Scatter Plot:");
                if let Some(token) = plot_ui.begin_plot("Scatter Plot Example") {
                    ScatterPlot::new("Points", &x_data, &y_data).plot();

                    token.end();
                }

                ui.separator();

                // Bar Plot Example
                ui.text("Bar Plot:");
                if let Some(token) = plot_ui.begin_plot("Bar Plot Example") {
                    BarPlot::new("Bars", &y_data).with_bar_size(0.8).plot();

                    token.end();
                }

                ui.separator();

                // Histogram Example
                ui.text("Histogram:");
                if let Some(token) = plot_ui.begin_plot("Histogram Example") {
                    HistogramPlot::new("Distribution", &histogram_data)
                        .with_bins(10)
                        .density()
                        .plot();

                    token.end();
                }

                ui.separator();

                // Heatmap Example
                ui.text("Heatmap:");
                if let Some(token) = plot_ui.begin_plot("Heatmap Example") {
                    HeatmapPlot::new("Heat", &heatmap_data, 10, 10)
                        .with_scale(-1.0, 1.0)
                        .with_bounds(0.0, 0.0, 1.0, 1.0)
                        .plot();

                    token.end();
                }

                ui.separator();

                // Pie Chart Example
                ui.text("Pie Chart:");
                if let Some(token) = plot_ui.begin_plot("Pie Chart Example") {
                    let labels = vec!["A", "B", "C", "D"];
                    let values = [0.25, 0.35, 0.20, 0.20];

                    PieChartPlot::new(labels, &values, 0.5, 0.5, 0.4)
                        .normalize()
                        .exploding()
                        .plot();

                    token.end();
                }

                ui.separator();

                // Error Bars Example
                ui.text("Error Bars:");
                if let Some(token) = plot_ui.begin_plot("Error Bars Example") {
                    let errors = [0.1, 0.2, 0.15, 0.3, 0.25, 0.2];

                    ErrorBarsPlot::new("Measurements", &x_data, &y_data, &errors).plot();

                    token.end();
                }

                ui.separator();

                // Shaded Plot Example
                ui.text("Shaded Plot:");
                if let Some(token) = plot_ui.begin_plot("Shaded Plot Example") {
                    ShadedPlot::new("Area", &x_data, &y_data)
                        .with_y_ref(5.0)
                        .plot();

                    token.end();
                }

                ui.separator();

                // Stem Plot Example
                ui.text("Stem Plot:");
                if let Some(token) = plot_ui.begin_plot("Stem Plot Example") {
                    StemPlot::new("Stems", &x_data, &y_data).plot();

                    token.end();
                }
            });

        // Break the loop for this example
        break;
    }

    Ok(())
}

/// Demonstrates advanced plot combinations
fn advanced_plot_example(plot_ui: &PlotUi) -> Result<(), PlotError> {
    if let Some(token) = plot_ui.begin_plot("Advanced Plot") {
        // Multiple plot types in one chart
        let x = [0.0, 1.0, 2.0, 3.0, 4.0];
        let y1 = [0.0, 1.0, 4.0, 9.0, 16.0];
        let y2 = [0.0, 0.5, 2.0, 4.5, 8.0];
        let errors = [0.1, 0.2, 0.3, 0.4, 0.5];

        // Line plot
        LinePlot::new("Function", &x, &y1).plot();

        // Scatter plot with different data
        ScatterPlot::new("Points", &x, &y2).plot();

        // Error bars
        ErrorBarsPlot::new("Errors", &x, &y1, &errors).plot();

        // Shaded area
        ShadedPlot::new("Area", &x, &y2).with_y_ref(0.0).plot();

        token.end();
    }

    Ok(())
}

/// Demonstrates subplot functionality
fn subplot_example(plot_ui: &PlotUi) -> Result<(), PlotError> {
    // Create a 2x2 subplot grid
    if let Ok(token) = SubplotGrid::new("Subplot Example", 2, 2)
        .with_size([800.0, 600.0])
        .with_flags(SubplotFlags::NONE)
        .begin()
    {
        // First subplot - Line plot
        if let Some(plot_token) = plot_ui.begin_plot("") {
            let x = [0.0, 1.0, 2.0, 3.0, 4.0];
            let y = [0.0, 1.0, 4.0, 9.0, 16.0];
            LinePlot::new("Quadratic", &x, &y).plot();
            plot_token.end();
        }

        // Second subplot - Bar plot
        if let Some(plot_token) = plot_ui.begin_plot("") {
            let data = [1.0, 3.0, 2.0, 4.0, 3.0];
            BarPlot::new("Bars", &data).plot();
            plot_token.end();
        }

        // Third subplot - Histogram
        if let Some(plot_token) = plot_ui.begin_plot("") {
            let data = [1.0, 2.0, 1.0, 3.0, 2.0, 4.0, 3.0, 2.0];
            HistogramPlot::new("Distribution", &data)
                .with_bins(5)
                .plot();
            plot_token.end();
        }

        // Fourth subplot - Scatter plot
        if let Some(plot_token) = plot_ui.begin_plot("") {
            let x = [0.0, 1.0, 2.0, 3.0, 4.0];
            let y = [2.0, 1.0, 3.0, 0.5, 2.5];
            ScatterPlot::new("Points", &x, &y).plot();
            plot_token.end();
        }

        token.end();
    }

    Ok(())
}

/// Demonstrates multi-axis plotting
fn multi_axis_example(plot_ui: &PlotUi) -> Result<(), PlotError> {
    let mut multi_plot = MultiAxisPlot::new("Multi-Axis Plot").with_size([600.0, 400.0]);

    // Add additional Y-axes
    multi_plot = multi_plot.add_y_axis(YAxisConfig {
        label: Some("Temperature (°C)"),
        flags: AxisFlags::NONE,
        range: Some((-10.0, 40.0)),
    });

    multi_plot = multi_plot.add_y_axis(YAxisConfig {
        label: Some("Pressure (hPa)"),
        flags: AxisFlags::NONE,
        range: Some((900.0, 1100.0)),
    });

    if let Ok(token) = multi_plot.begin() {
        let time = [0.0, 1.0, 2.0, 3.0, 4.0, 5.0];
        let temp = [20.0, 22.0, 25.0, 23.0, 21.0, 19.0];
        let pressure = [1013.0, 1015.0, 1012.0, 1010.0, 1014.0, 1016.0];

        // Plot temperature on first Y-axis
        token.set_y_axis(0);
        LinePlot::new("Temperature", &time, &temp).plot();

        // Plot pressure on second Y-axis
        token.set_y_axis(1);
        LinePlot::new("Pressure", &time, &pressure).plot();

        token.end();
    }

    Ok(())
}

/// Demonstrates plot customization
fn customization_example(plot_ui: &PlotUi) -> Result<(), PlotError> {
    if let Some(token) = plot_ui.begin_plot("Customized Plot") {
        let data = [1.0, 3.0, 2.0, 4.0, 3.0, 5.0];

        // Customized bar plot
        BarPlot::new("Custom Bars", &data)
            .with_bar_size(0.9)
            .with_flags(BarsFlags::HORIZONTAL)
            .plot();

        // Customized histogram
        HistogramPlot::new("Custom Histogram", &data)
            .with_bins(8)
            .cumulative()
            .density()
            .plot();

        token.end();
    }

    Ok(())
}

/// Demonstrates error handling
fn error_handling_example(plot_ui: &PlotUi) {
    // This will fail validation due to mismatched data lengths
    let x = [1.0, 2.0, 3.0];
    let y = [1.0, 2.0]; // Different length

    match plot_ui.line_plot("Invalid Data", &x, &y) {
        Ok(_) => println!("Plot created successfully"),
        Err(e) => println!("Plot failed: {}", e),
    }

    // This will fail due to empty data
    let empty_data: &[f64] = &[];
    match plot_ui.simple_line_plot("Empty Data", empty_data) {
        Ok(_) => println!("Plot created successfully"),
        Err(e) => println!("Plot failed: {}", e),
    }
}
