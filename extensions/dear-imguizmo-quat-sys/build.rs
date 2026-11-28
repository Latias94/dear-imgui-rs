use build_support::{
    binding::{BindingOwner::Extension, ExtensionBinding::ImGuizmoQuat},
    extension_build::{CppInclude, ExtensionSysSpec, build_extension_sys},
};

fn main() {
    build_extension_sys(ExtensionSysSpec {
        bindgen: cfg!(feature = "bindgen"),
        prebuilt: cfg!(feature = "prebuilt"),
        build_from_source: cfg!(feature = "build-from-source"),
        wasm: cfg!(feature = "wasm"),
        artifact_features: &[("freetype", cfg!(feature = "freetype"))],
        includes: &[CppInclude::Source("imGuIZMO.quat/imguizmo_quat")],
        // The forwarding translation unit already includes imGuIZMOquat.cpp.
        include_only: &["quat"],
        ..ExtensionSysSpec::new(Extension(ImGuizmoQuat), "IMGUIZMO_QUAT_SYS")
    });
}
