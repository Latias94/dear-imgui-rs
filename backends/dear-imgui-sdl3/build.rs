use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=wrapper.cpp");
    println!("cargo:rerun-if-env-changed=DEP_DEAR_IMGUI_IMGUI_BACKENDS_PATH");
    println!("cargo:rerun-if-env-changed=DEP_DEAR_IMGUI_THIRD_PARTY");
    println!("cargo:rerun-if-env-changed=DEP_DEAR_IMGUI_IMGUI_INCLUDE_PATH");

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
    println!(
        "cargo:rerun-if-changed={}",
        backends_root.join("imgui_impl_opengl3.cpp").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        backends_root.join("imgui_impl_opengl3.h").display()
    );

    let mut build = cc::Build::new();
    build.cpp(true).std("c++17");

    // Dear ImGui core includes
    build.include(&imgui_root);
    build.include(backends_parent);

    // SDL3 includes:
    //
    // 1. Allow explicit override via SDL3_INCLUDE_DIR.
    // 2. Try pkg-config "sdl3" (if available).
    // 3. Try a few common default locations (Homebrew, /usr/local).
    // 4. As a fallback, try the include path produced by sdl3-sys when
    //    building SDL3 from source (DEP_SDL3_OUT_DIR/include).
    let mut have_sdl_headers = false;

    if let Ok(dir) = env::var("SDL3_INCLUDE_DIR") {
        build.include(&dir);
        have_sdl_headers = true;
        println!("cargo:warning=dear-imgui-sdl3: using SDL3_INCLUDE_DIR={dir}");
    } else {
        // pkg-config (best-effort; ignore errors).
        if let Ok(lib) = pkg_config::Config::new()
            .print_system_libs(false)
            .probe("sdl3")
        {
            for p in lib.include_paths {
                build.include(&p);
            }
            have_sdl_headers = true;
            println!("cargo:warning=dear-imgui-sdl3: using SDL3 headers from pkg-config (sdl3)");
        } else {
            // Heuristic defaults for common setups (e.g. Homebrew).
            let candidates = [
                "/opt/homebrew/include",
                "/usr/local/include",
                "/opt/local/include",
            ];
            for c in candidates {
                let hdr = PathBuf::from(c).join("SDL3/SDL.h");
                if hdr.exists() {
                    build.include(c);
                    have_sdl_headers = true;
                    println!(
                        "cargo:warning=dear-imgui-sdl3: using SDL3 headers from {}",
                        c
                    );
                    break;
                }
            }
        }
    }

    // Fallback: use the include path from sdl3-sys when it built SDL3 from source.
    // sdl3-sys prints cargo metadata like `CMAKE_DIR` and `OUT_DIR`, which Cargo
    // exposes as DEP_SDL3_CMAKE_DIR / DEP_SDL3_OUT_DIR for dependents.
    if !have_sdl_headers && let Ok(out_dir) = env::var("DEP_SDL3_OUT_DIR") {
        let out = PathBuf::from(out_dir);
        let include_root = out.join("include");
        let hdr = include_root.join("SDL3/SDL.h");
        if hdr.exists() {
            build.include(&include_root);
            have_sdl_headers = true;
            println!(
                "cargo:warning=dear-imgui-sdl3: using SDL3 headers from sdl3-sys OUT_DIR={}",
                include_root.display()
            );
        }
    }

    if !have_sdl_headers {
        panic!(
            "dear-imgui-sdl3: could not find SDL3 headers. \
             Install SDL3 development files (e.g. `brew install sdl3`), \
             set SDL3_INCLUDE_DIR to the SDL3 include path, \
             or enable `build-from-source` on the `sdl3` crate so sdl3-sys can \
             build SDL3 and expose headers via DEP_SDL3_OUT_DIR."
        );
    }

    // Backend sources come from the upstream Dear ImGui tree packaged by dear-imgui-sys.
    build.file(backends_root.join("imgui_impl_sdl3.cpp"));

    // Optional OpenGL3 renderer backend.
    if cfg!(feature = "opengl3-renderer") {
        build.define("DEAR_IMGUI_SDL3_OPENGL3_RENDERER", None);
        build.file(backends_root.join("imgui_impl_opengl3.cpp"));
    }

    // C wrappers used by Rust FFI (see wrapper.cpp).
    build.file("wrapper.cpp");

    build.compile("dear-imgui-sdl3-backend");
}
