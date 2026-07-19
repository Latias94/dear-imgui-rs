#include "cimgui_test_engine.h"
#include "cimgui_test_engine_internal.h"

#include "imgui_te_engine.h"
#include "imgui_te_internal.h"
#include "imgui_te_ui.h"

#include <algorithm>
#include <atomic>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <stdexcept>
#include <unordered_map>

namespace dear_imgui_test_engine_abi {
namespace {

struct Diagnostic {
    char Message[kDiagnosticCapacity]{};
};

thread_local Diagnostic g_diagnostic;
thread_local ImGuiTestEngineExceptionPoint g_exception_point = ImGuiTestEngineExceptionPoint_None;

class SpinGuard {
public:
    explicit SpinGuard(std::atomic_flag& flag) noexcept : flag_(flag) {
        while (flag_.test_and_set(std::memory_order_acquire)) {
        }
    }

    ~SpinGuard() noexcept { flag_.clear(std::memory_order_release); }

private:
    std::atomic_flag& flag_;
};

enum class EngineState { Live, Destroying };

std::atomic_flag g_engine_lock = ATOMIC_FLAG_INIT;
std::unordered_map<ImGuiTestEngine*, EngineState> g_engines;

struct AtomicCounters {
    std::atomic<std::uint64_t> EnginesCreated{0};
    std::atomic<std::uint64_t> EnginesDestroyed{0};
    std::atomic<std::uint64_t> EnginesStarted{0};
    std::atomic<std::uint64_t> EnginesStopped{0};
    std::atomic<std::uint64_t> EnginesUnbound{0};
    std::atomic<std::uint64_t> ScriptsCreated{0};
    std::atomic<std::uint64_t> ScriptsDestroyed{0};
    std::atomic<std::uint64_t> ScriptsRegistered{0};
};

AtomicCounters g_counters;

std::atomic<std::uint64_t>& counter_ref(Counter counter) noexcept {
    switch (counter) {
        case Counter::EngineCreated:
            return g_counters.EnginesCreated;
        case Counter::EngineDestroyed:
            return g_counters.EnginesDestroyed;
        case Counter::EngineStarted:
            return g_counters.EnginesStarted;
        case Counter::EngineStopped:
            return g_counters.EnginesStopped;
        case Counter::EngineUnbound:
            return g_counters.EnginesUnbound;
        case Counter::ScriptCreated:
            return g_counters.ScriptsCreated;
        case Counter::ScriptDestroyed:
            return g_counters.ScriptsDestroyed;
        case Counter::ScriptRegistered:
            return g_counters.ScriptsRegistered;
    }
    return g_counters.EnginesCreated;
}

} // namespace

void clear_error() noexcept { g_diagnostic.Message[0] = '\0'; }

ImGuiTestEngineStatus fail(ImGuiTestEngineStatus status, const char* message) noexcept {
    const char* source = message != nullptr ? message : "unspecified error";
    std::snprintf(g_diagnostic.Message, kDiagnosticCapacity, "%s", source);
    g_diagnostic.Message[kDiagnosticCapacity - 1] = '\0';
    return status;
}

ImGuiTestEngineStatus fail_exception(const char* operation, const char* message) noexcept {
    std::snprintf(
        g_diagnostic.Message,
        kDiagnosticCapacity,
        "%s: %s",
        operation != nullptr ? operation : "test-engine operation",
        message != nullptr ? message : "C++ exception"
    );
    g_diagnostic.Message[kDiagnosticCapacity - 1] = '\0';
    return ImGuiTestEngineStatus_Exception;
}

void maybe_inject(ImGuiTestEngineExceptionPoint point) {
    if (g_exception_point != point) {
        return;
    }
    g_exception_point = ImGuiTestEngineExceptionPoint_None;
    if (point == ImGuiTestEngineExceptionPoint_EngineAllocation ||
        point == ImGuiTestEngineExceptionPoint_ScriptAllocation ||
        point == ImGuiTestEngineExceptionPoint_ScriptVectorGrowth) {
        throw std::bad_alloc();
    }
    throw std::runtime_error("injected upstream exception");
}

ImGuiTestEngineStatus require_engine(ImGuiTestEngine* engine) noexcept {
    if (engine == nullptr) {
        return fail(ImGuiTestEngineStatus_InvalidArgument, "engine must not be null");
    }
    SpinGuard guard(g_engine_lock);
    const auto found = g_engines.find(engine);
    if (found == g_engines.end()) {
        return fail(ImGuiTestEngineStatus_InvalidState, "engine is not managed by this ABI");
    }
    if (found->second != EngineState::Live) {
        return fail(ImGuiTestEngineStatus_InvalidState, "engine is not live");
    }
    return ImGuiTestEngineStatus_Success;
}

void register_engine(ImGuiTestEngine* engine) {
    SpinGuard guard(g_engine_lock);
    g_engines.insert_or_assign(engine, EngineState::Live);
}

void begin_destroy_engine(ImGuiTestEngine* engine) noexcept {
    SpinGuard guard(g_engine_lock);
    const auto found = g_engines.find(engine);
    if (found != g_engines.end()) {
        found->second = EngineState::Destroying;
    }
}

void cancel_destroy_engine(ImGuiTestEngine* engine) noexcept {
    SpinGuard guard(g_engine_lock);
    const auto found = g_engines.find(engine);
    if (found != g_engines.end() && found->second == EngineState::Destroying) {
        found->second = EngineState::Live;
    }
}

void finish_destroy_engine(ImGuiTestEngine* engine) noexcept {
    SpinGuard guard(g_engine_lock);
    g_engines.erase(engine);
}

bool has_live_engines() noexcept {
    SpinGuard guard(g_engine_lock);
    return !g_engines.empty();
}

void increment(Counter counter) noexcept {
    counter_ref(counter).fetch_add(1, std::memory_order_relaxed);
}

ImGuiTestEngineLifecycleCounters_c counters() noexcept {
    return {
        g_counters.EnginesCreated.load(std::memory_order_relaxed),
        g_counters.EnginesDestroyed.load(std::memory_order_relaxed),
        g_counters.EnginesStarted.load(std::memory_order_relaxed),
        g_counters.EnginesStopped.load(std::memory_order_relaxed),
        g_counters.EnginesUnbound.load(std::memory_order_relaxed),
        g_counters.ScriptsCreated.load(std::memory_order_relaxed),
        g_counters.ScriptsDestroyed.load(std::memory_order_relaxed),
        g_counters.ScriptsRegistered.load(std::memory_order_relaxed),
    };
}

void reset_counters() noexcept {
    g_counters.EnginesCreated.store(0, std::memory_order_relaxed);
    g_counters.EnginesDestroyed.store(0, std::memory_order_relaxed);
    g_counters.EnginesStarted.store(0, std::memory_order_relaxed);
    g_counters.EnginesStopped.store(0, std::memory_order_relaxed);
    g_counters.EnginesUnbound.store(0, std::memory_order_relaxed);
    g_counters.ScriptsCreated.store(0, std::memory_order_relaxed);
    g_counters.ScriptsDestroyed.store(0, std::memory_order_relaxed);
    g_counters.ScriptsRegistered.store(0, std::memory_order_relaxed);
}

ScopedCurrentContext::ScopedCurrentContext(ImGuiContext* context) noexcept
    : previous_(ImGui::GetCurrentContext()), target_(context) {
    if (target_ != nullptr && previous_ != target_) {
        ImGui::SetCurrentContext(target_);
    }
}

ScopedCurrentContext::~ScopedCurrentContext() noexcept {
    if (target_ != nullptr && previous_ != target_) {
        ImGui::SetCurrentContext(previous_);
    }
}

} // namespace dear_imgui_test_engine_abi

