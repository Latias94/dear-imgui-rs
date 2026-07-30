#include "cimgui_test_engine.h"
#include "cimgui_test_engine_capture_bridge.h"
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
#include <vector>

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

struct HiddenWindowState {
    ImGuiID WindowId = 0;
    ImS8 HiddenFramesForRenderOnly = 0;
};

struct EngineRecord {
    EngineState State = EngineState::Live;
    bool PresentationPending = false;
    ImGuiTestEnginePresentationTraceCallback_c PresentationTrace = nullptr;
    void* PresentationTraceUserData = nullptr;
    bool CaptureAbortRequested = false;
    bool CaptureWaitPending = false;
    ImGuiTestEngineCaptureCallback_c CaptureProvider = nullptr;
    void* CaptureProviderUserData = nullptr;
    bool CaptureProviderFailed = false;
    bool CaptureRollbackValid = false;
    bool HiddenWindowBackupValid = false;
    std::vector<HiddenWindowState> HiddenWindows;
    bool ScreenshotConfigActive = false;
    ImGuiTestRunSpeed ScreenshotRunSpeed = ImGuiTestRunSpeed_Normal;
    bool WindowMoveConfigActive = false;
    bool WindowMoveFromTitleBarOnly = false;
    bool VideoConfigActive = false;
    ImGuiTestRunSpeed VideoRunSpeed = ImGuiTestRunSpeed_Normal;
    bool VideoNoThrottle = false;
    float VideoFixedDeltaTime = 0.0f;
};

std::atomic_flag g_engine_lock = ATOMIC_FLAG_INIT;
std::unordered_map<ImGuiTestEngine*, EngineRecord> g_engines;

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

bool capture_active(ImGuiTestEngine* engine) noexcept {
    return engine->CaptureCurrentArgs != nullptr || engine->CaptureContext.IsCapturing() ||
           engine->CaptureTool._StateIsCapturing ||
           engine->CaptureTool._StateIsPickingWindow;
}

bool screen_capture_trampoline(
    ImGuiID viewport_id,
    int x,
    int y,
    int width,
    int height,
    unsigned int* pixels,
    void* engine_user_data
) noexcept {
    auto* engine = static_cast<ImGuiTestEngine*>(engine_user_data);
    ImGuiTestEngineCaptureCallback_c callback = nullptr;
    void* callback_user_data = nullptr;
    {
        SpinGuard guard(g_engine_lock);
        const auto found = g_engines.find(engine);
        if (found == g_engines.end()) {
            return false;
        }
        callback = found->second.CaptureProvider;
        callback_user_data = found->second.CaptureProviderUserData;
    }

    bool captured = false;
    if (callback != nullptr) {
        try {
            captured = callback(
                static_cast<std::uint32_t>(viewport_id),
                x,
                y,
                width,
                height,
                reinterpret_cast<std::uint32_t*>(pixels),
                callback_user_data
            );
        } catch (...) {
            captured = false;
        }
    }
    if (!captured) {
        SpinGuard guard(g_engine_lock);
        const auto found = g_engines.find(engine);
        if (found != g_engines.end()) {
            found->second.CaptureProviderFailed = true;
        }
    }
    return captured;
}

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
    if (found->second.State != EngineState::Live) {
        return fail(ImGuiTestEngineStatus_InvalidState, "engine is not live");
    }
    return ImGuiTestEngineStatus_Success;
}

void register_engine(ImGuiTestEngine* engine) {
    SpinGuard guard(g_engine_lock);
    g_engines.insert_or_assign(engine, EngineRecord{});
}

void begin_destroy_engine(ImGuiTestEngine* engine) noexcept {
    SpinGuard guard(g_engine_lock);
    const auto found = g_engines.find(engine);
    if (found != g_engines.end()) {
        found->second.State = EngineState::Destroying;
    }
}

void cancel_destroy_engine(ImGuiTestEngine* engine) noexcept {
    SpinGuard guard(g_engine_lock);
    const auto found = g_engines.find(engine);
    if (found != g_engines.end() && found->second.State == EngineState::Destroying) {
        found->second.State = EngineState::Live;
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
            abi::clear_capture_provider(engine);
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
        abi::clear_capture_provider(engine);
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
        abi::request_capture_abort(engine);
        abi::maybe_inject(ImGuiTestEngineExceptionPoint_UpstreamCall);
        abi::ScopedCurrentContext current(target);
        ImGuiTestEngine_Stop(engine);
        abi::clear_capture_provider(engine);
        abi::clear_capture_abort(engine);
        abi::increment(abi::Counter::EngineStopped);
        return ImGuiTestEngineStatus_Success;
    });
}

} // extern "C"

