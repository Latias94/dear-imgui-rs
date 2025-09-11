//! Simple test example for the new modular dear-implot system
//!
//! This example demonstrates basic usage of the new plot types
//! and validates that the API works as expected.

use dear_imgui::*;
use dear_implot::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing Dear ImPlot Modular System");

    // Test data generation
    let x_data: Vec<f64> = (0..10).map(|i| i as f64).collect();
    let y_data: Vec<f64> = x_data.iter().map(|&x| x * x).collect();
    let values = vec![1.0, 3.0, 2.0, 4.0, 3.0, 5.0, 4.0, 2.0];
    let histogram_data = vec![1.0, 2.0, 1.0, 3.0, 2.0, 4.0, 3.0, 2.0, 1.0, 2.0];

    println!("✓ Test data generated");

    // Test plot creation (without actual rendering)
    test_line_plot(&x_data, &y_data)?;
    test_scatter_plot(&x_data, &y_data)?;
    test_bar_plot(&values)?;
    test_histogram_plot(&histogram_data)?;
    test_heatmap_plot()?;
    test_pie_chart()?;
    test_error_bars(&x_data, &y_data)?;
    test_shaded_plot(&x_data, &y_data)?;
    test_stem_plot(&x_data, &y_data)?;

    // Test universal plot builder
    test_plot_builder(&x_data, &y_data, &values, &histogram_data)?;

    // Test error handling
    test_error_handling()?;

    println!("✓ All tests passed!");
    println!("Dear ImPlot modular system is working correctly.");

    Ok(())
}

fn test_line_plot(x_data: &[f64], y_data: &[f64]) -> Result<(), PlotError> {
    println!("Testing LinePlot...");

    let plot = LinePlot::new("Test Line", x_data, y_data);
    plot.validate()?;

    println!("  ✓ LinePlot validation passed");
    Ok(())
}

fn test_scatter_plot(x_data: &[f64], y_data: &[f64]) -> Result<(), PlotError> {
    println!("Testing ScatterPlot...");

    let plot = ScatterPlot::new("Test Scatter", x_data, y_data);
    plot.validate()?;

    println!("  ✓ ScatterPlot validation passed");
    Ok(())
}

fn test_bar_plot(values: &[f64]) -> Result<(), PlotError> {
    println!("Testing BarPlot...");

    let plot = BarPlot::new("Test Bars", values).with_bar_size(0.8);
    plot.validate()?;

    // Test positional bar plot
    let x_positions = vec![0.0, 1.0, 2.0, 3.0, 4.0];
    let positional_plot =
        PositionalBarPlot::new("Test Positional", &x_positions, &values[..5]).with_bar_size(0.5);
    positional_plot.validate()?;

    println!("  ✓ BarPlot validation passed");
    println!("  ✓ PositionalBarPlot validation passed");
    Ok(())
}

fn test_histogram_plot(data: &[f64]) -> Result<(), PlotError> {
    println!("Testing HistogramPlot...");

    let plot = HistogramPlot::new("Test Histogram", data)
        .with_bins(5)
        .density()
        .cumulative();
    plot.validate()?;

    // Test 2D histogram
    let x_data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
    let y_data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
    let hist2d = Histogram2DPlot::new("Test 2D Histogram", &x_data, &y_data).with_bins(3, 3);
    hist2d.validate()?;

    println!("  ✓ HistogramPlot validation passed");
    println!("  ✓ Histogram2DPlot validation passed");
    Ok(())
}

fn test_heatmap_plot() -> Result<(), PlotError> {
    println!("Testing HeatmapPlot...");

    let data: Vec<f64> = (0..25).map(|i| (i as f64 * 0.1).sin()).collect();

    let plot = HeatmapPlot::new("Test Heatmap", &data, 5, 5)
        .with_scale(-1.0, 1.0)
        .with_bounds(0.0, 0.0, 1.0, 1.0);
    plot.validate()?;

    // Test F32 version
    let data_f32: Vec<f32> = data.iter().map(|&x| x as f32).collect();
    let plot_f32 = HeatmapPlotF32::new("Test Heatmap F32", &data_f32, 5, 5).with_scale(-1.0, 1.0);
    plot_f32.validate()?;

    println!("  ✓ HeatmapPlot validation passed");
    println!("  ✓ HeatmapPlotF32 validation passed");
    Ok(())
}

fn test_pie_chart() -> Result<(), PlotError> {
    println!("Testing PieChartPlot...");

    let labels = vec!["A", "B", "C", "D"];
    let values = [0.25, 0.35, 0.20, 0.20];

    let plot = PieChartPlot::new(labels.clone(), &values, 0.5, 0.5, 0.4)
        .normalize()
        .exploding()
        .with_angle0(90.0);
    plot.validate()?;

    // Test F32 version
    let values_f32 = [0.25f32, 0.35, 0.20, 0.20];
    let plot_f32 = PieChartPlotF32::new(labels, &values_f32, 0.5, 0.5, 0.4).normalize();
    plot_f32.validate()?;

    println!("  ✓ PieChartPlot validation passed");
    println!("  ✓ PieChartPlotF32 validation passed");
    Ok(())
}

