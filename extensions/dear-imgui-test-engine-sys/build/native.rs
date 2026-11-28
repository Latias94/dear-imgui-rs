use std::{env, fs};

use build_support::extension_build::{CppProfile, ExtensionBuildContext, patch_cpp_file};

pub fn check(context: &ExtensionBuildContext<'_>, docs_rs: bool) -> Result<(), String> {
    if !docs_rs {
        context
            .sources
            .source_root()
            .map_err(|error| error.to_string())?;
    }
    let engine = context
        .sources
        .file("engine")
        .map_err(|error| error.to_string())?;
    let headers_present = context.imgui.exists() && (!docs_rs || context.cimgui.exists());
    if !headers_present {
        return Err(format!(
            "required Test Engine headers were not found (ImGui: {}, cimgui: {}, engine: {})",
            context.imgui.display(),
            context.cimgui.display(),
            engine.display(),
        ));
    }
    let enabled = env::var("DEP_DEAR_IMGUI_DEFINE_IMGUI_ENABLE_TEST_ENGINE")
        .or_else(|_| env::var("DEP_DEAR_IMGUI_DEFINE_IMGUITEST"));
    if enabled.as_deref() != Ok("1") {
        return Err(
            "requires dear-imgui-sys to be compiled with IMGUI_ENABLE_TEST_ENGINE. \
                    Enable the `test-engine` feature on dear-imgui-sys/dear-imgui-rs."
                .into(),
        );
    }
    Ok(())
}

pub fn build(context: &ExtensionBuildContext<'_>) -> Result<(), String> {
    let file = |id| context.sources.file(id).map_err(|error| error.to_string());
    let engine = file("engine")?;
    let engine_root = engine
        .parent()
        .expect("Test Engine implementation directory");
    let engine_checkout = engine_root
        .parent()
        .expect("Test Engine checkout directory");
    let mut build = context.cpp_build(CppProfile::Standard);

    build.define(
        "IMGUI_TEST_ENGINE_ENABLE_CAPTURE",
        Some(if cfg!(feature = "capture") { "1" } else { "0" }),
    );
    build.define(
        "IMGUI_TEST_ENGINE_ENABLE_COROUTINE_STDTHREAD_IMPL",
        Some("1"),
    );

    // Avoid link-time conflicts and workspace feature-unification footguns:
    // dear-imgui-sys provides the public hook symbols, while this crate builds upstream
    // implementations under renamed identifiers and registers them at runtime.
    build.define(
        "ImGuiTestEngineHook_ItemAdd",
        Some("DearImGuiRs_ImGuiTestEngineHook_ItemAdd_Impl"),
    );
    build.define(
        "ImGuiTestEngineHook_ItemInfo",
        Some("DearImGuiRs_ImGuiTestEngineHook_ItemInfo_Impl"),
    );
    build.define(
        "ImGuiTestEngineHook_Log",
        Some("DearImGuiRs_ImGuiTestEngineHook_Log_Impl"),
    );
    build.define(
        "ImGuiTestEngine_FindItemDebugLabel",
        Some("DearImGuiRs_ImGuiTestEngine_FindItemDebugLabel_Impl"),
    );
    build.define(
        "RegisterTests_Docking",
        Some("DearImGuiRsTestEngine_RegisterTests_Docking"),
    );
    build.define(
        "RegisterTests_Viewports",
        Some("DearImGuiRsTestEngine_RegisterTests_Viewports"),
    );
    for (function, renamed) in [
        (
            "ImGui_ImplNull_Init",
            "DearImGuiRsTestEngine_ImGui_ImplNull_Init",
        ),
        (
            "ImGui_ImplNull_Shutdown",
            "DearImGuiRsTestEngine_ImGui_ImplNull_Shutdown",
        ),
        (
            "ImGui_ImplNull_NewFrame",
            "DearImGuiRsTestEngine_ImGui_ImplNull_NewFrame",
        ),
        (
            "ImGui_ImplNullPlatform_Init",
            "DearImGuiRsTestEngine_ImGui_ImplNullPlatform_Init",
        ),
        (
            "ImGui_ImplNullPlatform_Shutdown",
            "DearImGuiRsTestEngine_ImGui_ImplNullPlatform_Shutdown",
        ),
        (
            "ImGui_ImplNullPlatform_NewFrame",
            "DearImGuiRsTestEngine_ImGui_ImplNullPlatform_NewFrame",
        ),
        (
            "ImGui_ImplNullRender_Init",
            "DearImGuiRsTestEngine_ImGui_ImplNullRender_Init",
        ),
        (
            "ImGui_ImplNullRender_Shutdown",
            "DearImGuiRsTestEngine_ImGui_ImplNullRender_Shutdown",
        ),
        (
            "ImGui_ImplNullRender_NewFrame",
            "DearImGuiRsTestEngine_ImGui_ImplNullRender_NewFrame",
        ),
        (
            "ImGui_ImplNullRender_RenderDrawData",
            "DearImGuiRsTestEngine_ImGui_ImplNullRender_RenderDrawData",
        ),
    ] {
        build.define(function, Some(renamed));
    }

    build
        .include(&context.imgui)
        .include(context.imgui.join("backends"))
        .include(&context.cimgui)
        .include(engine_root)
        .include(engine_checkout)
        .include(engine_root.join("thirdparty"))
        .include(context.sources.crate_root().join("shim"));

    let out = &context.out_dir;
    build.file(patch_cpp_file(
        &file("capture-tool")?,
        out.join("imgui_capture_tool_defined_geometry.cpp"),
        build_support::patch_test_engine_capture_cpp_for_defined_geometry,
    )?);
    build.file(patch_cpp_file(
        &file("context")?,
        out.join("imgui_te_context_presentation_abort.cpp"),
        build_support::patch_test_engine_context_cpp_for_presentation_abort,
    )?);
    build.file(patch_cpp_file(
        &file("coroutine")?,
        out.join("imgui_te_coroutine_exception_safe.cpp"),
        patch_coroutine_for_exception_safe_allocation,
    )?);
    build.file(patch_cpp_file(
        &engine,
        out.join("imgui_te_engine_presentation_abort.cpp"),
        |source| {
            let source = build_support::patch_test_engine_cpp_for_presentation_abort(source)?;
            patch_test_registration_for_exception_safe_append(&source).map_err(str::to_owned)
        },
    )?);
    let internal_header = context.imgui.join("imgui_internal.h");
    let internal_header = fs::read_to_string(&internal_header)
        .map_err(|error| format!("read {}: {error}", internal_header.display()))?;
    build.file(patch_cpp_file(
        &file("upstream-docking-tests")?,
        out.join("imgui_tests_docking.cpp"),
        |source| {
            build_support::patch_test_engine_docking_cpp_for_visible_tab(source, &internal_header)
        },
    )?);
    for id in [
        "exporters",
        "perftool",
        "ui",
        "utils",
        "shim",
        "default-tests",
        "hooks-register",
        "script-tests",
        "upstream-suite-bridge",
        "upstream-viewport-tests",
    ] {
        build.file(file(id)?);
    }
    build.file(context.imgui.join("backends/imgui_impl_null.cpp"));
    context.compile_archive(&mut build, "dear_imgui_test_engine");

    if context.target_os == "windows" && context.target_env == "gnu" && context.target_abi == "llvm"
    {
        // llvm-mingw provides the pthread compatibility archive without linking it implicitly.
        println!("cargo:rustc-link-lib=static:-bundle=pthread");
    }
    Ok(())
}

