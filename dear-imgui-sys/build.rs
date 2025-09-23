use std::env;
use std::path::{Path, PathBuf};

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
    let target_triple = env::var("TARGET").unwrap_or_default();

    // Legacy copy of ImGui sources is disabled under cimgui.

    // Register wrapper files for rerun detection
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=IMGUI_SYS_LIB_DIR");
    println!("cargo:rerun-if-env-changed=IMGUI_SYS_SKIP_CC");
    println!("cargo:rerun-if-env-changed=IMGUI_SYS_FORCE_BUILD");
    println!("cargo:rerun-if-env-changed=CARGO_NET_OFFLINE");

    // Special handling for docs.rs: generate bindings only, skip native build/link
    if std::env::var("DOCS_RS").is_ok() {
        println!("cargo:warning=DOCS_RS detected: generating bindings, skipping native build");
        println!("cargo:rustc-cfg=docsrs");

        if use_pregenerated_bindings(&out_path) {
            // done
        } else {
            // Fall back to bindgen from local headers (offline-safe)
            let cimgui_root = manifest_dir.join("third-party/cimgui");
            let imgui_src = cimgui_root.join("imgui");
            let mut bindings = bindgen::Builder::default()
                .header(cimgui_root.join("cimgui.h").to_string_lossy())
                .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
                .clang_arg(format!("-I{}", cimgui_root.display()))
                .clang_arg(format!("-I{}", imgui_src.display()))
                .allowlist_function("ig.*")
                .allowlist_function("Im.*")
                .allowlist_type("Im.*")
                .allowlist_var("Im.*")
                .clang_arg("-DCIMGUI_DEFINE_ENUMS_AND_STRUCTS")
                .derive_default(true)
                .derive_debug(true)
                .derive_copy(true)
                .derive_eq(true)
                .derive_partialeq(true)
                .derive_hash(true)
                .prepend_enum_name(false)
                .layout_tests(false);
            let bindings = bindings
                .generate()
                .expect("Unable to generate bindings from cimgui.h (docs.rs)");
            bindings
                .write_to_file(out_path.join("bindings.rs"))
                .expect("Couldn't write bindings (docs.rs)!");
        }
        // Export include paths for extensions that may rely on them in doc build
        let cimgui_root = manifest_dir.join("third-party/cimgui");
        let imgui_src = cimgui_root.join("imgui");
        println!("cargo:IMGUI_INCLUDE_PATH={}", imgui_src.display());
        println!("cargo:CIMGUI_INCLUDE_PATH={}", cimgui_root.display());
        return;
    }

    // Generate bindings
    // We now always generate bindings from cimgui (C API) for native targets
    // WASM-specific generation is disabled for now.
    {
        // Resolve cimgui paths
        let cimgui_root = manifest_dir.join("third-party/cimgui");
        let imgui_src = cimgui_root.join("imgui");

        // Generate bindings from cimgui.h
        let mut bindings = bindgen::Builder::default()
            .header(cimgui_root.join("cimgui.h").to_string_lossy())
            .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
            .clang_arg(format!("-I{}", cimgui_root.display()))
            .clang_arg(format!("-I{}", imgui_src.display()))
            // Expose C API types and functions
            .allowlist_function("ig.*")
            .allowlist_function("Im.*")
            .allowlist_type("Im.*")
            .allowlist_var("Im.*")
            // cimgui exposes enums/structs when this is defined
            .clang_arg("-DCIMGUI_DEFINE_ENUMS_AND_STRUCTS")
            .derive_default(true)
            .derive_debug(true)
            .derive_copy(true)
            .derive_eq(true)
            .derive_partialeq(true)
            .derive_hash(true)
            .prepend_enum_name(false)
            .layout_tests(false);

        #[cfg(feature = "freetype")]
        if let Ok(freetype) = pkg_config::probe_library("freetype2") {
            bindings = bindings.clang_arg("-DIMGUI_ENABLE_FREETYPE=1");
            for include in &freetype.include_paths {
                bindings = bindings.clang_args(["-I", &include.display().to_string()]);
            }
        }

        let bindings = bindings
            .generate()
            .expect("Unable to generate bindings from cimgui.h");
        bindings
            .write_to_file(out_path.join("bindings.rs"))
            .expect("Couldn't write bindings!");
    }

    // Try to link a prebuilt library first if provided
    let mut linked_prebuilt = false;
    if target_arch != "wasm32" {
        if let Some(lib_dir) = env::var_os("IMGUI_SYS_LIB_DIR") {
            let lib_dir = PathBuf::from(lib_dir);
            if try_link_prebuilt(&lib_dir, &target_env) {
                linked_prebuilt = true;
                println!(
                    "cargo:warning=Using prebuilt dear_imgui from {}",
                    lib_dir.display()
                );
            }
        }
        if !linked_prebuilt {
            if let Some(url) = env::var_os("IMGUI_SYS_PREBUILT_URL") {
                if let Ok(dl_dir) =
                    try_download_prebuilt(&out_path, &url.to_string_lossy(), &target_env)
                {
                    if try_link_prebuilt(&dl_dir, &target_env) {
                        linked_prebuilt = true;
                        println!(
                            "cargo:warning=Downloaded and using prebuilt dear_imgui from {}",
                            dl_dir.display()
                        );
                    }
                }
            }
        }
        if !linked_prebuilt {
            // Also probe repo-local prebuilt folder if present: third-party/prebuilt/<target>
            let repo_prebuilt = manifest_dir
                .join("third-party")
                .join("prebuilt")
                .join(&target_triple);
            if try_link_prebuilt(&repo_prebuilt, &target_env) {
                linked_prebuilt = true;
                println!(
                    "cargo:warning=Using repo prebuilt dear_imgui from {}",
                    repo_prebuilt.display()
                );
            }
        }
    }

    // Build ImGui
    // For WASM, we skip C++ compilation as it requires special setup
    // Users should link against a pre-compiled WASM version of ImGui
    if target_arch != "wasm32" && !linked_prebuilt && env::var("IMGUI_SYS_SKIP_CC").is_err() {
        // Prefer building with cc on all platforms for consistent flags/runtime
        // Use CMake only if explicitly requested via IMGUI_SYS_USE_CMAKE.
        let use_cmake = env::var("IMGUI_SYS_USE_CMAKE").ok().is_some();
        if use_cmake && build_with_cmake(&manifest_dir) {
            // cmake path handled printing of link flags
        } else {
            // Fallback: build with cc
            let mut build = cc::Build::new();
            build.cpp(true).std("c++17");

            let cimgui_root = manifest_dir.join("third-party/cimgui");
            let imgui_src = cimgui_root.join("imgui");

            // Include directories
            build.include(&cimgui_root);
            build.include(&imgui_src);

            // Do NOT define CIMGUI_DEFINE_ENUMS_AND_STRUCTS when compiling cimgui.cpp.
            // That macro is only for header parsing (bindgen), not for building the implementation.

            // Core ImGui compilation units
            build.file(imgui_src.join("imgui.cpp"));
            build.file(imgui_src.join("imgui_draw.cpp"));
            build.file(imgui_src.join("imgui_widgets.cpp"));
            build.file(imgui_src.join("imgui_tables.cpp"));
            build.file(imgui_src.join("imgui_demo.cpp"));

            // cimgui C API implementation
            build.file(cimgui_root.join("cimgui.cpp"));

            // Align MSVC runtime selection with Rust's target
            let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
            if target_env == "msvc" && target_os == "windows" {
                build.flag("/EHsc");

                let target_features = env::var("CARGO_CFG_TARGET_FEATURE").unwrap_or_default();
                let use_static_crt = target_features.split(',').any(|f| f == "crt-static");
                build.static_crt(use_static_crt);
                if use_static_crt {
                    build.flag("/MT");
                } else {
                    build.flag("/MD");
                }

                let profile = env::var("PROFILE").unwrap_or_else(|_| "release".to_string());
                if profile == "debug" {
                    build.debug(true);
                    build.opt_level(0);
                } else {
                    build.debug(false);
                    build.opt_level(2);
                }

                build.flag("/D_ITERATOR_DEBUG_LEVEL=0");
                // Ensure 32-bit ImWchar on MSVC path
                build.define("IMGUI_USE_WCHAR32", None);
            }

            #[cfg(feature = "freetype")]
            if let Ok(freetype) = pkg_config::probe_library("freetype2") {
                build.define("IMGUI_ENABLE_FREETYPE", Some("1"));
                for include in &freetype.include_paths {
                    build.include(include.display().to_string());
                }
                build.file(imgui_src.join("misc/freetype/imgui_freetype.cpp"));
            }

            build.compile("dear_imgui");
        }
    } else if !linked_prebuilt {
        if env::var("IMGUI_SYS_SKIP_CC").is_ok() {
            println!("cargo:warning=Skipping C/C++ build due to IMGUI_SYS_SKIP_CC");
        }
        println!("cargo:warning=WASM target is not supported.");
    }

    // Export paths and defines for extension crates (similar to imgui-sys)
    {
        // Export paths and defines for extension crates
        let cimgui_root = manifest_dir.join("third-party/cimgui");
        let imgui_src = cimgui_root.join("imgui");
        println!("cargo:THIRD_PARTY={}", imgui_src.display());
        println!("cargo:IMGUI_INCLUDE_PATH={}", imgui_src.display());
        println!("cargo:CIMGUI_INCLUDE_PATH={}", cimgui_root.display());

        // Export common defines that extensions might need
        println!("cargo:DEFINE_IMGUITEST=0");
        // Only export IMGUI_USE_WCHAR32 when actually enabled in our own build.
        // We emit a define for dependents to consume (e.g., dear-implot-sys).
        if cfg!(target_env = "msvc") {
            println!("cargo:DEFINE_IMGUI_USE_WCHAR32=1");
        }
    }
}

