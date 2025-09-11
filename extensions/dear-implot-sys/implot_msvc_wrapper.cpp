// MSVC ABI compatibility wrappers for ImPlot
// Similar to imgui_msvc_wrapper.cpp but for ImPlot functions

#ifdef _MSC_VER

#include "implot.h"

// POD types for MSVC ABI compatibility
struct ImVec2_Pod {
    float x, y;
};

struct ImPlotPoint_Pod {
    double x, y;
};

struct ImPlotRange_Pod {
    double Min, Max;
};

struct ImPlotRect_Pod {
    ImPlotRange_Pod X, Y;
};

// Helper functions for conversion
static inline ImVec2_Pod to_pod(const ImVec2& v) {
    ImVec2_Pod result;
    result.x = v.x;
    result.y = v.y;
    return result;
}

static inline ImPlotPoint_Pod to_pod(const ImPlotPoint& p) {
    ImPlotPoint_Pod result;
    result.x = p.x;
    result.y = p.y;
    return result;
}

static inline ImPlotRange_Pod to_pod(const ImPlotRange& r) {
    ImPlotRange_Pod result;
    result.Min = r.Min;
    result.Max = r.Max;
    return result;
}

static inline ImPlotRect_Pod to_pod(const ImPlotRect& r) {
    ImPlotRect_Pod result;
    result.X = to_pod(r.X);
    result.Y = to_pod(r.Y);
    return result;
}

extern "C" {

// MSVC ABI Fix: ImPlot Functions that return small structs by value
ImVec2_Pod ImPlot_GetPlotPos() {
    return to_pod(ImPlot::GetPlotPos());
}

ImVec2_Pod ImPlot_GetPlotSize() {
    return to_pod(ImPlot::GetPlotSize());
}

ImPlotPoint_Pod ImPlot_GetPlotMousePos(int y_axis) {
    return to_pod(ImPlot::GetPlotMousePos(y_axis));
}

ImPlotPoint_Pod ImPlot_PixelsToPlotVec2(ImVec2 pix, int y_axis) {
    return to_pod(ImPlot::PixelsToPlot(pix, y_axis));
}

ImPlotPoint_Pod ImPlot_PixelsToPlotFloat(float x, float y, int y_axis) {
    return to_pod(ImPlot::PixelsToPlot(x, y, y_axis));
}

ImVec2_Pod ImPlot_PlotToPixelsPlotPoint(ImPlotPoint plt, int y_axis) {
    return to_pod(ImPlot::PlotToPixels(plt, y_axis));
}

ImVec2_Pod ImPlot_PlotToPixelsDouble(double x, double y, int y_axis) {
    return to_pod(ImPlot::PlotToPixels(x, y, y_axis));
}

ImPlotRect_Pod ImPlot_GetPlotLimits(int x_axis, int y_axis) {
    return to_pod(ImPlot::GetPlotLimits(x_axis, y_axis));
}

} // extern "C"

#endif // _MSC_VER
