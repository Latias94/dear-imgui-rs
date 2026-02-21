// Minimal demo tests for validating Dear ImGui Test Engine integration.
// This file is part of dear-imgui-rs and is licensed under MIT OR Apache-2.0.

#define IMGUI_DEFINE_MATH_OPERATORS
#include "imgui.h"
#include "imgui_internal.h"

#include "imgui_te_engine.h"   // IM_REGISTER_TEST()
#include "imgui_te_context.h"  // ImGuiTestContext

#include "cimgui_test_engine.h"

extern "C" {

void imgui_test_engine_register_default_tests(ImGuiTestEngine* engine) {
    if (engine == nullptr) {
        return;
    }

    ImGuiTest* t = nullptr;

    // Demo: basic interaction (button + checkbox)
    t = IM_REGISTER_TEST(engine, "demo_tests", "basic_interaction");
    t->GuiFunc = [](ImGuiTestContext* ctx) {
        IM_UNUSED(ctx);
        ImGui::Begin("Test Window###DefaultTests", nullptr, ImGuiWindowFlags_NoSavedSettings);
        ImGui::TextUnformatted("Hello, automation world");
        // Note: avoid reusing the same `###id` for multiple items (it causes ID collisions).
        ImGui::Button("Click Me");
        if (ImGui::TreeNode("Node")) {
            static bool b = false;
            ImGui::Checkbox("Checkbox", &b);
            ImGui::TreePop();
        }
        ImGui::End();
    };
    t->TestFunc = [](ImGuiTestContext* ctx) {
        ctx->SetRef("Test Window###DefaultTests");
        ctx->ItemClick("Click Me");
        // Optional as ItemCheck("Node/Checkbox") can open parent tree nodes automatically.
        ctx->ItemCheck("Node/Checkbox");
        ctx->ItemUncheck("Node/Checkbox");
    };

    // Demo: value entry (slider int)
    t = IM_REGISTER_TEST(engine, "demo_tests", "input_value");
    struct TestVars2 {
        int MyInt = 42;
    };
    t->SetVarsDataType<TestVars2>();
    t->GuiFunc = [](ImGuiTestContext* ctx) {
        TestVars2& vars = ctx->GetVars<TestVars2>();
        ImGui::Begin("Test Window###DefaultTests", nullptr, ImGuiWindowFlags_NoSavedSettings);
        ImGui::SliderInt("Slider", &vars.MyInt, 0, 1000);
        ImGui::End();
    };
    t->TestFunc = [](ImGuiTestContext* ctx) {
        TestVars2& vars = ctx->GetVars<TestVars2>();
        ctx->SetRef("Test Window###DefaultTests");
        IM_CHECK_EQ(vars.MyInt, 42);
        ctx->ItemInputValue("Slider", 123);
        IM_CHECK_EQ(vars.MyInt, 123);
    };
}

} // extern "C"
