use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=imgui");
    println!("cargo:rerun-if-changed=wrapper.h");
    println!("cargo:rerun-if-changed=wrapper.cpp");

    // Build Dear ImGui C++ library
    build_imgui();

    // Generate Rust bindings
    generate_bindings();
}

fn build_imgui() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();

    let mut build = cc::Build::new();

    // Configure C++ compilation
    build
        .cpp(true)
        .include("third-party/imgui")
        .flag_if_supported("-std=c++17")
        .flag_if_supported("-fno-exceptions")
        .flag_if_supported("-fno-rtti")
        .warnings(false) // Suppress warnings from Dear ImGui
        .define("IMGUI_DEFINE_MATH_OPERATORS", None)
        .define("IMGUI_USE_WCHAR32", None) // Use 32-bit characters for Rust compatibility
        .define("IMGUI_DISABLE_OBSOLETE_FUNCTIONS", None)
        .define("IMGUI_DISABLE_OBSOLETE_KEYIO", None);

    // Platform-specific configuration
    match target_os.as_str() {
        "windows" => {
            build.define("WIN32_LEAN_AND_MEAN", None);
            build.define("NOMINMAX", None);
            build.define("IMGUI_DISABLE_WIN32_DEFAULT_CLIPBOARD_FUNCTIONS", None);
        }
        "macos" => {
            build.define("IMGUI_DISABLE_OSX_FUNCTIONS", None);
        }
        _ => {}
    }

    // Feature-specific configuration
    if cfg!(feature = "docking") {
        // Docking is enabled by default since we're using the docking branch
        println!("cargo:warning=Using Dear ImGui docking branch with full docking support.");
    }

    if cfg!(feature = "freetype") {
        build.define("IMGUI_ENABLE_FREETYPE", None);

        // Try to find FreeType library
        #[cfg(feature = "freetype")]
        match pkg_config::probe_library("freetype2") {
            Ok(freetype) => {
                for include_path in freetype.include_paths {
                    build.include(include_path);
                }
            }
            Err(_) => {
                println!("cargo:warning=FreeType not found via pkg-config. Please ensure FreeType is installed.");
            }
        }
    }

    // Use the wrapper.cpp file to compile everything together
    build.file("wrapper.cpp");

    // Compile the library
    build.compile("dear_imgui");
}

fn generate_bindings() {
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap();

    // Configure bindgen with simpler settings
    let mut builder = bindgen::Builder::default()
        .header("wrapper.h")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        // Generate bindings for ImGui functions and types
        .allowlist_function("ig.*")
        .allowlist_function("Im.*")
        .allowlist_type("Im.*")
        .allowlist_var("Im.*")
        // Derive common traits
        .derive_default(true)
        .derive_debug(true)
        .derive_copy(true)
        .derive_eq(true)
        .derive_partialeq(true)
        .derive_hash(true)
        // Prepend enum names to avoid conflicts
        .prepend_enum_name(false)
        // Generate layout tests
        .layout_tests(false); // Disable for now to avoid issues

    // MSVC ABI fix: blocklist functions that return ImVec2
    if target_env == "msvc" {
        println!("cargo:rerun-if-changed=msvc_blocklist.txt");
        println!("cargo:rerun-if-changed=hack_msvc.cpp");

        // Read blocklist file if it exists
        if let Ok(blocklist_content) = std::fs::read_to_string("msvc_blocklist.txt") {
            let mut blocked_functions = Vec::new();
            for line in blocklist_content.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                blocked_functions.push(line);
                builder = builder.blocklist_function(line);
            }

            // Print diagnostic information
            println!(
                "cargo:warning=Applied MSVC ABI fixes for {} functions",
                blocked_functions.len()
            );
            if std::env::var("DEAR_IMGUI_VERBOSE").is_ok() {
                for func in &blocked_functions {
                    println!("cargo:warning=  - Blocked function: {}", func);
                }
            }
        }

        // Include MSVC hack header
        builder = builder
            .header("hack_msvc.cpp")
            .allowlist_file("hack_msvc.cpp");
    }

    // Add basic clang arguments for C++
    builder = builder
        .clang_arg("-x")
        .clang_arg("c++")
        .clang_arg("-std=c++17")
        .clang_arg("-DIMGUI_USE_WCHAR32")
        .clang_arg("-DIMGUI_DISABLE_OBSOLETE_FUNCTIONS")
        .clang_arg("-DIMGUI_DISABLE_OBSOLETE_KEYIO");

    // Add feature-specific defines
    if cfg!(feature = "docking") {
        // Docking branch is used by default
        builder = builder.clang_arg("-DIMGUI_HAS_DOCK");
    }

    if cfg!(feature = "freetype") {
        builder = builder.clang_arg("-DIMGUI_ENABLE_FREETYPE");
    }

    // Generate the bindings
    let bindings = builder.generate().expect("Unable to generate bindings");

    // Write the bindings to the output file
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
