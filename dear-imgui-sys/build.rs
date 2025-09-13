use std::env;
use std::path::PathBuf;

#[allow(dead_code)]
fn generate_wasm_bindings(
    _imgui_src: &PathBuf,
    out_path: &PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    // For WASM, we'll use the reference bindings as a base and add WASM import module
    let reference_bindings_path = PathBuf::from("src/bindings_reference.rs");

    let base_content = if reference_bindings_path.exists() {
        std::fs::read_to_string(&reference_bindings_path)?
    } else {
        // If reference bindings don't exist, generate minimal WASM bindings
        return generate_minimal_wasm_bindings(out_path);
    };

    // Replace the extern "C" block with WASM import module
    let wasm_import_name =
        env::var("IMGUI_RS_WASM_IMPORT_NAME").unwrap_or_else(|_| "dear-imgui-sys".to_string());

    // Add WASM import module at the beginning
    let wasm_header = format!(
        r#"/* WASM bindings for Dear ImGui - based on reference bindings */

#[allow(nonstandard_style, clippy::all)]

// Override all extern "C" functions with WASM import module
#[link(wasm_import_module = "{}")]
extern "C" {{
"#,
        wasm_import_name
    );

    // Extract function declarations from the base content
    let mut functions = Vec::new();
    let lines: Vec<&str> = base_content.lines().collect();
    let mut in_extern_block = false;

    for line in lines {
        let trimmed = line.trim();
        if trimmed.starts_with("extern \"C\"") {
            in_extern_block = true;
            continue;
        }
        if in_extern_block {
            if trimmed == "}" {
                in_extern_block = false;
                continue;
            }
            if trimmed.starts_with("pub fn") && trimmed.ends_with(";") {
                functions.push(format!("    {}", trimmed));
            }
        }
    }

    // Build the complete WASM bindings
    let mut wasm_bindings = wasm_header;
    for func in functions {
        wasm_bindings.push_str(&func);
        wasm_bindings.push('\n');
    }
    wasm_bindings.push_str("}\n\n");

    // Add the rest of the content (types, constants, etc.) but remove extern blocks
    let mut filtered_content = String::new();
    let lines: Vec<&str> = base_content.lines().collect();
    let mut in_extern_block = false;
    let mut skip_line = false;

    for line in lines {
        let trimmed = line.trim();

        if trimmed.starts_with("extern \"C\"") {
            in_extern_block = true;
            skip_line = true;
            continue;
        }

        if in_extern_block {
            if trimmed == "}" {
                in_extern_block = false;
                skip_line = true;
                continue;
            }
            skip_line = true;
            continue;
        }

        if !skip_line {
            filtered_content.push_str(line);
            filtered_content.push('\n');
        }
        skip_line = false;
    }

    // Combine WASM imports with filtered content
    let final_content = format!("{}{}", wasm_bindings, filtered_content);

    let bindings_path = out_path.join("bindings.rs");
    std::fs::write(&bindings_path, final_content)?;

    println!(
        "cargo:warning=Generated WASM bindings with import module: {}",
        wasm_import_name
    );
    Ok(())
}

fn generate_minimal_wasm_bindings(out_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let wasm_import_name =
        env::var("IMGUI_RS_WASM_IMPORT_NAME").unwrap_or_else(|_| "dear-imgui-sys".to_string());

    let bindings_content = format!(
        r#"/* Minimal WASM bindings for Dear ImGui - automatically generated */

#[allow(nonstandard_style, clippy::all)]

// Basic types
pub type ImGuiContext = ::core::ffi::c_void;
pub type ImFontAtlas = ::core::ffi::c_void;
pub type ImDrawData = ::core::ffi::c_void;
pub type ImGuiIO = ::core::ffi::c_void;
pub type ImGuiStyle = ::core::ffi::c_void;
pub type ImGuiWindowFlags = i32;
pub type ImGuiCond = i32;

// Constants
pub const ImGuiCond_Always: i32 = 1 << 0;
pub const ImGuiCond_Once: i32 = 1 << 1;
pub const ImGuiCond_FirstUseEver: i32 = 1 << 2;
pub const ImGuiCond_Appearing: i32 = 1 << 3;

// Vector types
#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct ImVec2 {{
    pub x: f32,
    pub y: f32,
}}

// Core functions with WASM import module
#[link(wasm_import_module = "{wasm_import_name}")]
extern "C" {{
    pub fn ImGui_CreateContext(shared_font_atlas: *mut ImFontAtlas) -> *mut ImGuiContext;
    pub fn ImGui_DestroyContext(ctx: *mut ImGuiContext);
    pub fn ImGui_GetCurrentContext() -> *mut ImGuiContext;
    pub fn ImGui_SetCurrentContext(ctx: *mut ImGuiContext);
    pub fn ImGui_GetIO() -> *mut ImGuiIO;
    pub fn ImGui_GetStyle() -> *mut ImGuiStyle;
    pub fn ImGui_NewFrame();
    pub fn ImGui_Render();
    pub fn ImGui_GetDrawData() -> *mut ImDrawData;
    pub fn ImGui_ShowDemoWindow(p_open: *mut bool);
    pub fn ImGui_Begin(name: *const ::core::ffi::c_char, p_open: *mut bool, flags: ImGuiWindowFlags) -> bool;
    pub fn ImGui_End();
    pub fn ImGui_Text(fmt: *const ::core::ffi::c_char, ...);
    pub fn ImGui_Button(label: *const ::core::ffi::c_char, size: *const ImVec2) -> bool;
    pub fn ImGui_GetVersion() -> *const ::core::ffi::c_char;
}}
"#,
        wasm_import_name = wasm_import_name
    );

    let bindings_path = out_path.join("bindings.rs");
    std::fs::write(&bindings_path, bindings_content)?;

    println!(
        "cargo:warning=Generated minimal WASM bindings with import module: {}",
        wasm_import_name
    );
    Ok(())
}

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
    let imconfig_content = if target_arch == "wasm32" {
        r#"
// WASM-specific configuration
#define IMGUI_DISABLE_WIN32_DEFAULT_CLIPBOARD_FUNCTIONS
#define IMGUI_DISABLE_DEFAULT_SHELL_FUNCTIONS

// Only use the latest non-obsolete functions
#define IMGUI_DISABLE_OBSOLETE_FUNCTIONS
#define IMGUI_DISABLE_OBSOLETE_KEYIO
// A Rust char is 32-bits, just do that
#define IMGUI_USE_WCHAR32

// For WASM, use a simple global context instead of thread_local
struct ImGuiContext;
extern ImGuiContext* MyImGuiTLS;
#define GImGui MyImGuiTLS
"#
    } else {
        r#"
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
"#
    };

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
    if target_arch == "wasm32" {
        // For WASM, generate proper WASM bindings with import module
        // generate_wasm_bindings(&imgui_src, &out_path).expect("Failed to generate WASM bindings");
    } else {
        // Generate bindings for native targets
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
    }

    // Build ImGui
    // For WASM, we skip C++ compilation as it requires special setup
    // Users should link against a pre-compiled WASM version of ImGui
    if target_arch != "wasm32" {
        let mut build = cc::Build::new();
        build.cpp(true).std("c++17");

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
    } else {
        println!("cargo:warning=WASM target is not supported.");
        // println!("cargo:warning=For WASM, you'll need to provide ImGui rendering through JavaScript or a WASM backend.");
    }

    // Export paths and defines for extension crates (similar to imgui-sys)
    println!("cargo:THIRD_PARTY={}", imgui_src.display());
    println!("cargo:IMGUI_INCLUDE_PATH={}", imgui_src.display());

    // Export common defines that extensions might need
    println!("cargo:DEFINE_IMGUI_DEFINE_MATH_OPERATORS=");
    if target_env == "msvc" {
        println!("cargo:DEFINE_IMGUI_API=__declspec(dllexport)");
    }
}
