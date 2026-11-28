use build_support::{
    binding::{BindingOwner::Extension, ExtensionBinding::ImGuizmo},
    extension_build::{CppInclude, ExtensionSysSpec, SourceOverlay, build_extension_sys},
    patch_imguizmo_cpp,
};

fn main() {
    build_extension_sys(ExtensionSysSpec {
        bindgen: cfg!(feature = "bindgen"),
        prebuilt: cfg!(feature = "prebuilt"),
        build_from_source: cfg!(feature = "build-from-source"),
        wasm: cfg!(feature = "wasm"),
        artifact_features: &[("freetype", cfg!(feature = "freetype"))],
        includes: &[
            CppInclude::Source("ImGuizmo"),
            CppInclude::Source("ImGuizmo/src"),
            CppInclude::Crate("shim"),
        ],
        overlays: &[(
            "implementation",
            SourceOverlay::Text {
                output: "ImGuizmo-patched.cpp",
                patch: patch_imguizmo_cpp,
            },
        )],
        include_only: &["reset-header", "reset-implementation"],
        // A directory watch catches transitions between the two supported ImGuizmo layouts.
        rerun_paths: &["third-party/cimguizmo", "shim"],
        ..ExtensionSysSpec::new(Extension(ImGuizmo), "IMGUIZMO_SYS")
    });
}
