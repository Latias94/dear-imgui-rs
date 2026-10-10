use dear_imgui_cte::{CteUiExt, Language, TextEditor};
use dear_imgui_rs::{Context, FramePrepareOptions};
use std::{env, process::Command};

const CHILD_ENV: &str = "DEAR_IMGUI_CTE_PANIC_CALLBACK_CHILD";

#[test]
fn panicking_callback_aborts_without_crossing_native_frames() {
    if env::var_os(CHILD_ENV).is_some() {
        let context = Context::create();
        let mut editor = TextEditor::create(&context);
        editor
            .set_language_change_callback(|| panic!("intentional callback panic"))
            .unwrap();
        editor.set_language(Some(Language::Cpp));
        unreachable!("the callback panic must abort the process");
    }

    let output = Command::new(env::current_exe().unwrap())
        .arg("--exact")
        .arg("panicking_callback_aborts_without_crossing_native_frames")
        .arg("--nocapture")
        .env(CHILD_ENV, "1")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("dear-imgui-cte: panic in language-change callback"));
}

#[test]
fn panicking_line_number_callback_aborts_without_crossing_render_frames() {
    const LINE_NUMBER_CHILD_ENV: &str = "DEAR_IMGUI_CTE_PANIC_LINE_NUMBER_CHILD";
    if env::var_os(LINE_NUMBER_CHILD_ENV).is_some() {
        let mut context = Context::create();
        context
            .font_atlas()
            .try_claim_legacy_renderer()
            .unwrap()
            .build();
        let mut editor = TextEditor::create(&context);
        editor.set_text("alpha\nbeta").unwrap();
        editor.set_show_line_numbers(true);
        editor
            .set_custom_line_number_callback(|_, _| panic!("intentional render callback panic"))
            .unwrap();
        for _ in 0..3 {
            context.prepare_frame(FramePrepareOptions::new([640.0, 480.0], 1.0 / 60.0));
            let ui = context.frame();
            ui.text_editor(&mut editor, "Line-number panic")
                .size([500.0, 280.0])
                .build()
                .unwrap();
            drop(context.render_legacy());
        }
        unreachable!("the line-number callback panic must abort the process");
    }

    let output = Command::new(env::current_exe().unwrap())
        .arg("--exact")
        .arg("panicking_line_number_callback_aborts_without_crossing_render_frames")
        .arg("--nocapture")
        .env(LINE_NUMBER_CHILD_ENV, "1")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("dear-imgui-cte: panic in custom-line-number callback")
    );
}