fn patch_coroutine_for_exception_safe_allocation(source: &str) -> Result<String, &'static str> {
    let mut patched = source.to_owned();
    for (before, after, error) in [
        (
            "#include \"imgui_te_coroutine.h\"",
            "#include \"imgui_te_coroutine.h\"\n#include \"cimgui_test_engine_internal.h\"",
            "missing coroutine header include",
        ),
        (
            "#include <thread>",
            "#include <memory>\n#include <thread>",
            "missing std::thread include",
        ),
        (
            "Coroutine_ImplStdThreadData* data = new Coroutine_ImplStdThreadData();",
            "std::unique_ptr<Coroutine_ImplStdThreadData> data(new Coroutine_ImplStdThreadData());",
            "missing coroutine state allocation",
        ),
        (
            "data->Thread = new std::thread(CoroutineThreadMain, data, func, ctx);",
            "dear_imgui_test_engine_abi::maybe_inject(ImGuiTestEngineExceptionPoint_PostBind);\n    data->Thread = new std::thread(CoroutineThreadMain, data.get(), func, ctx);",
            "missing coroutine thread allocation",
        ),
        (
            "return (ImGuiTestCoroutineHandle)data;",
            "return (ImGuiTestCoroutineHandle)data.release();",
            "missing coroutine handle return",
        ),
    ] {
        if patched.matches(before).count() != 1 {
            return Err(error);
        }
        patched = patched.replacen(before, after, 1);
    }
    Ok(patched)
}

fn patch_test_registration_for_exception_safe_append(source: &str) -> Result<String, &'static str> {
    let before = "    engine->TestsAll.push_back(t);";
    let after = "    try\n    {\n        engine->TestsAll.push_back(t);\n    }\n    catch (...)\n    {\n        IM_DELETE(t);\n        throw;\n    }";
    if source.matches(before).count() != 1 {
        return Err("missing Test Engine registration append");
    }
    Ok(source.replacen(before, after, 1))
}
