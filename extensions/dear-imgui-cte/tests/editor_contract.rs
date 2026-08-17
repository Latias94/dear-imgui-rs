use dear_imgui_cte::{
    CteUiExt, Language, Palette, PaletteColor, Position, SearchOptions, Selection, SquiggleKind,
    TextEditor,
};
use dear_imgui_rs::{ChildFlags, Condition, Context, MouseButton, WindowFlags};

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

fn render_editor_host(
    context: &mut Context,
    editor: &mut TextEditor,
    initial_position: Option<[f32; 2]>,
) -> ([f32; 2], [f32; 2]) {
    let ui = context.frame();
    let window = ui
        .window("CTE interaction host")
        .size([420.0, 240.0], Condition::Always)
        .flags(WindowFlags::NO_SAVED_SETTINGS);
    let window = if let Some(position) = initial_position {
        window.position(position, Condition::Always)
    } else {
        window
    };

    let snapshot = window
        .build(|| {
            let window_position = ui.window_pos();
            let editor_origin = ui.cursor_screen_pos();
            ui.text_editor(editor, "Source")
                .build()
                .expect("editor render should succeed");
            (window_position, editor_origin)
        })
        .expect("host window should be visible");
    drop(context.render_legacy());
    snapshot
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
fn upstream_configuration_defaults_remain_representable() {
    let context = Context::create();
    let mut editor = TextEditor::create(&context);

    assert_eq!(editor.minimap_columns(), 0);
    editor.set_minimap_columns(48);
    assert_eq!(editor.minimap_columns(), 48);
    editor.set_minimap_columns(0);
    assert_eq!(editor.minimap_columns(), 0);

    assert_eq!(
        SearchOptions::default(),
        SearchOptions {
            case_sensitive: true,
            whole_word: false,
        }
    );
}

#[test]
fn default_occurrence_search_preserves_upstream_case_sensitivity() {
    let context = Context::create();
    let mut editor = TextEditor::create(&context);
    editor.set_text("Beta beta").unwrap();

    editor
        .select_first_occurrence("beta", SearchOptions::default())
        .unwrap();

    assert_eq!(
        editor.main_cursor_selection(),
        Selection::new(Position::new(0, 5), Position::new(0, 9))
    );
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

    for _ in 0..2 {
        let ui = context.frame();
        assert!(
            ui.text_editor(&mut first, "First")
                .size([300.0, 160.0])
                .build()
                .is_ok()
        );
        assert!(
            ui.text_editor(&mut second, "Second")
                .size([300.0, 160.0])
                .build()
                .is_ok()
        );
        drop(context.render_legacy());
    }

    assert_eq!(first.text().unwrap(), "first");
    assert_eq!(second.text().unwrap(), "second");
    assert!(first.line_height().is_some_and(|value| value > 0.0));
    assert!(first.glyph_width().is_some_and(|value| value > 0.0));
}

#[test]
fn layout_dependent_mouse_queries_are_safe_before_visible_layout() {
    let context = Context::create();
    let editor = TextEditor::create(&context);
    let mouse_position = [8.0, 8.0];

    assert_eq!(editor.line_height(), None);
    assert_eq!(editor.glyph_width(), None);
    assert!(!editor.is_mouse_over_glyph(mouse_position).unwrap());
    assert!(!editor.is_mouse_over_text_area(mouse_position).unwrap());
    assert_eq!(
        editor.position_at_mouse(mouse_position).unwrap(),
        Position::default()
    );
    assert_eq!(editor.word_at_mouse(mouse_position).unwrap(), "");
}

#[test]
fn layout_dependent_mouse_queries_are_invalidated_after_document_changes() {
    let mut context = render_context();
    let mut editor = TextEditor::create(&context);
    let text = (0..100)
        .map(|line| format!("line {line} with enough text"))
        .collect::<Vec<_>>()
        .join("\n");
    editor.set_text(&text).unwrap();

    let (_, editor_origin) = render_editor_host(&mut context, &mut editor, Some([80.0, 80.0]));
    let mouse_position = [editor_origin[0] + 12.0, editor_origin[1] + 12.0];
    assert!(editor.line_height().is_some_and(|value| value > 0.0));
    assert!(editor.glyph_width().is_some_and(|value| value > 0.0));

    editor.set_text("short").unwrap();

    assert_eq!(editor.line_height(), None);
    assert_eq!(editor.glyph_width(), None);
    assert!(!editor.is_mouse_over_glyph(mouse_position).unwrap());
    assert!(!editor.is_mouse_over_text_area(mouse_position).unwrap());
    assert_eq!(
        editor.position_at_mouse(mouse_position).unwrap(),
        Position::default()
    );
    assert_eq!(editor.word_at_mouse(mouse_position).unwrap(), "");

    render_editor_host(&mut context, &mut editor, None);
    assert!(editor.line_height().is_some_and(|value| value > 0.0));
    assert!(editor.glyph_width().is_some_and(|value| value > 0.0));
}

#[test]
fn drag_selection_stays_in_the_editor_instead_of_moving_its_host_window() {
    let mut context = render_context();
    let mut editor = TextEditor::create(&context);
    editor
        .set_text("alpha beta gamma delta epsilon zeta eta theta")
        .unwrap();

    let (initial_window_position, editor_origin) =
        render_editor_host(&mut context, &mut editor, Some([80.0, 80.0]));

    let mut drag_points = None;
    'rows: for y_offset in (2..40).step_by(2) {
        let mut start = None;
        for x_offset in (2..320).step_by(2) {
            let mouse_position = [
                editor_origin[0] + x_offset as f32,
                editor_origin[1] + y_offset as f32,
            ];
            if !editor.is_mouse_over_glyph(mouse_position).unwrap() {
                continue;
            }

            let position = editor.position_at_mouse(mouse_position).unwrap();
            if position.line != 0 || position.column < 2 {
                continue;
            }

            let (start_position, start_mouse) = *start.get_or_insert((position, mouse_position));
            if position.column >= start_position.column + 5 {
                drag_points = Some((start_mouse, mouse_position));
                break 'rows;
            }
        }
    }
    let (press_position, drag_position) =
        drag_points.expect("the rendered first line should expose draggable glyphs");

    context.io_mut().add_mouse_pos_event(press_position);
    let (hovered_window_position, _) = render_editor_host(&mut context, &mut editor, None);

    context
        .io_mut()
        .add_mouse_button_event(MouseButton::Left, true);
    let (pressed_window_position, _) = render_editor_host(&mut context, &mut editor, None);

    context.io_mut().add_mouse_pos_event(drag_position);
    let (dragged_window_position, _) = render_editor_host(&mut context, &mut editor, None);

    context
        .io_mut()
        .add_mouse_button_event(MouseButton::Left, false);
    let (released_window_position, _) = render_editor_host(&mut context, &mut editor, None);

    assert_eq!(hovered_window_position, initial_window_position);
    assert_eq!(pressed_window_position, initial_window_position);
    assert_eq!(dragged_window_position, initial_window_position);
    assert_eq!(released_window_position, initial_window_position);
    assert!(editor.current_cursor_has_selection());
    assert!(!editor.main_cursor_selection().is_empty());
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
