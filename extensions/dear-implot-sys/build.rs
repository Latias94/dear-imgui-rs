use std::{env, path::PathBuf};

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());

    // Get include paths from dear-imgui-sys environment variables
    let imgui_src = env::var_os("DEP_DEAR_IMGUI_IMGUI_INCLUDE_PATH")
        .or_else(|| env::var_os("DEP_DEAR_IMGUI_THIRD_PARTY"))
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest_dir.join("../../dear-imgui-sys/imgui"));
    let cimgui_root = env::var_os("DEP_DEAR_IMGUI_CIMGUI_INCLUDE_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest_dir.join("../../dear-imgui-sys/third-party/cimgui"));

    // cimplot root (cimplot repo contains cimplot.{h,cpp} and an embedded implot/ directory)
    let cimplot_root = manifest_dir.join("third-party/cimplot");

    // Verify sources exist
    if !imgui_src.exists() {
        panic!(
            "ImGui source not found at {:?}. Did you forget to initialize git submodules?",
            imgui_src
        );
    }
    if !cimplot_root.exists() {
        panic!(
            "cimplot source not found at {:?}. Did you forget to initialize git submodules?",
            cimplot_root
        );
    }

    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();

    // docs.rs build: only generate bindings, skip native compilation
    let is_docs_rs = std::env::var("DOCS_RS").is_ok();

    // Generate bindings using bindgen from cimplot C API
    let bindings = bindgen::Builder::default()
        .header(cimplot_root.join("cimplot.h").to_string_lossy())
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        // Only allow ImPlot-specific functions and types
        .allowlist_function("ImPlot.*")
        .allowlist_type("ImPlot.*")
        .allowlist_var("ImPlot.*")
        .allowlist_var("IMPLOT_.*")
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
        .derive_default(true)
        .derive_debug(true)
        .derive_copy(true)
        .derive_eq(true)
        .derive_partialeq(true)
        .derive_hash(true)
        .prepend_enum_name(false)
        .layout_tests(false)
        .clang_arg(format!("-I{}", imgui_src.display()))
        .clang_arg(format!("-I{}", cimgui_root.display()))
        .clang_arg(format!("-I{}", cimplot_root.display()))
        .clang_arg(format!("-I{}", cimplot_root.join("implot").display()))
        .clang_arg("-DCIMGUI_DEFINE_ENUMS_AND_STRUCTS")
        .clang_arg("-x")
        .clang_arg("c++")
        .clang_arg("-std=c++17");

    // TODO: Handle MSVC-specific issues later
    // if target_env == "msvc" {
    //     let blocklist_file = manifest_dir.join("msvc_blocklist.txt");
    //     if let Ok(content) = std::fs::read_to_string(&blocklist_file) {
    //         for line in content.lines() {
    //             let line = line.trim();
    //             if line.is_empty() || line.starts_with('#') {
    //                 continue;
    //             }
    //             bindings = bindings.blocklist_function(line);
    //         }
    //     }

    //     let msvc_wrapper_src = manifest_dir.join("implot_msvc_wrapper.cpp");
    //     if msvc_wrapper_src.exists() {
    //         bindings = bindings
    //             .header(msvc_wrapper_src.to_string_lossy())
    //             .allowlist_file(msvc_wrapper_src.to_string_lossy());
    //     }
    // }

    let bindings = bindings.generate().expect("Unable to generate bindings");

    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");

    // Env change tracking
    println!("cargo:rerun-if-env-changed=IMPLOT_SYS_LIB_DIR");
    println!("cargo:rerun-if-env-changed=IMPLOT_SYS_SKIP_CC");
    println!("cargo:rerun-if-env-changed=IMPLOT_SYS_PREBUILT_URL");

    // Try link prebuilt/system library if provided
    let mut linked_prebuilt = false;
    if let Ok(dir) = env::var("IMPLOT_SYS_LIB_DIR") {
        if try_link_prebuilt(PathBuf::from(dir), &target_env) {
            linked_prebuilt = true;
            // Still need to link dear_imgui from dear-imgui-sys
            println!("cargo:rustc-link-lib=static=dear_imgui");
        } else {
            println!(
                "cargo:warning=IMPLOT_SYS_LIB_DIR set but no library found; falling back to build"
            );
        }
    }

    // Try download prebuilt if URL provided
    if !linked_prebuilt {
        if let Ok(url) = env::var("IMPLOT_SYS_PREBUILT_URL") {
            match try_download_prebuilt(&out_path, &url, &target_env) {
                Ok(dir) => {
                    if try_link_prebuilt(dir.clone(), &target_env) {
                        linked_prebuilt = true;
                        println!("cargo:rustc-link-lib=static=dear_imgui");
                    } else {
                        println!(
                            "cargo:warning=Downloaded prebuilt library but failed to link from {}",
                            dir.display()
                        );
                    }
                }
                Err(e) => {
                    println!(
                        "cargo:warning=Failed to download prebuilt dear_implot: {}",
                        e
                    );
                }
            }
        }
    }

    if !is_docs_rs && !linked_prebuilt && env::var("IMPLOT_SYS_SKIP_CC").is_err() {
        // Build ImPlot (via cimplot + implot sources)
        let mut build = cc::Build::new();
        if target_arch == "wasm32" {
            build.define("IMGUI_DISABLE_DEFAULT_SHELL_FUNCTIONS", "1");
        } else {
            build.cpp(true).std("c++17");
        }

        // MSVC C++ runtime and exception model align with dear-imgui-sys
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
        }

        // Take over imgui preprocessor defines from the dear-imgui-sys crate
        env::vars()
            .filter_map(|(key, val)| {
                key.strip_prefix("DEP_DEAR_IMGUI_DEFINE_")
                    .map(|suffix| (suffix.to_string(), val.to_string()))
            })
            .for_each(|(k, v)| {
                build.define(&k, v.as_str());
            });

        // Common defines and include directories
        build.define("IMGUI_DEFINE_MATH_OPERATORS", Some("1"));
        build.include(&imgui_src);
        build.include(&cimgui_root);
        build.include(&cimplot_root);
        build.include(cimplot_root.join("implot"));

        // Compile cimplot + implot sources
        build.file(cimplot_root.join("cimplot.cpp"));
        build.file(cimplot_root.join("implot/implot.cpp"));
        build.file(cimplot_root.join("implot/implot_items.cpp"));

        build.compile("dear_implot");

        // Rerun hints
        println!("cargo:rerun-if-changed=build.rs");
        println!("cargo:rerun-if-changed=third-party/cimplot/cimplot.h");
        println!("cargo:rerun-if-changed=third-party/cimplot/cimplot.cpp");
        println!("cargo:rerun-if-changed=third-party/cimplot/implot/implot.h");
        println!("cargo:rerun-if-changed=third-party/cimplot/implot/implot.cpp");
        println!("cargo:rerun-if-changed=third-party/cimplot/implot/implot_items.cpp");
        // Track dear-imgui-sys changes (defines/headers)
        println!("cargo:rerun-if-changed=../../dear-imgui-sys");
    } else if is_docs_rs {
        // docs.rs path: still propagate include paths for dependents if any
        println!("cargo:IMGUI_INCLUDE_PATH={}", imgui_src.display());
        println!("cargo:CIMGUI_INCLUDE_PATH={}", cimgui_root.display());
    }
}

fn expected_lib_name(target_env: &str) -> &'static str {
    if target_env == "msvc" {
        "dear_implot.lib"
    } else {
        "libdear_implot.a"
    }
}

fn try_link_prebuilt(dir: PathBuf, target_env: &str) -> bool {
    let lib_name = expected_lib_name(target_env);
    let lib_path = dir.join(lib_name);
    if !lib_path.exists() {
        println!(
            "cargo:warning=prebuilt dear_implot not found at {}",
            lib_path.display()
        );
        return false;
    }
    println!("cargo:rustc-link-search=native={}", dir.display());
    println!("cargo:rustc-link-lib=static=dear_implot");
    true
}

fn try_download_prebuilt(
    out_dir: &PathBuf,
    url: &str,
    target_env: &str,
) -> Result<PathBuf, String> {
    let lib_name = expected_lib_name(target_env);
    let dl_dir = out_dir.join("prebuilt");
    let _ = std::fs::create_dir_all(&dl_dir);
    let dst = dl_dir.join(lib_name);

    if dst.exists() {
        return Ok(dl_dir);
    }

    println!(
        "cargo:warning=Downloading prebuilt dear_implot from {}",
        url
    );
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
