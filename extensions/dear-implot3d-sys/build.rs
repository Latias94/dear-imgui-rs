use build_support::{
    binding::{BindingOwner::Extension, ExtensionBinding::ImPlot3d},
    extension_build::{
        CppInclude, CppProfile, ExtensionSysSpec, NativeProbe, SourceOverlay, build_extension_sys,
    },
};

fn main() {
    build_extension_sys(ExtensionSysSpec {
        bindgen: cfg!(feature = "bindgen"),
        prebuilt: cfg!(feature = "prebuilt"),
        build_from_source: cfg!(feature = "build-from-source"),
        wasm: cfg!(feature = "wasm"),
        includes: &[CppInclude::Source("implot3d")],
        defines: &[("IMGUI_DEFINE_MATH_OPERATORS", "1")],
        overlays: &[
            ("core", SourceOverlay::ImPlot3d),
            ("items", SourceOverlay::ImPlot3d),
            ("demo", SourceOverlay::ImPlot3d),
        ],
        rerun_paths: &[
            "third-party/cimplot3d/implot3d/implot3d.h",
            "third-party/cimplot3d/implot3d/implot3d_internal.h",
        ],
        probe: Some(NativeProbe {
            feature: "surface-test-probe",
            enabled: cfg!(feature = "surface-test-probe"),
            source: "src/surface_test_probe.cpp",
            library: "dear_implot3d_surface_test_probe",
            cpp_profile: CppProfile::CrtOnly,
        }),
        ..ExtensionSysSpec::new(Extension(ImPlot3d), "IMPLOT3D_SYS")
    });
}
