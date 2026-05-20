use super::*;

#[test]
fn settings_scope_restores_previous_settings() {
    with_settings(|settings| {
        settings.vec_mut().insertable = true;
    });

    with_settings_scope(|| {
        with_settings(|settings| {
            settings.vec_mut().insertable = false;
        });
        assert!(!current_settings().vec().insertable);
    });

    assert!(current_settings().vec().insertable);
}
