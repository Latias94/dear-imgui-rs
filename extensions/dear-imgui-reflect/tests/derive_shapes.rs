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
    context
        .font_atlas()
        .try_claim_legacy_renderer()
        .expect("headless test requires the legacy font-atlas capability")
        .build();
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
fn derived_tuple_attributes_override_inherited_layout() {
    #[derive(Default)]
    struct CellProbe {
        drawn_column: Option<(usize, f32)>,
    }

    impl reflect::ImGuiValue for CellProbe {
        fn imgui_value(
            inspector: &mut reflect::Inspector<'_, '_>,
            _: &str,
            value: &mut Self,
        ) -> bool {
            let ui = inspector.ui();
            value.drawn_column = Some((ui.table_get_column_count(), ui.cursor_screen_pos()[0]));
            ui.dummy([1.0, 1.0]);
            false
        }
    }

    #[derive(ImGuiReflect, Default)]
    struct Layout {
        #[imgui(tuple_render = "grid", tuple_columns = 2, tuple_min_width = 120.0)]
        grid: (CellProbe, CellProbe, CellProbe),
        #[imgui(tuple_dropdown)]
        collapsed: (CellProbe, CellProbe),
    }

    let _guard = test_guard();
    let mut session = reflect::ReflectSession::new();
    session.settings_mut().tuples_mut().same_line = false;
    let mut context = make_context();
    let mut layout = Layout::default();
    let ui = context.frame();
    let mut inspector = session.inspector(ui);
    ui.window("Derived tuple layout")
        .size([700.0, 450.0], reflect::imgui::Condition::Always)
        .build(|| {
            ui.set_next_item_open(true);
            assert!(!inspector.input("Layout", &mut layout));
        });

    let (first_count, first_x) = layout.grid.0.drawn_column.unwrap();
    let (second_count, second_x) = layout.grid.1.drawn_column.unwrap();
    let (third_count, third_x) = layout.grid.2.drawn_column.unwrap();
    assert_eq!([first_count, second_count, third_count], [2; 3]);
    assert_eq!(third_x, first_x);
    assert!((120.0..140.0).contains(&(second_x - first_x)));
    assert!(layout.collapsed.0.drawn_column.is_none());
    assert!(layout.collapsed.1.drawn_column.is_none());
    drop(context.render_legacy());
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

#[test]
fn derives_render_local_borrows_without_static_bounds() {
    #[derive(ImGuiReflect)]
    struct Borrowed<'a, T, const N: usize> {
        title: &'a str,
        items: [T; N],
    }

    #[derive(ImGuiReflect, Default)]
    enum BorrowedPayload<'a> {
        #[default]
        Empty,
        Tuple(&'a str),
        Named {
            note: &'a str,
            #[imgui(skip)]
            _borrow: std::marker::PhantomData<&'a ()>,
        },
    }

    let _guard = test_guard();
    let mut context = make_context();
    let mut session = reflect::ReflectSession::new();
    *session.settings_mut().arrays_mut() = reflect::ArraySettings {
        dropdown: false,
        reorderable: false,
    };
    let text = String::from("frame-owned text");
    let mut data = Borrowed {
        title: &text,
        items: [
            Some(BorrowedPayload::Tuple(&text)),
            Some(BorrowedPayload::Named {
                note: &text,
                _borrow: std::marker::PhantomData,
            }),
        ],
    };

    let ui = context.frame();
    let mut inspector = session.inspector(ui);
    ui.window("Borrowed values").build(|| {
        ui.set_next_item_open(true);
        assert!(!inspector.input("Borrowed", &mut data));
        assert!(inspector.current_path().is_none());
    });
}