namespace dear_imgui_test_engine_abi {

bool begin_presentation(ImGuiTestEngine* engine) noexcept {
    SpinGuard guard(g_engine_lock);
    const auto found = g_engines.find(engine);
    if (found == g_engines.end() || found->second.PresentationPending) {
        return false;
    }
    found->second.PresentationPending = true;
    return true;
}

bool presentation_pending(ImGuiTestEngine* engine) noexcept {
    SpinGuard guard(g_engine_lock);
    const auto found = g_engines.find(engine);
    return found != g_engines.end() && found->second.PresentationPending;
}

void finish_presentation(ImGuiTestEngine* engine) noexcept {
    SpinGuard guard(g_engine_lock);
    const auto found = g_engines.find(engine);
    if (found != g_engines.end()) {
        found->second.PresentationPending = false;
        if (!capture_active(engine)) {
            found->second.CaptureRollbackValid = false;
            found->second.HiddenWindowBackupValid = false;
            found->second.HiddenWindows.clear();
        }
    }
}

void trace_presentation(
    ImGuiTestEngine* engine,
    ImGuiTestEnginePresentationEvent event
) {
    ImGuiTestEnginePresentationTraceCallback_c callback = nullptr;
    void* user_data = nullptr;
    {
        SpinGuard guard(g_engine_lock);
        const auto found = g_engines.find(engine);
        if (found == g_engines.end()) {
            return;
        }
        callback = found->second.PresentationTrace;
        user_data = found->second.PresentationTraceUserData;
    }
    if (callback != nullptr) {
        callback(event, user_data);
    }
}

bool set_presentation_trace(
    ImGuiTestEngine* engine,
    ImGuiTestEnginePresentationTraceCallback_c callback,
    void* user_data
) noexcept {
    SpinGuard guard(g_engine_lock);
    const auto found = g_engines.find(engine);
    if (found == g_engines.end() || found->second.PresentationPending) {
        return false;
    }
    found->second.PresentationTrace = callback;
    found->second.PresentationTraceUserData = user_data;
    return true;
}

bool get_capture_state(
    ImGuiTestEngine* engine,
    ImGuiTestEngineCaptureState_c* out_state
) noexcept {
    SpinGuard guard(g_engine_lock);
    const auto found = g_engines.find(engine);
    if (found == g_engines.end()) {
        return false;
    }
    const EngineRecord& record = found->second;
    *out_state = {};
    out_state->PresentationPending = record.PresentationPending;
    out_state->CaptureAbortRequested = record.CaptureAbortRequested;
    out_state->CaptureWaitPending = record.CaptureWaitPending;
    out_state->ProviderInstalled = record.CaptureProvider != nullptr;
    out_state->ContextCapturing = engine->CaptureContext.IsCapturing();
    out_state->ToolCapturing = engine->CaptureTool._StateIsCapturing;
    out_state->ToolPicking = engine->CaptureTool._StateIsPickingWindow;
    out_state->IoCapturing = engine->IO.IsCapturing;
    out_state->EngineAbort = engine->Abort;
    out_state->CaptureRollbackValid = record.CaptureRollbackValid;
    out_state->HiddenWindowBackupValid = record.HiddenWindowBackupValid;
    out_state->ScreenshotConfigActive = record.ScreenshotConfigActive;
    out_state->WindowMoveConfigActive = record.WindowMoveConfigActive;
    out_state->VideoConfigActive = record.VideoConfigActive;
    return true;
}

bool set_interactive_capture_state(
    ImGuiTestEngine* engine,
    bool capturing,
    bool picking
) noexcept {
    SpinGuard guard(g_engine_lock);
    const auto found = g_engines.find(engine);
    if (found == g_engines.end() || found->second.PresentationPending ||
        found->second.CaptureWaitPending) {
        return false;
    }
    engine->CaptureTool._StateIsCapturing = capturing;
    engine->CaptureTool._StateIsPickingWindow = picking;
    engine->IO.IsCapturing = capturing || picking;
    return true;
}

void finish_capture_rollback(ImGuiTestEngine* engine) noexcept {
    if (capture_active(engine)) {
        return;
    }
    SpinGuard guard(g_engine_lock);
    const auto found = g_engines.find(engine);
    if (found == g_engines.end()) {
        return;
    }
    found->second.CaptureRollbackValid = false;
    found->second.HiddenWindowBackupValid = false;
    found->second.HiddenWindows.clear();
}

void record_hidden_window_rollback(ImGuiTestEngine* engine) {
    if (engine->UiContextTarget == nullptr) {
        return;
    }
    SpinGuard guard(g_engine_lock);
    const auto found = g_engines.find(engine);
    if (found == g_engines.end()) {
        return;
    }
    EngineRecord& record = found->second;
    if (record.HiddenWindowBackupValid) {
        return;
    }
    record.HiddenWindows.clear();
    record.HiddenWindows.reserve(static_cast<std::size_t>(engine->UiContextTarget->Windows.Size));
    for (ImGuiWindow* window : engine->UiContextTarget->Windows) {
        record.HiddenWindows.push_back({window->ID, window->HiddenFramesForRenderOnly});
    }
    record.HiddenWindowBackupValid = true;
}

void record_capture_wait(ImGuiTestEngine* engine) {
    {
        SpinGuard guard(g_engine_lock);
        const auto found = g_engines.find(engine);
        if (found == g_engines.end()) {
            return;
        }
        found->second.CaptureWaitPending = true;
    }
    record_hidden_window_rollback(engine);
}

void record_capture_rollback(ImGuiTestEngine* engine) {
    if (capture_active(engine)) {
        record_hidden_window_rollback(engine);
    }
}

void commit_capture_rollback(ImGuiTestEngine* engine) noexcept {
    SpinGuard guard(g_engine_lock);
    const auto found = g_engines.find(engine);
    if (found == g_engines.end()) {
        return;
    }
    found->second.CaptureRollbackValid =
        capture_active(engine) && engine->CaptureContext._FrameNo > 0;
}

bool capture_provider_failed(ImGuiTestEngine* engine) noexcept {
    SpinGuard guard(g_engine_lock);
    const auto found = g_engines.find(engine);
    return found != g_engines.end() && found->second.CaptureProviderFailed;
}

bool take_capture_provider_failure(ImGuiTestEngine* engine) noexcept {
    SpinGuard guard(g_engine_lock);
    const auto found = g_engines.find(engine);
    if (found == g_engines.end()) {
        return false;
    }
    const bool failed = found->second.CaptureProviderFailed;
    found->second.CaptureProviderFailed = false;
    return failed;
}

void clear_capture_provider(ImGuiTestEngine* engine) noexcept {
    if (engine == nullptr) {
        return;
    }
    {
        SpinGuard guard(g_engine_lock);
        const auto found = g_engines.find(engine);
        if (found == g_engines.end()) {
            return;
        }
        found->second.CaptureProvider = nullptr;
        found->second.CaptureProviderUserData = nullptr;
        found->second.CaptureProviderFailed = false;
    }
    engine->IO.ScreenCaptureFunc = nullptr;
    engine->IO.ScreenCaptureUserData = nullptr;
    engine->CaptureContext.ScreenCaptureFunc = nullptr;
    engine->CaptureContext.ScreenCaptureUserData = nullptr;
}

bool capture_abort_requested(ImGuiTestEngine* engine) noexcept {
    SpinGuard guard(g_engine_lock);
    const auto found = g_engines.find(engine);
    return found != g_engines.end() && found->second.CaptureAbortRequested;
}

bool capture_wait_pending(ImGuiTestEngine* engine) noexcept {
    SpinGuard guard(g_engine_lock);
    const auto found = g_engines.find(engine);
    return found != g_engines.end() && found->second.CaptureWaitPending;
}

void clear_capture_abort(ImGuiTestEngine* engine) noexcept {
    SpinGuard guard(g_engine_lock);
    const auto found = g_engines.find(engine);
    if (found == g_engines.end()) {
        return;
    }
    EngineRecord& record = found->second;
    record.PresentationPending = false;
    record.CaptureAbortRequested = false;
    record.CaptureWaitPending = false;
    record.CaptureProviderFailed = false;
    record.CaptureRollbackValid = false;
    record.HiddenWindowBackupValid = false;
    record.HiddenWindows.clear();
    record.ScreenshotConfigActive = false;
    record.WindowMoveConfigActive = false;
    record.VideoConfigActive = false;
}

void clear_settled_capture_abort(ImGuiTestEngine* engine) noexcept {
    if (engine == nullptr || engine->Abort || capture_active(engine)) {
        return;
    }
    bool settled = false;
    {
        SpinGuard guard(g_engine_lock);
        const auto found = g_engines.find(engine);
        settled = found != g_engines.end() &&
                  found->second.CaptureAbortRequested &&
                  !found->second.CaptureWaitPending &&
                  !found->second.PresentationPending;
    }
    if (settled) {
        clear_capture_abort(engine);
    }
}

void begin_screenshot_config(
    ImGuiTestEngine* engine,
    ImGuiTestRunSpeed run_speed
) noexcept {
    SpinGuard guard(g_engine_lock);
    const auto found = g_engines.find(engine);
    if (found == g_engines.end() || found->second.ScreenshotConfigActive) {
        return;
    }
    found->second.ScreenshotConfigActive = true;
    found->second.ScreenshotRunSpeed = run_speed;
}

void restore_screenshot_config(ImGuiTestEngine* engine) noexcept {
    SpinGuard guard(g_engine_lock);
    const auto found = g_engines.find(engine);
    if (found == g_engines.end() || !found->second.ScreenshotConfigActive) {
        return;
    }
    engine->IO.ConfigRunSpeed = found->second.ScreenshotRunSpeed;
    found->second.ScreenshotConfigActive = false;
}

void begin_window_move_config(ImGuiTestEngine* engine, bool value) noexcept {
    SpinGuard guard(g_engine_lock);
    const auto found = g_engines.find(engine);
    if (found == g_engines.end() || found->second.WindowMoveConfigActive) {
        return;
    }
    found->second.WindowMoveConfigActive = true;
    found->second.WindowMoveFromTitleBarOnly = value;
}

void restore_window_move_config(ImGuiTestEngine* engine) noexcept {
    SpinGuard guard(g_engine_lock);
    const auto found = g_engines.find(engine);
    if (found == g_engines.end() || !found->second.WindowMoveConfigActive ||
        engine->UiContextTarget == nullptr) {
        return;
    }
    engine->UiContextTarget->IO.ConfigWindowsMoveFromTitleBarOnly =
        found->second.WindowMoveFromTitleBarOnly;
    found->second.WindowMoveConfigActive = false;
}

void begin_video_config(ImGuiTestEngine* engine) noexcept {
    SpinGuard guard(g_engine_lock);
    const auto found = g_engines.find(engine);
    if (found == g_engines.end() || found->second.VideoConfigActive) {
        return;
    }
    EngineRecord& record = found->second;
    record.VideoConfigActive = true;
    record.VideoRunSpeed = engine->IO.ConfigRunSpeed;
    record.VideoNoThrottle = engine->IO.ConfigNoThrottle;
    record.VideoFixedDeltaTime = engine->IO.ConfigFixedDeltaTime;
}

void restore_video_config(ImGuiTestEngine* engine) noexcept {
    SpinGuard guard(g_engine_lock);
    const auto found = g_engines.find(engine);
    if (found == g_engines.end() || !found->second.VideoConfigActive) {
        return;
    }
    EngineRecord& record = found->second;
    engine->IO.ConfigRunSpeed = record.VideoRunSpeed;
    engine->IO.ConfigNoThrottle = record.VideoNoThrottle;
    engine->IO.ConfigFixedDeltaTime = record.VideoFixedDeltaTime;
    record.VideoConfigActive = false;
}

void cancel_capture(ImGuiTestEngine* engine) noexcept {
    if (engine == nullptr) {
        return;
    }
    ScopedCurrentContext current(engine->UiContextTarget);

    bool capture_rollback_valid = false;
    {
        SpinGuard guard(g_engine_lock);
        const auto found = g_engines.find(engine);
        if (found == g_engines.end()) {
            return;
        }
        EngineRecord& record = found->second;
        record.CaptureAbortRequested = true;
        capture_rollback_valid = record.CaptureRollbackValid;
        if (record.HiddenWindowBackupValid && engine->UiContextTarget != nullptr) {
            for (const HiddenWindowState& backup : record.HiddenWindows) {
                if (ImGuiWindow* window = ImGui::FindWindowByID(backup.WindowId)) {
                    window->HiddenFramesForRenderOnly = backup.HiddenFramesForRenderOnly;
                }
            }
        }
        record.CaptureRollbackValid = false;
        record.HiddenWindowBackupValid = false;
        record.HiddenWindows.clear();
    }

    ImGuiCaptureContext& capture = engine->CaptureContext;
    if (capture_rollback_valid) {
        capture.RestoreBackedUpData();
    }
    if (capture._VideoEncoderPipe != nullptr) {
        FILE* encoder_pipe = capture._VideoEncoderPipe;
        capture._VideoEncoderPipe = nullptr;
        ImOsPClose(encoder_pipe);
    }
    capture._VideoRecording = false;
    capture._CaptureBuf.Clear();
    capture._WindowsData.clear();
    capture.ClearState();
    restore_screenshot_config(engine);
    restore_window_move_config(engine);
    restore_video_config(engine);

    engine->CaptureCurrentArgs = nullptr;
    engine->CaptureTool._StateIsCapturing = false;
    engine->CaptureTool._StateIsPickingWindow = false;
    engine->IO.IsCapturing = false;
    engine->PreSwapCalled = false;
    engine->PostSwapCalled = false;
}

void request_capture_abort(ImGuiTestEngine* engine) noexcept {
    if (engine == nullptr) {
        return;
    }
    {
        ScopedCurrentContext current(engine->UiContextTarget);
        ImGuiTestEngine_AbortCurrentTest(engine);
    }
    {
        SpinGuard guard(g_engine_lock);
        const auto found = g_engines.find(engine);
        if (found != g_engines.end()) {
            found->second.PresentationPending = false;
        }
    }
    cancel_capture(engine);
}

void finish_capture_wait(ImGuiTestEngine* engine) noexcept {
    if (engine == nullptr) {
        return;
    }
    {
        SpinGuard guard(g_engine_lock);
        const auto found = g_engines.find(engine);
        if (found == g_engines.end()) {
            return;
        }
        found->second.CaptureWaitPending = false;
        if (!engine->Abort) {
            found->second.CaptureAbortRequested = false;
        }
    }
    finish_capture_rollback(engine);
}

} // namespace dear_imgui_test_engine_abi