fn expected_lib_name(target_env: &str) -> &'static str {
    if target_env == "msvc" {
        "dear_imgui.lib"
    } else {
        "libdear_imgui.a"
    }
}

fn try_link_prebuilt(dir: &Path, target_env: &str) -> bool {
    let lib_name = expected_lib_name(target_env);
    let lib_path = dir.join(lib_name);
    if !lib_path.exists() {
        return false;
    }
    println!("cargo:rustc-link-search=native={}", dir.display());
    println!("cargo:rustc-link-lib=static=dear_imgui");
    true
}

fn try_download_prebuilt(out_dir: &Path, url: &str, target_env: &str) -> Result<PathBuf, String> {
    let lib_name = expected_lib_name(target_env);
    let dl_dir = out_dir.join("prebuilt");
    let _ = std::fs::create_dir_all(&dl_dir);
    let dst = dl_dir.join(lib_name);

    if dst.exists() {
        return Ok(dl_dir);
    }

    println!("cargo:warning=Downloading prebuilt dear_imgui from {}", url);
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| format!("create http client: {}", e))?;
    let resp = client
        .get(url)
        .send()
        .map_err(|e| format!("http get: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("http status {}", resp.status()));
    }
    let bytes = resp.bytes().map_err(|e| format!("read body: {}", e))?;
    std::fs::write(&dst, &bytes).map_err(|e| format!("write {}: {}", dst.display(), e))?;
    Ok(dl_dir)
}

