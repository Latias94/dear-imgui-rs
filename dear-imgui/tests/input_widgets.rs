use dear_imgui_rs as imgui;
use std::panic::{AssertUnwindSafe, catch_unwind, panic_any};
use std::sync::{Mutex, OnceLock};

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(())).lock().unwrap()
}

fn input_context() -> imgui::Context {
    let mut ctx = imgui::Context::create();
    ctx.io_mut().set_display_size([800.0, 600.0]);
    ctx.font_atlas()
        .try_claim_legacy_renderer()
        .expect("legacy renderer font atlas should be available")
        .build();
    ctx.set_ini_filename::<std::path::PathBuf>(None).unwrap();
    ctx
}

#[test]
fn edit_callback_reads_native_growth_and_writes_utf8_back_to_the_string() {
    struct Append<'a> {
        seen: &'a mut String,
        ui: &'a imgui::Ui,
    }
    impl imgui::InputTextCallbackHandler for Append<'_> {
        fn on_edit(&mut self, mut data: imgui::TextCallbackData<'_>) {
            self.ui.is_popup_open("callback query".repeat(1024));
            self.seen.push_str(data.str().unwrap());
            data.push_str(" — done");
        }
    }

    let _guard = test_guard();
    let mut ctx = input_context();
    let input = "café with more text";
    let mut text = String::new();
    let mut seen = String::new();
    for frame in 0..3 {
        if frame == 2 {
            ctx.io_mut().add_input_characters_utf8(input);
        }
        let ui = ctx.frame();
        ui.window("edit callback")
            .size([400.0, 180.0], imgui::Condition::Always)
            .build(|| {
                if frame == 0 {
                    ui.set_keyboard_focus_here();
                }
                ui.input_text("title", &mut text)
                    .hint("type here")
                    .callback_flags(imgui::InputTextCallback::EDIT)
                    .callback(Append {
                        seen: &mut seen,
                        ui,
                    })
                    .build();
            });
        let _ = ctx.render_legacy();
    }
    assert_eq!(seen, input);
    assert_eq!(text, format!("{input} — done"));
}

#[test]
fn text_callback_panic_resumes_after_the_native_edit_is_written_back() {
    #[derive(Debug, PartialEq, Eq)]
    struct CallbackPanic;

    struct EditThenPanic;

    impl imgui::InputTextCallbackHandler for EditThenPanic {
        fn on_edit(&mut self, mut data: imgui::TextCallbackData<'_>) {
            data.push_str(" — après");
            panic_any(CallbackPanic);
        }
    }

    let _guard = test_guard();
    for multiline in [false, true] {
        let mut ctx = input_context();
        let mut text = String::new();
        for frame in 0..3 {
            if frame == 2 {
                ctx.io_mut().add_input_characters_utf8("café");
            }
            let ui = ctx.frame();
            ui.window("panic callback")
                .size([400.0, 180.0], imgui::Condition::Always)
                .build(|| {
                    if frame == 0 {
                        ui.set_keyboard_focus_here();
                    }
                    let result = catch_unwind(AssertUnwindSafe(|| {
                        if multiline {
                            ui.input_text_multiline("input", &mut text, [300.0, 80.0])
                                .callback(imgui::InputTextCallback::EDIT, EditThenPanic)
                                .build()
                        } else {
                            ui.input_text("input", &mut text)
                                .hint("type here")
                                .callback_flags(imgui::InputTextCallback::EDIT)
                                .callback(EditThenPanic)
                                .build()
                        }
                    }));
                    if frame == 2 {
                        let payload = result.expect_err("expected the callback to panic");
                        assert_eq!(
                            payload.downcast_ref::<CallbackPanic>(),
                            Some(&CallbackPanic)
                        );
                        assert_eq!(text, "café — après");
                    } else {
                        assert!(result.is_ok());
                    }
                });
            let _ = ctx.render_legacy();
        }
    }
}

#[test]
fn input_text_growable_buffer_no_panic() {
    let _guard = test_guard();
    let mut ctx = input_context();

    // Long initial string to exercise our owned buffer path
    let mut text = String::from_iter(std::iter::repeat_n('a', 2048));

    let ui = ctx.frame();
    let _ = ui.input_text("LongText", &mut text).build();
}

#[test]
fn input_text_multiline_growable_buffer_no_panic() {
    let _guard = test_guard();
    let mut ctx = input_context();

    let mut text = String::from_iter(std::iter::repeat_n('b', 8192));

    let ui = ctx.frame();
    let _ = ui
        .input_text_multiline("LongMultiline", &mut text, [300.0, 120.0])
        .build();
}

#[test]
fn input_scalars_build_no_panic() {
    let _guard = test_guard();
    let mut ctx = input_context();
    let ui = ctx.frame();

    let mut i = 42i32;
    let mut f = std::f32::consts::PI;
    let mut d = std::f64::consts::E;
    let mut i2 = [1i32, 2];
    let mut i3 = [3i32, 4, 5];
    let mut f2 = [1.0f32, 2.0];
    let mut f3 = [3.0f32, 4.0, 5.0];

    let _ = ui.input_int("int", &mut i);
    let _ = ui.input_float("float", &mut f);
    let _ = ui.input_double("double", &mut d);
    let _ = ui.input_int2("int2", &mut i2).build();
    let _ = ui.input_int3("int3", &mut i3).build();
    let _ = ui.input_float2("float2", &mut f2).build();
    let _ = ui.input_float3("float3", &mut f3).build();
}
