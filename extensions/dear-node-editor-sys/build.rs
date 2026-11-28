use build_support::{
    binding::{BindingOwner::Extension, ExtensionBinding::NodeEditor},
    extension_build::{CppInclude, ExtensionSysSpec, build_extension_sys},
};

fn main() {
    build_extension_sys(ExtensionSysSpec {
        bindgen: cfg!(feature = "bindgen"),
        prebuilt: cfg!(feature = "prebuilt"),
        build_from_source: cfg!(feature = "build-from-source"),
        artifact_features: &[
            ("freetype", cfg!(feature = "freetype")),
            ("stack-layout", cfg!(feature = "stack-layout")),
        ],
        includes: &[
            CppInclude::Source("imgui-node-editor"),
            CppInclude::Crate("shim"),
        ],
        // Upstream crude_json.cpp uses std::terminate without including <exception>.
        force_include: Some("exception"),
        rerun_paths: &[
            "third-party/cimnodes_editor/cimnodes_editor.h",
            "third-party/cimnodes_editor/imgui-node-editor/imgui_node_editor.h",
        ],
        ..ExtensionSysSpec::new(Extension(NodeEditor), "NODE_EDITOR_SYS")
    });
}
