/// Native artifact capability for the repository-owned safe demo-window ABI.
pub const SAFE_DEMO_FONT_BOUNDARY_ARTIFACT_FEATURE: &str = "safe-demo-font-boundary-v1";

/// Extra C symbols exported by the WebAssembly cimgui provider.
pub const SAFE_DEMO_FONT_BOUNDARY_WASM_EXPORTS: &[&str] = &[
    "dear_imgui_rs_show_demo_window_without_font_atlas",
    "dear_imgui_rs_show_font_atlas_debug_panel",
    "dear_imgui_rs_show_metrics_window_without_font_atlas",
    "dear_imgui_rs_show_style_editor_without_font_atlas",
];

/// Patch Dear ImGui Test Engine capture waits so an aborted presentation can stop safely.
pub fn patch_test_engine_cpp_for_presentation_abort(source: &str) -> Result<String, String> {
    let (mut patched, newline) = normalize_cpp_source(source);

    patched = replace_cpp_source_once(
        &patched,
        "#include \"imgui_te_internal.h\"",
        concat!(
            "#include \"imgui_te_internal.h\"\n",
            "#include \"cimgui_test_engine_capture_bridge.h\""
        ),
        "Test Engine capture bridge include",
    )?;
    patched = replace_cpp_source_once(
        &patched,
        concat!(
            "    engine->TestQueueCoroutineShouldExit = false;\n",
            "    engine->Started = true;"
        ),
        concat!(
            "    engine->TestQueueCoroutineShouldExit = false;\n",
            "    dear_imgui_rs_test_engine_clear_capture_abort(engine);\n",
            "    engine->Started = true;"
        ),
        "Test Engine start cancellation reset",
    )?;
    patched = replace_cpp_source_once(
        &patched,
        concat!(
            "    if (engine->TestQueueCoroutine != nullptr)\n",
            "    {\n",
            "        // Run until the coroutine exits\n",
            "        engine->TestQueueCoroutineShouldExit = true;"
        ),
        concat!(
            "    if (engine->TestQueueCoroutine != nullptr)\n",
            "    {\n",
            "        // Release capture waits before repeatedly resuming the coroutine.\n",
            "        dear_imgui_rs_test_engine_request_capture_abort(engine);\n",
            "        // Run until the coroutine exits\n",
            "        engine->TestQueueCoroutineShouldExit = true;"
        ),
        "Test Engine stop capture cancellation",
    )?;
    patched = replace_cpp_source_once(
        &patched,
        "bool ImGuiTestEngine_CaptureScreenshot(ImGuiTestEngine* engine, ImGuiCaptureArgs* args)\n{",
        concat!(
            "bool ImGuiTestEngine_CaptureScreenshot(ImGuiTestEngine* engine, ImGuiCaptureArgs* args)\n",
            "{\n",
            "    if (dear_imgui_rs_test_engine_capture_should_abort(engine))\n",
            "        return false;"
        ),
        "CaptureScreenshot cancellation entry",
    )?;
    patched = replace_cpp_source_once(
        &patched,
        concat!(
            "bool ImGuiTestEngine_CaptureScreenshot(ImGuiTestEngine* engine, ImGuiCaptureArgs* args)\n",
            "{\n",
            "    if (dear_imgui_rs_test_engine_capture_should_abort(engine))\n",
            "        return false;\n",
            "    if (engine->IO.ScreenCaptureFunc == nullptr)\n",
            "    {\n",
            "        IM_ASSERT(0);\n",
            "        return false;\n",
            "    }\n\n",
            "    IM_ASSERT(engine->CaptureCurrentArgs == nullptr && \"Nested captures are not supported.\");"
        ),
        concat!(
            "bool ImGuiTestEngine_CaptureScreenshot(ImGuiTestEngine* engine, ImGuiCaptureArgs* args)\n",
            "{\n",
            "    if (dear_imgui_rs_test_engine_capture_should_abort(engine))\n",
            "        return false;\n",
            "    if (engine->IO.ScreenCaptureFunc == nullptr)\n",
            "        return false;\n\n",
            "    IM_ASSERT(engine->CaptureCurrentArgs == nullptr && \"Nested captures are not supported.\");"
        ),
        "CaptureScreenshot missing-provider rejection",
    )?;
    patched = replace_cpp_source_once(
        &patched,
        concat!(
            "bool ImGuiTestEngine_CaptureScreenshot(ImGuiTestEngine* engine, ImGuiCaptureArgs* args)\n",
            "{\n",
            "    if (dear_imgui_rs_test_engine_capture_should_abort(engine))\n",
            "        return false;\n",
            "    if (engine->IO.ScreenCaptureFunc == nullptr)\n",
            "        return false;\n\n",
            "    IM_ASSERT(engine->CaptureCurrentArgs == nullptr && \"Nested captures are not supported.\");"
        ),
        concat!(
            "bool ImGuiTestEngine_CaptureScreenshot(ImGuiTestEngine* engine, ImGuiCaptureArgs* args)\n",
            "{\n",
            "    if (dear_imgui_rs_test_engine_capture_should_abort(engine))\n",
            "        return false;\n",
            "    if (engine->IO.ScreenCaptureFunc == nullptr)\n",
            "        return false;\n\n",
            "    DearImGuiRsCaptureWaitGuard capture_wait(engine);\n",
            "    IM_ASSERT(engine->CaptureCurrentArgs == nullptr && \"Nested captures are not supported.\");"
        ),
        "CaptureScreenshot initial-yield rollback",
    )?;
    patched = replace_cpp_source_once(
        &patched,
        concat!(
            "    // Can only yield in the test func!\n",
            "    if (ctx)\n",
            "    {"
        ),
        concat!(
            "    // Can only yield in the test func!\n",
            "    if (ctx && !dear_imgui_rs_test_engine_capture_should_abort(engine))\n",
            "    {"
        ),
        "Yield cancellation window visibility",
    )?;
    patched = replace_cpp_source_once(
        &patched,
        concat!(
            "    const ImGuiTestRunSpeed backup_run_speed = engine->IO.ConfigRunSpeed;\n",
            "    engine->IO.ConfigRunSpeed = ImGuiTestRunSpeed_Fast;"
        ),
        concat!(
            "    const ImGuiTestRunSpeed backup_run_speed = engine->IO.ConfigRunSpeed;\n",
            "    dear_imgui_rs_test_engine_begin_screenshot_config(engine, backup_run_speed);\n",
            "    engine->IO.ConfigRunSpeed = ImGuiTestRunSpeed_Fast;"
        ),
        "CaptureScreenshot configuration backup",
    )?;
    patched = replace_cpp_source_once(
        &patched,
        concat!(
            "    if ((args->InFlags & ImGuiCaptureFlags_Instant) == 0)\n",
            "        ImGuiTestEngine_Yield(engine);"
        ),
        concat!(
            "    if ((args->InFlags & ImGuiCaptureFlags_Instant) == 0)\n",
            "    {\n",
            "        ImGuiTestEngine_Yield(engine);\n",
            "        if (dear_imgui_rs_test_engine_capture_should_abort(engine))\n",
            "        {\n",
            "            dear_imgui_rs_test_engine_restore_screenshot_config(engine);\n",
            "            return false;\n",
            "        }\n",
            "    }"
        ),
        "CaptureScreenshot initial-yield cancellation",
    )?;
    patched = replace_cpp_source_once(
        &patched,
        "    while (engine->CaptureCurrentArgs != nullptr)\n    {",
        concat!(
            "    while (engine->CaptureCurrentArgs != nullptr &&\n",
            "           !dear_imgui_rs_test_engine_capture_should_abort(engine))\n",
            "    {"
        ),
        "CaptureScreenshot wait cancellation",
    )?;
    patched = replace_cpp_source_once(
        &patched,
        concat!(
            "        ImGuiTestEngine_Yield(engine);\n",
            "        frames_yielded++;\n",
            "        if (frames_yielded > 4)\n",
            "            IM_ASSERT(engine->PostSwapCalled && \"ImGuiTestEngine_PostSwap() is not being called by application! Must be called in order.\");"
        ),
        concat!(
            "        ImGuiTestEngine_Yield(engine);\n",
            "        frames_yielded++;\n",
            "        if (dear_imgui_rs_test_engine_capture_should_abort(engine))\n",
            "            break;\n",
            "        if (frames_yielded > 4)\n",
            "            IM_ASSERT(engine->PostSwapCalled && \"ImGuiTestEngine_PostSwap() is not being called by application! Must be called in order.\");"
        ),
        "CaptureScreenshot post-yield cancellation",
    )?;
    patched = replace_cpp_source_once(
        &patched,
        concat!(
            "    // Verify that the ImGuiCaptureFlags_Instant flag got honored\n",
            "    if (args->InFlags & ImGuiCaptureFlags_Instant)"
        ),
        concat!(
            "    if (dear_imgui_rs_test_engine_capture_should_abort(engine))\n",
            "    {\n",
            "        engine->CaptureCurrentArgs = nullptr;\n",
            "        dear_imgui_rs_test_engine_restore_screenshot_config(engine);\n",
            "        return false;\n",
            "    }\n\n",
            "    // Verify that the ImGuiCaptureFlags_Instant flag got honored\n",
            "    if (args->InFlags & ImGuiCaptureFlags_Instant)"
        ),
        "CaptureScreenshot cancelled return",
    )?;
    patched = replace_cpp_source_once(
        &patched,
        concat!(
            "    engine->IO.ConfigRunSpeed = backup_run_speed;\n",
            "    return true;"
        ),
        concat!(
            "    const bool capture_completed =\n",
            "        !dear_imgui_rs_test_engine_take_capture_provider_failure(engine);\n",
            "    dear_imgui_rs_test_engine_restore_screenshot_config(engine);\n",
            "    return capture_completed;"
        ),
        "CaptureScreenshot provider result propagation",
    )?;
    patched = replace_cpp_source_once(
        &patched,
        "bool ImGuiTestEngine_CaptureBeginVideo(ImGuiTestEngine* engine, ImGuiCaptureArgs* args)\n{",
        concat!(
            "bool ImGuiTestEngine_CaptureBeginVideo(ImGuiTestEngine* engine, ImGuiCaptureArgs* args)\n",
            "{\n",
            "    if (dear_imgui_rs_test_engine_capture_should_abort(engine))\n",
            "        return false;"
        ),
        "CaptureBeginVideo cancellation entry",
    )?;
    patched = replace_cpp_source_once(
        &patched,
        concat!(
            "bool ImGuiTestEngine_CaptureBeginVideo(ImGuiTestEngine* engine, ImGuiCaptureArgs* args)\n",
            "{\n",
            "    if (dear_imgui_rs_test_engine_capture_should_abort(engine))\n",
            "        return false;\n",
            "    if (engine->IO.ScreenCaptureFunc == nullptr)\n",
            "    {\n",
            "        IM_ASSERT(0);\n",
            "        return false;\n",
            "    }\n\n",
            "    IM_ASSERT(engine->CaptureCurrentArgs == nullptr && \"Nested captures are not supported.\");"
        ),
        concat!(
            "bool ImGuiTestEngine_CaptureBeginVideo(ImGuiTestEngine* engine, ImGuiCaptureArgs* args)\n",
            "{\n",
            "    if (dear_imgui_rs_test_engine_capture_should_abort(engine))\n",
            "        return false;\n",
            "    if (engine->IO.ScreenCaptureFunc == nullptr)\n",
            "        return false;\n\n",
            "    IM_ASSERT(engine->CaptureCurrentArgs == nullptr && \"Nested captures are not supported.\");"
        ),
        "CaptureBeginVideo missing-provider rejection",
    )?;
    patched = replace_cpp_source_once(
        &patched,
        concat!(
            "    engine->BackupConfigRunSpeed = engine->IO.ConfigRunSpeed;\n",
            "    engine->BackupConfigNoThrottle = engine->IO.ConfigNoThrottle;"
        ),
        concat!(
            "    dear_imgui_rs_test_engine_begin_video_config(engine);\n",
            "    engine->BackupConfigRunSpeed = engine->IO.ConfigRunSpeed;\n",
            "    engine->BackupConfigNoThrottle = engine->IO.ConfigNoThrottle;"
        ),
        "CaptureBeginVideo configuration backup",
    )?;
    patched = replace_cpp_source_once(
        &patched,
        "    while (engine->CaptureCurrentArgs != nullptr)   // Wait until last frame is captured and gif is saved.\n        ImGuiTestEngine_Yield(engine);",
        concat!(
            "    while (engine->CaptureCurrentArgs != nullptr &&\n",
            "           !dear_imgui_rs_test_engine_capture_should_abort(engine))\n",
            "        ImGuiTestEngine_Yield(engine);"
        ),
        "CaptureEndVideo wait cancellation",
    )?;
    patched = replace_cpp_source_once(
        &patched,
        concat!(
            "    engine->IO.ConfigRunSpeed = engine->BackupConfigRunSpeed;\n",
            "    engine->IO.ConfigNoThrottle = engine->BackupConfigNoThrottle;\n",
            "    engine->IO.ConfigFixedDeltaTime = 0;\n",
            "    engine->CaptureCurrentArgs = nullptr;\n",
            "    return true;"
        ),
        concat!(
            "    const bool capture_completed =\n",
            "        !dear_imgui_rs_test_engine_capture_should_abort(engine) &&\n",
            "        !dear_imgui_rs_test_engine_take_capture_provider_failure(engine);\n",
            "    dear_imgui_rs_test_engine_restore_video_config(engine);\n",
            "    engine->CaptureCurrentArgs = nullptr;\n",
            "    return capture_completed;"
        ),
        "CaptureEndVideo configuration restore",
    )?;

    Ok(restore_cpp_newlines(patched, newline))
}

