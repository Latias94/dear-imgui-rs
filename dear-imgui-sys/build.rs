use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=third-party/imgui");
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
        .flag_if_supported("-std=c++11")
        .flag_if_supported("-fno-exceptions")
        .flag_if_supported("-fno-rtti")
        .warnings(false) // Suppress warnings from Dear ImGui
        .define("IMGUI_DEFINE_MATH_OPERATORS", None)
        .define("IMGUI_USE_WCHAR32", None); // Use 32-bit characters for Rust compatibility

    // Platform-specific configuration
    match target_os.as_str() {
        "windows" => {
            build.define("WIN32_LEAN_AND_MEAN", None);
            build.define("NOMINMAX", None);
            build.define("IMGUI_DISABLE_WIN32_FUNCTIONS", None);
        }
        "macos" => {
            build.define("IMGUI_DISABLE_OSX_FUNCTIONS", None);
        }
        _ => {}
    }

    // Feature-specific configuration
    if cfg!(feature = "docking") {
        build.define("IMGUI_ENABLE_DOCKING", None);
        println!("cargo:warning=Docking feature enabled. Using Dear ImGui docking branch for full docking support.");
    }

    if cfg!(feature = "multi-viewport") {
        build.define("IMGUI_ENABLE_VIEWPORTS", None);
        println!("cargo:warning=Multi-viewport feature enabled. Note: This requires docking branch and platform backend support.");
    }

    if cfg!(feature = "tables") {
        build.define("IMGUI_ENABLE_TABLES", None);
    }

    if cfg!(feature = "freetype") {
        build.define("IMGUI_ENABLE_FREETYPE", None);

        // Try to find FreeType library
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

    // Add basic clang arguments for C++
    builder = builder
        .clang_arg("-x")
        .clang_arg("c++")
        .clang_arg("-std=c++11")
        .clang_arg("-DIMGUI_USE_WCHAR32");

    // Add feature-specific defines
    if cfg!(feature = "docking") {
        builder = builder.clang_arg("-DIMGUI_ENABLE_DOCKING");
    }

    if cfg!(feature = "multi-viewport") {
        builder = builder.clang_arg("-DIMGUI_ENABLE_VIEWPORTS");
    }

    if cfg!(feature = "tables") {
        builder = builder.clang_arg("-DIMGUI_ENABLE_TABLES");
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
