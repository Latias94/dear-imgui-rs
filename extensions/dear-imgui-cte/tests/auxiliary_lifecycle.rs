use dear_imgui_cte::{CteError, Notifications, TextDiff, dejavu_font_source};
use dear_imgui_rs::{Context, sys};

#[test]
fn auxiliary_owners_bind_their_context_for_drop_and_restore_the_previous_one() {
    let context_a = Context::create();
    let diff = TextDiff::create(&context_a);
    let notifications = Notifications::create(&context_a);
    let suspended_a = context_a.suspend_or_panic();

    let context_b = Context::create();
    let raw_b = context_b.as_raw();
    drop(diff);
    assert_eq!(unsafe { sys::igGetCurrentContext() }, raw_b);
    drop(notifications);
    assert_eq!(unsafe { sys::igGetCurrentContext() }, raw_b);

    drop(context_b);
    drop(suspended_a.activate_or_panic());
}

#[test]
fn dead_owner_drop_does_not_touch_another_context() {
    let context_a = Context::create();
    let diff = TextDiff::create(&context_a);
    let notifications = Notifications::create(&context_a);
    let suspended_a = context_a.suspend_or_panic();
    drop(suspended_a);

    let context_b = Context::create();
    let raw_b = context_b.as_raw();
    drop(diff);
    drop(notifications);
    assert_eq!(unsafe { sys::igGetCurrentContext() }, raw_b);
    drop(context_b);
}

#[test]
fn auxiliary_instances_have_independent_native_owners() {
    let context = Context::create();
    let first_diff = TextDiff::create(&context);
    let second_diff = TextDiff::create(&context);
    let first_notifications = Notifications::create(&context);
    let second_notifications = Notifications::create(&context);

    assert_ne!(unsafe { first_diff.as_raw() }, unsafe {
        second_diff.as_raw()
    });
    assert_ne!(unsafe { first_notifications.as_raw() }, unsafe {
        second_notifications.as_raw()
    });
}

#[test]
fn bundled_dejavu_source_builds_through_the_managed_font_atlas() {
    let context = Context::create();
    let source = dejavu_font_source(15.0).unwrap();
    let _font = context.font_atlas().add_font(&[source]);
    context
        .font_atlas()
        .try_claim_legacy_renderer()
        .expect("headless CTE tests require the legacy font-atlas capability")
        .build();
}

#[test]
fn bundled_dejavu_source_rejects_invalid_reference_sizes() {
    assert!(matches!(
        dejavu_font_source(0.0),
        Err(CteError::InvalidValue { .. })
    ));
    assert!(matches!(
        dejavu_font_source(f32::NAN),
        Err(CteError::NonFinite { .. })
    ));
}