/// Patch Test Engine capture geometry so sentinel rectangles are never converted to integers.
pub fn patch_test_engine_capture_cpp_for_defined_geometry(source: &str) -> Result<String, String> {
    let (mut patched, newline) = normalize_cpp_source(source);

    patched = replace_cpp_source_once(
        &patched,
        "    const ImRect clip_rect = viewport_rect;",
        concat!(
            "    if (!instant_capture && _FrameNo < 2)\n",
            "    {\n",
            "        _FrameNo++;\n",
            "        return ImGuiCaptureStatus_InProgress;\n",
            "    }\n",
            "\n",
            "    const ImRect clip_rect = viewport_rect;"
        ),
        "Test Engine capture geometry readiness transition",
    )?;

    Ok(restore_cpp_newlines(patched, newline))
}

/// Patch Test Engine Context capture configuration so cancellation restores it synchronously.
pub fn patch_test_engine_context_cpp_for_presentation_abort(
    source: &str,
) -> Result<String, String> {
    let (mut patched, newline) = normalize_cpp_source(source);

    patched = replace_cpp_source_once(
        &patched,
        "#include \"imgui_te_internal.h\"",
        concat!(
            "#include \"imgui_te_internal.h\"\n",
            "#include \"cimgui_test_engine_capture_bridge.h\""
        ),
        "Test Engine Context capture bridge include",
    )?;
    patched = replace_cpp_source_once(
        &patched,
        concat!(
            "    bool backup_io_config_move_window_from_title_bar_only = io.ConfigWindowsMoveFromTitleBarOnly;\n",
            "    if (capture_flags & ImGuiCaptureFlags_StitchAll)"
        ),
        concat!(
            "    bool backup_io_config_move_window_from_title_bar_only = io.ConfigWindowsMoveFromTitleBarOnly;\n",
            "    dear_imgui_rs_test_engine_begin_window_move_config(\n",
            "        Engine, backup_io_config_move_window_from_title_bar_only);\n",
            "    if (capture_flags & ImGuiCaptureFlags_StitchAll)"
        ),
        "CaptureScreenshot window-move configuration backup",
    )?;
    patched = replace_cpp_source_once(
        &patched,
        concat!(
            "    if (capture_flags & ImGuiCaptureFlags_StitchAll)\n",
            "        io.ConfigWindowsMoveFromTitleBarOnly = backup_io_config_move_window_from_title_bar_only;\n\n",
            "    return ret;"
        ),
        concat!(
            "    dear_imgui_rs_test_engine_restore_window_move_config(Engine);\n\n",
            "    return ret;"
        ),
        "CaptureScreenshot window-move configuration restore",
    )?;

    Ok(restore_cpp_newlines(patched, newline))
}

