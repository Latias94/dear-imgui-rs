use dear_imgui_reflect as reflect;
use dear_imgui_reflect::imgui::Context;
use reflect::ImGuiReflect;

mod common;

use common::test_guard;

fn make_context() -> Context {
    let mut context = Context::create();
    {
        let io = context.io_mut();
        io.set_display_size([800.0, 600.0]);
        io.set_delta_time(1.0 / 60.0);
    }
    let _ = context.font_atlas().build();
    let _ = context.set_ini_filename::<std::path::PathBuf>(None);
    context
}

#[derive(ImGuiReflect, Default)]
struct UnitStruct;

#[derive(ImGuiReflect, Default)]
struct Newtype(i32);

#[derive(ImGuiReflect, Default)]
struct Pair(i32, bool);

#[derive(ImGuiReflect, Default)]
struct NestedTupleStruct {
    inner: Pair,
    unit: UnitStruct,
    newtype: Newtype,
}

#[derive(ImGuiReflect, Default)]
enum PayloadEnum {
    #[default]
    Unit,
    Tuple(i32, bool),
    Struct {
        #[imgui(name = "Count")]
        a: i32,
        #[imgui(read_only)]
        b: String,
    },
}

#[derive(ImGuiReflect)]
enum HygienePayloadEnum {
    Named {
        inspector: i32,
        ui: bool,
        label: String,
        __changed: f32,
        __imgui_reflect_settings: i32,
    },
}

#[derive(ImGuiReflect, Default)]
#[imgui(enum_style = "radio")]
enum RadioPayloadEnum {
    #[default]
    A,
    B(i32),
}

#[derive(ImGuiReflect, Default)]
enum EmptyNamedVariantEnum {
    #[default]
    A,
    Empty {},
    Skipped {
        #[imgui(skip)]
        _x: i32,
    },
}

#[test]
fn tuple_and_unit_structs_no_panic() {
    let _guard = test_guard();
    let session = reflect::ReflectSession::new();
    let mut ctx = make_context();
    let ui = ctx.frame();
    let mut inspector = session.inspector(ui);

    let mut unit = UnitStruct;
    let _ = inspector.input("UnitStruct", &mut unit);

    let mut newtype = Newtype(42);
    let _ = inspector.input("Newtype", &mut newtype);

    let mut pair = Pair(7, true);
    let _ = inspector.input("Pair", &mut pair);

    let mut nested = NestedTupleStruct {
        inner: Pair(1, false),
        unit: UnitStruct,
        newtype: Newtype(3),
    };
    let _ = inspector.input("NestedTupleStruct", &mut nested);
}

#[test]
fn enum_payloads_no_panic() {
    let _guard = test_guard();
    let mut session = reflect::ReflectSession::new();
    let mut ctx = make_context();
    let ui = ctx.frame();

    {
        let s = session.settings_mut();
        // Per-variant payload settings use the key scheme `Variant.field` / `Variant.0`.
        s.for_member::<PayloadEnum>("Tuple.0").read_only = true;
        s.for_member::<PayloadEnum>("Struct.b").read_only = true;
    }

    let mut inspector = session.inspector(ui);

    let mut value = PayloadEnum::Tuple(123, true);
    let _ = inspector.input("PayloadEnumTuple", &mut value);

    let mut value = PayloadEnum::Struct {
        a: 7,
        b: "hello".to_owned(),
    };
    let _ = inspector.input("PayloadEnumStruct", &mut value);

    let mut value = RadioPayloadEnum::B(9);
    let _ = inspector.input("RadioPayloadEnum", &mut value);

    let mut value = EmptyNamedVariantEnum::Skipped { _x: 1 };
    let _ = inspector.input("EmptyNamedVariantEnum", &mut value);
}

#[test]
fn named_enum_payload_fields_do_not_shadow_derive_internals() {
    let _guard = test_guard();
    let mut context = make_context();
    let ui = context.frame();
    let session = reflect::ReflectSession::new();
    let mut inspector = session.inspector(ui);
    let mut value = HygienePayloadEnum::Named {
        inspector: 1,
        ui: true,
        label: "value".to_owned(),
        __changed: 0.5,
        __imgui_reflect_settings: 2,
    };

    inspector.input("Hygiene payload", &mut value);
}