fn test_error_bars(x_data: &[f64], y_data: &[f64]) -> Result<(), PlotError> {
    println!("Testing ErrorBarsPlot...");

    let errors = vec![0.1, 0.2, 0.15, 0.3, 0.25, 0.2, 0.1, 0.15, 0.25, 0.2];

    let plot = ErrorBarsPlot::new("Test Error Bars", x_data, y_data, &errors);
    plot.validate()?;

    // Test asymmetric error bars
    let neg_errors = vec![0.05, 0.1, 0.075, 0.15, 0.125, 0.1, 0.05, 0.075, 0.125, 0.1];
    let pos_errors = vec![0.15, 0.3, 0.225, 0.45, 0.375, 0.3, 0.15, 0.225, 0.375, 0.3];

    let asym_plot =
        AsymmetricErrorBarsPlot::new("Test Asymmetric", x_data, y_data, &neg_errors, &pos_errors);
    asym_plot.validate()?;

    println!("  ✓ ErrorBarsPlot validation passed");
    println!("  ✓ AsymmetricErrorBarsPlot validation passed");
    Ok(())
}

fn test_shaded_plot(x_data: &[f64], y_data: &[f64]) -> Result<(), PlotError> {
    println!("Testing ShadedPlot...");

    let plot = ShadedPlot::new("Test Shaded", x_data, y_data).with_y_ref(0.0);
    plot.validate()?;

    // Test shaded between
    let y2_data: Vec<f64> = y_data.iter().map(|&y| y + 10.0).collect();
    let between_plot = ShadedBetweenPlot::new("Test Between", x_data, y_data, &y2_data);
    between_plot.validate()?;

    println!("  ✓ ShadedPlot validation passed");
    println!("  ✓ ShadedBetweenPlot validation passed");
    Ok(())
}

fn test_stem_plot(x_data: &[f64], y_data: &[f64]) -> Result<(), PlotError> {
    println!("Testing StemPlot...");

    let plot = StemPlot::new("Test Stems", x_data, y_data).with_y_ref(0.0);
    plot.validate()?;

    println!("  ✓ StemPlot validation passed");
    Ok(())
}

fn test_plot_builder(
    x_data: &[f64],
    y_data: &[f64],
    values: &[f64],
    hist_data: &[f64],
) -> Result<(), PlotError> {
    println!("Testing PlotBuilder...");

    // Test different plot types through the builder
    let _line_builder = PlotBuilder::line("Builder Line", x_data, y_data);
    let _scatter_builder = PlotBuilder::scatter("Builder Scatter", x_data, y_data);
    let _bar_builder = PlotBuilder::bar("Builder Bar", values);
    let _histogram_builder = PlotBuilder::histogram("Builder Histogram", hist_data);

    let heatmap_data: Vec<f64> = (0..16).map(|i| i as f64).collect();
    let _heatmap_builder = PlotBuilder::heatmap("Builder Heatmap", &heatmap_data, 4, 4);

    let labels = vec!["A", "B", "C"];
    let pie_values = [1.0, 2.0, 3.0];
    let _pie_builder = PlotBuilder::pie_chart(labels, &pie_values, (0.5, 0.5), 0.4);

    println!("  ✓ PlotBuilder creation passed");
    Ok(())
}

fn test_error_handling() -> Result<(), PlotError> {
    println!("Testing error handling...");

    // Test empty data
    let empty_data: &[f64] = &[];
    let x_data = [1.0, 2.0, 3.0];

    match LinePlot::new("Empty", empty_data, &x_data).validate() {
        Err(PlotError::EmptyData) => println!("  ✓ Empty data error caught correctly"),
        _ => {
            return Err(PlotError::InvalidParameter(
                "Expected empty data error".to_string(),
            ))
        }
    }

    // Test mismatched lengths
    let short_data = [1.0, 2.0];
    let long_data = [1.0, 2.0, 3.0, 4.0];

    match LinePlot::new("Mismatched", &short_data, &long_data).validate() {
        Err(PlotError::DataLengthMismatch {
            expected: 2,
            actual: 4,
        }) => {
            println!("  ✓ Data length mismatch error caught correctly");
        }
        _ => {
            return Err(PlotError::InvalidParameter(
                "Expected length mismatch error".to_string(),
            ))
        }
    }

    println!("  ✓ Error handling tests passed");
    Ok(())
}