/// Patch `imgui.cpp` so the metrics window can omit the destructive font-atlas panel.
pub fn patch_imgui_cpp_for_safe_demo(source: &str) -> Result<String, String> {
    let (mut patched, newline) = normalize_cpp_source(source);

    patched = replace_cpp_source_once(
        &patched,
        "void ImGui::ShowMetricsWindow(bool* p_open)\n{",
        concat!(
            "static void DearImGuiRsShowMetricsWindowInternal(bool* p_open, bool show_font_atlas)\n",
            "{\n",
            "    using namespace ImGui;"
        ),
        "ShowMetricsWindow definition",
    )?;

    patched = replace_cpp_source_once(
        &patched,
        concat!(
            "    // Details for Fonts\n",
            "    for (ImFontAtlas* atlas : g.FontAtlases)\n",
            "        if (TreeNode((void*)atlas, \"Fonts (%d), Textures (%d)\", atlas->Fonts.Size, atlas->TexList.Size))\n",
            "        {\n",
            "            ShowFontAtlas(atlas);\n",
            "            TreePop();\n",
            "        }"
        ),
        concat!(
            "    // Details for Fonts\n",
            "    if (show_font_atlas)\n",
            "    {\n",
            "        for (ImFontAtlas* atlas : g.FontAtlases)\n",
            "            if (TreeNode((void*)atlas, \"Fonts (%d), Textures (%d)\", atlas->Fonts.Size, atlas->TexList.Size))\n",
            "            {\n",
            "                ShowFontAtlas(atlas);\n",
            "                TreePop();\n",
            "            }\n",
            "    }"
        ),
        "ShowMetricsWindow font-atlas section",
    )?;

    patched = replace_cpp_source_once(
        &patched,
        "    End();\n}\n\nvoid ImGui::DebugBreakClearData()",
        concat!(
            "    End();\n",
            "}\n\n",
            "void ImGui::ShowMetricsWindow(bool* p_open)\n",
            "{\n",
            "    ::DearImGuiRsShowMetricsWindowInternal(p_open, true);\n",
            "}\n\n",
            "void DearImGuiRsShowMetricsWindowWithoutFontAtlas(bool* p_open)\n",
            "{\n",
            "    DearImGuiRsShowMetricsWindowInternal(p_open, false);\n",
            "}\n\n",
            "void ImGui::DebugBreakClearData()"
        ),
        "ShowMetricsWindow wrapper boundary",
    )?;

    patched = replace_cpp_source_once(
        &patched,
        "void ImGui::ShowMetricsWindow(bool*) {}",
        concat!(
            "void ImGui::ShowMetricsWindow(bool*) {}\n",
            "void DearImGuiRsShowMetricsWindowWithoutFontAtlas(bool*) {}"
        ),
        "disabled ShowMetricsWindow stub",
    )?;

    Ok(restore_cpp_newlines(patched, newline))
}

/// Patch `imgui_demo.cpp` so demo and style windows can omit font-atlas debug controls.
pub fn patch_imgui_demo_cpp_for_safe_demo(source: &str) -> Result<String, String> {
    let (mut patched, newline) = normalize_cpp_source(source);

    patched = replace_cpp_source_once(
        &patched,
        "void ImGui::ShowDemoWindow(bool* p_open)\n{",
        concat!(
            "void DearImGuiRsShowMetricsWindowWithoutFontAtlas(bool* p_open);\n",
            "void DearImGuiRsShowStyleEditorWithoutFontAtlas(ImGuiStyle* ref);\n\n",
            "static void DearImGuiRsShowDemoWindowInternal(bool* p_open, bool show_font_atlas)\n",
            "{\n",
            "    using namespace ImGui;"
        ),
        "ShowDemoWindow definition",
    )?;

    patched = replace_cpp_source_once(
        &patched,
        "    if (demo_data.ShowMetrics)              { ImGui::ShowMetricsWindow(&demo_data.ShowMetrics); }",
        concat!(
            "    if (demo_data.ShowMetrics)\n",
            "    {\n",
            "        if (show_font_atlas)\n",
            "            ImGui::ShowMetricsWindow(&demo_data.ShowMetrics);\n",
            "        else\n",
            "            DearImGuiRsShowMetricsWindowWithoutFontAtlas(&demo_data.ShowMetrics);\n",
            "    }"
        ),
        "ShowDemoWindow metrics call",
    )?;

    patched = replace_cpp_source_once(
        &patched,
        concat!(
            "    if (demo_data.ShowStyleEditor)\n",
            "    {\n",
            "        ImGui::Begin(\"Dear ImGui Style Editor\", &demo_data.ShowStyleEditor);\n",
            "        ImGui::ShowStyleEditor();\n",
            "        ImGui::End();\n",
            "    }"
        ),
        concat!(
            "    if (demo_data.ShowStyleEditor)\n",
            "    {\n",
            "        ImGui::Begin(\"Dear ImGui Style Editor\", &demo_data.ShowStyleEditor);\n",
            "        if (show_font_atlas)\n",
            "            ImGui::ShowStyleEditor();\n",
            "        else\n",
            "            DearImGuiRsShowStyleEditorWithoutFontAtlas(NULL);\n",
            "        ImGui::End();\n",
            "    }"
        ),
        "ShowDemoWindow style-editor call",
    )?;

    patched = replace_cpp_source_once(
        &patched,
        "static void DemoWindowWidgets(ImGuiDemoWindowData* demo_data);",
        "static void DemoWindowWidgets(ImGuiDemoWindowData* demo_data, bool show_font_atlas);",
        "DemoWindowWidgets declaration",
    )?;
    patched = replace_cpp_source_once(
        &patched,
        "    DemoWindowWidgets(&demo_data);",
        "    DemoWindowWidgets(&demo_data, show_font_atlas);",
        "DemoWindowWidgets call",
    )?;
    patched = replace_cpp_source_once(
        &patched,
        "static void DemoWindowWidgets(ImGuiDemoWindowData* demo_data)\n{",
        "static void DemoWindowWidgets(ImGuiDemoWindowData* demo_data, bool show_font_atlas)\n{",
        "DemoWindowWidgets definition",
    )?;
    patched = replace_cpp_source_once(
        &patched,
        "    DemoWindowWidgetsFonts();",
        "    if (show_font_atlas)\n        DemoWindowWidgetsFonts();",
        "DemoWindowWidgets font-atlas call",
    )?;

    patched = replace_cpp_source_once(
        &patched,
        concat!(
            "    ImGui::PopItemWidth();\n",
            "    ImGui::End();\n",
            "}\n\n",
            "//-----------------------------------------------------------------------------\n",
            "// [SECTION] DemoWindowMenuBar()"
        ),
        concat!(
            "    ImGui::PopItemWidth();\n",
            "    ImGui::End();\n",
            "}\n\n",
            "void ImGui::ShowDemoWindow(bool* p_open)\n",
            "{\n",
            "    ::DearImGuiRsShowDemoWindowInternal(p_open, true);\n",
            "}\n\n",
            "void DearImGuiRsShowDemoWindowWithoutFontAtlas(bool* p_open)\n",
            "{\n",
            "    DearImGuiRsShowDemoWindowInternal(p_open, false);\n",
            "}\n\n",
            "//-----------------------------------------------------------------------------\n",
            "// [SECTION] DemoWindowMenuBar()"
        ),
        "ShowDemoWindow wrapper boundary",
    )?;

    patched = replace_cpp_source_once(
        &patched,
        "void ImGui::ShowStyleEditor(ImGuiStyle* ref)\n{",
        concat!(
            "static void DearImGuiRsShowStyleEditorInternal(ImGuiStyle* ref, bool show_font_atlas)\n",
            "{\n",
            "    using namespace ImGui;"
        ),
        "ShowStyleEditor definition",
    )?;
    patched = replace_cpp_source_once(
        &patched,
        "        if (BeginTabItem(\"Fonts\"))",
        "        if (show_font_atlas && BeginTabItem(\"Fonts\"))",
        "ShowStyleEditor font-atlas tab",
    )?;
    patched = replace_cpp_source_once(
        &patched,
        concat!(
            "    PopItemWidth();\n",
            "}\n\n",
            "//-----------------------------------------------------------------------------\n",
            "// [SECTION] User Guide / ShowUserGuide()"
        ),
        concat!(
            "    PopItemWidth();\n",
            "}\n\n",
            "void ImGui::ShowStyleEditor(ImGuiStyle* ref)\n",
            "{\n",
            "    ::DearImGuiRsShowStyleEditorInternal(ref, true);\n",
            "}\n\n",
            "void DearImGuiRsShowStyleEditorWithoutFontAtlas(ImGuiStyle* ref)\n",
            "{\n",
            "    DearImGuiRsShowStyleEditorInternal(ref, false);\n",
            "}\n\n",
            "//-----------------------------------------------------------------------------\n",
            "// [SECTION] User Guide / ShowUserGuide()"
        ),
        "ShowStyleEditor wrapper boundary",
    )?;

    patched = replace_cpp_source_once(
        &patched,
        "void ImGui::ShowDemoWindow(bool*) {}",
        concat!(
            "void ImGui::ShowDemoWindow(bool*) {}\n",
            "void DearImGuiRsShowDemoWindowWithoutFontAtlas(bool*) {}"
        ),
        "disabled ShowDemoWindow stub",
    )?;
    patched = replace_cpp_source_once(
        &patched,
        "void ImGui::ShowStyleEditor(ImGuiStyle*) {}",
        concat!(
            "void ImGui::ShowStyleEditor(ImGuiStyle*) {}\n",
            "void DearImGuiRsShowStyleEditorWithoutFontAtlas(ImGuiStyle*) {}"
        ),
        "disabled ShowStyleEditor stub",
    )?;

    Ok(restore_cpp_newlines(patched, newline))
}

