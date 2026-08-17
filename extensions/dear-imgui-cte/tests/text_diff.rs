use dear_imgui_cte::{CteError, CteUiExt, Language, Palette, PaletteColor, TextDiff};
use dear_imgui_rs::{ChildFlags, Condition, Context, FramePrepareOptions, WindowFlags};

fn render_context() -> Context {
    let mut context = Context::create();
    context.prepare_frame(FramePrepareOptions::new([640.0, 480.0], 1.0 / 60.0));
    context
        .font_atlas()
        .try_claim_legacy_renderer()
        .expect("headless CTE tests require the legacy font-atlas capability")
        .build();
    context
}

#[test]
fn text_diffs_keep_configuration_independent() {
    let context = Context::create();
    let mut first = TextDiff::create(&context);
    let mut second = TextDiff::create(&context);

    first.set_text("alpha\nbeta", "alpha\ngamma").unwrap();
    second.set_text("left", "right").unwrap();
    first.set_side_by_side(true);
    second.set_side_by_side(false);
    first.set_tab_size(2).unwrap();
    second.set_tab_size(8).unwrap();
    first.set_line_spacing(4.0).unwrap();
    second.set_line_spacing(1.25).unwrap();
    first.set_language(Some(Language::Cpp));
    second.set_language(Some(Language::Python));
    first.set_word_wrap_enabled(true);
    first.set_show_whitespaces(true);
    second.set_show_scrollbar_minimap(false);
    first.set_colors(0x1122_3344, 0x5566_7788);

    let mut palette = Palette::dark();
    palette.set(PaletteColor::Keyword, 0xAABB_CCDD);
    first.set_palette(&palette).unwrap();

    assert_ne!(unsafe { first.as_raw() }, unsafe { second.as_raw() });
    assert!(first.is_side_by_side());
    assert!(!second.is_side_by_side());
    assert_eq!(first.tab_size(), 2);
    assert_eq!(second.tab_size(), 8);
    assert_eq!(first.line_spacing(), 2.0);
    assert_eq!(second.line_spacing(), 1.25);
    assert_eq!(first.language(), Some(Language::Cpp));
    assert_eq!(second.language(), Some(Language::Python));
    assert!(first.is_word_wrap_enabled());
    assert!(first.shows_whitespaces());
    assert!(!second.shows_scrollbar_minimap());
    assert_eq!(first.palette().get(PaletteColor::Keyword), 0xAABB_CCDD);
    assert_ne!(first.palette(), second.palette());
}

#[test]
fn integrated_and_side_by_side_views_render_across_frames() {
    let mut context = render_context();
    let mut integrated = TextDiff::create(&context);
    let mut side_by_side = TextDiff::create(&context);
    integrated
        .set_text("same\nremoved\ntail", "same\nadded\ntail")
        .unwrap();
    side_by_side
        .set_text("fn old() {}", "fn new() {\n    true\n}")
        .unwrap();
    side_by_side.set_side_by_side(true);
    side_by_side.focus();

    for frame in 0..3 {
        let ui = context.frame();
        ui.window("Text diff host")
            .size([620.0, 420.0], Condition::Always)
            .build(|| {
                ui.text_diff(&mut integrated, format!("Integrated##{frame}"))
                    .size([300.0, 180.0])
                    .build()
                    .unwrap();
                ui.text_diff(&mut side_by_side, format!("SideBySide##{frame}"))
                    .size([300.0, 180.0])
                    .window_flags(WindowFlags::NO_SAVED_SETTINGS)
                    .build()
                    .unwrap();
            });
        let draw_data = context.render_legacy();
        assert!(draw_data.total_vtx_count() > 0);
    }
}

#[test]
fn text_diff_validates_inputs_before_rendering() {
    let context = Context::create();
    let mut diff = TextDiff::create(&context);

    assert!(matches!(
        diff.set_tab_size(0),
        Err(CteError::InvalidValue { .. })
    ));
    assert!(matches!(
        diff.set_line_spacing(f32::INFINITY),
        Err(CteError::NonFinite { .. })
    ));
    assert!(matches!(
        diff.set_text("valid", "bad\0right"),
        Err(CteError::InteriorNul { .. })
    ));
    drop(diff);
    drop(context);

    let mut context = render_context();
    let mut diff = TextDiff::create(&context);
    let ui = context.frame();
    assert!(matches!(
        ui.text_diff(&mut diff, "bad\0title").build(),
        Err(CteError::InteriorNul { .. })
    ));
    assert!(matches!(
        ui.text_diff(&mut diff, "bad size")
            .size([f32::NAN, 10.0])
            .build(),
        Err(CteError::NonFinite { .. })
    ));
    assert!(matches!(
        ui.text_diff(&mut diff, "bad child flags")
            .child_flags(ChildFlags::from_bits_retain(u32::MAX))
            .build(),
        Err(CteError::InvalidValue { .. })
    ));
    assert!(matches!(
        ui.text_diff(&mut diff, "bad window flags")
            .window_flags(WindowFlags::from_bits_retain(i32::MAX))
            .build(),
        Err(CteError::InvalidValue { .. })
    ));
    drop(context.render_legacy());
}
