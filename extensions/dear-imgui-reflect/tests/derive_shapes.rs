use dear_imgui_reflect as reflect;
use dear_imgui_reflect::imgui::Context;
use reflect::ImGuiReflect;

mod common;

use common::test_guard;

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
    let mut ctx = Context::create();
    {
        let io = ctx.io_mut();
        io.set_display_size([800.0, 600.0]);
        io.set_delta_time(1.0 / 60.0);
    }
    let _ = ctx.font_atlas_mut().build();
    let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);
    let ui = ctx.frame();

    let mut unit = UnitStruct;
    let _ = reflect::input(ui, "UnitStruct", &mut unit);

    let mut newtype = Newtype(42);
    let _ = reflect::input(ui, "Newtype", &mut newtype);

    let mut pair = Pair(7, true);
    let _ = reflect::input(ui, "Pair", &mut pair);

    let mut nested = NestedTupleStruct {
        inner: Pair(1, false),
        unit: UnitStruct,
        newtype: Newtype(3),
    };
    let _ = reflect::input(ui, "NestedTupleStruct", &mut nested);
}

#[test]
fn enum_payloads_no_panic() {
    let _guard = test_guard();
    let mut ctx = Context::create();
    {
        let io = ctx.io_mut();
        io.set_display_size([800.0, 600.0]);
        io.set_delta_time(1.0 / 60.0);
    }
    let _ = ctx.font_atlas_mut().build();
    let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);
    let ui = ctx.frame();

    reflect::with_settings_scope(|| {
        reflect::with_settings(|s| {
            // Per-variant payload settings use the key scheme `Variant.field` / `Variant.0`.
            s.for_member::<PayloadEnum>("Tuple.0").read_only = true;
            s.for_member::<PayloadEnum>("Struct.b").read_only = true;
        });

        let mut value = PayloadEnum::Tuple(123, true);
        let _ = reflect::input(ui, "PayloadEnumTuple", &mut value);

        let mut value = PayloadEnum::Struct {
            a: 7,
            b: "hello".to_owned(),
        };
        let _ = reflect::input(ui, "PayloadEnumStruct", &mut value);

        let mut value = RadioPayloadEnum::B(9);
        let _ = reflect::input(ui, "RadioPayloadEnum", &mut value);

        let mut value = EmptyNamedVariantEnum::Skipped { _x: 1 };
        let _ = reflect::input(ui, "EmptyNamedVariantEnum", &mut value);
    });
}