/// Patch Dear ImGui's numeric formatting and text parsing paths so safe Rust
/// wrappers isolate the ASCII numeric directive from UTF-8 decorations, never
/// depend on mismatched C varargs, and avoid out-of-range `sscanf` conversions.
///
/// The checked-in upstream source remains untouched. Exact source markers make
/// this transform fail closed whenever Dear ImGui changes either implementation,
/// so every upstream upgrade must review and deliberately refresh the patch.
pub fn patch_imgui_widgets_cpp_for_defined_numeric_conversions(
    source: &str,
) -> Result<String, String> {
    let (mut patched, newline) = normalize_cpp_source(source);

    patched = replace_cpp_source_once(
        &patched,
        "#include <stdint.h>     // intptr_t",
        concat!(
            "#include <stdint.h>     // intptr_t\n",
            "#include <errno.h>      // errno, ERANGE\n",
            "#include <float.h>      // FLT_MAX, DBL_MAX\n",
            "#include <inttypes.h>   // intmax_t, uintmax_t, strtoimax, strtoumax\n",
            "#include <stdlib.h>     // strtof, strtod"
        ),
        "imgui_widgets numeric conversion includes",
    )?;

    patched = replace_cpp_source_once(
        &patched,
        r#"int ImGui::DataTypeFormatString(char* buf, int buf_size, ImGuiDataType data_type, const void* p_data, const char* format)
{
    // Signedness doesn't matter when pushing integer arguments
    if (data_type == ImGuiDataType_S32 || data_type == ImGuiDataType_U32)
        return ImFormatString(buf, buf_size, format, *(const ImU32*)p_data);
    if (data_type == ImGuiDataType_S64 || data_type == ImGuiDataType_U64)
        return ImFormatString(buf, buf_size, format, *(const ImU64*)p_data);
    if (data_type == ImGuiDataType_Float)
        return ImFormatString(buf, buf_size, format, *(const float*)p_data);
    if (data_type == ImGuiDataType_Double)
        return ImFormatString(buf, buf_size, format, *(const double*)p_data);
    if (data_type == ImGuiDataType_S8)
        return ImFormatString(buf, buf_size, format, *(const ImS8*)p_data);
    if (data_type == ImGuiDataType_U8)
        return ImFormatString(buf, buf_size, format, *(const ImU8*)p_data);
    if (data_type == ImGuiDataType_S16)
        return ImFormatString(buf, buf_size, format, *(const ImS16*)p_data);
    if (data_type == ImGuiDataType_U16)
        return ImFormatString(buf, buf_size, format, *(const ImU16*)p_data);
    IM_ASSERT(0);
    return 0;
}"#,
        r#"struct DearImguiRsDataTypeTextOutput
{
    char* Buf;
    int Capacity;
    int Written;
    int Required;
};

static void DearImguiRsDataTypeAppendByte(DearImguiRsDataTypeTextOutput& output, char value)
{
    if (output.Buf != NULL && output.Written + 1 < output.Capacity)
        output.Buf[output.Written++] = value;
    if (output.Required < INT_MAX)
        output.Required++;
}

static void DearImguiRsDataTypeAppendBytes(DearImguiRsDataTypeTextOutput& output, const char* begin, const char* end)
{
    while (begin < end)
        DearImguiRsDataTypeAppendByte(output, *begin++);
}

static void DearImguiRsDataTypeAppendFormatDecoration(DearImguiRsDataTypeTextOutput& output, const char* format, const char* format_end)
{
    while (format < format_end)
    {
        char value = *format++;
        if (value == '%' && format < format_end && *format == '%')
            format++;
        DearImguiRsDataTypeAppendByte(output, value);
    }
}

static int DearImguiRsDataTypeFinishTextOutput(DearImguiRsDataTypeTextOutput& output)
{
    if (output.Buf != NULL && output.Capacity > 0)
        output.Buf[output.Written] = 0;
    return output.Buf != NULL ? output.Written : output.Required;
}

static int DearImguiRsDataTypeFormatDirective(char* buf, int buf_size, ImGuiDataType data_type, const void* p_data, const char* format_value)
{
    // Pass the exact promoted type required by each printf conversion. A signedness
    // mismatch in a variadic call is undefined behavior even when the bit pattern
    // and register representation happen to be identical on a given ABI.
    if (data_type == ImGuiDataType_S32)
        return ImFormatString(buf, buf_size, format_value, *(const ImS32*)p_data);
    if (data_type == ImGuiDataType_U32)
        return ImFormatString(buf, buf_size, format_value, *(const ImU32*)p_data);
    if (data_type == ImGuiDataType_S64)
        return ImFormatString(buf, buf_size, format_value, *(const ImS64*)p_data);
    if (data_type == ImGuiDataType_U64)
        return ImFormatString(buf, buf_size, format_value, *(const ImU64*)p_data);
    if (data_type == ImGuiDataType_Float)
        return ImFormatString(buf, buf_size, format_value, (double)*(const float*)p_data);
    if (data_type == ImGuiDataType_Double)
        return ImFormatString(buf, buf_size, format_value, *(const double*)p_data);
    if (data_type == ImGuiDataType_S8)
        return ImFormatString(buf, buf_size, format_value, (int)*(const ImS8*)p_data);
    if (data_type == ImGuiDataType_U8)
        return ImFormatString(buf, buf_size, format_value, (unsigned int)*(const ImU8*)p_data);
    if (data_type == ImGuiDataType_S16)
        return ImFormatString(buf, buf_size, format_value, (int)*(const ImS16*)p_data);
    if (data_type == ImGuiDataType_U16)
        return ImFormatString(buf, buf_size, format_value, (unsigned int)*(const ImU16*)p_data);
    IM_ASSERT(0);
    return 0;
}

int ImGui::DataTypeFormatString(char* buf, int buf_size, ImGuiDataType data_type, const void* p_data, const char* format)
{
    const ImGuiDataTypeInfo* type_info = DataTypeGetInfo(data_type);
    if (format == NULL)
        format = type_info->PrintFmt;

    DearImguiRsDataTypeTextOutput output = { buf, buf_size > 0 ? buf_size : 0, 0, 0 };
    const char* format_value_start = ImParseFormatFindStart(format);
    if (format_value_start[0] != '%')
    {
        DearImguiRsDataTypeAppendFormatDecoration(output, format, format_value_start);
        return DearImguiRsDataTypeFinishTextOutput(output);
    }

    const char* format_value_end = ImParseFormatFindEnd(format_value_start);
    const size_t format_value_size = (size_t)(format_value_end - format_value_start);
    if (format_value_size < 2 || format_value_size >= 32 || format_value_end[-1] == '%')
    {
        IM_ASSERT(0 && "invalid numeric format directive");
        return DearImguiRsDataTypeFinishTextOutput(output);
    }

    char format_value[32];
    for (size_t index = 0; index < format_value_size; index++)
    {
        const unsigned char byte = (unsigned char)format_value_start[index];
        if (byte >= 0x80)
        {
            IM_ASSERT(0 && "numeric format directive must be ASCII");
            return DearImguiRsDataTypeFinishTextOutput(output);
        }
        format_value[index] = (char)byte;
    }
    format_value[format_value_size] = 0;

    // NumericFormat limits width to 31 and precision to 99. The worst supported
    // fixed representation is 309 integer digits, a sign, a decimal point, and
    // 99 fractional digits, so 512 bytes also leaves room for the terminator.
    char numeric_text[512];
    const int numeric_text_size = DearImguiRsDataTypeFormatDirective(numeric_text, IM_COUNTOF(numeric_text), data_type, p_data, format_value);

    DearImguiRsDataTypeAppendFormatDecoration(output, format, format_value_start);
    DearImguiRsDataTypeAppendBytes(output, numeric_text, numeric_text + numeric_text_size);
    DearImguiRsDataTypeAppendFormatDecoration(output, format_value_end, format + strlen(format));
    return DearImguiRsDataTypeFinishTextOutput(output);
}"#,
        "DataTypeFormatString implementation",
    )?;

    patched = replace_cpp_source_once(
        &patched,
        r#"// User can input math operators (e.g. +100) to edit a numerical values.
// NB: This is _not_ a full expression evaluator. We should probably add one and replace this dumb mess..
bool ImGui::DataTypeApplyFromText(const char* buf, ImGuiDataType data_type, void* p_data, const char* format, void* p_data_when_empty)
{
    // Copy the value in an opaque buffer so we can compare at the end of the function if it changed at all.
    const ImGuiDataTypeInfo* type_info = DataTypeGetInfo(data_type);
    ImGuiDataTypeStorage data_backup;
    memcpy(&data_backup, p_data, type_info->Size);

    while (ImCharIsBlankA(*buf))
        buf++;
    if (!buf[0])
    {
        if (p_data_when_empty != NULL)
        {
            memcpy(p_data, p_data_when_empty, type_info->Size);
            return memcmp(&data_backup, p_data, type_info->Size) != 0;
        }
        return false;
    }

    // Sanitize format
    // - For float/double we have to ignore format with precision (e.g. "%.2f") because sscanf doesn't take them in, so force them into %f and %lf
    char format_sanitized[32];
    if (data_type == ImGuiDataType_Float || data_type == ImGuiDataType_Double)
    {
        format = type_info->ScanFmt;
    }
    else
    {
        format = ImParseFormatSanitizeForScanning(format, format_sanitized, IM_COUNTOF(format_sanitized));
        if (format[0] == '\0')
            format = type_info->ScanFmt; // Format doesn't want us to show the number currently, but we still need to parse the resulting input
    }

    // Small types need a 32-bit buffer to receive the result from scanf()
    int v32 = 0;
    if (sscanf(buf, format, type_info->Size >= 4 ? p_data : &v32) < 1)
        return false;
    if (type_info->Size < 4)
    {
        if (data_type == ImGuiDataType_S8)
            *(ImS8*)p_data = (ImS8)ImClamp(v32, (int)IM_S8_MIN, (int)IM_S8_MAX);
        else if (data_type == ImGuiDataType_U8)
            *(ImU8*)p_data = (ImU8)ImClamp(v32, (int)IM_U8_MIN, (int)IM_U8_MAX);
        else if (data_type == ImGuiDataType_S16)
            *(ImS16*)p_data = (ImS16)ImClamp(v32, (int)IM_S16_MIN, (int)IM_S16_MAX);
        else if (data_type == ImGuiDataType_U16)
            *(ImU16*)p_data = (ImU16)ImClamp(v32, (int)IM_U16_MIN, (int)IM_U16_MAX);
        else
            IM_ASSERT(0);
    }

    return memcmp(&data_backup, p_data, type_info->Size) != 0;
}"#,
        r#"static bool DearImguiRsDataTypeMatchFormatDecoration(const char*& input, const char* format, const char* format_end)
{
    while (format[0] != 0 && (format_end == NULL || format < format_end))
    {
        char expected = *format++;
        if (expected == '%' && format[0] == '%' && (format_end == NULL || format < format_end))
        {
            expected = '%';
            format++;
        }
        if (*input != expected)
            return false;
        input++;
    }
    return true;
}

static size_t DearImguiRsDataTypeDecodedDecorationSize(const char* format)
{
    size_t size = 0;
    while (*format != 0)
    {
        if (*format++ == '%' && *format == '%')
            format++;
        size++;
    }
    return size;
}

static bool DearImguiRsDataTypeMatchFormatSuffixAtEnd(const char* input, const char* input_end, const char* format_suffix, const char** out_numeric_end)
{
    const size_t suffix_size = DearImguiRsDataTypeDecodedDecorationSize(format_suffix);
    if ((size_t)(input_end - input) < suffix_size)
        return false;

    const char* suffix_input = input_end - suffix_size;
    const char* suffix_input_end = suffix_input;
    if (!DearImguiRsDataTypeMatchFormatDecoration(suffix_input_end, format_suffix, NULL) || suffix_input_end != input_end)
        return false;
    *out_numeric_end = suffix_input;
    return true;
}

static bool DearImguiRsDataTypeStripFormatSuffix(const char* input, const char* format_suffix, const char** out_numeric_end)
{
    const char* input_end = input + strlen(input);
    if (format_suffix == NULL)
    {
        while (input_end > input && ImCharIsBlankA(input_end[-1]))
            input_end--;
        *out_numeric_end = input_end;
        return true;
    }

    // Match the decoded suffix before parsing the number. Otherwise suffixes
    // such as "123", "F", or "e3" are valid numeric characters and strto*()
    // would consume them as part of the value.
    for (;;)
    {
        if (DearImguiRsDataTypeMatchFormatSuffixAtEnd(input, input_end, format_suffix, out_numeric_end))
            return true;
        if (input_end == input || !ImCharIsBlankA(input_end[-1]))
            return false;
        input_end--;
    }
}

static bool DearImguiRsDataTypeFinishNumericParse(const char* input)
{
    while (ImCharIsBlankA(*input))
        input++;
    return *input == 0;
}

static void DearImguiRsDataTypeStoreSignedInteger(ImGuiDataType data_type, intmax_t value, void* p_data)
{
    if (data_type == ImGuiDataType_S8)
        *(ImS8*)p_data = (ImS8)ImClamp(value, (intmax_t)IM_S8_MIN, (intmax_t)IM_S8_MAX);
    else if (data_type == ImGuiDataType_S16)
        *(ImS16*)p_data = (ImS16)ImClamp(value, (intmax_t)IM_S16_MIN, (intmax_t)IM_S16_MAX);
    else if (data_type == ImGuiDataType_S32)
        *(ImS32*)p_data = (ImS32)ImClamp(value, (intmax_t)IM_S32_MIN, (intmax_t)IM_S32_MAX);
    else if (data_type == ImGuiDataType_S64)
        *(ImS64*)p_data = (ImS64)ImClamp(value, (intmax_t)IM_S64_MIN, (intmax_t)IM_S64_MAX);
    else
        IM_ASSERT(0);
}

static void DearImguiRsDataTypeStoreUnsignedInteger(ImGuiDataType data_type, uintmax_t value, void* p_data)
{
    if (data_type == ImGuiDataType_U8)
        *(ImU8*)p_data = (ImU8)ImMin(value, (uintmax_t)IM_U8_MAX);
    else if (data_type == ImGuiDataType_U16)
        *(ImU16*)p_data = (ImU16)ImMin(value, (uintmax_t)IM_U16_MAX);
    else if (data_type == ImGuiDataType_U32)
        *(ImU32*)p_data = (ImU32)ImMin(value, (uintmax_t)IM_U32_MAX);
    else if (data_type == ImGuiDataType_U64)
        *(ImU64*)p_data = (ImU64)ImMin(value, (uintmax_t)IM_U64_MAX);
    else
        IM_ASSERT(0);
}

static bool DearImguiRsDataTypeIsSignedInteger(ImGuiDataType data_type)
{
    return data_type == ImGuiDataType_S8 || data_type == ImGuiDataType_S16 || data_type == ImGuiDataType_S32 || data_type == ImGuiDataType_S64;
}

static bool DearImguiRsDataTypeIsUnsignedInteger(ImGuiDataType data_type)
{
    return data_type == ImGuiDataType_U8 || data_type == ImGuiDataType_U16 || data_type == ImGuiDataType_U32 || data_type == ImGuiDataType_U64;
}

// User can input a leading '+' to edit numerical values. Parsing deliberately
// rejects malformed input and negative unsigned values. Integer overflow clamps
// to the destination type; floating overflow clamps to its finite limit, finite
// underflow keeps the value returned by strtof()/strtod(), and NaN is rejected.
bool ImGui::DataTypeApplyFromText(const char* buf, ImGuiDataType data_type, void* p_data, const char* format, void* p_data_when_empty)
{
    // Copy the value in an opaque buffer so failed parses leave the destination untouched.
    const ImGuiDataTypeInfo* type_info = DataTypeGetInfo(data_type);
    ImGuiDataTypeStorage data_backup;
    memcpy(&data_backup, p_data, type_info->Size);

    const char* input_trimmed = buf;
    while (ImCharIsBlankA(*input_trimmed))
        input_trimmed++;
    if (!input_trimmed[0])
    {
        if (p_data_when_empty != NULL)
        {
            memcpy(p_data, p_data_when_empty, type_info->Size);
            return memcmp(&data_backup, p_data, type_info->Size) != 0;
        }
        return false;
    }

    if (format == NULL)
        format = type_info->PrintFmt;
    const char* format_value_start = ImParseFormatFindStart(format);
    const bool format_has_value = format_value_start[0] == '%';
    const char* format_value_end = format_has_value ? ImParseFormatFindEnd(format_value_start) : format_value_start;
    if (format_has_value && (format_value_end <= format_value_start + 1 || format_value_end[-1] == '%'))
        return false;

    const char* input = format_has_value && format_value_start != format ? buf : input_trimmed;
    if (format_has_value && !DearImguiRsDataTypeMatchFormatDecoration(input, format, format_value_start))
        return false;
    while (ImCharIsBlankA(*input))
        input++;

    char conversion = 0;
    if (format_has_value)
        conversion = format_value_end[-1];
    else if (DearImguiRsDataTypeIsSignedInteger(data_type))
        conversion = 'd';
    else if (DearImguiRsDataTypeIsUnsignedInteger(data_type))
        conversion = 'u';
    else if (data_type == ImGuiDataType_Float || data_type == ImGuiDataType_Double)
        conversion = 'f';
    else
        return false;

    const char* format_suffix = format_has_value ? format_value_end : NULL;
    const char* numeric_input_end = NULL;
    if (!DearImguiRsDataTypeStripFormatSuffix(input, format_suffix, &numeric_input_end))
        return false;
    const size_t numeric_input_size = (size_t)(numeric_input_end - input);
    if (numeric_input_size >= 512)
        return false;
    char numeric_input[512];
    memcpy(numeric_input, input, numeric_input_size);
    numeric_input[numeric_input_size] = 0;

    char* input_end = NULL;
    if (DearImguiRsDataTypeIsSignedInteger(data_type))
    {
        const int base = conversion == 'd' ? 10 : conversion == 'i' ? 0 : -1;
        if (base < 0)
            return false;
        errno = 0;
        const intmax_t value = strtoimax(numeric_input, &input_end, base);
        if (input_end == numeric_input || (errno != 0 && errno != ERANGE) || !DearImguiRsDataTypeFinishNumericParse(input_end))
            return false;
        DearImguiRsDataTypeStoreSignedInteger(data_type, value, p_data);
    }
    else if (DearImguiRsDataTypeIsUnsignedInteger(data_type))
    {
        const int base = conversion == 'u' ? 10 : conversion == 'o' ? 8 : (conversion == 'x' || conversion == 'X') ? 16 : -1;
        if (base < 0 || numeric_input[0] == '-')
            return false;
        errno = 0;
        const uintmax_t value = strtoumax(numeric_input, &input_end, base);
        if (input_end == numeric_input || (errno != 0 && errno != ERANGE) || !DearImguiRsDataTypeFinishNumericParse(input_end))
            return false;
        DearImguiRsDataTypeStoreUnsignedInteger(data_type, value, p_data);
    }
    else if (data_type == ImGuiDataType_Float)
    {
        if (conversion != 'e' && conversion != 'E' && conversion != 'f' && conversion != 'F' && conversion != 'g' && conversion != 'G' && conversion != 'a' && conversion != 'A')
            return false;
        errno = 0;
        float value = strtof(numeric_input, &input_end);
        if (input_end == numeric_input || (errno != 0 && errno != ERANGE) || !DearImguiRsDataTypeFinishNumericParse(input_end) || value != value)
            return false;
        if (value > FLT_MAX)
            value = FLT_MAX;
        else if (value < -FLT_MAX)
            value = -FLT_MAX;
        *(float*)p_data = value;
    }
    else if (data_type == ImGuiDataType_Double)
    {
        if (conversion != 'e' && conversion != 'E' && conversion != 'f' && conversion != 'F' && conversion != 'g' && conversion != 'G' && conversion != 'a' && conversion != 'A')
            return false;
        errno = 0;
        double value = strtod(numeric_input, &input_end);
        if (input_end == numeric_input || (errno != 0 && errno != ERANGE) || !DearImguiRsDataTypeFinishNumericParse(input_end) || value != value)
            return false;
        if (value > DBL_MAX)
            value = DBL_MAX;
        else if (value < -DBL_MAX)
            value = -DBL_MAX;
        *(double*)p_data = value;
    }
    else
    {
        return false;
    }

    return memcmp(&data_backup, p_data, type_info->Size) != 0;
}"#,
        "DataTypeApplyFromText implementation",
    )?;

    Ok(restore_cpp_newlines(patched, newline))
}

