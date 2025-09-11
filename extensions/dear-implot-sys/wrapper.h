#pragma once

#ifdef __cplusplus
extern "C" {
#endif

// C-style wrapper functions for ImPlot template functions

// Basic plot types
void ImPlot_PlotLine_double(const char* label_id, const double* xs, const double* ys, int count);
void ImPlot_PlotScatter_double(const char* label_id, const double* xs, const double* ys, int count);
void ImPlot_PlotBars_double(const char* label_id, const double* values, int count, double width, double shift);

// Heatmap functions
void ImPlot_PlotHeatmap_float(const char* label_id, const float* values, int rows, int cols,
                              double scale_min, double scale_max, const char* label_fmt,
                              double bounds_min_x, double bounds_min_y,
                              double bounds_max_x, double bounds_max_y, int flags);
void ImPlot_PlotHeatmap_double(const char* label_id, const double* values, int rows, int cols,
                               double scale_min, double scale_max, const char* label_fmt,
                               double bounds_min_x, double bounds_min_y,
                               double bounds_max_x, double bounds_max_y, int flags);

// Histogram functions
double ImPlot_PlotHistogram_float(const char* label_id, const float* values, int count,
                                  int bins, double bar_scale, double range_min, double range_max, int flags);
double ImPlot_PlotHistogram_double(const char* label_id, const double* values, int count,
                                   int bins, double bar_scale, double range_min, double range_max, int flags);
double ImPlot_PlotHistogram2D_float(const char* label_id, const float* xs, const float* ys, int count,
                                    int x_bins, int y_bins, double range_x_min, double range_x_max,
                                    double range_y_min, double range_y_max, int flags);
double ImPlot_PlotHistogram2D_double(const char* label_id, const double* xs, const double* ys, int count,
                                     int x_bins, int y_bins, double range_x_min, double range_x_max,
                                     double range_y_min, double range_y_max, int flags);

// Pie chart functions
void ImPlot_PlotPieChart_float(const char* const label_ids[], const float* values, int count,
                               double x, double y, double radius, const char* label_fmt,
                               double angle0, int flags);
void ImPlot_PlotPieChart_double(const char* const label_ids[], const double* values, int count,
                                double x, double y, double radius, const char* label_fmt,
                                double angle0, int flags);

// Additional plot types
void ImPlot_PlotShaded_double(const char* label_id, const double* xs, const double* ys, int count,
                              double yref, int flags);
void ImPlot_PlotStems_double(const char* label_id, const double* xs, const double* ys, int count,
                             double yref, int flags);
void ImPlot_PlotErrorBars_double(const char* label_id, const double* xs, const double* ys,
                                 const double* err, int count, int flags);

// Stairs plot functions
void ImPlot_PlotStairs_float(const char* label_id, const float* xs, const float* ys, int count, int flags);
void ImPlot_PlotStairs_double(const char* label_id, const double* xs, const double* ys, int count, int flags);

// Bar groups functions
void ImPlot_PlotBarGroups_float(const char* const label_ids[], const float* values,
                                int item_count, int group_count, double group_size, double shift, int flags);
void ImPlot_PlotBarGroups_double(const char* const label_ids[], const double* values,
                                 int item_count, int group_count, double group_size, double shift, int flags);

// Digital plot functions
void ImPlot_PlotDigital_float(const char* label_id, const float* xs, const float* ys, int count, int flags);
void ImPlot_PlotDigital_double(const char* label_id, const double* xs, const double* ys, int count, int flags);

// Text and dummy functions
void ImPlot_PlotText(const char* text, double x, double y, double pix_offset_x, double pix_offset_y, int flags);
void ImPlot_PlotDummy(const char* label_id, int flags);

// Advanced plotting functions
bool ImPlot_BeginSubplots(const char* title_id, int rows, int cols, float size_x, float size_y, int flags, float* row_ratios, float* col_ratios);
void ImPlot_EndSubplots();
void ImPlot_SetupAxis(int axis, const char* label, int flags);
void ImPlot_SetupAxisLimits(int axis, double v_min, double v_max, int cond);
void ImPlot_SetupLegend(int location, int flags);
void ImPlot_SetAxes(int x_axis, int y_axis);
bool ImPlot_BeginLegendPopup(const char* label_id, int mouse_button);
void ImPlot_EndLegendPopup();

#ifdef __cplusplus
}
#endif
