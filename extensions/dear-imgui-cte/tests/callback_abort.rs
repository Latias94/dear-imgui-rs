use dear_imgui_cte::{Language, TextEditor};
use dear_imgui_rs::Context;
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