/// Patch ImNodes persistence to use Dear ImGui's file-handle abstraction.
///
/// Upstream ImNodes currently assigns `ImFileOpen()` to `FILE*` and calls the
/// C runtime directly. That is incompatible with `IMGUI_DISABLE_FILE_FUNCTIONS`,
/// where Dear ImGui intentionally defines `ImFileHandle` as an opaque pointer
/// and provides fail-closed inline file helpers.
pub fn patch_imnodes_cpp_for_file_handle(source: &str) -> Result<String, String> {
    let (mut patched, newline) = normalize_cpp_source(source);

    patched = replace_cpp_source_once(
        &patched,
        concat!(
            "    size_t      data_size = 0u;\n",
            "    const char* data = SaveEditorStateToIniString(editor, &data_size);\n",
            "    FILE*       file = ImFileOpen(file_name, \"wt\");"
        ),
        concat!(
            "    size_t       data_size = 0u;\n",
            "    const char*  data = SaveEditorStateToIniString(editor, &data_size);\n",
            "    ImFileHandle file = ImFileOpen(file_name, \"wt\");"
        ),
        "ImNodes save-state file handle",
    )?;
    patched = replace_cpp_source_once(
        &patched,
        concat!(
            "    fwrite(data, sizeof(char), data_size, file);\n",
            "    fclose(file);"
        ),
        concat!(
            "    ImFileWrite(data, sizeof(char), data_size, file);\n",
            "    ImFileClose(file);"
        ),
        "ImNodes save-state file operations",
    )?;

    Ok(restore_cpp_newlines(patched, newline))
}

