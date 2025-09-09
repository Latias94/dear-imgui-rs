use std::env;
use std::path::{Path, PathBuf};

/// Check if we need to copy files based on modification times
fn should_copy_files(imgui_ori: &Path, imgui_src_files: &[&str], copy_marker: &Path) -> bool {
    // If marker doesn't exist, we need to copy
    if !copy_marker.exists() {
        return true;
    }

    // Get marker modification time
    let marker_time = match std::fs::metadata(copy_marker).and_then(|m| m.modified()) {
        Ok(time) => time,
        Err(_) => return true, // If we can't read marker, copy
    };

    // Check if any source file is newer than the marker
    for ori in imgui_src_files {
        let src = imgui_ori.join(ori);
        if src.exists() {
            if let Ok(metadata) = std::fs::metadata(&src) {
                if let Ok(src_time) = metadata.modified() {
                    if src_time > marker_time {
                        return true; // Source file is newer, need to copy
                    }
                }
            }
        }
    }

    // Check freetype files
    for ori in ["imgui_freetype.cpp", "imgui_freetype.h"] {
        let src = imgui_ori.join("misc/freetype").join(ori);
        if src.exists() {
            if let Ok(metadata) = std::fs::metadata(&src) {
                if let Ok(src_time) = metadata.modified() {
                    if src_time > marker_time {
                        return true; // Source file is newer, need to copy
                    }
                }
            }
        }
    }

    false // All files are up to date
}

/// Check if we need to regenerate bindings
fn should_regenerate_bindings(
    imgui_src: &Path,
    bindings_marker: &Path,
    target_env: &str,
    manifest_dir: &Path,
) -> bool {
    // If marker doesn't exist, we need to regenerate
    if !bindings_marker.exists() {
        return true;
    }

    // Get marker modification time
    let marker_time = match std::fs::metadata(bindings_marker).and_then(|m| m.modified()) {
        Ok(time) => time,
        Err(_) => return true, // If we can't read marker, regenerate
    };

    // Check if any header file is newer than the marker
    for header in ["imgui.h", "imgui_internal.h", "imconfig.h"] {
        let header_path = imgui_src.join(header);
        if header_path.exists() {
            if let Ok(metadata) = std::fs::metadata(&header_path) {
                if let Ok(header_time) = metadata.modified() {
                    if header_time > marker_time {
                        return true; // Header file is newer, need to regenerate
                    }
                }
            }
        }
    }

    // Check MSVC-specific files
    if target_env == "msvc" {
        for file in ["msvc_blocklist.txt", "imgui_msvc_wrapper.cpp"] {
            let file_path = manifest_dir.join(file);
            if file_path.exists() {
                if let Ok(metadata) = std::fs::metadata(&file_path) {
                    if let Ok(file_time) = metadata.modified() {
                        if file_time > marker_time {
                            return true; // MSVC file is newer, need to regenerate
                        }
                    }
                }
            }
        }
    }

    false // All files are up to date
}

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap();

    // ImGui source files to copy and track
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

    let imgui_ori = manifest_dir.join("third-party").join("imgui");
    let imgui_src = out_path.join("imgui_src");
    let imgui_misc_ft = imgui_src.join("misc/freetype");

    // Create output directories if they don't exist
    std::fs::create_dir_all(&imgui_src).unwrap();
    std::fs::create_dir_all(&imgui_misc_ft).unwrap();

    // Check if we need to copy files (smart caching)
    let copy_marker = imgui_src.join(".copy_complete");
    let need_copy = should_copy_files(&imgui_ori, &imgui_src_files, &copy_marker);

    if need_copy {
        // Copy ImGui source files
        for ori in imgui_src_files {
            let src = imgui_ori.join(ori);
            let dst = imgui_src.join(ori);
            if src.exists() {
                std::fs::copy(&src, &dst).unwrap();
            }
        }

        // Copy freetype files
        for ori in ["imgui_freetype.cpp", "imgui_freetype.h"] {
            let src = imgui_ori.join("misc/freetype").join(ori);
            let dst = imgui_misc_ft.join(ori);
            if src.exists() {
                std::fs::copy(&src, &dst).unwrap();
            }
        }

        // Mark copy as complete
        std::fs::write(&copy_marker, "complete").unwrap();
    }

    // Always register rerun triggers
    for ori in imgui_src_files {
        let src = imgui_ori.join(ori);
        if src.exists() {
            println!("cargo:rerun-if-changed={}", src.display());
        }
    }
    for ori in ["imgui_freetype.cpp", "imgui_freetype.h"] {
        let src = imgui_ori.join("misc/freetype").join(ori);
        if src.exists() {
            println!("cargo:rerun-if-changed={}", src.display());
        }
    }

    // Register wrapper files for rerun detection
    println!(
        "cargo:rerun-if-changed={}",
        manifest_dir.join("wrapper.cpp").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        manifest_dir.join("imgui_msvc_wrapper.cpp").display()
    );
    println!("cargo:rerun-if-changed=build.rs");

    // Write custom imconfig.h only if it doesn't exist or content differs
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

    let should_write_imconfig = if imconfig_path.exists() {
        match std::fs::read_to_string(&imconfig_path) {
            Ok(existing_content) => existing_content != imconfig_content,
            Err(_) => true,
        }
    } else {
        true
    };

    if should_write_imconfig {
        std::fs::write(&imconfig_path, imconfig_content).unwrap();
    }

    println!("cargo:THIRD_PARTY={}", imgui_src.display());
    println!("cargo:rerun-if-changed=wrapper.cpp");

    // Track MSVC files
    if target_env == "msvc" {
        println!("cargo:rerun-if-changed=msvc_blocklist.txt");
        println!("cargo:rerun-if-changed=imgui_msvc_wrapper.cpp");
    }

    // Generate bindings
    generate_bindings(&manifest_dir, &out_path, &imgui_src, &target_env);

    // Build ImGui
    build_imgui(&manifest_dir, &imgui_src, &target_arch, &target_env);
}

