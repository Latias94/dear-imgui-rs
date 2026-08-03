// Narrow bridge for the upstream docking and viewport Test Engine suites.

#include <cstring>

#include "imgui.h"
#include "imgui_test_engine/imgui_te_engine.h"

#include "cimgui_test_engine.h"
#include "cimgui_test_engine_internal.h"

void RegisterTests_Docking(ImGuiTestEngine* engine);
void RegisterTests_Viewports(ImGuiTestEngine* engine);

namespace abi = dear_imgui_test_engine_abi;

namespace {

struct SuiteSpec {
    const char* category;
    ImGuiTestEngineStatus (*register_tests)(ImGuiTestEngine* engine);
};

int count_category(ImGuiTestEngine* engine, const char* category) {
    ImVector<ImGuiTest*> tests;
    ImGuiTestEngine_GetTestList(engine, &tests);

    int count = 0;
    for (ImGuiTest* test : tests) {
        if (test != nullptr && test->Category != nullptr &&
            std::strcmp(test->Category, category) == 0) {
            count++;
        }
    }
    return count;
}

const ImGuiTest* find_category_test(
    ImGuiTestEngine* engine,
    const char* category,
    int index
) {
    ImVector<ImGuiTest*> tests;
    ImGuiTestEngine_GetTestList(engine, &tests);

    int category_index = 0;
    for (ImGuiTest* test : tests) {
        if (test == nullptr || test->Category == nullptr ||
            std::strcmp(test->Category, category) != 0) {
            continue;
        }
        if (category_index == index) {
            return test;
        }
        category_index++;
    }
    return nullptr;
}

ImGuiTestEngineStatus register_native_defaults(ImGuiTestEngine* engine) {
    return imgui_test_engine_register_default_tests(engine);
}

ImGuiTestEngineStatus register_upstream_docking(ImGuiTestEngine* engine) {
    RegisterTests_Docking(engine);
    return ImGuiTestEngineStatus_Success;
}

ImGuiTestEngineStatus register_upstream_viewports(ImGuiTestEngine* engine) {
    RegisterTests_Viewports(engine);
    return ImGuiTestEngineStatus_Success;
}

ImGuiTestEngineStatus resolve_suite(int suite, SuiteSpec* out_spec) {
    switch (suite) {
        case ImGuiTestEngineBuiltinTestSuite_NativeDefaults:
            *out_spec = {"demo_tests", register_native_defaults};
            return ImGuiTestEngineStatus_Success;
        case ImGuiTestEngineBuiltinTestSuite_UpstreamDocking:
            *out_spec = {"docking", register_upstream_docking};
            return ImGuiTestEngineStatus_Success;
        case ImGuiTestEngineBuiltinTestSuite_UpstreamViewports:
            *out_spec = {"viewport", register_upstream_viewports};
            return ImGuiTestEngineStatus_Success;
        default:
            return abi::fail(ImGuiTestEngineStatus_OutOfRange, "suite is out of range");
    }
}

ImGuiTestEngineStatus require_viewport_backend() {
    ImGuiContext* context = ImGui::GetCurrentContext();
    if (context == nullptr) {
        return abi::fail(
            ImGuiTestEngineStatus_InvalidState,
            "viewport suite registration requires a current Dear ImGui context"
        );
    }

    const ImGuiIO& io = ImGui::GetIO();
    if ((io.ConfigFlags & ImGuiConfigFlags_ViewportsEnable) == 0) {
        return abi::fail(
            ImGuiTestEngineStatus_Unsupported,
            "viewport suite requires ImGuiConfigFlags_ViewportsEnable"
        );
    }
    if ((io.ConfigFlags & ImGuiConfigFlags_DockingEnable) == 0) {
        return abi::fail(
            ImGuiTestEngineStatus_Unsupported,
            "viewport suite requires ImGuiConfigFlags_DockingEnable"
        );
    }
    if ((io.BackendFlags & ImGuiBackendFlags_PlatformHasViewports) == 0) {
        return abi::fail(
            ImGuiTestEngineStatus_Unsupported,
            "viewport suite requires ImGuiBackendFlags_PlatformHasViewports"
        );
    }
    if ((io.BackendFlags & ImGuiBackendFlags_RendererHasViewports) == 0) {
        return abi::fail(
            ImGuiTestEngineStatus_Unsupported,
            "viewport suite requires ImGuiBackendFlags_RendererHasViewports"
        );
    }
    return ImGuiTestEngineStatus_Success;
}

} // namespace

