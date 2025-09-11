// ImPlot C++ wrapper for dear-imgui-sys compatibility
// This file includes ImPlot sources and provides C++ bindings

// Define required macros before including headers
#define IMGUI_DEFINE_MATH_OPERATORS

// Include Dear ImGui headers only (implementation comes from dear-imgui-sys)
#include "imgui.h"
#include "imgui_internal.h"

// Include ImPlot implementation
#include "third-party/implot/implot.cpp"
#include "third-party/implot/implot_items.cpp"
#include "third-party/implot/implot_demo.cpp"

// C-style wrapper functions for template functions
extern "C" {
    // Basic plot types
    void ImPlot_PlotLine_double(const char* label_id, const double* xs, const double* ys, int count) {
        ImPlot::PlotLine(label_id, xs, ys, count);
    }

    void ImPlot_PlotScatter_double(const char* label_id, const double* xs, const double* ys, int count) {
        ImPlot::PlotScatter(label_id, xs, ys, count);
    }

    void ImPlot_PlotBars_double(const char* label_id, const double* values, int count, double width = 0.67, double shift = 0) {
        ImPlot::PlotBars(label_id, values, count, width, shift);
    }

    // Heatmap functions
    void ImPlot_PlotHeatmap_float(const char* label_id, const float* values, int rows, int cols,
                                  double scale_min, double scale_max, const char* label_fmt,
                                  double bounds_min_x, double bounds_min_y,
                                  double bounds_max_x, double bounds_max_y, int flags) {
        ImPlotPoint bounds_min(bounds_min_x, bounds_min_y);
        ImPlotPoint bounds_max(bounds_max_x, bounds_max_y);
        ImPlot::PlotHeatmap(label_id, values, rows, cols, scale_min, scale_max, label_fmt,
                           bounds_min, bounds_max, static_cast<ImPlotHeatmapFlags>(flags));
    }

    void ImPlot_PlotHeatmap_double(const char* label_id, const double* values, int rows, int cols,
                                   double scale_min, double scale_max, const char* label_fmt,
                                   double bounds_min_x, double bounds_min_y,
                                   double bounds_max_x, double bounds_max_y, int flags) {
        ImPlotPoint bounds_min(bounds_min_x, bounds_min_y);
        ImPlotPoint bounds_max(bounds_max_x, bounds_max_y);
        ImPlot::PlotHeatmap(label_id, values, rows, cols, scale_min, scale_max, label_fmt,
                           bounds_min, bounds_max, static_cast<ImPlotHeatmapFlags>(flags));
    }

    // Histogram functions
    double ImPlot_PlotHistogram_float(const char* label_id, const float* values, int count,
                                      int bins, double bar_scale, double range_min, double range_max, int flags) {
        ImPlotRange range(range_min, range_max);
        return ImPlot::PlotHistogram(label_id, values, count, bins, bar_scale, range,
                                    static_cast<ImPlotHistogramFlags>(flags));
    }

    double ImPlot_PlotHistogram_double(const char* label_id, const double* values, int count,
                                       int bins, double bar_scale, double range_min, double range_max, int flags) {
        ImPlotRange range(range_min, range_max);
        return ImPlot::PlotHistogram(label_id, values, count, bins, bar_scale, range,
                                    static_cast<ImPlotHistogramFlags>(flags));
    }

    double ImPlot_PlotHistogram2D_float(const char* label_id, const float* xs, const float* ys, int count,
                                        int x_bins, int y_bins, double range_x_min, double range_x_max,
                                        double range_y_min, double range_y_max, int flags) {
        ImPlotRect range;
        range.X.Min = range_x_min;
        range.X.Max = range_x_max;
        range.Y.Min = range_y_min;
        range.Y.Max = range_y_max;
        return ImPlot::PlotHistogram2D(label_id, xs, ys, count, x_bins, y_bins, range,
                                      static_cast<ImPlotHistogramFlags>(flags));
    }

    double ImPlot_PlotHistogram2D_double(const char* label_id, const double* xs, const double* ys, int count,
                                         int x_bins, int y_bins, double range_x_min, double range_x_max,
                                         double range_y_min, double range_y_max, int flags) {
        ImPlotRect range;
        range.X.Min = range_x_min;
        range.X.Max = range_x_max;
        range.Y.Min = range_y_min;
        range.Y.Max = range_y_max;
        return ImPlot::PlotHistogram2D(label_id, xs, ys, count, x_bins, y_bins, range,
                                      static_cast<ImPlotHistogramFlags>(flags));
    }

    // Pie chart functions
    void ImPlot_PlotPieChart_float(const char* const label_ids[], const float* values, int count,
                                   double x, double y, double radius, const char* label_fmt,
                                   double angle0, int flags) {
        ImPlot::PlotPieChart(label_ids, values, count, x, y, radius, label_fmt, angle0,
                            static_cast<ImPlotPieChartFlags>(flags));
    }

    void ImPlot_PlotPieChart_double(const char* const label_ids[], const double* values, int count,
                                    double x, double y, double radius, const char* label_fmt,
                                    double angle0, int flags) {
        ImPlot::PlotPieChart(label_ids, values, count, x, y, radius, label_fmt, angle0,
                            static_cast<ImPlotPieChartFlags>(flags));
    }

    // Additional plot types
    void ImPlot_PlotShaded_double(const char* label_id, const double* xs, const double* ys, int count,
                                  double yref, int flags) {
        ImPlot::PlotShaded(label_id, xs, ys, count, yref, static_cast<ImPlotShadedFlags>(flags));
    }

    void ImPlot_PlotStems_double(const char* label_id, const double* xs, const double* ys, int count,
                                 double yref, int flags) {
        ImPlot::PlotStems(label_id, xs, ys, count, yref, static_cast<ImPlotStemsFlags>(flags));
    }

    void ImPlot_PlotErrorBars_double(const char* label_id, const double* xs, const double* ys,
                                     const double* err, int count, int flags) {
        ImPlot::PlotErrorBars(label_id, xs, ys, err, count, static_cast<ImPlotErrorBarsFlags>(flags));
    }

    // Stairs plot functions
    void ImPlot_PlotStairs_float(const char* label_id, const float* xs, const float* ys, int count, int flags) {
        ImPlot::PlotStairs(label_id, xs, ys, count, static_cast<ImPlotStairsFlags>(flags));
    }

    void ImPlot_PlotStairs_double(const char* label_id, const double* xs, const double* ys, int count, int flags) {
        ImPlot::PlotStairs(label_id, xs, ys, count, static_cast<ImPlotStairsFlags>(flags));
    }

    // Bar groups functions
    void ImPlot_PlotBarGroups_float(const char* const label_ids[], const float* values,
                                    int item_count, int group_count, double group_size, double shift, int flags) {
        ImPlot::PlotBarGroups(label_ids, values, item_count, group_count, group_size, shift, static_cast<ImPlotBarGroupsFlags>(flags));
    }

    void ImPlot_PlotBarGroups_double(const char* const label_ids[], const double* values,
                                     int item_count, int group_count, double group_size, double shift, int flags) {
        ImPlot::PlotBarGroups(label_ids, values, item_count, group_count, group_size, shift, static_cast<ImPlotBarGroupsFlags>(flags));
    }

    // Digital plot functions
    void ImPlot_PlotDigital_float(const char* label_id, const float* xs, const float* ys, int count, int flags) {
        ImPlot::PlotDigital(label_id, xs, ys, count, static_cast<ImPlotDigitalFlags>(flags));
    }

    void ImPlot_PlotDigital_double(const char* label_id, const double* xs, const double* ys, int count, int flags) {
        ImPlot::PlotDigital(label_id, xs, ys, count, static_cast<ImPlotDigitalFlags>(flags));
    }

    // Text and dummy functions
    void ImPlot_PlotText(const char* text, double x, double y, double pix_offset_x, double pix_offset_y, int flags) {
        ImVec2 pix_offset(pix_offset_x, pix_offset_y);
        ImPlot::PlotText(text, x, y, pix_offset, static_cast<ImPlotTextFlags>(flags));
    }

    void ImPlot_PlotDummy(const char* label_id, int flags) {
        ImPlot::PlotDummy(label_id, static_cast<ImPlotDummyFlags>(flags));
    }

    // Advanced plotting functions
    bool ImPlot_BeginSubplots(const char* title_id, int rows, int cols, float size_x, float size_y, int flags, float* row_ratios, float* col_ratios) {
        return ImPlot::BeginSubplots(title_id, rows, cols, ImVec2(size_x, size_y), flags, row_ratios, col_ratios);
    }

    void ImPlot_EndSubplots() {
        ImPlot::EndSubplots();
    }

    void ImPlot_SetupAxis(int axis, const char* label, int flags) {
        ImPlot::SetupAxis(axis, label, flags);
    }

    void ImPlot_SetupAxisLimits(int axis, double v_min, double v_max, int cond) {
        ImPlot::SetupAxisLimits(axis, v_min, v_max, cond);
    }

    void ImPlot_SetupLegend(int location, int flags) {
        ImPlot::SetupLegend(location, flags);
    }

    void ImPlot_SetAxes(int x_axis, int y_axis) {
        ImPlot::SetAxes(x_axis, y_axis);
    }

    bool ImPlot_BeginLegendPopup(const char* label_id, int mouse_button) {
        return ImPlot::BeginLegendPopup(label_id, mouse_button);
    }

    void ImPlot_EndLegendPopup() {
        ImPlot::EndLegendPopup();
    }
}

#ifdef _MSC_VER
#include "implot_msvc_wrapper.cpp"
#endif
