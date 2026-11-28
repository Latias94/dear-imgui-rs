use dear_imgui_cte::{CteError, Palette, TextDiff, dejavu_font_source, noto_sans_sc_font_source};
use dear_imgui_rs::{Context, sys};

#[test]
fn auxiliary_owners_bind_their_context_for_drop_and_restore_the_previous_one() {
    let context_a = Context::create();
    let diff = TextDiff::create(&context_a);

    let context_b = Context::create();
    let raw_b = context_b.as_raw();
    drop(diff);
    assert_eq!(unsafe { sys::igGetCurrentContext() }, raw_b);

    drop(context_b);
    drop(context_a);
}

#[test]
fn dead_owner_drop_preserves_another_current_context() {
    let context_a = Context::create();
    let diff = TextDiff::create(&context_a);
    drop(context_a);

    let context_b = Context::create();
    let raw_b = context_b.as_raw();
    drop(diff);
    assert_eq!(unsafe { sys::igGetCurrentContext() }, raw_b);
    drop(context_b);
}

#[test]
fn auxiliary_instances_have_independent_native_owners() {
    let context = Context::create();
    let first_diff = TextDiff::create(&context);
    let second_diff = TextDiff::create(&context);

    assert_ne!(unsafe { first_diff.as_raw() }, unsafe {
        second_diff.as_raw()
    });
}

#[test]
fn built_in_palettes_respect_the_imgui_owner_thread() {
    let context = Context::create();
    let other_thread = std::thread::spawn(|| std::panic::catch_unwind(Palette::dark).is_err());
    assert!(other_thread.join().unwrap());
    assert_ne!(Palette::dark(), Palette::light());
    drop(context);
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

#[test]
fn bundled_noto_source_builds_through_the_managed_font_atlas() {
    let context = Context::create();
    let source = noto_sans_sc_font_source(15.0).unwrap();
    let _font = context.font_atlas().add_font(&[source]);
    context
        .font_atlas()
        .try_claim_legacy_renderer()
        .expect("headless CTE tests require the legacy font-atlas capability")
        .build();
    assert!(matches!(
        noto_sans_sc_font_source(0.0),
        Err(CteError::InvalidValue { .. })
    ));
    assert!(matches!(
        noto_sans_sc_font_source(f32::NAN),
        Err(CteError::NonFinite { .. })
    ));
}
