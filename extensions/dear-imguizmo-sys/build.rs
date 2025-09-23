use std::{env, path::{Path, PathBuf}};

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());

    // Resolve include paths from dear-imgui-sys
    let imgui_src = env::var_os("DEP_DEAR_IMGUI_IMGUI_INCLUDE_PATH")
        .or_else(|| env::var_os("DEP_DEAR_IMGUI_THIRD_PARTY"))
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest_dir.join("../../dear-imgui-sys/third-party/cimgui/imgui"));
    let cimgui_root = env::var_os("DEP_DEAR_IMGUI_CIMGUI_INCLUDE_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest_dir.join("../../dear-imgui-sys/third-party/cimgui"));

    // cimguizmo root
    let cimguizmo_root = manifest_dir.join("third-party/cimguizmo");

    if !imgui_src.exists() {
        panic!("ImGui include not found at {:?}", imgui_src);
    }
    if !cimgui_root.exists() {
        panic!("cimgui root not found at {:?}", cimgui_root);
    }
    if !cimguizmo_root.exists() {
        panic!("cimguizmo root not found at {:?}. Did you init submodules?", cimguizmo_root);
    }

    // Rerun hints and env tracking
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=third-party/cimguizmo/cimguizmo.h");
    println!("cargo:rerun-if-changed=third-party/cimguizmo/cimguizmo.cpp");
    println!("cargo:rerun-if-changed=third-party/cimguizmo/ImGuizmo/ImGuizmo.cpp");
    println!("cargo:rerun-if-changed=../../dear-imgui-sys");
    println!("cargo:rerun-if-env-changed=IMGUIZMO_SYS_LIB_DIR");
    println!("cargo:rerun-if-env-changed=IMGUIZMO_SYS_SKIP_CC");
    println!("cargo:rerun-if-env-changed=IMGUIZMO_SYS_PREBUILT_URL");

    // Generate bindings from cimguizmo.h
    let bindings = bindgen::Builder::default()
        .header(cimguizmo_root.join("cimguizmo.h").to_string_lossy())
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .allowlist_function("ImGuizmo_.*")
        .allowlist_function("Style_.*")
        .allowlist_type("(Style|COLOR|MODE|OPERATION)")
        .allowlist_var("(COLOR|MODE|OPERATION|COUNT|TRANSLATE.*|ROTATE.*|SCALE.*|UNIVERSAL)")
        .blocklist_type("ImVec2")
        .blocklist_type("ImVec4")
        .blocklist_type("ImGuiContext")
        .blocklist_type("ImDrawList")
        .blocklist_type("ImGuiID")
        .clang_arg(format!("-I{}", cimgui_root.display()))
        .clang_arg(format!("-I{}", imgui_src.display()))
        .clang_arg(format!("-I{}", cimguizmo_root.display()))
        .clang_arg(format!("-I{}", cimguizmo_root.join("ImGuizmo").display()))
        .clang_arg("-DCIMGUI_DEFINE_ENUMS_AND_STRUCTS")
        .derive_default(true)
        .derive_debug(true)
        .derive_copy(true)
        .derive_eq(true)
        .derive_partialeq(true)
        .derive_hash(true)
        .prepend_enum_name(false)
        .layout_tests(false)
        .clang_arg("-x")
        .clang_arg("c++")
        .clang_arg("-std=c++17");

    let bindings = bindings.generate().expect("Unable to generate cimguizmo bindings");
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write cimguizmo bindings!");

    // Try prebuilt paths
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    let mut linked_prebuilt = false;
    if let Ok(dir) = env::var("IMGUIZMO_SYS_LIB_DIR") {
        if try_link_prebuilt(PathBuf::from(dir.clone()), &target_env) {
            linked_prebuilt = true;
            // Do not link dear_imgui here; rely on dear-imgui-sys dependency to provide the correct native lib
        } else {
            println!("cargo:warning=IMGUIZMO_SYS_LIB_DIR set but library not found in {}", dir);
        }
    }
    if !linked_prebuilt {
        if let Ok(url) = env::var("IMGUIZMO_SYS_PREBUILT_URL") {
            match try_download_prebuilt(&out_path, &url, &target_env) {
                Ok(dir) => {
                    if try_link_prebuilt(dir.clone(), &target_env) {
                        linked_prebuilt = true;
                        // Do not link dear_imgui here; rely on dear-imgui-sys dependency to provide the correct native lib
                    }
                }
                Err(e) => println!("cargo:warning=Failed to download prebuilt dear_imguizmo: {}", e),
            }
        }
    }

    // Build from source if not linking prebuilt and not told to skip
    if !linked_prebuilt && env::var("IMGUIZMO_SYS_SKIP_CC").is_err() {
        let mut build = cc::Build::new();
        build.cpp(true).std("c++17");

        // Propagate dear-imgui defines
        for (k, v) in env::vars() {
            if let Some(suffix) = k.strip_prefix("DEP_DEAR_IMGUI_DEFINE_") {
                build.define(suffix, v.as_str());
            }
        }

        build.include(&imgui_src);
        build.include(&cimgui_root);
        build.include(&cimguizmo_root);
        build.include(cimguizmo_root.join("ImGuizmo"));

        build.file(cimguizmo_root.join("cimguizmo.cpp"));
        build.file(cimguizmo_root.join("ImGuizmo/ImGuizmo.cpp"));

        // Align MSVC runtime and exceptions to dear-imgui-sys
        let target_env_now = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
        let target_os_now = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
        if target_env_now == "msvc" && target_os_now == "windows" {
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

        build.compile("dear_imguizmo");
    }
}

fn expected_lib_name(target_env: &str) -> &'static str {
    if target_env == "msvc" { "dear_imguizmo.lib" } else { "libdear_imguizmo.a" }
}

fn try_link_prebuilt(dir: PathBuf, target_env: &str) -> bool {
    let lib_name = expected_lib_name(target_env);
    let lib_path = dir.join(lib_name);
    if !lib_path.exists() {
        return false;
    }
    println!("cargo:rustc-link-search=native={}", dir.display());
    println!("cargo:rustc-link-lib=static=dear_imguizmo");
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
    println!("cargo:warning=Downloading prebuilt dear_imguizmo from {}", url);
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| format!("http client: {}", e))?;
    let resp = client.get(url).send().map_err(|e| format!("http get: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("http status {}", resp.status()));
    }
    let bytes = resp.bytes().map_err(|e| format!("read body: {}", e))?;
    std::fs::write(&dst, &bytes).map_err(|e| format!("write {}: {}", dst.display(), e))?;
    Ok(dl_dir)
}
