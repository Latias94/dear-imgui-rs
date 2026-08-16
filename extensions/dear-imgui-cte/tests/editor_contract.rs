use dear_imgui_cte::{
    CteUiExt, Language, Palette, PaletteColor, Position, SearchOptions, Selection, SquiggleKind,
    TextEditor,
};
use dear_imgui_rs::{ChildFlags, Context};

fn render_context() -> Context {
    let mut context = Context::create();
    context.io_mut().set_display_size([640.0, 480.0]);
    context.io_mut().set_delta_time(1.0 / 60.0);
    context
        .font_atlas()
        .try_claim_legacy_renderer()
        .expect("headless CTE tests require the legacy font-atlas capability")
        .build();
    context
}

#[test]
fn editors_keep_configuration_and_document_state_independent() {
    let context = Context::create();
    let mut first = TextEditor::create(&context);
    let mut second = TextEditor::create(&context);

    first.set_text("alpha beta\nsecond line\nthird").unwrap();
    second.set_text("independent").unwrap();
    first.set_language(Some(Language::Cpp));
    second.set_language(Some(Language::Python));
    first.set_tab_size(2).unwrap();
    second.set_tab_size(8).unwrap();
    first.set_word_wrap_enabled(true);
    second.set_word_wrap_enabled(false);

    let mut first_palette = Palette::dark();
    first_palette.set(PaletteColor::Keyword, 0x1122_3344);
    first.set_palette(&first_palette).unwrap();
    second.set_palette(&Palette::light()).unwrap();

    assert_eq!(first.text().unwrap(), "alpha beta\nsecond line\nthird");
    assert_eq!(second.text().unwrap(), "independent");
    assert_eq!(first.language(), Some(Language::Cpp));
    assert_eq!(second.language(), Some(Language::Python));
    assert_eq!(first.language_name().unwrap(), "C++");
    assert_eq!(second.language_name().unwrap(), "Python");
    assert_eq!(first.tab_size(), 2);
    assert_eq!(second.tab_size(), 8);
    assert!(first.is_word_wrap_enabled());
    assert!(!second.is_word_wrap_enabled());
    assert_eq!(first.palette().get(PaletteColor::Keyword), 0x1122_3344);
    assert_ne!(first.palette(), second.palette());
}

#[test]
fn editing_navigation_diagnostics_and_undo_are_safe() {
    let context = Context::create();
    let mut editor = TextEditor::create(&context);
    editor.set_text("alpha beta\nsecond line\nthird").unwrap();

    let alpha = Selection::new(Position::new(0, 0), Position::new(0, 5));
    assert_eq!(editor.section_text(alpha).unwrap(), "alpha");
    editor.select_region(alpha).unwrap();
    assert_eq!(editor.main_cursor_selection(), alpha);
    editor.replace_section(alpha, "omega").unwrap();
    assert_eq!(editor.line_text(0).unwrap(), "omega beta");
    assert!(editor.can_undo());
    editor.undo();
    assert_eq!(editor.line_text(0).unwrap(), "alpha beta");
    assert!(editor.can_redo());
    editor.redo();

    editor
        .select_first_occurrence("beta", SearchOptions::default())
        .unwrap();
    assert!(editor.current_cursor_has_selection());
    editor.add_marker(1, 1, 2, "line", "text").unwrap();
    assert!(editor.has_markers());
    editor
        .add_squiggle(
            Selection::new(Position::new(1, 0), Position::new(1, 6)),
            SquiggleKind::new(7),
            0xFF00_00FF,
            "diagnostic",
        )
        .unwrap();
    assert!(editor.has_squiggles());
    editor.clear_squiggles_of_kind(SquiggleKind::new(7));
    editor.clear_markers();
    assert!(!editor.has_markers());

    editor.select_line(1).unwrap();
    editor.indent_lines();
    editor.deindent_lines();
    editor.toggle_comments();
    editor.toggle_comments();
    editor.scroll_to_line(2, Default::default()).unwrap();
    editor.set_cursor(Position::new(2, 5)).unwrap();
    assert_eq!(editor.current_cursor_position(), Position::new(2, 5));
}

#[test]
fn two_editors_render_across_multiple_frames() {
    let mut context = render_context();
    let mut first = TextEditor::create(&context);
    let mut second = TextEditor::create(&context);
    first.set_text("first").unwrap();
    second.set_text("second").unwrap();

    for frame in 0..2 {
        let ui = context.frame();
        assert!(
            ui.text_editor(&mut first, format!("First##{frame}"))
                .size([300.0, 160.0])
                .build()
                .is_ok()
        );
        assert!(
            ui.text_editor(&mut second, format!("Second##{frame}"))
                .size([300.0, 160.0])
                .build()
                .is_ok()
        );
        drop(context.render_legacy());
    }

    assert_eq!(first.text().unwrap(), "first");
    assert_eq!(second.text().unwrap(), "second");
}

#[test]
fn render_builder_rejects_invalid_values_before_native_rendering() {
    let mut context = render_context();
    let mut editor = TextEditor::create(&context);
    let ui = context.frame();

    assert!(matches!(
        ui.text_editor(&mut editor, "bad\0title").build(),
        Err(dear_imgui_cte::CteError::InteriorNul { .. })
    ));
    assert!(matches!(
        ui.text_editor(&mut editor, "bad size")
            .size([f32::NAN, 10.0])
            .build(),
        Err(dear_imgui_cte::CteError::NonFinite { .. })
    ));
    assert!(matches!(
        ui.text_editor(&mut editor, "bad flags")
            .child_flags(ChildFlags::from_bits_retain(u32::MAX))
            .build(),
        Err(dear_imgui_cte::CteError::InvalidValue { .. })
    ));

    drop(context.render_legacy());
}