namespace abi = dear_imgui_test_engine_abi;

extern "C" {

ImGuiTestEngineStatus imgui_test_engine_get_last_error(
    char* buffer,
    size_t buffer_size,
    size_t* out_required_size
) noexcept {
    if (out_required_size == nullptr) {
        return ImGuiTestEngineStatus_InvalidArgument;
    }

    const char* message = abi::g_diagnostic.Message;
    const size_t required_size = std::strlen(message) + 1;
    *out_required_size = required_size;

    if (buffer == nullptr) {
        return buffer_size == 0 ? ImGuiTestEngineStatus_Success
                                : ImGuiTestEngineStatus_InvalidArgument;
    }
    if (buffer_size == 0) {
        return ImGuiTestEngineStatus_OutOfRange;
    }

    const size_t copy_size = std::min(required_size - 1, buffer_size - 1);
    if (copy_size != 0) {
        std::memcpy(buffer, message, copy_size);
    }
    buffer[copy_size] = '\0';
    return buffer_size < required_size ? ImGuiTestEngineStatus_OutOfRange
                                       : ImGuiTestEngineStatus_Success;
}

ImGuiTestEngineStatus imgui_test_engine_create_context(ImGuiTestEngine** out_engine) {
    return abi::boundary("imgui_test_engine_create_context", [&]() {
        if (out_engine == nullptr) {
            return abi::fail(ImGuiTestEngineStatus_InvalidArgument, "out_engine must not be null");
        }
        *out_engine = nullptr;
        abi::maybe_inject(ImGuiTestEngineExceptionPoint_EngineAllocation);
        ImGuiTestEngine* engine = ImGuiTestEngine_CreateContext();
        if (engine == nullptr) {
            return abi::fail(ImGuiTestEngineStatus_Exception, "upstream context allocation failed");
        }
        try {
            abi::register_engine(engine);
        } catch (...) {
            ImGuiTestEngine_DestroyContext(engine);
            throw;
        }
        abi::increment(abi::Counter::EngineCreated);
        *out_engine = engine;
        return ImGuiTestEngineStatus_Success;
    });
}

ImGuiTestEngineStatus imgui_test_engine_destroy_context(ImGuiTestEngine* engine) {
    return abi::boundary("imgui_test_engine_destroy_context", [&]() {
        if (engine == nullptr) {
            return abi::fail(ImGuiTestEngineStatus_InvalidArgument, "engine must not be null");
        }
        ImGuiTestEngineStatus status = abi::require_engine(engine);
        if (status != ImGuiTestEngineStatus_Success) {
            return status;
        }
        if (engine->Started || engine->UiContextTarget != nullptr) {
            return abi::fail(
                ImGuiTestEngineStatus_InvalidState,
                "engine must be stopped and unbound before destruction"
            );
        }

        abi::begin_destroy_engine(engine);
        try {
            abi::maybe_inject(ImGuiTestEngineExceptionPoint_UpstreamCall);
            ImGuiTestEngine_DestroyContext(engine);
            abi::cleanup_scripts(engine);
        } catch (...) {
            abi::cancel_destroy_engine(engine);
            throw;
        }
        abi::finish_destroy_engine(engine);
        abi::increment(abi::Counter::EngineDestroyed);
        return ImGuiTestEngineStatus_Success;
    });
}

ImGuiTestEngineStatus imgui_test_engine_get_ui_context_target(
    ImGuiTestEngine* engine,
    ImGuiContext** out_ui_context
) {
    return abi::boundary("imgui_test_engine_get_ui_context_target", [&]() {
        if (out_ui_context == nullptr) {
            return abi::fail(ImGuiTestEngineStatus_InvalidArgument, "out_ui_context must not be null");
        }
        *out_ui_context = nullptr;
        const ImGuiTestEngineStatus status = abi::require_engine(engine);
        if (status != ImGuiTestEngineStatus_Success) {
            return status;
        }
        *out_ui_context = engine->UiContextTarget;
        return *out_ui_context != nullptr
                   ? ImGuiTestEngineStatus_Success
                   : abi::fail(ImGuiTestEngineStatus_NotFound, "engine has no bound UI context");
    });
}

ImGuiTestEngineStatus imgui_test_engine_is_bound(ImGuiTestEngine* engine, bool* out_bound) {
    return abi::boundary("imgui_test_engine_is_bound", [&]() {
        if (out_bound == nullptr) {
            return abi::fail(ImGuiTestEngineStatus_InvalidArgument, "out_bound must not be null");
        }
        *out_bound = false;
        const ImGuiTestEngineStatus status = abi::require_engine(engine);
        if (status != ImGuiTestEngineStatus_Success) {
            return status;
        }
        *out_bound = engine->UiContextTarget != nullptr;
        return ImGuiTestEngineStatus_Success;
    });
}

ImGuiTestEngineStatus imgui_test_engine_is_started(ImGuiTestEngine* engine, bool* out_started) {
    return abi::boundary("imgui_test_engine_is_started", [&]() {
        if (out_started == nullptr) {
            return abi::fail(ImGuiTestEngineStatus_InvalidArgument, "out_started must not be null");
        }
        *out_started = false;
        const ImGuiTestEngineStatus status = abi::require_engine(engine);
        if (status != ImGuiTestEngineStatus_Success) {
            return status;
        }
        *out_started = engine->Started;
        return ImGuiTestEngineStatus_Success;
    });
}

ImGuiTestEngineStatus imgui_test_engine_unbind(ImGuiTestEngine* engine) {
    return abi::boundary("imgui_test_engine_unbind", [&]() {
        const ImGuiTestEngineStatus status = abi::require_engine(engine);
        if (status != ImGuiTestEngineStatus_Success) {
            return status;
        }
        if (engine->Started) {
            return abi::fail(ImGuiTestEngineStatus_InvalidState, "engine must be stopped before unbind");
        }
        ImGuiContext* target = engine->UiContextTarget;
        if (target == nullptr) {
            return abi::fail(ImGuiTestEngineStatus_InvalidState, "engine is not bound");
        }
        if (target->TestEngine != engine) {
            return abi::fail(
                ImGuiTestEngineStatus_InvalidState,
                "bound UI context does not reference this engine"
            );
        }
        abi::maybe_inject(ImGuiTestEngineExceptionPoint_UpstreamCall);
        abi::ScopedCurrentContext current(target);
        abi::unregister_context_shutdown_observer(engine, target);
        ImGuiTestEngine_UnbindImGuiContext(engine, target);
        abi::increment(abi::Counter::EngineUnbound);
        return ImGuiTestEngineStatus_Success;
    });
}

ImGuiTestEngineStatus imgui_test_engine_start(ImGuiTestEngine* engine, ImGuiContext* ui_ctx) {
    return abi::boundary("imgui_test_engine_start", [&]() {
        const ImGuiTestEngineStatus status = abi::require_engine(engine);
        if (status != ImGuiTestEngineStatus_Success) {
            return status;
        }
        if (ui_ctx == nullptr) {
            return abi::fail(ImGuiTestEngineStatus_InvalidArgument, "ui_ctx must not be null");
        }
        if (engine->Started || engine->UiContextTarget != nullptr) {
            return abi::fail(ImGuiTestEngineStatus_InvalidState, "engine is already started or bound");
        }
        if (ui_ctx->TestEngine != nullptr) {
            return abi::fail(
                ImGuiTestEngineStatus_InvalidState,
                "UI context is already bound to a test engine"
            );
        }
        if (engine->IO.CoroutineFuncs == nullptr) {
            return abi::fail(
                ImGuiTestEngineStatus_InvalidState,
                "engine has no coroutine implementation"
            );
        }
        abi::register_imgui_hooks();
        abi::maybe_inject(ImGuiTestEngineExceptionPoint_UpstreamCall);
        abi::ScopedCurrentContext current(ui_ctx);
        ImGuiTestEngine_Start(engine, ui_ctx);
        try {
            abi::register_context_shutdown_observer(engine, ui_ctx);
        } catch (...) {
            ImGuiTestEngine_Stop(engine);
            ImGuiTestEngine_UnbindImGuiContext(engine, ui_ctx);
            throw;
        }
        abi::increment(abi::Counter::EngineStarted);
        return ImGuiTestEngineStatus_Success;
    });
}

ImGuiTestEngineStatus imgui_test_engine_stop(ImGuiTestEngine* engine) {
    return abi::boundary("imgui_test_engine_stop", [&]() {
        const ImGuiTestEngineStatus status = abi::require_engine(engine);
        if (status != ImGuiTestEngineStatus_Success) {
            return status;
        }
        if (!engine->Started || engine->UiContextTarget == nullptr) {
            return abi::fail(ImGuiTestEngineStatus_InvalidState, "engine is not started and bound");
        }
        ImGuiContext* target = engine->UiContextTarget;
        abi::maybe_inject(ImGuiTestEngineExceptionPoint_UpstreamCall);
        abi::ScopedCurrentContext current(target);
        ImGuiTestEngine_Stop(engine);
        abi::increment(abi::Counter::EngineStopped);
        return ImGuiTestEngineStatus_Success;
    });
}

ImGuiTestEngineStatus imgui_test_engine_post_swap(ImGuiTestEngine* engine) {
    return abi::boundary("imgui_test_engine_post_swap", [&]() {
        const ImGuiTestEngineStatus status = abi::require_engine(engine);
        if (status != ImGuiTestEngineStatus_Success) {
            return status;
        }
        if (!engine->Started || engine->UiContextTarget == nullptr) {
            return abi::fail(ImGuiTestEngineStatus_InvalidState, "engine is not started and bound");
        }
        abi::maybe_inject(ImGuiTestEngineExceptionPoint_UpstreamCall);
        abi::ScopedCurrentContext current(engine->UiContextTarget);
        ImGuiTestEngine_PostSwap(engine);
        return ImGuiTestEngineStatus_Success;
    });
}

ImGuiTestEngineStatus imgui_test_engine_show_windows(ImGuiTestEngine* engine, bool* p_open) {
    return abi::boundary("imgui_test_engine_show_windows", [&]() {
        const ImGuiTestEngineStatus status = abi::require_engine(engine);
        if (status != ImGuiTestEngineStatus_Success) {
            return status;
        }
        if (engine->UiContextTarget == nullptr) {
            return abi::fail(ImGuiTestEngineStatus_InvalidState, "engine is not bound");
        }
        if (!engine->UiContextTarget->WithinFrameScope) {
            return abi::fail(
                ImGuiTestEngineStatus_InvalidState,
                "test engine windows require an active ImGui frame"
            );
        }
        abi::maybe_inject(ImGuiTestEngineExceptionPoint_UpstreamCall);
        abi::ScopedCurrentContext current(engine->UiContextTarget);
        ImGuiTestEngine_ShowTestEngineWindows(engine, p_open);
        return ImGuiTestEngineStatus_Success;
    });
}

ImGuiTestEngineStatus imgui_test_engine_queue_tests(
    ImGuiTestEngine* engine,
    int group,
    const char* filter,
    int run_flags
) {
    return abi::boundary("imgui_test_engine_queue_tests", [&]() {
        const ImGuiTestEngineStatus status = abi::require_engine(engine);
        if (status != ImGuiTestEngineStatus_Success) {
            return status;
        }
        if (filter == nullptr) {
            return abi::fail(ImGuiTestEngineStatus_InvalidArgument, "filter must not be null");
        }
        if (group < ImGuiTestEngineGroup_Unknown || group > ImGuiTestEngineGroup_Perfs) {
            return abi::fail(ImGuiTestEngineStatus_OutOfRange, "group is out of range");
        }
        constexpr int known_flags = ImGuiTestEngineRunFlags_GuiFuncDisable |
                                    ImGuiTestEngineRunFlags_GuiFuncOnly |
                                    ImGuiTestEngineRunFlags_NoSuccessMsg |
                                    ImGuiTestEngineRunFlags_EnableRawInputs |
                                    ImGuiTestEngineRunFlags_RunFromGui |
                                    ImGuiTestEngineRunFlags_RunFromCommandLine |
                                    ImGuiTestEngineRunFlags_NoError |
                                    ImGuiTestEngineRunFlags_ShareVars |
                                    ImGuiTestEngineRunFlags_ShareTestContext;
        if (run_flags < 0 || (run_flags & ~known_flags) != 0) {
            return abi::fail(ImGuiTestEngineStatus_OutOfRange, "run_flags contains unknown bits");
        }
        if (engine->UiContextTarget != nullptr &&
            engine->FrameCount < engine->UiContextTarget->FrameCount - 2) {
            return abi::fail(
                ImGuiTestEngineStatus_InvalidState,
                "engine frame hooks are not receiving the bound UI context"
            );
        }
        abi::maybe_inject(ImGuiTestEngineExceptionPoint_UpstreamCall);
        ImGuiTestEngine_QueueTests(
            engine,
            static_cast<ImGuiTestGroup>(group),
            filter[0] != '\0' ? filter : nullptr,
            static_cast<ImGuiTestRunFlags>(run_flags)
        );
        return ImGuiTestEngineStatus_Success;
    });
}

ImGuiTestEngineStatus imgui_test_engine_is_test_queue_empty(
    ImGuiTestEngine* engine,
    bool* out_empty
) {
    return abi::boundary("imgui_test_engine_is_test_queue_empty", [&]() {
        if (out_empty == nullptr) {
            return abi::fail(ImGuiTestEngineStatus_InvalidArgument, "out_empty must not be null");
        }
        *out_empty = true;
        const ImGuiTestEngineStatus status = abi::require_engine(engine);
        if (status != ImGuiTestEngineStatus_Success) {
            return status;
        }
        *out_empty = ImGuiTestEngine_IsTestQueueEmpty(engine);
        return ImGuiTestEngineStatus_Success;
    });
}

ImGuiTestEngineStatus imgui_test_engine_try_abort_engine(
    ImGuiTestEngine* engine,
    bool* out_aborted
) {
    return abi::boundary("imgui_test_engine_try_abort_engine", [&]() {
        if (out_aborted == nullptr) {
            return abi::fail(ImGuiTestEngineStatus_InvalidArgument, "out_aborted must not be null");
        }
        *out_aborted = false;
        const ImGuiTestEngineStatus status = abi::require_engine(engine);
        if (status != ImGuiTestEngineStatus_Success) {
            return status;
        }
        abi::maybe_inject(ImGuiTestEngineExceptionPoint_UpstreamCall);
        *out_aborted = ImGuiTestEngine_TryAbortEngine(engine);
        return ImGuiTestEngineStatus_Success;
    });
}

ImGuiTestEngineStatus imgui_test_engine_abort_current_test(ImGuiTestEngine* engine) {
    return abi::boundary("imgui_test_engine_abort_current_test", [&]() {
        const ImGuiTestEngineStatus status = abi::require_engine(engine);
        if (status != ImGuiTestEngineStatus_Success) {
            return status;
        }
        abi::maybe_inject(ImGuiTestEngineExceptionPoint_UpstreamCall);
        ImGuiTestEngine_AbortCurrentTest(engine);
        return ImGuiTestEngineStatus_Success;
    });
}

ImGuiTestEngineStatus imgui_test_engine_get_result_summary(
    ImGuiTestEngine* engine,
    ImGuiTestEngineResultSummary_c* out_summary
) {
    return abi::boundary("imgui_test_engine_get_result_summary", [&]() {
        if (out_summary == nullptr) {
            return abi::fail(ImGuiTestEngineStatus_InvalidArgument, "out_summary must not be null");
        }
        *out_summary = {};
        const ImGuiTestEngineStatus status = abi::require_engine(engine);
        if (status != ImGuiTestEngineStatus_Success) {
            return status;
        }
        for (int n = 0; n < engine->TestsAll.Size; n++) {
            ImGuiTest* test = engine->TestsAll[n];
            if (test == nullptr) {
                continue;
            }
            const ImGuiTestStatus test_status = test->Output.Status;
            if (test_status == ImGuiTestStatus_Unknown) {
                continue;
            }
            if (test_status == ImGuiTestStatus_Queued || test_status == ImGuiTestStatus_Running) {
                out_summary->CountInQueue++;
                continue;
            }
            out_summary->CountTested++;
            if (test_status == ImGuiTestStatus_Success) {
                out_summary->CountSuccess++;
            }
        }
        return ImGuiTestEngineStatus_Success;
    });
}

ImGuiTestEngineStatus imgui_test_engine_set_run_speed(
    ImGuiTestEngine* engine,
    int speed
) {
    return abi::boundary("imgui_test_engine_set_run_speed", [&]() {
        const ImGuiTestEngineStatus status = abi::require_engine(engine);
        if (status != ImGuiTestEngineStatus_Success) {
            return status;
        }
        if (speed < ImGuiTestEngineRunSpeed_Fast || speed > ImGuiTestEngineRunSpeed_Cinematic) {
            return abi::fail(ImGuiTestEngineStatus_OutOfRange, "run speed is out of range");
        }
        ImGuiTestEngine_GetIO(engine).ConfigRunSpeed = static_cast<ImGuiTestRunSpeed>(speed);
        return ImGuiTestEngineStatus_Success;
    });
}

ImGuiTestEngineStatus imgui_test_engine_set_verbose_level(
    ImGuiTestEngine* engine,
    int level
) {
    return abi::boundary("imgui_test_engine_set_verbose_level", [&]() {
        const ImGuiTestEngineStatus status = abi::require_engine(engine);
        if (status != ImGuiTestEngineStatus_Success) {
            return status;
        }
        if (level < ImGuiTestEngineVerboseLevel_Silent || level > ImGuiTestEngineVerboseLevel_Trace) {
            return abi::fail(ImGuiTestEngineStatus_OutOfRange, "verbose level is out of range");
        }
        ImGuiTestEngine_GetIO(engine).ConfigVerboseLevel = static_cast<ImGuiTestVerboseLevel>(level);
        return ImGuiTestEngineStatus_Success;
    });
}

ImGuiTestEngineStatus imgui_test_engine_set_capture_enabled(ImGuiTestEngine* engine, bool enabled) {
    return abi::boundary("imgui_test_engine_set_capture_enabled", [&]() {
        const ImGuiTestEngineStatus status = abi::require_engine(engine);
        if (status != ImGuiTestEngineStatus_Success) {
            return status;
        }
        ImGuiTestEngine_GetIO(engine).ConfigCaptureEnabled = enabled;
        return ImGuiTestEngineStatus_Success;
    });
}

ImGuiTestEngineStatus imgui_test_engine_is_running_tests(
    ImGuiTestEngine* engine,
    bool* out_running
) {
    return abi::boundary("imgui_test_engine_is_running_tests", [&]() {
        if (out_running == nullptr) {
            return abi::fail(ImGuiTestEngineStatus_InvalidArgument, "out_running must not be null");
        }
        *out_running = false;
        const ImGuiTestEngineStatus status = abi::require_engine(engine);
        if (status != ImGuiTestEngineStatus_Success) {
            return status;
        }
        *out_running = ImGuiTestEngine_GetIO(engine).IsRunningTests;
        return ImGuiTestEngineStatus_Success;
    });
}

ImGuiTestEngineStatus imgui_test_engine_is_requesting_max_app_speed(
    ImGuiTestEngine* engine,
    bool* out_requesting
) {
    return abi::boundary("imgui_test_engine_is_requesting_max_app_speed", [&]() {
        if (out_requesting == nullptr) {
            return abi::fail(ImGuiTestEngineStatus_InvalidArgument, "out_requesting must not be null");
        }
        *out_requesting = false;
        const ImGuiTestEngineStatus status = abi::require_engine(engine);
        if (status != ImGuiTestEngineStatus_Success) {
            return status;
        }
        *out_requesting = ImGuiTestEngine_GetIO(engine).IsRequestingMaxAppSpeed;
        return ImGuiTestEngineStatus_Success;
    });
}

ImGuiTestEngineStatus imgui_test_engine_install_default_crash_handler(void) {
    return abi::boundary("imgui_test_engine_install_default_crash_handler", []() {
        abi::maybe_inject(ImGuiTestEngineExceptionPoint_UpstreamCall);
        ImGuiTestEngine_InstallDefaultCrashHandler();
        return ImGuiTestEngineStatus_Success;
    });
}

ImGuiTestEngineStatus imgui_test_engine_test_set_exception_injection(
    int point
) {
    return abi::boundary("imgui_test_engine_test_set_exception_injection", [&]() {
        if (point < ImGuiTestEngineExceptionPoint_None ||
            point > ImGuiTestEngineExceptionPoint_UpstreamCall) {
            return abi::fail(ImGuiTestEngineStatus_OutOfRange, "exception point is out of range");
        }
        abi::g_exception_point = static_cast<ImGuiTestEngineExceptionPoint>(point);
        return ImGuiTestEngineStatus_Success;
    });
}

ImGuiTestEngineStatus imgui_test_engine_test_get_lifecycle_counters(
    ImGuiTestEngineLifecycleCounters_c* out_counters
) {
    return abi::boundary("imgui_test_engine_test_get_lifecycle_counters", [&]() {
        if (out_counters == nullptr) {
            return abi::fail(ImGuiTestEngineStatus_InvalidArgument, "out_counters must not be null");
        }
        *out_counters = abi::counters();
        return ImGuiTestEngineStatus_Success;
    });
}

ImGuiTestEngineStatus imgui_test_engine_test_reset_lifecycle_counters(void) {
    return abi::boundary("imgui_test_engine_test_reset_lifecycle_counters", []() {
        if (abi::has_live_engines() || abi::has_live_scripts()) {
            return abi::fail(
                ImGuiTestEngineStatus_InvalidState,
                "lifecycle counters cannot be reset while resources are live"
            );
        }
        abi::reset_counters();
        return ImGuiTestEngineStatus_Success;
    });
}

} // extern "C"
