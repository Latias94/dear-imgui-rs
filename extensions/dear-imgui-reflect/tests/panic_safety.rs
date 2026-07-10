use dear_imgui_reflect as reflect;
use dear_imgui_reflect::imgui::Context;

mod common;

use common::test_guard;

struct PanickingValue;

impl reflect::ImGuiValue for PanickingValue {
    fn imgui_value(
        inspector: &mut reflect::Inspector<'_, '_>,
        _label: &str,
        _value: &mut Self,
    ) -> bool {
        assert_eq!(inspector.current_path().as_deref(), Some("value"));
        panic!("derived field unwind probe");
    }
}

#[derive(reflect::ImGuiReflect)]
struct PanicOwner {
    value: PanickingValue,
}

#[test]
fn inspector_path_restores_on_panic() {
    let _guard = test_guard();
    let mut context = Context::create();
    {
        let io = context.io_mut();
        io.set_display_size([640.0, 480.0]);
        io.set_delta_time(1.0 / 60.0);
    }
    let _ = context.font_atlas_mut().build();
    let _ = context.set_ini_filename::<std::path::PathBuf>(None);
    let ui = context.frame();
    let session = reflect::ReflectSession::new();
    let inspector = session.inspector(ui);

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _outer = inspector.push_path_static("outer");
        let _inner = inspector.push_path("inner");
        assert_eq!(inspector.current_path().as_deref(), Some("outer.inner"));
        panic!("path unwind probe");
    }));

    assert!(result.is_err());
    assert!(inspector.current_path().is_none());
}

#[test]
fn derive_field_path_restores_on_panic() {
    let _guard = test_guard();
    let mut context = Context::create();
    {
        let io = context.io_mut();
        io.set_display_size([640.0, 480.0]);
        io.set_delta_time(1.0 / 60.0);
    }
    let _ = context.font_atlas_mut().build();
    let _ = context.set_ini_filename::<std::path::PathBuf>(None);
    let ui = context.frame();
    let session = reflect::ReflectSession::new();
    let mut inspector = session.inspector(ui);
    let mut owner = PanicOwner {
        value: PanickingValue,
    };

    ui.set_next_item_open(true);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        inspector.input("Owner", &mut owner);
    }));

    assert!(result.is_err());
    assert!(inspector.current_path().is_none());
}
