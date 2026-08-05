pub(super) fn validate_git_revision(name: &str, value: &str) -> Result<(), String> {
    if value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(format!(
            "{name} must be exactly 40 ASCII hexadecimal characters"
        ))
    }
}

pub(super) fn validate_stable_hash(name: &str, value: &str) -> Result<(), String> {
    let Some(hex) = value.strip_prefix("fnv1a64:") else {
        return Err(format!(
            "binding provenance {name} hash has an invalid prefix"
        ));
    };
    if hex.len() == 16 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(format!(
            "binding provenance {name} hash must contain 16 ASCII hexadecimal characters"
        ))
    }
}