#[test]
#[allow(non_upper_case_globals)]
fn explicit_numeric_bounds_run_once_and_can_borrow_the_owner() {
    use std::cell::Cell;

    fn bound(count: &Cell<usize>, value: i32) -> i32 {
        count.set(count.get() + 1);
        value
    }

    #[derive(ImGuiReflect)]
    struct Bounds<'a, const min: i32, const max: i32> {
        #[imgui(skip)]
        min_count: &'a Cell<usize>,
        #[imgui(skip)]
        max_count: &'a Cell<usize>,
        #[imgui(slider, min = bound(self.min_count, min), max = bound(self.max_count, max), clamp, format = "%d")]
        value: i32,
    }

    let _guard = test_guard();
    let min_count = Cell::new(0);
    let max_count = Cell::new(0);
    let mut value = Bounds::<0, 10> {
        min_count: &min_count,
        max_count: &max_count,
        value: 14,
    };
    let mut context = make_context();
    let session = reflect::ReflectSession::new();
    let ui = context.frame();
    let mut inspector = session.inspector(ui);

    ui.window("Numeric bounds").build(|| {
        ui.set_next_item_open(true);
        assert!(inspector.input("Bounds", &mut value));
    });
    assert_eq!(value.value, 10);
    assert_eq!((min_count.get(), max_count.get()), (1, 1));
}

#[test]
fn session_defaults_and_member_overrides_reach_nested_containers() {
    use std::collections::{BTreeMap, HashMap};

    type WideTuple = (i32, i32, i32, i32, i32, i32, i32, i32, i32, i32, i32, i32);

    #[derive(ImGuiReflect)]
    struct Nested {
        list: Vec<Option<[i32; 3]>>,
        ordered: BTreeMap<String, WideTuple>,
        hash: HashMap<String, Option<i32>>,
        tuple: (
            i32,
            i32,
            i32,
            i32,
            i32,
            i32,
            i32,
            i32,
            i32,
            i32,
            i32,
            i32,
            i32,
            i32,
            i32,
            i32,
            i32,
        ),
    }

    fn layout_session() -> reflect::ReflectSession {
        let mut session = reflect::ReflectSession::new();
        let settings = session.settings_mut();
        *settings.vec_mut() = reflect::VecSettings {
            dropdown: false,
            ..reflect::VecSettings::fixed()
        };
        *settings.arrays_mut() = reflect::ArraySettings {
            dropdown: false,
            reorderable: false,
        };
        *settings.maps_mut() = reflect::MapSettings {
            dropdown: false,
            use_table: false,
            ..reflect::MapSettings::const_map()
        };
        settings.tuples_mut().render_mode = reflect::TupleRenderMode::Grid;
        settings.tuples_mut().same_line = false;
        settings.tuples_mut().columns = 6;
        session
    }

    let _guard = test_guard();
    let left = {
        let mut session = layout_session();
        let settings = session.settings_mut();
        *settings.numerics_i32_mut() = reflect::I32NumericSettings {
            widget: reflect::NumericWidgetKind::Slider,
            range: reflect::NumericRange::Explicit {
                min: 0.0,
                max: 10.0,
            },
            clamp: true,
            ..Default::default()
        };
        settings.for_path("list[0][1]").read_only = true;
        settings
            .for_member::<Nested>("tuple[0]")
            .numerics_i32_slider_range(0, 20, true);
        session
    };
    let right = layout_session();
    let mut context = make_context();

    for (session, expected, first_tuple) in [(&right, 30, 30), (&left, 10, 20)] {
        let mut nested = Nested {
            list: vec![Some([30; 3])],
            ordered: BTreeMap::from([(
                "row".to_owned(),
                (30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30),
            )]),
            hash: HashMap::from([("row".to_owned(), Some(30))]),
            tuple: (
                30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30,
            ),
        };
        let ui = context.frame();
        let mut inspector = session.inspector(ui);
        ui.window("Nested values")
            .size([750.0, 550.0], reflect::imgui::Condition::Always)
            .build(|| {
                ui.set_next_item_open(true);
                assert_eq!(inspector.input("Nested", &mut nested), expected == 10);
            });
        assert_eq!(nested.list[0], Some([expected, 30, expected]));
        assert_eq!(nested.ordered["row"].11, expected);
        assert_eq!(nested.hash["row"], Some(expected));
        assert_eq!(nested.tuple.0, first_tuple);
        assert_eq!(nested.tuple.16, expected);
        drop(context.render_legacy());
    }
}
