use super::{BINDGEN_EXTRA_CLANG_ARGS_PREFIX, CORE_WASM_TARGET};

pub fn bindgen_rerun_env_vars(target: &str) -> Vec<String> {
    let mut names = vec![BINDGEN_EXTRA_CLANG_ARGS_PREFIX.to_owned()];
    if !target.is_empty() {
        names.push(format!("{BINDGEN_EXTRA_CLANG_ARGS_PREFIX}_{target}"));
        names.push(format!(
            "{BINDGEN_EXTRA_CLANG_ARGS_PREFIX}_{}",
            target.replace('-', "_")
        ));
    }
    names.sort_unstable();
    names.dedup();
    names
}

pub fn validate_bindgen_environment<I, K>(names: I) -> Result<(), String>
where
    I: IntoIterator<Item = K>,
    K: AsRef<str>,
{
    let mut forbidden = names
        .into_iter()
        .map(|name| name.as_ref().to_owned())
        .filter(|name| name.starts_with(BINDGEN_EXTRA_CLANG_ARGS_PREFIX))
        .collect::<Vec<_>>();
    forbidden.sort_unstable();
    forbidden.dedup();
    if forbidden.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "canonical core binding generation rejects environment overrides: {}",
            forbidden.join(", ")
        ))
    }
}

pub fn is_supported_wasm_target(target_triple: &str) -> bool {
    target_triple == CORE_WASM_TARGET
}

pub fn validate_wasm_feature_contract(
    target_triple: &str,
    wasm_feature_enabled: bool,
) -> Result<(), String> {
    if is_supported_wasm_target(target_triple) {
        if wasm_feature_enabled {
            Ok(())
        } else {
            Err(format!(
                "{CORE_WASM_TARGET} requires the explicit `wasm` feature"
            ))
        }
    } else if target_triple.starts_with("wasm32") {
        Err(format!(
            "unsupported Dear ImGui WASM target `{target_triple}`; \
             only `{CORE_WASM_TARGET}` with the `wasm` feature is supported"
        ))
    } else {
        Ok(())
    }
}