fn normalize_cpp_source(source: &str) -> (String, &'static str) {
    let newline = if source.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    (source.replace("\r\n", "\n"), newline)
}

fn restore_cpp_newlines(source: String, newline: &str) -> String {
    if newline == "\r\n" {
        source.replace('\n', "\r\n")
    } else {
        source
    }
}

fn replace_cpp_source_once(
    source: &str,
    marker: &str,
    replacement: &str,
    description: &str,
) -> Result<String, String> {
    let count = source.match_indices(marker).count();
    if count != 1 {
        return Err(format!(
            "{description}: expected exactly one source marker, found {count}"
        ));
    }
    Ok(source.replacen(marker, replacement, 1))
}

#[cfg(test)]
mod tests {
    use super::{
        patch_imgui_cpp_for_safe_demo, patch_imgui_demo_cpp_for_safe_demo,
        patch_imgui_widgets_cpp_for_defined_numeric_conversions, patch_imnodes_cpp_for_file_handle,
        patch_test_engine_capture_cpp_for_defined_geometry,
        patch_test_engine_cpp_for_presentation_abort,
    };

    #[test]
    fn metrics_patch_guards_only_the_font_atlas_section() {
        let source = concat!(
            "void ImGui::ShowMetricsWindow(bool* p_open)\n",
            "{\n",
            "    KeepOrdinaryMetrics();\n",
            "    // Details for Fonts\n",
            "    for (ImFontAtlas* atlas : g.FontAtlases)\n",
            "        if (TreeNode((void*)atlas, \"Fonts (%d), Textures (%d)\", atlas->Fonts.Size, atlas->TexList.Size))\n",
            "        {\n",
            "            ShowFontAtlas(atlas);\n",
            "            TreePop();\n",
            "        }\n",
            "    End();\n",
            "}\n\n",
            "void ImGui::DebugBreakClearData()\n",
            "{\n",
            "}\n\n",
            "void ImGui::ShowMetricsWindow(bool*) {}\n",
        );

        let patched = patch_imgui_cpp_for_safe_demo(source).unwrap();

        assert!(patched.contains("KeepOrdinaryMetrics();"));
        assert!(patched.contains("if (show_font_atlas)"));
        assert!(patched.contains("void ImGui::ShowMetricsWindow(bool* p_open)"));
        assert!(
            patched.contains("void DearImGuiRsShowMetricsWindowWithoutFontAtlas(bool* p_open)")
        );
    }

