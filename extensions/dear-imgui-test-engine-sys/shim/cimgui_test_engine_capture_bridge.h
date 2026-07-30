#pragma once

#include "imgui_te_engine.h"

bool dear_imgui_rs_test_engine_capture_should_abort(ImGuiTestEngine* engine) noexcept;
void dear_imgui_rs_test_engine_request_capture_abort(ImGuiTestEngine* engine) noexcept;
void dear_imgui_rs_test_engine_clear_capture_abort(ImGuiTestEngine* engine) noexcept;
bool dear_imgui_rs_test_engine_take_capture_provider_failure(
    ImGuiTestEngine* engine
) noexcept;
void dear_imgui_rs_test_engine_begin_capture_wait(ImGuiTestEngine* engine) noexcept;
void dear_imgui_rs_test_engine_end_capture_wait(ImGuiTestEngine* engine) noexcept;

class DearImGuiRsCaptureWaitGuard {
public:
    explicit DearImGuiRsCaptureWaitGuard(ImGuiTestEngine* engine) noexcept : engine_(engine) {
        dear_imgui_rs_test_engine_begin_capture_wait(engine_);
    }

    ~DearImGuiRsCaptureWaitGuard() noexcept {
        dear_imgui_rs_test_engine_end_capture_wait(engine_);
    }

    DearImGuiRsCaptureWaitGuard(const DearImGuiRsCaptureWaitGuard&) = delete;
    DearImGuiRsCaptureWaitGuard& operator=(const DearImGuiRsCaptureWaitGuard&) = delete;

private:
    ImGuiTestEngine* engine_;
};
void dear_imgui_rs_test_engine_begin_screenshot_config(
    ImGuiTestEngine* engine,
    ImGuiTestRunSpeed run_speed
) noexcept;
void dear_imgui_rs_test_engine_restore_screenshot_config(ImGuiTestEngine* engine) noexcept;
void dear_imgui_rs_test_engine_begin_window_move_config(
    ImGuiTestEngine* engine,
    bool move_from_title_bar_only
) noexcept;
void dear_imgui_rs_test_engine_restore_window_move_config(ImGuiTestEngine* engine) noexcept;
void dear_imgui_rs_test_engine_begin_video_config(ImGuiTestEngine* engine) noexcept;
void dear_imgui_rs_test_engine_restore_video_config(ImGuiTestEngine* engine) noexcept;
