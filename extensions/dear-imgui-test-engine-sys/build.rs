use build_support::{
    binding::BindingOwner::TestEngine,
    extension_build::{ExtensionSysSpec, SourceHooks, build_extension_sys},
};

#[path = "build/native.rs"]
mod native;

fn main() {
    build_extension_sys(ExtensionSysSpec {
        bindgen: cfg!(feature = "bindgen"),
        rerun_paths: &["build/native.rs", "shim"],
        source_hooks: Some(SourceHooks {
            check: native::check,
            build: native::build,
        }),
        ..ExtensionSysSpec::new(TestEngine, "IMGUI_TEST_ENGINE_SYS")
    });
}