    #[test]
    fn demo_patch_preserves_non_font_demo_and_style_controls() {
        let source = concat!(
            "static void DemoWindowWidgets(ImGuiDemoWindowData* demo_data);\n",
            "void ImGui::ShowDemoWindow(bool* p_open)\n",
            "{\n",
            "    KeepOrdinaryDemo();\n",
            "    if (demo_data.ShowMetrics)              { ImGui::ShowMetricsWindow(&demo_data.ShowMetrics); }\n",
            "    if (demo_data.ShowStyleEditor)\n",
            "    {\n",
            "        ImGui::Begin(\"Dear ImGui Style Editor\", &demo_data.ShowStyleEditor);\n",
            "        ImGui::ShowStyleEditor();\n",
            "        ImGui::End();\n",
            "    }\n",
            "    DemoWindowWidgets(&demo_data);\n",
            "    ImGui::PopItemWidth();\n",
            "    ImGui::End();\n",
            "}\n\n",
            "//-----------------------------------------------------------------------------\n",
            "// [SECTION] DemoWindowMenuBar()\n",
            "static void DemoWindowWidgets(ImGuiDemoWindowData* demo_data)\n",
            "{\n",
            "    DemoWindowWidgetsFonts();\n",
            "}\n",
            "void ImGui::ShowStyleEditor(ImGuiStyle* ref)\n",
            "{\n",
            "        if (BeginTabItem(\"Colors\"))\n",
            "            KeepColorEditor();\n",
            "        if (BeginTabItem(\"Fonts\"))\n",
            "            ShowFontAtlas(atlas);\n",
            "    PopItemWidth();\n",
            "}\n\n",
            "//-----------------------------------------------------------------------------\n",
            "// [SECTION] User Guide / ShowUserGuide()\n",
            "void ImGui::ShowDemoWindow(bool*) {}\n",
            "void ImGui::ShowStyleEditor(ImGuiStyle*) {}\n",
        );

        let patched = patch_imgui_demo_cpp_for_safe_demo(source).unwrap();

        assert!(patched.contains("KeepOrdinaryDemo();"));
        assert!(patched.contains("KeepColorEditor();"));
        assert!(
            patched.contains(
                "DemoWindowWidgets(ImGuiDemoWindowData* demo_data, bool show_font_atlas)"
            )
        );
        assert!(patched.contains("if (show_font_atlas)\n        DemoWindowWidgetsFonts();"));
        assert!(patched.contains("show_font_atlas && BeginTabItem(\"Fonts\")"));
        assert!(patched.contains("void DearImGuiRsShowDemoWindowWithoutFontAtlas(bool* p_open)"));
        assert!(
            patched.contains("void DearImGuiRsShowStyleEditorWithoutFontAtlas(ImGuiStyle* ref)")
        );
    }

    #[test]
    fn imgui_widgets_numeric_patch_uses_defined_formatting_and_parsing() {
        let source = include_str!(concat!(
            "../../../dear-imgui-sys/third-party/cimgui/imgui/",
            "imgui_widgets.cpp"
        ));
        let source_lf = source.replace("\r\n", "\n");
        let patched = patch_imgui_widgets_cpp_for_defined_numeric_conversions(&source_lf).unwrap();

        assert!(patched.contains("#include <inttypes.h>   // intmax_t, uintmax_t"));
        assert!(patched.contains("format_value, (unsigned int)*(const ImU16*)p_data"));
        assert!(patched.contains("format_value, *(const ImS64*)p_data"));
        assert!(patched.contains("char numeric_text[512];"));
        assert!(patched.contains("numeric format directive must be ASCII"));
        assert!(patched.contains(
            "DearImguiRsDataTypeAppendFormatDecoration(output, format, format_value_start);"
        ));
        assert!(patched.contains(
            "DearImguiRsDataTypeAppendFormatDecoration(output, format_value_end, format + strlen(format));"
        ));
        assert!(!patched.contains("ImFormatString(buf, buf_size, format,"));
        assert!(patched.contains("char numeric_input[512];"));
        assert!(
            patched.contains("const intmax_t value = strtoimax(numeric_input, &input_end, base);")
        );
        assert!(
            patched.contains("const uintmax_t value = strtoumax(numeric_input, &input_end, base);")
        );
        assert!(patched.contains("float value = strtof(numeric_input, &input_end);"));
        assert!(patched.contains("double value = strtod(numeric_input, &input_end);"));
        assert!(patched.contains("!DearImguiRsDataTypeFinishNumericParse(input_end)"));
        assert!(!patched.contains("sscanf(buf, format, type_info->Size >= 4 ? p_data : &v32)"));

        let strip_suffix = patched
            .find("DearImguiRsDataTypeStripFormatSuffix(input, format_suffix")
            .unwrap();
        let parse_integer = patched
            .find("strtoimax(numeric_input, &input_end, base)")
            .unwrap();
        assert!(strip_suffix < parse_integer);
    }