fn build_with_cmake(manifest_dir: &Path) -> bool {
    let cimgui_root = manifest_dir.join("third-party/cimgui");
    if !cimgui_root.join("CMakeLists.txt").exists() {
        return false;
    }
    println!("cargo:warning=Building cimgui with CMake");
    let mut cfg = cmake::Config::new(&cimgui_root);
    // Static lib to match our expected linking
    cfg.define("IMGUI_STATIC", "ON");
    // Build profile: avoid MSVC Debug CRT in debug to reduce iterator-debug issues
    let profile = env::var("PROFILE").unwrap_or_else(|_| "release".into());
    let cmake_profile = if cfg!(target_env = "msvc") && profile == "debug" {
        "RelWithDebInfo"
    } else if profile == "debug" {
        "Debug"
    } else {
        "Release"
    };
    cfg.profile(cmake_profile);
    // Ensure 32-bit ImWchar for consistency when using MSVC toolchain
    if cfg!(target_env = "msvc") {
        // Prefer using the official CMake option for cimgui
        cfg.define("IMGUI_WCHAR32", "ON");

        // Align MSVC runtime library with Rust's target (static/dynamic CRT)
        let target_features = env::var("CARGO_CFG_TARGET_FEATURE").unwrap_or_default();
        let use_static_crt = target_features.split(',').any(|f| f == "crt-static");
        let msvc_runtime = if use_static_crt {
            "MultiThreaded"
        } else {
            "MultiThreadedDLL"
        };
        // Requires CMake 3.15+
        cfg.define("CMAKE_MSVC_RUNTIME_LIBRARY", msvc_runtime);
    }

    let dst = cfg.build();

    // Library name is cimgui (no prefix on Windows, lib prefix on Unix)
    // CMake may copy libraries to dst/lib or generator-specific config dirs
    let mut lib_dir_candidates = Vec::new();
    lib_dir_candidates.push(dst.join("lib"));
    lib_dir_candidates.push(dst.join("build"));
    lib_dir_candidates.push(dst.clone());
    // Common MSVC profiles
    lib_dir_candidates.push(dst.join("build").join("RelWithDebInfo"));
    lib_dir_candidates.push(dst.join("build").join("Release"));
    lib_dir_candidates.push(dst.join("build").join("Debug"));
    lib_dir_candidates.push(dst.join("RelWithDebInfo"));
    lib_dir_candidates.push(dst.join("Release"));
    lib_dir_candidates.push(dst.join("Debug"));
    for lib_dir in lib_dir_candidates.iter() {
        if lib_dir.exists() {
            println!("cargo:rustc-link-search=native={}", lib_dir.display());
        }
    }
    println!("cargo:rustc-link-lib=static=cimgui");
    // Also export include paths for extensions
    let imgui_src = cimgui_root.join("imgui");
    println!("cargo:IMGUI_INCLUDE_PATH={}", imgui_src.display());
    println!("cargo:CIMGUI_INCLUDE_PATH={}", cimgui_root.display());
    true
}

fn use_pregenerated_bindings(out_path: &Path) -> bool {
    let preg = Path::new("src").join("bindings_pregenerated.rs");
    if preg.exists() {
        match std::fs::copy(&preg, out_path.join("bindings.rs")) {
            Ok(_) => {
                println!(
                    "cargo:warning=Using pregenerated bindings: {}",
                    preg.display()
                );
                true
            }
            Err(e) => {
                println!("cargo:warning=Failed to copy pregenerated bindings: {}", e);
                false
            }
        }
    } else {
        false
    }
}
