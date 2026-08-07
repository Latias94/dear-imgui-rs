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

#[derive(Default)]
struct TuplePanickingValue;

impl reflect::ImGuiValue for TuplePanickingValue {
    fn imgui_value(
        inspector: &mut reflect::Inspector<'_, '_>,
        _label: &str,
        _value: &mut Self,
    ) -> bool {
        assert_eq!(inspector.current_path().as_deref(), Some("Tuple.0"));
        panic!("tuple enum unwind probe");
    }
}

#[derive(reflect::ImGuiReflect)]
enum PanicTupleOwner {
    Tuple(TuplePanickingValue),
}

#[derive(Default)]
struct NamedPanickingValue;

impl reflect::ImGuiValue for NamedPanickingValue {
    fn imgui_value(
        inspector: &mut reflect::Inspector<'_, '_>,
        _label: &str,
        _value: &mut Self,
    ) -> bool {
        assert_eq!(inspector.current_path().as_deref(), Some("Named.value"));
        panic!("named enum unwind probe");
    }
}

#[derive(reflect::ImGuiReflect)]
enum PanicNamedOwner {
    Named { value: NamedPanickingValue },
}

#[derive(Default)]
struct OptionalPanickingValue;

impl reflect::ImGuiValue for OptionalPanickingValue {
    fn imgui_value(
        inspector: &mut reflect::Inspector<'_, '_>,
        _label: &str,
        _value: &mut Self,
    ) -> bool {
        assert_eq!(inspector.current_path().as_deref(), Some("value"));
        panic!("optional value unwind probe");
    }
}

#[derive(reflect::ImGuiReflect)]
struct PanicOptionalOwner {
    value: Option<OptionalPanickingValue>,
}

fn test_context() -> Context {
    let mut context = Context::create();
    {
        let io = context.io_mut();
        io.set_display_size([640.0, 480.0]);
        io.set_delta_time(1.0 / 60.0);
    }
    context
        .font_atlas()
        .try_claim_legacy_renderer()
        .expect("headless test requires the legacy font-atlas capability")
        .build();
    let _ = context.set_ini_filename::<std::path::PathBuf>(None);
    context
}

fn assert_indent_restored_after_panic(ui: &reflect::imgui::Ui, render: impl FnOnce()) {
    let cursor_x = ui.cursor_pos_x();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(render));

    assert!(result.is_err());
    assert_eq!(ui.cursor_pos_x(), cursor_x);
}

#[test]
fn inspector_path_restores_on_panic() {
    let _guard = test_guard();
    let mut context = test_context();
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
    let mut context = test_context();
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

#[test]
fn derive_tuple_enum_restores_indent_on_panic() {
    let _guard = test_guard();
    let mut context = test_context();
    let ui = context.frame();
    let session = reflect::ReflectSession::new();
    let mut inspector = session.inspector(ui);
    let mut owner = PanicTupleOwner::Tuple(TuplePanickingValue);

    ui.window("tuple enum panic safety").build(|| {
        assert_indent_restored_after_panic(ui, || {
            inspector.input("Owner", &mut owner);
        });
    });

    assert!(inspector.current_path().is_none());
}

#[test]
fn derive_named_enum_restores_indent_on_panic() {
    let _guard = test_guard();
    let mut context = test_context();
    let ui = context.frame();
    let session = reflect::ReflectSession::new();
    let mut inspector = session.inspector(ui);
    let mut owner = PanicNamedOwner::Named {
        value: NamedPanickingValue,
    };

    ui.window("named enum panic safety").build(|| {
        assert_indent_restored_after_panic(ui, || {
            inspector.input("Owner", &mut owner);
        });
    });

    assert!(inspector.current_path().is_none());
}

#[test]
fn optional_value_restores_indent_on_panic() {
    let _guard = test_guard();
    let mut context = test_context();
    let ui = context.frame();
    let session = reflect::ReflectSession::new();
    let mut inspector = session.inspector(ui);
    let mut owner = PanicOptionalOwner {
        value: Some(OptionalPanickingValue),
    };

    ui.window("optional value panic safety").build(|| {
        ui.set_next_item_open(true);
        assert_indent_restored_after_panic(ui, || {
            inspector.input("Owner", &mut owner);
        });
    });

    assert!(inspector.current_path().is_none());
}