bool dear_imgui_rs_test_engine_capture_should_abort(ImGuiTestEngine* engine) noexcept {
    return engine == nullptr || engine->Abort || engine->TestQueueCoroutineShouldExit ||
           abi::capture_abort_requested(engine);
}

void dear_imgui_rs_test_engine_request_capture_abort(ImGuiTestEngine* engine) noexcept {
    abi::request_capture_abort(engine);
}

void dear_imgui_rs_test_engine_clear_capture_abort(ImGuiTestEngine* engine) noexcept {
    abi::clear_capture_abort(engine);
}

bool dear_imgui_rs_test_engine_take_capture_provider_failure(
    ImGuiTestEngine* engine
) noexcept {
    return abi::take_capture_provider_failure(engine);
}

void dear_imgui_rs_test_engine_begin_capture_wait(ImGuiTestEngine* engine) noexcept {
    try {
        abi::record_capture_wait(engine);
    } catch (...) {
        abi::request_capture_abort(engine);
    }
}

void dear_imgui_rs_test_engine_end_capture_wait(ImGuiTestEngine* engine) noexcept {
    abi::finish_capture_wait(engine);
}

void dear_imgui_rs_test_engine_begin_screenshot_config(
    ImGuiTestEngine* engine,
    ImGuiTestRunSpeed run_speed
) noexcept {
    abi::begin_screenshot_config(engine, run_speed);
}

