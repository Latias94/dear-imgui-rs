use build_support::{
    binding::{BindingOwner::Extension, ExtensionBinding::ImPlot},
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
        artifact_features: &[("freetype", cfg!(feature = "freetype"))],
        cpp_profile: CppProfile::ImPlot,
        includes: &[CppInclude::Source("implot")],
        defines: &[("IMGUI_DEFINE_MATH_OPERATORS", "1")],
        overlays: &[
            ("core", SourceOverlay::ImPlot),
            ("items", SourceOverlay::ImPlot),
            ("demo", SourceOverlay::ImPlot),
        ],
        rerun_paths: &[
            "third-party/cimplot/implot/implot.h",
            "third-party/cimplot/implot/implot_internal.h",
        ],
        probe: Some(NativeProbe {
            feature: "native-test-probe",
            enabled: cfg!(feature = "native-test-probe"),
            source: "src/line_test_probe.cpp",
            library: "dear_implot_line_test_probe",
            cpp_profile: CppProfile::ImPlot,
        }),
        ..ExtensionSysSpec::new(Extension(ImPlot), "IMPLOT_SYS")
    });
}