fn generate_bindings(
    manifest_dir: &PathBuf,
    out_path: &PathBuf,
    imgui_src: &PathBuf,
    target_env: &str,
) {
    let bindings_path = out_path.join("bindings.rs");
    let bindings_marker = out_path.join(".bindings_complete");

    // Check if we need to regenerate bindings
    if should_regenerate_bindings(&imgui_src, &bindings_marker, target_env, manifest_dir) {
        println!("cargo:warning=Regenerating bindings...");

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
            .write_to_file(&bindings_path)
            .expect("Couldn't write bindings!");

        // Mark bindings as complete
        std::fs::write(&bindings_marker, "complete").unwrap();
    } else {
        println!("cargo:warning=Using cached bindings...");
    }
}

fn build_imgui(manifest_dir: &PathBuf, imgui_src: &PathBuf, target_arch: &str, target_env: &str) {
    let mut build = cc::Build::new();

    if target_arch == "wasm32" {
        build.define("IMGUI_DISABLE_DEFAULT_SHELL_FUNCTIONS", "1");
    } else {
        build.cpp(true).std("c++20");
    }

    build.include(imgui_src);
    build.file(manifest_dir.join("wrapper.cpp"));

    #[cfg(feature = "freetype")]
    if let Ok(freetype) = pkg_config::probe_library("freetype2") {
        build.define("IMGUI_ENABLE_FREETYPE", "1");
        for include in &freetype.include_paths {
            build.include(include);
        }
    }

    if target_env == "msvc" {
        let msvc_wrapper_src = manifest_dir.join("imgui_msvc_wrapper.cpp");
        if msvc_wrapper_src.exists() {
            build.file(msvc_wrapper_src);
        }
    }

    build.compile("dear_imgui");
}
