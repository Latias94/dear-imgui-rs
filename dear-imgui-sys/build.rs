use std::env;
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap();

    // ImGui source files to copy
    let imgui_src_files = [
        "imgui.h",
        "imgui_internal.h",
        "imstb_textedit.h",
        "imstb_rectpack.h",
        "imstb_truetype.h",
        "imgui.cpp",
        "imgui_widgets.cpp",
        "imgui_draw.cpp",
        "imgui_tables.cpp",
        "imgui_demo.cpp",
    ];

    let imgui_ori = manifest_dir.join("imgui");
    let imgui_src = out_path.join("imgui_src");
    let imgui_misc_ft = imgui_src.join("misc/freetype");

    // Create output directories if they don't exist
    std::fs::create_dir_all(&imgui_src).unwrap();
    std::fs::create_dir_all(&imgui_misc_ft).unwrap();

    // Helper function to check if file needs copying
    let needs_copy = |src: &std::path::Path, dst: &std::path::Path| -> bool {
        if !dst.exists() {
            return true;
        }
        let src_time = std::fs::metadata(src).unwrap().modified().unwrap();
        let dst_time = std::fs::metadata(dst).unwrap().modified().unwrap();
        src_time > dst_time
    };

    // Copy ImGui source files only if needed
    for ori in imgui_src_files {
        let src = imgui_ori.join(ori);
        let dst = imgui_src.join(ori);
        if needs_copy(&src, &dst) {
            std::fs::copy(&src, &dst).unwrap();
        }
        println!("cargo:rerun-if-changed={}", src.display());
    }

    // Copy freetype files only if needed
    for ori in ["imgui_freetype.cpp", "imgui_freetype.h"] {
        let src = imgui_ori.join("misc/freetype").join(ori);
        let dst = imgui_misc_ft.join(ori);
        if needs_copy(&src, &dst) {
            std::fs::copy(&src, &dst).unwrap();
        }
        println!("cargo:rerun-if-changed={}", src.display());
    }

    // Write custom imconfig.h only if needed
    let imconfig_path = imgui_src.join("imconfig.h");
    let imconfig_content = r#"
// This only works on windows, the arboard crate has better cross-support
#define IMGUI_DISABLE_WIN32_DEFAULT_CLIPBOARD_FUNCTIONS

// Only use the latest non-obsolete functions
#define IMGUI_DISABLE_OBSOLETE_FUNCTIONS
#define IMGUI_DISABLE_OBSOLETE_KEYIO
// A Rust char is 32-bits, just do that
#define IMGUI_USE_WCHAR32

// Try to play thread-safe-ish. The variable definition is in wrapper.cpp
struct ImGuiContext;
extern thread_local ImGuiContext* MyImGuiTLS;
#define GImGui MyImGuiTLS
"#;

    let should_write = if imconfig_path.exists() {
        std::fs::read_to_string(&imconfig_path).unwrap() != imconfig_content
    } else {
        true
    };

    if should_write {
        std::fs::write(&imconfig_path, imconfig_content).unwrap();
    }

    println!("cargo:THIRD_PARTY={}", imgui_src.display());

    // Register wrapper files for rerun detection
    println!("cargo:rerun-if-changed=wrapper.cpp");
    println!("cargo:rerun-if-changed=imgui_msvc_wrapper.cpp");
    println!("cargo:rerun-if-changed=build.rs");

    // Generate bindings

    let mut bindings = bindgen::Builder::default()
        .header(imgui_src.join("imgui.h").to_string_lossy())
        .header(imgui_src.join("imgui_internal.h").to_string_lossy())
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .allowlist_function("ig.*")
        .allowlist_function("Im.*")
        .allowlist_type("Im.*")
        .allowlist_var("Im.*")
        .derive_default(true)
        .derive_debug(true)
        .derive_copy(true)
        .derive_eq(true)
        .derive_partialeq(true)
        .derive_hash(true)
        .prepend_enum_name(false)
        .layout_tests(false)
        .clang_arg(format!("-I{}", imgui_src.display()))
        .clang_arg("-x")
        .clang_arg("c++")
        .clang_arg("-std=c++17");

    if target_env == "msvc" {
        let blocklist_file = manifest_dir.join("msvc_blocklist.txt");
        if let Ok(content) = std::fs::read_to_string(&blocklist_file) {
            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                bindings = bindings.blocklist_function(line);
            }
        }

        let msvc_wrapper_src = manifest_dir.join("imgui_msvc_wrapper.cpp");
        if msvc_wrapper_src.exists() {
            bindings = bindings
                .header(msvc_wrapper_src.to_string_lossy())
                .allowlist_file(msvc_wrapper_src.to_string_lossy());
        }
    }

    #[cfg(feature = "freetype")]
    if let Ok(freetype) = pkg_config::probe_library("freetype2") {
        bindings = bindings.clang_arg("-DIMGUI_ENABLE_FREETYPE=1");
        for include in &freetype.include_paths {
            bindings = bindings.clang_args(["-I", &include.display().to_string()]);
        }
    }

    let bindings = bindings.generate().expect("Unable to generate bindings");

    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");

    // Build ImGui
    let mut build = cc::Build::new();
    if target_arch == "wasm32" {
        build.define("IMGUI_DISABLE_DEFAULT_SHELL_FUNCTIONS", "1");
    } else {
        build.cpp(true).std("c++17");
    }

    build.include(&imgui_src);
    build.file(manifest_dir.join("wrapper.cpp"));

    // Note: wrapper.cpp already includes all ImGui source files via #include
    // So we don't need to add them separately to avoid duplicate symbols

    #[cfg(feature = "freetype")]
    if let Ok(freetype) = pkg_config::probe_library("freetype2") {
        build.define("IMGUI_ENABLE_FREETYPE", "1");
        for include in &freetype.include_paths {
            build.include(include.display().to_string());
        }
        build.file(imgui_misc_ft.join("imgui_freetype.cpp"));
    }

    build.compile("dear_imgui");

    // Export paths and defines for extension crates (similar to imgui-sys)
    println!("cargo:THIRD_PARTY={}", imgui_src.display());
    println!("cargo:IMGUI_INCLUDE_PATH={}", imgui_src.display());

    // Export common defines that extensions might need
    println!("cargo:DEFINE_IMGUI_DEFINE_MATH_OPERATORS=");
    if target_env == "msvc" {
        println!("cargo:DEFINE_IMGUI_API=__declspec(dllexport)");
    }
}
