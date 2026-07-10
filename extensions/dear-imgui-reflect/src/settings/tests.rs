use crate::ReflectSession;

#[test]
fn sessions_own_independent_settings() {
    let mut left = ReflectSession::new();
    let mut right = ReflectSession::new();

    left.settings_mut().vec_mut().insertable = false;
    right.settings_mut().vec_mut().reorderable = false;

    assert!(!left.settings().vec().insertable);
    assert!(right.settings().vec().insertable);
    assert!(left.settings().vec().reorderable);
    assert!(!right.settings().vec().reorderable);
}
