use build_support::{
    binding::{BindingOwner::Extension, ExtensionBinding::Cte},
    extension_build::{
        CppInclude, CppProfile, ExtensionSysSpec, SourceOverlay, build_extension_sys,
    },
    patch_cte_text_editor,
};

fn main() {
    build_extension_sys(ExtensionSysSpec {
        bindgen: cfg!(feature = "bindgen"),
        prebuilt: cfg!(feature = "prebuilt"),
        build_from_source: cfg!(feature = "build-from-source"),
        wasm: cfg!(feature = "wasm"),
        cpp_profile: CppProfile::Cte,
        includes: &[CppInclude::Source("ImGuiColorTextEdit")],
        overlays: &[(
            "text-editor",
            SourceOverlay::Text {
                output: "cte-text-editor.cpp",
                patch: patch_cte_text_editor,
            },
        )],
        rerun_paths: &[
            "third-party/cimCTE/ImGuiColorTextEdit/TextEditor.h",
            "third-party/cimCTE/ImGuiColorTextEdit/TextEditorLanguages.h",
            "third-party/cimCTE/ImGuiColorTextEdit/languages",
            "third-party/cimCTE/ImGuiColorTextEdit/TextDiff.h",
            "third-party/cimCTE/ImGuiColorTextEdit/dtl.h",
            "third-party/cimCTE/ImGuiColorTextEdit/example/dejavu.h",
            "third-party/cimCTE/ImGuiColorTextEdit/example/notosans.h",
            "third-party/cimCTE/ImGuiColorTextEdit/extras/TrieAutoComplete.h",
        ],
        ..ExtensionSysSpec::new(Extension(Cte), "CTE_SYS")
    });
}
