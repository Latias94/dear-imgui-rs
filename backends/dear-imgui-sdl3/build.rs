use std::env;
use std::path::PathBuf;

fn feature_enabled(feature: &str) -> bool {
    let env_name = format!(
        "CARGO_FEATURE_{}",
        feature.replace('-', "_").to_ascii_uppercase()
    );
    env::var_os(env_name).is_some()
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=wrapper.cpp");
    println!("cargo:rerun-if-env-changed=DEP_DEAR_IMGUI_IMGUI_BACKENDS_PATH");
    println!("cargo:rerun-if-env-changed=DEP_DEAR_IMGUI_THIRD_PARTY");
    println!("cargo:rerun-if-env-changed=DEP_DEAR_IMGUI_IMGUI_INCLUDE_PATH");
    println!("cargo:rerun-if-env-changed=SDL3_INCLUDE_DIR");
    println!("cargo:rerun-if-env-changed=DEP_SDL3_INCLUDE_PATH");
    println!("cargo:rerun-if-env-changed=DEP_SDL3_INCLUDE_DIR");
    println!("cargo:rerun-if-env-changed=DEP_SDL3_OUT_DIR");

    // The upstream SDL3 backend uses Win32 APIs (e.g. GetWindowLong/SetWindowLong) on Windows.
    if env::var("CARGO_CFG_TARGET_OS")
        .map(|os| os == "windows")
        .unwrap_or(false)
    {
        println!("cargo:rustc-link-lib=user32");
    }

    // Upstream Dear ImGui core headers (imgui.h etc.) are provided by dear-imgui-sys.
    let imgui_root = env::var("DEP_DEAR_IMGUI_THIRD_PARTY")
        .or_else(|_| env::var("DEP_DEAR_IMGUI_IMGUI_INCLUDE_PATH"))
        .expect(
            "DEP_DEAR_IMGUI_THIRD_PARTY or DEP_DEAR_IMGUI_IMGUI_INCLUDE_PATH not set. \
             Make sure dear-imgui-sys is built before dear-imgui-sdl3.",
        );

    let imgui_root = PathBuf::from(imgui_root);
    let backends_root = env::var("DEP_DEAR_IMGUI_IMGUI_BACKENDS_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| imgui_root.join("backends"));
    let backends_parent = backends_root.parent().unwrap_or(&imgui_root);
    let enable_sdlrenderer3 = feature_enabled("sdlrenderer3-renderer");
    let enable_sdlgpu3 = feature_enabled("sdlgpu3-renderer");

    if !backends_root.join("imgui_impl_sdl3.h").exists() {
        panic!(
            "dear-imgui-sdl3: could not find Dear ImGui backend sources at {}. \
             Make sure dear-imgui-sys packages the upstream imgui/backends directory.",
            backends_root.display()
        );
    }

    println!(
        "cargo:rerun-if-changed={}",
        backends_root.join("imgui_impl_sdl3.cpp").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        backends_root.join("imgui_impl_sdl3.h").display()
    );
    if enable_sdlrenderer3 {
        if !backends_root.join("imgui_impl_sdlrenderer3.h").exists() {
            panic!(
                "dear-imgui-sdl3: could not find Dear ImGui SDLRenderer3 backend sources at {}. \
                 Make sure dear-imgui-sys packages the upstream imgui/backends directory.",
                backends_root.display()
            );
        }
        println!(
            "cargo:rerun-if-changed={}",
            backends_root.join("imgui_impl_sdlrenderer3.cpp").display()
        );
        println!(
            "cargo:rerun-if-changed={}",
            backends_root.join("imgui_impl_sdlrenderer3.h").display()
        );
    }
    if enable_sdlgpu3 {
        if !backends_root.join("imgui_impl_sdlgpu3.h").exists() {
            panic!(
                "dear-imgui-sdl3: could not find Dear ImGui SDLGPU3 backend sources at {}. \
                 Make sure dear-imgui-sys packages the upstream imgui/backends directory.",
                backends_root.display()
            );
        }
        println!(
            "cargo:rerun-if-changed={}",
            backends_root.join("imgui_impl_sdlgpu3.cpp").display()
        );
        println!(
            "cargo:rerun-if-changed={}",
            backends_root.join("imgui_impl_sdlgpu3.h").display()
        );
        println!(
            "cargo:rerun-if-changed={}",
            backends_root.join("imgui_impl_sdlgpu3_shaders.h").display()
        );
    }

    let mut build = cc::Build::new();
    build.cpp(true).std("c++17");

    // Dear ImGui core includes
    build.include(&imgui_root);
    build.include(backends_parent);

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let sdl3_headers = build_support::find_sdl3_include_paths(build_support::Sdl3SearchConfig {
        out_dir: &out_dir,
        target_os: &target_os,
        use_pkg_config: true,
        use_vcpkg: true,
    })
    .unwrap_or_else(|message| {
        if target_os == "android" {
            panic!(
                "dear-imgui-sdl3: could not find SDL3 headers for Android. \
                     {message} Set SDL3_INCLUDE_DIR to an include root containing \
                     SDL3/SDL.h, or make the final application dependency graph \
                     enable `sdl3/build-from-source` so sdl3-sys can expose headers \
                     via DEP_SDL3_OUT_DIR. Android application / NDK / CMake setup \
                     still belongs to the consuming application."
            );
        }
        if target_os == "ios" {
            panic!(
                "dear-imgui-sdl3: could not find SDL3 headers for iOS. \
                     {message} Set SDL3_INCLUDE_DIR to an include root containing \
                     SDL3/SDL.h, or make the final application dependency graph \
                     enable `sdl3/build-from-source` so sdl3-sys can expose headers \
                     via DEP_SDL3_OUT_DIR. If your app links an SDL3.xcframework \
                     from Xcode, framework packaging, signing, and the host app \
                     entry point still belong to the consuming application."
            );
        }

        panic!(
            "dear-imgui-sdl3: could not find SDL3 headers. {message} \
                 Install SDL3 development files through pkg-config/vcpkg, set \
                 SDL3_INCLUDE_DIR to the SDL3 include path, or make the final \
                 dependency graph enable `sdl3/build-from-source` so sdl3-sys can \
                 build SDL3 and expose headers via DEP_SDL3_OUT_DIR."
        );
    });

    for include_path in sdl3_headers.include_paths {
        build.include(&include_path);
    }
    println!(
        "cargo:warning=dear-imgui-sdl3: using SDL3 headers from {}",
        sdl3_headers.source
    );

    // Backend sources come from the upstream Dear ImGui tree packaged by dear-imgui-sys.
    build.file(backends_root.join("imgui_impl_sdl3.cpp"));
    if enable_sdlrenderer3 {
        build.define("DEAR_IMGUI_SDL3_ENABLE_SDLRENDERER3", None);
        build.file(backends_root.join("imgui_impl_sdlrenderer3.cpp"));
    }
    if enable_sdlgpu3 {
        build.define("DEAR_IMGUI_SDL3_ENABLE_SDLGPU3", None);
        build.file(backends_root.join("imgui_impl_sdlgpu3.cpp"));
    }

    // C wrappers used by Rust FFI (see wrapper.cpp).
    build.file("wrapper.cpp");

    build.compile("dear-imgui-sdl3-backend");
}
