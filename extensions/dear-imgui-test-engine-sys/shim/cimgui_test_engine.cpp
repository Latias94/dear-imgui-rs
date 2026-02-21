#include "cimgui_test_engine.h"

#include "imgui_te_engine.h"
#include "imgui_te_internal.h"
#include "imgui_te_ui.h"

// Implemented in script_tests.cpp (internal cleanup hook).
void imgui_test_engine__script_cleanup(ImGuiTestEngine* engine);

extern "C" {

ImGuiTestEngine* imgui_test_engine_create_context(void) {
    return ImGuiTestEngine_CreateContext();
}

void imgui_test_engine_destroy_context(ImGuiTestEngine* engine) {
    if (engine == nullptr) {
        return;
    }

    imgui_test_engine__script_cleanup(engine);

    // Upstream asserts if the engine is still bound to an ImGuiContext while saved settings are enabled.
    // For safety/ergonomics in Rust (where drop order may be hard to control, e.g. when using helper
    // runners), we degrade gracefully by disabling saved settings in that case to avoid aborting.
    if (engine->IO.ConfigSavedSettings && engine->UiContextTarget != nullptr) {
        engine->IO.ConfigSavedSettings = false;
    }

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

    // Upstream `ImGuiTestEngine_GetResultSummary()` asserts if any test is currently running.
    // For bindings, we prefer returning a best-effort snapshot instead of aborting.
    int count_tested = 0;
    int count_success = 0;
    int count_remaining = 0;
    for (int n = 0; n < engine->TestsAll.Size; n++) {
        ImGuiTest* test = engine->TestsAll[n];
        ImGuiTestStatus status = test->Output.Status;
        if (status == ImGuiTestStatus_Unknown) {
            continue;
        }
        if (status == ImGuiTestStatus_Queued || status == ImGuiTestStatus_Running) {
            count_remaining++;
            continue;
        }
        count_tested++;
        if (status == ImGuiTestStatus_Success) {
            count_success++;
        }
    }

    out_summary->CountTested = count_tested;
    out_summary->CountSuccess = count_success;
    out_summary->CountInQueue = count_remaining;
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
