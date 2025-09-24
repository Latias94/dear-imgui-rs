use std::{
    env,
    path::{Path, PathBuf},
};

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

    // cimnodes root (contains cimnodes.{h,cpp} and an embedded imnodes/ directory)
    let cimnodes_root = manifest_dir.join("third-party/cimnodes");

    if !imgui_src.exists() {
        panic!(
            "ImGui include not found at {:?}. Did you forget to initialize dear-imgui-sys third-party?",
            imgui_src
        );
    }
    if !cimnodes_root.exists() {
        panic!(
            "cimnodes root not found at {:?}. Did you init submodules?",
            cimnodes_root
        );
    }

    // Rerun hints and env tracking
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=third-party/cimnodes/cimnodes.h");
    println!("cargo:rerun-if-changed=third-party/cimnodes/cimnodes.cpp");
    println!("cargo:rerun-if-changed=third-party/cimnodes/imnodes/imnodes.h");
    println!("cargo:rerun-if-changed=third-party/cimnodes/imnodes/imnodes.cpp");
    println!("cargo:rerun-if-changed=../../dear-imgui-sys");
    println!("cargo:rerun-if-env-changed=IMNODES_SYS_LIB_DIR");
    println!("cargo:rerun-if-env-changed=IMNODES_SYS_SKIP_CC");
    println!("cargo:rerun-if-env-changed=IMNODES_SYS_PREBUILT_URL");

    // Generate bindings from cimnodes.h
    let bindings = bindgen::Builder::default()
        .header(cimnodes_root.join("cimnodes.h").to_string_lossy())
        .header(manifest_dir.join("shim/imnodes_extra.h").to_string_lossy())
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .allowlist_function("imnodes_.*")
        .allowlist_function("EmulateThreeButtonMouse_.*")
        .allowlist_function("LinkDetachWithModifierClick_.*")
        .allowlist_function("MultipleSelectModifier_.*")
        .allowlist_function("getIOKeyCtrlPtr")
        .allowlist_function("imnodes_getIOKeyShiftPtr")
        .allowlist_function("imnodes_getIOKeyAltPtr")
        .allowlist_type("ImNodes.*")
        .allowlist_var("ImNodes.*")
        .blocklist_type("ImVec2")
        .blocklist_type("ImVec4")
        .blocklist_type("ImGuiContext")
        .blocklist_type("ImDrawList")
        .clang_arg(format!("-I{}", cimgui_root.display()))
        .clang_arg(format!("-I{}", imgui_src.display()))
        .clang_arg(format!("-I{}", cimnodes_root.display()))
        .clang_arg(format!("-I{}", cimnodes_root.join("imnodes").display()))
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

    let bindings = bindings
        .generate()
        .expect("Unable to generate cimnodes bindings");
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write cimnodes bindings!");

    // Try prebuilt paths
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    let mut linked_prebuilt = false;
    if let Ok(dir) = env::var("IMNODES_SYS_LIB_DIR") {
        if try_link_prebuilt(PathBuf::from(dir.clone()), &target_env) {
            linked_prebuilt = true;
        } else {
            println!(
                "cargo:warning=IMNODES_SYS_LIB_DIR set but library not found in {}",
                dir
            );
        }
    }
    if !linked_prebuilt {
        if let Ok(url) = env::var("IMNODES_SYS_PREBUILT_URL") {
            match try_download_prebuilt(&out_path, &url, &target_env) {
                Ok(dir) => {
                    if try_link_prebuilt(dir.clone(), &target_env) {
                        linked_prebuilt = true;
                    }
                }
                Err(e) => println!(
                    "cargo:warning=Failed to download prebuilt dear_imnodes: {}",
                    e
                ),
            }
        }
    }

    // Build from source if not linking prebuilt and not told to skip
    if !linked_prebuilt && env::var("IMNODES_SYS_SKIP_CC").is_err() {
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
        build.include(&cimnodes_root);
        build.include(manifest_dir.join("shim"));
        build.include(cimnodes_root.join("imnodes"));
        // Ensure namespace expected by cimnodes.cpp matches the header's namespace
        build.define("IMNODES_NAMESPACE", Some("imnodes"));

        build.file(cimnodes_root.join("cimnodes.cpp"));
        build.file(cimnodes_root.join("imnodes/imnodes.cpp"));
        build.file(manifest_dir.join("shim/imnodes_extra.cpp"));

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

        build.compile("dear_imnodes");
    }
}

fn expected_lib_name(target_env: &str) -> &'static str {
    if target_env == "msvc" {
        "dear_imnodes.lib"
    } else {
        "libdear_imnodes.a"
    }
}

fn try_link_prebuilt(dir: PathBuf, target_env: &str) -> bool {
    let lib_name = expected_lib_name(target_env);
    let lib_path = dir.join(lib_name);
    if !lib_path.exists() {
        return false;
    }
    println!("cargo:rustc-link-search=native={}", dir.display());
    println!("cargo:rustc-link-lib=static=dear_imnodes");
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
    println!(
        "cargo:warning=Downloading prebuilt dear_imnodes from {}",
        url
    );
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| format!("http client: {}", e))?;
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
