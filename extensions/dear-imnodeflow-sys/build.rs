use std::{env, path::PathBuf};

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());

    // Get ImGui source path from dear-imgui-sys environment variables
    let imgui_src = env::var_os("DEP_DEAR_IMGUI_THIRD_PARTY")
        .or_else(|| env::var_os("DEP_DEAR_IMGUI_IMGUI_INCLUDE_PATH"))
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest_dir.join("../../dear-imgui-sys/imgui"));

    let imnodeflow_src = manifest_dir.join("../../repo-ref/ImNodeFlow");

    // Verify sources exist
    if !imgui_src.exists() {
        panic!(
            "ImGui source not found at {:?}. Did you forget to initialize git submodules?",
            imgui_src
        );
    }
    if !imnodeflow_src.exists() {
        panic!(
            "ImNodeFlow source not found at {:?}. Did you forget to initialize git submodules?",
            imnodeflow_src
        );
    }

    let _target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();

    // Generate bindings using bindgen
    let bindings = bindgen::Builder::default()
        .header("wrapper.h")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        // Allow our custom wrapper functions
        .allowlist_function("ImNodeFlow_.*")
        .allowlist_function("BaseNode_.*")
        .allowlist_function("Pin_.*")
        .allowlist_function("Link_.*")
        .allowlist_function("PinStyle_.*")
        .allowlist_function("NodeStyle_.*")
        // Allow basic types
        .allowlist_type("ImNodeFlowPtr")
        .allowlist_type("BaseNodePtr")
        .allowlist_type("PinPtr")
        .allowlist_type("LinkPtr")
        .allowlist_type("PinStylePtr")
        .allowlist_type("NodeStylePtr")
        .allowlist_type("PinUID")
        .allowlist_type("NodeUID")
        // Block ImGui types that we'll re-export from dear-imgui-sys
        .blocklist_type("ImVec2")
        .blocklist_type("ImVec4")
        .blocklist_type("ImGuiCond")
        .blocklist_type("ImTextureID")
        .blocklist_type("ImGuiContext")
        .blocklist_type("ImDrawList")
        .blocklist_type("ImGuiMouseButton")
        .blocklist_type("ImGuiDragDropFlags")
        .blocklist_type("ImGuiIO")
        .blocklist_type("ImFontAtlas")
        .blocklist_type("ImDrawData")
        .blocklist_type("ImGuiStyle")
        .blocklist_type("ImGuiKeyModFlags")
        .blocklist_type("ImGuiKey")
        .blocklist_type("ImU32")
        .blocklist_type("ImColor")
        .derive_default(true)
        .derive_debug(true)
        .derive_copy(true)
        .derive_eq(true)
        .derive_partialeq(true)
        .derive_hash(true)
        .prepend_enum_name(false)
        .layout_tests(false)
        .clang_arg(format!("-I{}", imgui_src.display()))
        .clang_arg(format!("-I{}", imnodeflow_src.join("include").display()))
        .clang_arg(format!("-I{}", imnodeflow_src.join("src").display()))
        .clang_arg("-DIMGUI_DEFINE_MATH_OPERATORS")
        .clang_arg("-x")
        .clang_arg("c++")
        .clang_arg("-std=c++17");

    let bindings = bindings.generate().expect("Unable to generate bindings");

    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");

    // Build ImNodeFlow
    let mut build = cc::Build::new();
    if target_arch == "wasm32" {
        build.define("IMGUI_DISABLE_DEFAULT_SHELL_FUNCTIONS", "1");
    } else {
        build.cpp(true).std("c++17");
    }

    // Take over imgui preprocessor defines from the dear-imgui-sys crate
    env::vars()
        .filter_map(|(key, val)| {
            key.strip_prefix("DEP_DEAR_IMGUI_DEFINE_")
                .map(|suffix| (suffix.to_string(), val.to_string()))
        })
        .for_each(|(key, value)| {
            build.define(&key, value.as_str());
        });

    // Include directories
    build.include(&imgui_src);
    build.include(&imnodeflow_src.join("include"));
    build.include(&imnodeflow_src.join("src"));

    // Add wrapper that includes both ImGui and ImNodeFlow implementations
    build.file(manifest_dir.join("wrapper.cpp"));

    #[cfg(feature = "freetype")]
    if let Ok(freetype) = pkg_config::probe_library("freetype2") {
        build.define("IMGUI_ENABLE_FREETYPE", "1");
        for include in &freetype.include_paths {
            build.include(include.display().to_string());
        }
    }

    // Link against dear-imgui-sys
    println!("cargo:rustc-link-lib=static=dear_imgui");

    build.compile("dear_imnodeflow");

    // Tell cargo to rerun if dear-imgui-sys changes
    println!("cargo:rerun-if-changed=../../dear-imgui-sys");
    println!("cargo:rerun-if-changed=../../repo-ref/ImNodeFlow");
}