    #[test]
    fn imgui_widgets_numeric_patch_decodes_decorations_and_strips_suffixes_first() {
        fn decode_decoration(format: &str) -> Vec<u8> {
            let bytes = format.as_bytes();
            let mut decoded = Vec::with_capacity(bytes.len());
            let mut offset = 0;
            while offset < bytes.len() {
                decoded.push(bytes[offset]);
                if bytes[offset] == b'%' && bytes.get(offset + 1) == Some(&b'%') {
                    offset += 1;
                }
                offset += 1;
            }
            decoded
        }

        fn strip_decoded_suffix<'a>(input: &'a str, format_suffix: &str) -> Option<&'a str> {
            let decoded = decode_decoration(format_suffix);
            let mut candidate = input.as_bytes();
            loop {
                if let Some(prefix) = candidate.strip_suffix(decoded.as_slice()) {
                    return std::str::from_utf8(prefix).ok();
                }
                match candidate.last() {
                    Some(b' ' | b'\t') => candidate = &candidate[..candidate.len() - 1],
                    _ => return None,
                }
            }
        }

        assert_eq!(decode_decoration("literal %% only"), b"literal % only");
        assert_eq!(
            decode_decoration("UTF-8 前缀 %% 后缀"),
            "UTF-8 前缀 % 后缀".as_bytes()
        );
        assert_eq!(strip_decoded_suffix("7123", "123"), Some("7"));
        assert_eq!(strip_decoded_suffix("10F", "F"), Some("10"));
        assert_eq!(strip_decoded_suffix("1.000000e3", "e3"), Some("1.000000"));
        assert_eq!(strip_decoded_suffix("42%  ", "%%"), Some("42"));
    }

    #[test]
    fn imgui_widgets_numeric_patch_preserves_crlf_inputs() {
        let checked_in_source = include_str!(concat!(
            "../../../dear-imgui-sys/third-party/cimgui/imgui/",
            "imgui_widgets.cpp"
        ));
        let source_lf = checked_in_source.replace("\r\n", "\n");
        let source_crlf = source_lf.replace('\n', "\r\n");
        let patched =
            patch_imgui_widgets_cpp_for_defined_numeric_conversions(&source_crlf).unwrap();

        assert!(patched.contains("#include <errno.h>      // errno, ERANGE\r\n#include <float.h>"));
        assert!(
            patched
                .contains("const intmax_t value = strtoimax(numeric_input, &input_end, base);\r\n")
        );
        assert!(patched.contains(
            "DearImguiRsDataTypeAppendFormatDecoration(output, format_value_end, format + strlen(format));\r\n"
        ));
        assert!(!patched.replace("\r\n", "").contains('\n'));
    }

    #[test]
    fn imgui_widgets_numeric_patch_rejects_upstream_drift() {
        let checked_in_source = include_str!(concat!(
            "../../../dear-imgui-sys/third-party/cimgui/imgui/",
            "imgui_widgets.cpp"
        ));
        let source_lf = checked_in_source.replace("\r\n", "\n");

        let include_drift = source_lf.replacen(
            "#include <stdint.h>     // intptr_t",
            "#include <stdint.h>     // upstream changed include layout",
            1,
        );
        let include_error =
            patch_imgui_widgets_cpp_for_defined_numeric_conversions(&include_drift).unwrap_err();
        assert!(include_error.contains("imgui_widgets numeric conversion includes"));
        assert!(include_error.contains("found 0"));

        let format_drift = source_lf.replacen(
            "    // Signedness doesn't matter when pushing integer arguments",
            "    // Upstream changed numeric formatting",
            1,
        );
        let format_error =
            patch_imgui_widgets_cpp_for_defined_numeric_conversions(&format_drift).unwrap_err();
        assert!(format_error.contains("DataTypeFormatString implementation"));
        assert!(format_error.contains("found 0"));

        let parsing_drift = source_lf.replacen(
            "    // Small types need a 32-bit buffer to receive the result from scanf()",
            "    // Upstream changed numeric parsing",
            1,
        );
        let parsing_error =
            patch_imgui_widgets_cpp_for_defined_numeric_conversions(&parsing_drift).unwrap_err();
        assert!(parsing_error.contains("DataTypeApplyFromText implementation"));
        assert!(parsing_error.contains("found 0"));
    }

    #[test]
    fn imnodes_file_patch_uses_imgui_file_handles_when_file_functions_are_disabled() {
        let source = include_str!(concat!(
            "../../../extensions/dear-imnodes-sys/third-party/",
            "cimnodes/imnodes/imnodes.cpp"
        ));
        let patched = patch_imnodes_cpp_for_file_handle(source).unwrap();

        assert!(patched.contains("ImFileHandle file = ImFileOpen(file_name, \"wt\");"));
        assert!(patched.contains("ImFileWrite(data, sizeof(char), data_size, file);"));
        assert!(patched.contains("ImFileClose(file);"));
        assert!(!patched.contains("FILE*       file = ImFileOpen(file_name, \"wt\");"));
        assert!(!patched.contains("fwrite(data, sizeof(char), data_size, file);"));
        assert!(!patched.contains("fclose(file);"));
    }

    #[test]
    fn imnodes_file_patch_preserves_crlf_inputs() {
        let checked_in_source = include_str!(concat!(
            "../../../extensions/dear-imnodes-sys/third-party/",
            "cimnodes/imnodes/imnodes.cpp"
        ));
        let source_lf = checked_in_source.replace("\r\n", "\n");
        let source_crlf = source_lf.replace('\n', "\r\n");
        let patched = patch_imnodes_cpp_for_file_handle(&source_crlf).unwrap();

        assert!(patched.contains(
            "ImFileWrite(data, sizeof(char), data_size, file);\r\n    ImFileClose(file);"
        ));
        assert!(!patched.replace("\r\n", "").contains('\n'));
    }

    #[test]
    fn test_engine_patch_cancels_every_capture_wait_and_restores_video_config() {
        let source = include_str!(concat!(
            "../../../extensions/dear-imgui-test-engine-sys/third-party/",
            "imgui_test_engine/imgui_test_engine/imgui_te_engine.cpp"
        ));
        let patched = patch_test_engine_cpp_for_presentation_abort(source).unwrap();

        assert!(patched.contains("cimgui_test_engine_capture_bridge.h"));
        assert_eq!(
            patched
                .matches("dear_imgui_rs_test_engine_request_capture_abort(engine)")
                .count(),
            1
        );
        assert!(
            patched
                .matches("dear_imgui_rs_test_engine_capture_should_abort(engine)")
                .count()
                >= 6
        );
        assert!(patched.contains("dear_imgui_rs_test_engine_begin_video_config(engine)"));
        assert!(patched.contains("dear_imgui_rs_test_engine_restore_video_config(engine)"));
        assert!(patched.contains("DearImGuiRsCaptureWaitGuard capture_wait(engine)"));
        assert!(
            patched.contains("if (ctx && !dear_imgui_rs_test_engine_capture_should_abort(engine))")
        );
    }

    #[test]
    fn test_engine_capture_patch_defers_integer_conversion_until_geometry_is_ready() {
        let source = include_str!(concat!(
            "../../../extensions/dear-imgui-test-engine-sys/third-party/",
            "imgui_test_engine/imgui_test_engine/imgui_capture_tool.cpp"
        ));
        let patched = patch_test_engine_capture_cpp_for_defined_geometry(source).unwrap();
        let expected_guard = [
            "if (!instant_capture && _FrameNo < 2)",
            "{",
            "_FrameNo++;",
            "return ImGuiCaptureStatus_InProgress;",
            "}",
            "",
            "const ImRect clip_rect = viewport_rect;",
        ];
        let logical_lines = patched.lines().map(str::trim).collect::<Vec<_>>();

        assert!(
            logical_lines
                .windows(expected_guard.len())
                .any(|lines| { lines == expected_guard.as_slice() })
        );
        assert_eq!(
            patched
                .matches(
                    "const int capture_height = ImMin((int)io.DisplaySize.y, (int)_CaptureRect.GetHeight());"
                )
                .count(),
            1
        );
        assert_eq!(
            patched
                .matches("if (!instant_capture && _FrameNo < 2)")
                .count(),
            1
        );
    }
}
