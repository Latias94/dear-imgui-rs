#include "cimgui_test_engine.h"

#include "imgui_te_engine.h"
#include "imgui_te_ui.h"

extern "C" {

ImGuiTestEngine* imgui_test_engine_create_context(void) {
    return ImGuiTestEngine_CreateContext();
}

void imgui_test_engine_destroy_context(ImGuiTestEngine* engine) {
    ImGuiTestEngine_DestroyContext(engine);
}

void imgui_test_engine_start(ImGuiTestEngine* engine, ImGuiContext* ui_ctx) {
    if (engine == nullptr || ui_ctx == nullptr) {
        return;
    }
    ImGuiTestEngine_Start(engine, ui_ctx);
}

void imgui_test_engine_stop(ImGuiTestEngine* engine) {
    if (engine == nullptr) {
        return;
    }
    ImGuiTestEngine_Stop(engine);
}

void imgui_test_engine_post_swap(ImGuiTestEngine* engine) {
    if (engine == nullptr) {
        return;
    }
    ImGuiTestEngine_PostSwap(engine);
}

void imgui_test_engine_show_windows(ImGuiTestEngine* engine, bool* p_open) {
    if (engine == nullptr) {
        return;
    }
    ImGuiTestEngine_ShowTestEngineWindows(engine, p_open);
}

void imgui_test_engine_queue_tests(
    ImGuiTestEngine* engine,
    ImGuiTestEngineGroup group,
    const char* filter,
    int run_flags
) {
    if (engine == nullptr) {
        return;
    }
    ImGuiTestEngine_QueueTests(
        engine,
        static_cast<ImGuiTestGroup>(group),
        filter,
        static_cast<ImGuiTestRunFlags>(run_flags)
    );
}

bool imgui_test_engine_is_test_queue_empty(ImGuiTestEngine* engine) {
    if (engine == nullptr) {
        return true;
    }
    return ImGuiTestEngine_IsTestQueueEmpty(engine);
}

bool imgui_test_engine_try_abort_engine(ImGuiTestEngine* engine) {
    if (engine == nullptr) {
        return true;
    }
    return ImGuiTestEngine_TryAbortEngine(engine);
}

void imgui_test_engine_abort_current_test(ImGuiTestEngine* engine) {
    if (engine == nullptr) {
        return;
    }
    ImGuiTestEngine_AbortCurrentTest(engine);
}

void imgui_test_engine_get_result_summary(
    ImGuiTestEngine* engine,
    ImGuiTestEngineResultSummary_c* out_summary
) {
    if (engine == nullptr || out_summary == nullptr) {
        return;
    }

    ImGuiTestEngineResultSummary summary{};
    ImGuiTestEngine_GetResultSummary(engine, &summary);

    out_summary->CountTested = summary.CountTested;
    out_summary->CountSuccess = summary.CountSuccess;
    out_summary->CountInQueue = summary.CountInQueue;
}

void imgui_test_engine_set_run_speed(ImGuiTestEngine* engine, ImGuiTestEngineRunSpeed speed) {
    if (engine == nullptr) {
        return;
    }
    ImGuiTestEngine_GetIO(engine).ConfigRunSpeed = static_cast<ImGuiTestRunSpeed>(speed);
}

void imgui_test_engine_set_verbose_level(
    ImGuiTestEngine* engine,
    ImGuiTestEngineVerboseLevel level
) {
    if (engine == nullptr) {
        return;
    }
    ImGuiTestEngine_GetIO(engine).ConfigVerboseLevel = static_cast<ImGuiTestVerboseLevel>(level);
}

void imgui_test_engine_set_capture_enabled(ImGuiTestEngine* engine, bool enabled) {
    if (engine == nullptr) {
        return;
    }
    ImGuiTestEngine_GetIO(engine).ConfigCaptureEnabled = enabled;
}

bool imgui_test_engine_is_running_tests(ImGuiTestEngine* engine) {
    if (engine == nullptr) {
        return false;
    }
    return ImGuiTestEngine_GetIO(engine).IsRunningTests;
}

bool imgui_test_engine_is_requesting_max_app_speed(ImGuiTestEngine* engine) {
    if (engine == nullptr) {
        return false;
    }
    return ImGuiTestEngine_GetIO(engine).IsRequestingMaxAppSpeed;
}

void imgui_test_engine_install_default_crash_handler(void) {
    ImGuiTestEngine_InstallDefaultCrashHandler();
}

} // extern "C"
