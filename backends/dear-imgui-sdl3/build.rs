use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=wrapper.cpp");

    // Upstream Dear ImGui source tree is provided by dear-imgui-sys.
    let imgui_root = env::var("DEP_DEAR_IMGUI_THIRD_PARTY")
        .or_else(|_| env::var("DEP_DEAR_IMGUI_IMGUI_INCLUDE_PATH"))
        .expect(
            "DEP_DEAR_IMGUI_THIRD_PARTY or DEP_DEAR_IMGUI_IMGUI_INCLUDE_PATH not set. \
             Make sure dear-imgui-sys is built before dear-imgui-sdl3.",
        );

    let imgui_root = PathBuf::from(imgui_root);
    let imgui_backends = imgui_root.join("backends");

    let mut build = cc::Build::new();
    build.cpp(true).std("c++17");

    // Dear ImGui includes
    build.include(&imgui_root);
    build.include(&imgui_backends);

    // SDL3 includes:
    //
    // 1. Allow explicit override via SDL3_INCLUDE_DIR.
    // 2. Try pkg-config "sdl3" (if available).
    // 3. Try a few common default locations (Homebrew, /usr/local).
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

    if !have_sdl_headers {
        panic!(
            "dear-imgui-sdl3: could not find SDL3 headers. \
             Install SDL3 development files (e.g. `brew install sdl3`) \
             or set SDL3_INCLUDE_DIR to the SDL3 include path."
        );
    }

    // Backend sources: SDL3 platform + OpenGL3 renderer.
    build.file(imgui_backends.join("imgui_impl_sdl3.cpp"));
    build.file(imgui_backends.join("imgui_impl_opengl3.cpp"));

    // C wrappers used by Rust FFI (see wrapper.cpp).
    build.file("wrapper.cpp");

    build.compile("dear-imgui-sdl3-backend");
}