extern "C" {

ImGuiTestEngineStatus imgui_test_engine_register_builtin_test_suite(
    ImGuiTestEngine* engine,
    int suite,
    int* out_registered_count
) {
    return abi::boundary("imgui_test_engine_register_builtin_test_suite", [&]() {
        if (out_registered_count == nullptr) {
            return abi::fail(
                ImGuiTestEngineStatus_InvalidArgument,
                "out_registered_count must not be null"
            );
        }
        *out_registered_count = 0;

        const ImGuiTestEngineStatus engine_status = abi::require_engine(engine);
        if (engine_status != ImGuiTestEngineStatus_Success) {
            return engine_status;
        }
        if (ImGuiTestEngine_GetIO(engine).IsRunningTests) {
            return abi::fail(
                ImGuiTestEngineStatus_InvalidState,
                "built-in tests cannot be registered while tests are running"
            );
        }

        SuiteSpec spec{};
        const ImGuiTestEngineStatus suite_status = resolve_suite(suite, &spec);
        if (suite_status != ImGuiTestEngineStatus_Success) {
            return suite_status;
        }
        if (count_category(engine, spec.category) != 0) {
            return abi::fail(
                ImGuiTestEngineStatus_InvalidState,
                "the requested built-in test category is already registered"
            );
        }
        auto register_suite = [&]() {
            abi::maybe_inject(ImGuiTestEngineExceptionPoint_UpstreamCall);
            const ImGuiTestEngineStatus registration_status = spec.register_tests(engine);
            if (registration_status != ImGuiTestEngineStatus_Success) {
                return registration_status;
            }
            *out_registered_count = count_category(engine, spec.category);
            return ImGuiTestEngineStatus_Success;
        };

        if (suite == ImGuiTestEngineBuiltinTestSuite_UpstreamViewports) {
            ImGuiContext* target = nullptr;
            if (imgui_test_engine_get_ui_context_target(engine, &target) !=
                ImGuiTestEngineStatus_Success) {
                return abi::fail(
                    ImGuiTestEngineStatus_InvalidState,
                    "viewport suite registration requires an engine-bound UI context"
                );
            }
            abi::ScopedCurrentContext current(target);
            const ImGuiTestEngineStatus viewport_status = require_viewport_backend();
            if (viewport_status != ImGuiTestEngineStatus_Success) {
                return viewport_status;
            }
            return register_suite();
        }
        return register_suite();
    });
}

ImGuiTestEngineStatus imgui_test_engine_get_registered_test_count(
    ImGuiTestEngine* engine,
    const char* category,
    int* out_count
) {
    return abi::boundary("imgui_test_engine_get_registered_test_count", [&]() {
        if (category == nullptr || category[0] == '\0') {
            return abi::fail(
                ImGuiTestEngineStatus_InvalidArgument,
                "category must name a non-empty test category"
            );
        }
        if (out_count == nullptr) {
            return abi::fail(ImGuiTestEngineStatus_InvalidArgument, "out_count must not be null");
        }
        *out_count = 0;

        const ImGuiTestEngineStatus status = abi::require_engine(engine);
        if (status != ImGuiTestEngineStatus_Success) {
            return status;
        }
        *out_count = count_category(engine, category);
        return ImGuiTestEngineStatus_Success;
    });
}

ImGuiTestEngineStatus imgui_test_engine_get_registered_test_name(
    ImGuiTestEngine* engine,
    const char* category,
    int index,
    char* buffer,
    size_t buffer_size,
    size_t* out_required_size
) {
    return abi::boundary("imgui_test_engine_get_registered_test_name", [&]() {
        if (out_required_size == nullptr) {
            return abi::fail(
                ImGuiTestEngineStatus_InvalidArgument,
                "out_required_size must not be null"
            );
        }
        *out_required_size = 0;
        if (category == nullptr || category[0] == '\0') {
            return abi::fail(
                ImGuiTestEngineStatus_InvalidArgument,
                "category must name a non-empty test category"
            );
        }
        if (index < 0) {
            return abi::fail(ImGuiTestEngineStatus_OutOfRange, "index must not be negative");
        }
        if (buffer == nullptr && buffer_size != 0) {
            return abi::fail(
                ImGuiTestEngineStatus_InvalidArgument,
                "a null buffer requires zero capacity"
            );
        }

        const ImGuiTestEngineStatus status = abi::require_engine(engine);
        if (status != ImGuiTestEngineStatus_Success) {
            return status;
        }
        const ImGuiTest* test = find_category_test(engine, category, index);
        if (test == nullptr || test->Name == nullptr) {
            return abi::fail(
                ImGuiTestEngineStatus_NotFound,
                "no registered test exists at the requested category index"
            );
        }

        const size_t required_size = std::strlen(test->Name) + 1;
        *out_required_size = required_size;
        if (buffer == nullptr) {
            return ImGuiTestEngineStatus_Success;
        }
        if (buffer_size < required_size) {
            if (buffer_size != 0) {
                buffer[0] = '\0';
            }
            return abi::fail(
                ImGuiTestEngineStatus_OutOfRange,
                "registered test name buffer is too small"
            );
        }

        std::memcpy(buffer, test->Name, required_size);
        return ImGuiTestEngineStatus_Success;
    });
}

ImGuiTestEngineStatus imgui_test_engine_get_registered_test_succeeded(
    ImGuiTestEngine* engine,
    const char* category,
    int index,
    bool* out_succeeded
) {
    return abi::boundary("imgui_test_engine_get_registered_test_succeeded", [&]() {
        if (out_succeeded == nullptr) {
            return abi::fail(
                ImGuiTestEngineStatus_InvalidArgument,
                "out_succeeded must not be null"
            );
        }
        *out_succeeded = false;
        if (category == nullptr || category[0] == '\0') {
            return abi::fail(
                ImGuiTestEngineStatus_InvalidArgument,
                "category must name a non-empty test category"
            );
        }
        if (index < 0) {
            return abi::fail(ImGuiTestEngineStatus_OutOfRange, "index must not be negative");
        }

        const ImGuiTestEngineStatus status = abi::require_engine(engine);
        if (status != ImGuiTestEngineStatus_Success) {
            return status;
        }
        const ImGuiTest* test = find_category_test(engine, category, index);
        if (test == nullptr) {
            return abi::fail(
                ImGuiTestEngineStatus_NotFound,
                "no registered test exists at the requested category index"
            );
        }
        *out_succeeded = test->Output.Status == ImGuiTestStatus_Success;
        return ImGuiTestEngineStatus_Success;
    });
}

} // extern "C"