void dear_imgui_rs_test_engine_restore_screenshot_config(ImGuiTestEngine* engine) noexcept {
    abi::restore_screenshot_config(engine);
}

void dear_imgui_rs_test_engine_begin_window_move_config(
    ImGuiTestEngine* engine,
    bool move_from_title_bar_only
) noexcept {
    abi::begin_window_move_config(engine, move_from_title_bar_only);
}

void dear_imgui_rs_test_engine_restore_window_move_config(ImGuiTestEngine* engine) noexcept {
    abi::restore_window_move_config(engine);
}

void dear_imgui_rs_test_engine_begin_video_config(ImGuiTestEngine* engine) noexcept {
    abi::begin_video_config(engine);
}

void dear_imgui_rs_test_engine_restore_video_config(ImGuiTestEngine* engine) noexcept {
    abi::restore_video_config(engine);
}

extern "C" {

ImGuiTestEngineStatus imgui_test_engine_pre_swap(ImGuiTestEngine* engine) {
    return abi::boundary("imgui_test_engine_pre_swap", [&]() {
        const ImGuiTestEngineStatus status = abi::require_engine(engine);
        if (status != ImGuiTestEngineStatus_Success) {
            return status;
        }
        if (!engine->Started || engine->UiContextTarget == nullptr) {
            return abi::fail(ImGuiTestEngineStatus_InvalidState, "engine is not started and bound");
        }
        if (abi::presentation_pending(engine)) {
            return abi::fail(
                ImGuiTestEngineStatus_InvalidState,
                "a presentation cycle is already pending"
            );
        }
        abi::clear_settled_capture_abort(engine);
        abi::maybe_inject(ImGuiTestEngineExceptionPoint_UpstreamCall);
        abi::ScopedCurrentContext current(engine->UiContextTarget);
        ImGuiTestEngine_PreSwap(engine);
        abi::trace_presentation(engine, ImGuiTestEnginePresentationEvent_PreSwap);
        if (!abi::begin_presentation(engine)) {
            return abi::fail(
                ImGuiTestEngineStatus_InvalidState,
                "presentation state changed during pre-swap"
            );
        }
        return ImGuiTestEngineStatus_Success;
    });
}

ImGuiTestEngineStatus imgui_test_engine_post_swap(
    ImGuiTestEngine* engine,
    bool* out_presentation_completed
) {
    return abi::boundary("imgui_test_engine_post_swap", [&]() {
        if (out_presentation_completed == nullptr) {
            return abi::fail(
                ImGuiTestEngineStatus_InvalidArgument,
                "out_presentation_completed must not be null"
            );
        }
        *out_presentation_completed = false;
        const ImGuiTestEngineStatus status = abi::require_engine(engine);
        if (status != ImGuiTestEngineStatus_Success) {
            return status;
        }
        if (!engine->Started || engine->UiContextTarget == nullptr) {
            return abi::fail(ImGuiTestEngineStatus_InvalidState, "engine is not started and bound");
        }
        if (!abi::presentation_pending(engine)) {
            return abi::fail(
                ImGuiTestEngineStatus_InvalidState,
                "post-swap requires a pending presentation cycle"
            );
        }
        abi::maybe_inject(ImGuiTestEngineExceptionPoint_UpstreamCall);
        abi::ScopedCurrentContext current(engine->UiContextTarget);
        abi::record_capture_rollback(engine);
        ImGuiTestEngine_PostSwap(engine);
        abi::commit_capture_rollback(engine);
        abi::finish_presentation(engine);
        abi::trace_presentation(engine, ImGuiTestEnginePresentationEvent_PostSwap);
        *out_presentation_completed = true;
        if (abi::capture_provider_failed(engine)) {
            return abi::fail(
                ImGuiTestEngineStatus_CaptureFailed,
                "the framebuffer capture provider rejected a capture request"
            );
        }
        return ImGuiTestEngineStatus_Success;
    });
}

ImGuiTestEngineStatus imgui_test_engine_abort_presentation(ImGuiTestEngine* engine) {
    return abi::boundary("imgui_test_engine_abort_presentation", [&]() {
        const ImGuiTestEngineStatus status = abi::require_engine(engine);
        if (status != ImGuiTestEngineStatus_Success) {
            return status;
        }
        if (!engine->Started || engine->UiContextTarget == nullptr) {
            return abi::fail(ImGuiTestEngineStatus_InvalidState, "engine is not started and bound");
        }
        if (!abi::presentation_pending(engine)) {
            return abi::fail(
                ImGuiTestEngineStatus_InvalidState,
                "presentation abort requires a pending presentation cycle"
            );
        }
        abi::request_capture_abort(engine);
        return ImGuiTestEngineStatus_Success;
    });
}

ImGuiTestEngineStatus imgui_test_engine_install_capture_provider(
    ImGuiTestEngine* engine,
    ImGuiTestEngineCaptureCallback_c callback,
    void* user_data
) {
    return abi::boundary("imgui_test_engine_install_capture_provider", [&]() {
        const ImGuiTestEngineStatus status = abi::require_engine(engine);
        if (status != ImGuiTestEngineStatus_Success) {
            return status;
        }
#if !IMGUI_TEST_ENGINE_ENABLE_CAPTURE
        IM_UNUSED(callback);
        IM_UNUSED(user_data);
        return abi::fail(
            ImGuiTestEngineStatus_Unsupported,
            "capture support was not compiled into the Test Engine"
        );
#else
        if (callback == nullptr) {
            return abi::fail(
                ImGuiTestEngineStatus_InvalidArgument,
                "capture callback must not be null"
            );
        }
        if (!engine->Started || engine->UiContextTarget == nullptr) {
            return abi::fail(
                ImGuiTestEngineStatus_InvalidState,
                "capture provider installation requires a started engine"
            );
        }
        if (abi::presentation_pending(engine) || abi::capture_active(engine)) {
            return abi::fail(
                ImGuiTestEngineStatus_InvalidState,
                "capture provider cannot change during an active capture or presentation"
            );
        }
        {
            abi::SpinGuard guard(abi::g_engine_lock);
            const auto found = abi::g_engines.find(engine);
            if (found == abi::g_engines.end()) {
                return abi::fail(
                    ImGuiTestEngineStatus_InvalidState,
                    "engine is not managed by this ABI"
                );
            }
            if (found->second.CaptureProvider != nullptr) {
                return abi::fail(
                    ImGuiTestEngineStatus_InvalidState,
                    "a capture provider is already installed"
                );
            }
            found->second.CaptureProvider = callback;
            found->second.CaptureProviderUserData = user_data;
            found->second.CaptureProviderFailed = false;
        }
        // PostSwap copies the engine IO callbacks into CaptureContext every frame, so the IO
        // fields are the source of truth. Set the context too for immediate interactive capture.
        engine->IO.ScreenCaptureFunc = abi::screen_capture_trampoline;
        engine->IO.ScreenCaptureUserData = engine;
        engine->CaptureContext.ScreenCaptureFunc = abi::screen_capture_trampoline;
        engine->CaptureContext.ScreenCaptureUserData = engine;
        return ImGuiTestEngineStatus_Success;
#endif
    });
}

ImGuiTestEngineStatus imgui_test_engine_clear_capture_provider(
    ImGuiTestEngine* engine,
    void* user_data
) {
    return abi::boundary("imgui_test_engine_clear_capture_provider", [&]() {
        const ImGuiTestEngineStatus status = abi::require_engine(engine);
        if (status != ImGuiTestEngineStatus_Success) {
            return status;
        }
        bool installed = false;
        {
            abi::SpinGuard guard(abi::g_engine_lock);
            const auto found = abi::g_engines.find(engine);
            if (found == abi::g_engines.end()) {
                return abi::fail(
                    ImGuiTestEngineStatus_InvalidState,
                    "engine is not managed by this ABI"
                );
            }
            installed = found->second.CaptureProvider != nullptr;
            if (installed && found->second.CaptureProviderUserData != user_data) {
                return abi::fail(
                    ImGuiTestEngineStatus_InvalidArgument,
                    "capture provider owner does not match the installed provider"
                );
            }
        }
        if (!installed) {
            return ImGuiTestEngineStatus_Success;
        }
        if (abi::presentation_pending(engine)) {
            abi::request_capture_abort(engine);
        } else if (abi::capture_active(engine) || abi::capture_wait_pending(engine)) {
            const bool wait_pending = abi::capture_wait_pending(engine);
            abi::cancel_capture(engine);
            if (!wait_pending) {
                abi::clear_capture_abort(engine);
            }
        }
        abi::clear_capture_provider(engine);
        if (!abi::presentation_pending(engine) &&
            !abi::capture_active(engine) &&
            !abi::capture_wait_pending(engine)) {
            abi::clear_capture_abort(engine);
        }
        return ImGuiTestEngineStatus_Success;
    });
}

ImGuiTestEngineStatus imgui_test_engine_has_capture_provider(
    ImGuiTestEngine* engine,
    bool* out_installed
) {
    return abi::boundary("imgui_test_engine_has_capture_provider", [&]() {
        if (out_installed == nullptr) {
            return abi::fail(
                ImGuiTestEngineStatus_InvalidArgument,
                "out_installed must not be null"
            );
        }
        *out_installed = false;
        const ImGuiTestEngineStatus status = abi::require_engine(engine);
        if (status != ImGuiTestEngineStatus_Success) {
            return status;
        }
        abi::SpinGuard guard(abi::g_engine_lock);
        const auto found = abi::g_engines.find(engine);
        *out_installed = found != abi::g_engines.end() &&
                         found->second.CaptureProvider != nullptr;
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
        abi::record_capture_rollback(engine);
        ImGuiTestEngine_ShowTestEngineWindows(engine, p_open);
        abi::finish_capture_rollback(engine);
        if (abi::capture_provider_failed(engine)) {
            abi::take_capture_provider_failure(engine);
            return abi::fail(
                ImGuiTestEngineStatus_CaptureFailed,
                "the framebuffer capture provider rejected a capture request"
            );
        }
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
        abi::maybe_inject(ImGuiTestEngineExceptionPoint_UpstreamCall);
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

ImGuiTestEngineStatus imgui_test_engine_set_verbose_level_on_error(
    ImGuiTestEngine* engine,
    int level
) {
    return abi::boundary("imgui_test_engine_set_verbose_level_on_error", [&]() {
        const ImGuiTestEngineStatus status = abi::require_engine(engine);
        if (status != ImGuiTestEngineStatus_Success) {
            return status;
        }
        if (level < ImGuiTestEngineVerboseLevel_Silent || level > ImGuiTestEngineVerboseLevel_Trace) {
            return abi::fail(ImGuiTestEngineStatus_OutOfRange, "error verbose level is out of range");
        }
        ImGuiTestEngine_GetIO(engine).ConfigVerboseLevelOnError =
            static_cast<ImGuiTestVerboseLevel>(level);
        return ImGuiTestEngineStatus_Success;
    });
}

ImGuiTestEngineStatus imgui_test_engine_set_log_to_tty(
    ImGuiTestEngine* engine,
    bool enabled
) {
    return abi::boundary("imgui_test_engine_set_log_to_tty", [&]() {
        const ImGuiTestEngineStatus status = abi::require_engine(engine);
        if (status != ImGuiTestEngineStatus_Success) {
            return status;
        }
        ImGuiTestEngine_GetIO(engine).ConfigLogToTTY = enabled;
        return ImGuiTestEngineStatus_Success;
    });
}

ImGuiTestEngineStatus imgui_test_engine_set_capture_output_enabled(
    ImGuiTestEngine* engine,
    bool enabled
) {
    return abi::boundary("imgui_test_engine_set_capture_output_enabled", [&]() {
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
        abi::maybe_inject(ImGuiTestEngineExceptionPoint_UpstreamCall);
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

ImGuiTestEngineStatus imgui_test_engine_test_set_presentation_trace(
    ImGuiTestEngine* engine,
    ImGuiTestEnginePresentationTraceCallback_c callback,
    void* user_data
) {
    return abi::boundary("imgui_test_engine_test_set_presentation_trace", [&]() {
        const ImGuiTestEngineStatus status = abi::require_engine(engine);
        if (status != ImGuiTestEngineStatus_Success) {
            return status;
        }
        if (callback == nullptr && user_data != nullptr) {
            return abi::fail(
                ImGuiTestEngineStatus_InvalidArgument,
                "user_data requires a presentation trace callback"
            );
        }
        if (!abi::set_presentation_trace(engine, callback, user_data)) {
            return abi::fail(
                ImGuiTestEngineStatus_InvalidState,
                "presentation trace cannot change during a pending presentation"
            );
        }
        return ImGuiTestEngineStatus_Success;
    });
}

ImGuiTestEngineStatus imgui_test_engine_test_get_capture_state(
    ImGuiTestEngine* engine,
    ImGuiTestEngineCaptureState_c* out_state
) {
    return abi::boundary("imgui_test_engine_test_get_capture_state", [&]() {
        if (out_state == nullptr) {
            return abi::fail(ImGuiTestEngineStatus_InvalidArgument, "out_state must not be null");
        }
        *out_state = {};
        const ImGuiTestEngineStatus status = abi::require_engine(engine);
        if (status != ImGuiTestEngineStatus_Success) {
            return status;
        }
        if (!abi::get_capture_state(engine, out_state)) {
            return abi::fail(ImGuiTestEngineStatus_InvalidState, "capture state is unavailable");
        }
        return ImGuiTestEngineStatus_Success;
    });
}

ImGuiTestEngineStatus imgui_test_engine_test_set_interactive_capture_state(
    ImGuiTestEngine* engine,
    bool capturing,
    bool picking
) {
    return abi::boundary("imgui_test_engine_test_set_interactive_capture_state", [&]() {
        const ImGuiTestEngineStatus status = abi::require_engine(engine);
        if (status != ImGuiTestEngineStatus_Success) {
            return status;
        }
#if !IMGUI_TEST_ENGINE_ENABLE_CAPTURE
        IM_UNUSED(capturing);
        IM_UNUSED(picking);
        return abi::fail(
            ImGuiTestEngineStatus_Unsupported,
            "capture support was not compiled into the Test Engine"
        );
#else
        if (!engine->Started || engine->UiContextTarget == nullptr) {
            return abi::fail(
                ImGuiTestEngineStatus_InvalidState,
                "interactive capture state requires a started engine"
            );
        }
        if (!abi::set_interactive_capture_state(engine, capturing, picking)) {
            return abi::fail(
                ImGuiTestEngineStatus_InvalidState,
                "interactive capture state cannot change during presentation or capture wait"
            );
        }
        return ImGuiTestEngineStatus_Success;
#endif
    });
}

} // extern "C"
