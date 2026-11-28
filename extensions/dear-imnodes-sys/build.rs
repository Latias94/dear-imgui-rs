use build_support::{
    binding::{BindingOwner::Extension, ExtensionBinding::ImNodes},
    extension_build::{CppInclude, ExtensionSysSpec, SourceOverlay, build_extension_sys},
    patch_imnodes_cpp_for_imgui,
};

fn main() {
    build_extension_sys(ExtensionSysSpec {
        bindgen: cfg!(feature = "bindgen"),
        prebuilt: cfg!(feature = "prebuilt"),
        build_from_source: cfg!(feature = "build-from-source"),
        wasm: cfg!(feature = "wasm"),
        artifact_features: &[("freetype", cfg!(feature = "freetype"))],
        includes: &[CppInclude::Crate("shim"), CppInclude::Source("imnodes")],
        defines: &[("IMNODES_NAMESPACE", "imnodes")],
        overlays: &[(
            "core",
            SourceOverlay::Text {
                output: "imnodes-imgui-193.cpp",
                patch: patch_imnodes_cpp_for_imgui,
            },
        )],
        rerun_paths: &[
            "third-party/cimnodes/imnodes/imnodes.h",
            "third-party/cimnodes/imnodes/imnodes_internal.h",
        ],
        ..ExtensionSysSpec::new(Extension(ImNodes), "IMNODES_SYS")
    });
}
