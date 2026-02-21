#include "cimgui_test_engine.h"

#include "imgui_te_engine.h"
#include "imgui_te_internal.h"
#include "imgui_te_ui.h"

// Implemented in script_tests.cpp (internal cleanup hook).
void imgui_test_engine__script_cleanup(ImGuiTestEngine* engine);

extern "C" {

void dear_imgui_test_engine_sys_register_imgui_hooks(void);

static ImGuiContext* imgui_test_engine__set_current_if_needed(ImGuiContext* ctx) {
    ImGuiContext* prev = ImGui::GetCurrentContext();
    if (ctx != nullptr && prev != ctx) {
        ImGui::SetCurrentContext(ctx);
    }
    return prev;
}

static void imgui_test_engine__restore_current_if_needed(ImGuiContext* prev, ImGuiContext* ctx) {
    if (ctx != nullptr && prev != ctx) {
        ImGui::SetCurrentContext(prev);
    }
}

ImGuiTestEngine* imgui_test_engine_create_context(void) {
    return ImGuiTestEngine_CreateContext();
}

void imgui_test_engine_destroy_context(ImGuiTestEngine* engine) {
    if (engine == nullptr) {
        return;
    }

    imgui_test_engine__script_cleanup(engine);

    ImGuiContext* target = engine->UiContextTarget;
    ImGuiContext* prev = imgui_test_engine__set_current_if_needed(target);

    // Ensure the engine isn't still bound: upstream requires `UiContextTarget == nullptr` when destroying.
    // This also makes Rust drop order less error-prone.
    if (engine->UiContextTarget != nullptr) {
        if (engine->Started) {
            ImGuiTestEngine_Stop(engine);
        }
        ImGuiTestEngine_UnbindImGuiContext(engine, target);
    }

    // Upstream asserts if the engine is still bound to an ImGuiContext while saved settings are enabled.
    // For safety/ergonomics in Rust (where drop order may be hard to control, e.g. when using helper
    // runners), we degrade gracefully by disabling saved settings in that case to avoid aborting.
    if (engine->IO.ConfigSavedSettings && engine->UiContextTarget != nullptr) {
        engine->IO.ConfigSavedSettings = false;
    }

    ImGuiTestEngine_DestroyContext(engine);

    imgui_test_engine__restore_current_if_needed(prev, target);
}

ImGuiContext* imgui_test_engine_get_ui_context_target(ImGuiTestEngine* engine) {
    if (engine == nullptr) {
        return nullptr;
    }
    return engine->UiContextTarget;
}

bool imgui_test_engine_is_bound(ImGuiTestEngine* engine) {
    return imgui_test_engine_get_ui_context_target(engine) != nullptr;
}

bool imgui_test_engine_is_started(ImGuiTestEngine* engine) {
    if (engine == nullptr) {
        return false;
    }
    return engine->Started;
}

void imgui_test_engine_unbind(ImGuiTestEngine* engine) {
    if (engine == nullptr) {
        return;
    }

    if (engine->Started) {
        ImGuiTestEngine_Stop(engine);
    }

    ImGuiContext* target = engine->UiContextTarget;
    if (target == nullptr) {
        return;
    }

    ImGuiContext* prev = imgui_test_engine__set_current_if_needed(target);
    ImGuiTestEngine_UnbindImGuiContext(engine, target);
    imgui_test_engine__restore_current_if_needed(prev, target);
}

void imgui_test_engine_start(ImGuiTestEngine* engine, ImGuiContext* ui_ctx) {
    if (engine == nullptr || ui_ctx == nullptr) {
        return;
    }
    dear_imgui_test_engine_sys_register_imgui_hooks();
    ImGuiContext* prev = imgui_test_engine__set_current_if_needed(ui_ctx);
    ImGuiTestEngine_Start(engine, ui_ctx);
    imgui_test_engine__restore_current_if_needed(prev, ui_ctx);
}

void imgui_test_engine_stop(ImGuiTestEngine* engine) {
    if (engine == nullptr) {
        return;
    }
    if (!engine->Started) {
        return;
    }
    ImGuiContext* target = engine->UiContextTarget;
    ImGuiContext* prev = imgui_test_engine__set_current_if_needed(target);
    ImGuiTestEngine_Stop(engine);
    imgui_test_engine__restore_current_if_needed(prev, target);
}

void imgui_test_engine_post_swap(ImGuiTestEngine* engine) {
    if (engine == nullptr) {
        return;
    }
    if (engine->UiContextTarget == nullptr) {
        return;
    }
    ImGuiContext* target = engine->UiContextTarget;
    ImGuiContext* prev = imgui_test_engine__set_current_if_needed(target);
    ImGuiTestEngine_PostSwap(engine);
    imgui_test_engine__restore_current_if_needed(prev, target);
}

void imgui_test_engine_show_windows(ImGuiTestEngine* engine, bool* p_open) {
    if (engine == nullptr) {
        return;
    }
    if (engine->UiContextTarget == nullptr) {
        return;
    }
    ImGuiContext* target = engine->UiContextTarget;
    ImGuiContext* prev = imgui_test_engine__set_current_if_needed(target);
    ImGuiTestEngine_ShowTestEngineWindows(engine, p_open);
    imgui_test_engine__restore_current_if_needed(prev, target);
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
