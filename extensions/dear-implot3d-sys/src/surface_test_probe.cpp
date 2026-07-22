#include "imgui.h"
#include "imgui_internal.h"
#include "implot3d.h"
#include "cimplot3d.h"

namespace {
constexpr int kCapacity = 16;

struct SurfaceCapture {
    float xs[kCapacity]{};
    float ys[kCapacity]{};
    float zs[kCapacity]{};
    int x_count = 0;
    int y_count = 0;
    int point_count = 0;
};

thread_local SurfaceCapture capture;

void record_surface(
    const float* xs,
    const float* ys,
    const float* zs,
    int x_count,
    int y_count) {
    capture = SurfaceCapture{};
    if (xs == nullptr || ys == nullptr || zs == nullptr || x_count <= 0 || y_count <= 0) {
        return;
    }
    const long long point_count =
        static_cast<long long>(x_count) * static_cast<long long>(y_count);
    if (point_count <= 0 || point_count > kCapacity) {
        return;
    }

    capture.x_count = x_count;
    capture.y_count = y_count;
    capture.point_count = static_cast<int>(point_count);
    for (int index = 0; index < capture.point_count; ++index) {
        capture.xs[index] = xs[index];
        capture.ys[index] = ys[index];
        capture.zs[index] = zs[index];
    }
}
} // namespace

extern "C" void dear_implot3d_surface_probe_reset() {
    capture = SurfaceCapture{};
}

extern "C" void dear_implot3d_surface_probe_plot(
    const char* label_id,
    const float* xs,
    const float* ys,
    const float* zs,
    int x_count,
    int y_count,
    double scale_min,
    double scale_max,
    const ImPlot3DSpec_c spec) {
    record_surface(xs, ys, zs, x_count, y_count);
    ImPlot3D_PlotSurface_FloatPtr(
        label_id,
        xs,
        ys,
        zs,
        x_count,
        y_count,
        scale_min,
        scale_max,
        spec);
}

extern "C" int dear_implot3d_surface_probe_read(
    float* xs,
    float* ys,
    float* zs,
    int capacity,
    int* x_count,
    int* y_count) {
    if (xs == nullptr || ys == nullptr || zs == nullptr || x_count == nullptr ||
        y_count == nullptr || capacity < capture.point_count) {
        return -1;
    }
    *x_count = capture.x_count;
    *y_count = capture.y_count;
    for (int index = 0; index < capture.point_count; ++index) {
        xs[index] = capture.xs[index];
        ys[index] = capture.ys[index];
        zs[index] = capture.zs[index];
    }
    return capture.point_count;
}
