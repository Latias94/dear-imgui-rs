use dear_imgui_reflect as reflect;
use dear_imgui_reflect::imgui::{Condition, Context, Key, MouseButton, Ui, WindowFlags};
use reflect::{ImGuiReflect, ImGuiValue, ReflectEvent, ReflectResponse};
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap};

mod common;

use common::test_guard;

fn new_test_ctx() -> Context {
    let mut ctx = Context::create();
    {
        let io = ctx.io_mut();
        io.set_display_size([800.0, 600.0]);
        io.set_delta_time(1.0 / 60.0);
        // Mimic typical platform backend flags so mouse interactions behave
        // consistently in headless tests.
        let mut backend_flags = io.backend_flags();
        backend_flags.insert(
            dear_imgui_reflect::imgui::BackendFlags::HAS_MOUSE_CURSORS
                | dear_imgui_reflect::imgui::BackendFlags::HAS_SET_MOUSE_POS,
        );
        io.set_backend_flags(backend_flags);
        // Disable input-event trickling so our synthetic events are applied in
        // the same frame they are queued.
        io.set_config_input_trickle_event_queue(false);
        // Ensure the context starts focused; otherwise some ImGui versions
        // may ignore the first mouse interactions in headless tests.
        io.add_focus_event(true);
        io.add_mouse_pos_event([0.0, 0.0]);
    }
    ctx.font_atlas()
        .try_claim_legacy_renderer()
        .expect("headless test requires the legacy font-atlas capability")
        .build();
    let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);
    ctx
}

fn rect_center(min: [f32; 2], max: [f32; 2]) -> [f32; 2] {
    [(min[0] + max[0]) * 0.5, (min[1] + max[1]) * 0.5]
}

fn queue_mouse_left(ctx: &mut Context, pos: [f32; 2], down: bool) {
    let io = ctx.io_mut();
    io.set_delta_time(1.0 / 60.0);
    io.add_mouse_pos_event(pos);
    io.add_mouse_button_event(MouseButton::Left, down);
}

fn interaction_frame<R>(
    ctx: &mut Context,
    session: &reflect::ReflectSession,
    draw: impl FnOnce(&mut reflect::Inspector<'_, '_>) -> R,
) -> R {
    let ui = ctx.frame();
    let mut inspector = session.inspector(ui);
    let result = ui
        .window("Reflect interactions")
        .flags(WindowFlags::NO_MOVE | WindowFlags::NO_RESIZE | WindowFlags::NO_COLLAPSE)
        .position([0.0, 0.0], Condition::Always)
        .size([640.0, 500.0], Condition::Always)
        .focused(true)
        .build(|| draw(&mut inspector))
        .expect("test window is visible");
    drop(ctx.render_legacy());
    result
}

fn item_center(ui: &Ui) -> [f32; 2] {
    rect_center(ui.item_rect_min(), ui.item_rect_max())
}

fn click_item<R>(
    ctx: &mut Context,
    center: [f32; 2],
    mut draw: impl FnMut(&mut Context) -> R,
) -> R {
    for down in [false, true] {
        queue_mouse_left(ctx, center, down);
        draw(ctx);
    }
    queue_mouse_left(ctx, center, false);
    draw(ctx)
}

#[test]
fn imgui_small_button_click_can_be_simulated() {
    let _guard = test_guard();
    let mut ctx = new_test_ctx();

    let button_center = {
        let ui = ctx.frame();
        let mut min = [0.0, 0.0];
        let mut max = [0.0, 0.0];
        ui.window("ClickProbe")
            .flags(WindowFlags::NO_MOVE | WindowFlags::NO_RESIZE | WindowFlags::NO_COLLAPSE)
            .position([0.0, 0.0], Condition::Always)
            .size([240.0, 120.0], Condition::Always)
            .focused(true)
            .build(|| {
                let _ = ui.small_button("Probe");
                min = ui.item_rect_min();
                max = ui.item_rect_max();
            });
        rect_center(min, max)
    };
    drop(ctx.render_legacy());

    // Warm-up frame: let the window take focus before interacting with items.
    queue_mouse_left(&mut ctx, button_center, false);
    {
        let ui = ctx.frame();
        ui.window("ClickProbe")
            .flags(WindowFlags::NO_MOVE | WindowFlags::NO_RESIZE | WindowFlags::NO_COLLAPSE)
            .position([0.0, 0.0], Condition::Always)
            .size([240.0, 120.0], Condition::Always)
            .focused(true)
            .build(|| {
                let _ = ui.small_button("Probe");
            });
    }
    drop(ctx.render_legacy());

    // Press frame.
    queue_mouse_left(&mut ctx, button_center, true);
    {
        let ui = ctx.frame();
        let mut pressed = false;
        let mut hovered = false;
        ui.window("ClickProbe")
            .flags(WindowFlags::NO_MOVE | WindowFlags::NO_RESIZE | WindowFlags::NO_COLLAPSE)
            .position([0.0, 0.0], Condition::Always)
            .size([240.0, 120.0], Condition::Always)
            .focused(true)
            .build(|| {
                pressed = ui.small_button("Probe");
                hovered = ui.is_item_hovered();
            });
        assert!(!pressed, "unexpected press on mouse-down frame");
        assert!(hovered);
        assert!(ui.io().mouse_down(MouseButton::Left));
    }
    drop(ctx.render_legacy());

    // Release frame: should click.
    queue_mouse_left(&mut ctx, button_center, false);
    {
        let ui = ctx.frame();
        let mut pressed = false;
        let mut hovered = false;
        ui.window("ClickProbe")
            .flags(WindowFlags::NO_MOVE | WindowFlags::NO_RESIZE | WindowFlags::NO_COLLAPSE)
            .position([0.0, 0.0], Condition::Always)
            .size([240.0, 120.0], Condition::Always)
            .focused(true)
            .build(|| {
                pressed = ui.small_button("Probe");
                hovered = ui.is_item_hovered();
            });
        assert!(hovered);
        assert!(!ui.io().mouse_down(MouseButton::Left));
        assert!(pressed, "expected click on mouse-up frame");
    }
    drop(ctx.render_legacy());
}

struct VecHarness<T> {
    items: Vec<T>,
    settings: reflect::VecSettings,
    last_item_min: [f32; 2],
    last_item_max: [f32; 2],
    last_item_hovered: bool,
    last_item_active: bool,
    last_item_clicked: bool,
}

impl<T> VecHarness<T> {
    fn new(items: Vec<T>, settings: reflect::VecSettings) -> Self {
        Self {
            items,
            settings,
            last_item_min: [0.0, 0.0],
            last_item_max: [0.0, 0.0],
            last_item_hovered: false,
            last_item_active: false,
            last_item_clicked: false,
        }
    }

    fn draw(&mut self, inspector: &mut reflect::Inspector<'_, '_>) -> bool
    where
        T: ImGuiValue + Default,
    {
        let ui = inspector.ui();
        let mut changed = false;
        ui.window("VecHarness")
            .flags(WindowFlags::NO_MOVE | WindowFlags::NO_RESIZE | WindowFlags::NO_COLLAPSE)
            .position([0.0, 0.0], Condition::Always)
            .size([400.0, 300.0], Condition::Always)
            .focused(true)
            .build(|| {
                let path = inspector.push_path_static("items");
                changed = reflect::imgui_vec_with_settings(
                    inspector,
                    "items",
                    &mut self.items,
                    &self.settings,
                );
                drop(path);
                self.last_item_min = ui.item_rect_min();
                self.last_item_max = ui.item_rect_max();
                self.last_item_hovered = ui.is_item_hovered();
                self.last_item_active = ui.is_item_active();
                self.last_item_clicked = ui.is_item_clicked();
            });
        changed
    }
}

impl<T> ImGuiReflect for VecHarness<T>
where
    T: ImGuiValue + Default,
{
    fn imgui_reflect(&mut self, inspector: &mut reflect::Inspector<'_, '_>, _label: &str) -> bool {
        self.draw(inspector)
    }
}

#[derive(Default)]
struct NoWidget;

impl ImGuiValue for NoWidget {
    fn imgui_value(
        _inspector: &mut reflect::Inspector<'_, '_>,
        _label: &str,
        _value: &mut Self,
    ) -> bool {
        false
    }
}

#[test]
fn vec_add_button_emits_event_on_click() {
    let _guard = test_guard();
    let mut ctx = new_test_ctx();
    let session = reflect::ReflectSession::new();

    let mut harness = VecHarness::new(
        Vec::<i32>::new(),
        reflect::VecSettings {
            insertable: true,
            removable: false,
            reorderable: false,
            dropdown: false,
        },
    );

    // Frame 1: render once to get the "+" button rectangle.
    let add_center = {
        let ui = ctx.frame();
        let mut inspector = session.inspector(ui);
        let _ = harness.draw(&mut inspector);
        rect_center(harness.last_item_min, harness.last_item_max)
    };
    drop(ctx.render_legacy());

    // Warm-up frame (see `imgui_small_button_click_can_be_simulated`).
    queue_mouse_left(&mut ctx, add_center, false);
    {
        let ui = ctx.frame();
        let mut inspector = session.inspector(ui);
        let _ = harness.draw(&mut inspector);
    }
    drop(ctx.render_legacy());

    // Frame 2: press the "+" button (no action yet).
    queue_mouse_left(&mut ctx, add_center, true);
    {
        let ui = ctx.frame();
        let mut inspector = session.inspector(ui);
        let changed = inspector.input("Root", &mut harness);
        let resp = inspector.into_response();
        assert!(!changed);
        assert!(resp.is_empty());
        assert!(harness.items.is_empty());
    }
    drop(ctx.render_legacy());

    // Frame 3: release the button (triggers insertion and response event).
    queue_mouse_left(&mut ctx, add_center, false);
    {
        let ui = ctx.frame();
        let mut inspector = session.inspector(ui);
        let changed = inspector.input("Root", &mut harness);
        let resp = inspector.into_response();
        assert!(
            changed || !resp.is_empty(),
            "expected click to change value; hovered={}, active={}, clicked={}, rect={:?}-{:?}, mouse={:?}",
            harness.last_item_hovered,
            harness.last_item_active,
            harness.last_item_clicked,
            harness.last_item_min,
            harness.last_item_max,
            ctx.io().mouse_pos()
        );
        assert_eq!(harness.items.len(), 1);

        let events = resp.events();
        assert_eq!(events.len(), 1);
        match &events[0] {
            reflect::ReflectEvent::VecInserted { path, index } => {
                assert_eq!(path.as_deref(), Some("items"));
                assert_eq!(*index, 0);
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }
    drop(ctx.render_legacy());
}

#[test]
fn vec_remove_button_emits_event_on_click() {
    let _guard = test_guard();
    let mut ctx = new_test_ctx();
    let session = reflect::ReflectSession::new();

    let mut harness = VecHarness::new(
        vec![NoWidget],
        reflect::VecSettings {
            insertable: false,
            removable: true,
            reorderable: false,
            dropdown: false,
        },
    );

    // Frame 1: render once to get the "-" button rectangle.
    let remove_center = {
        let ui = ctx.frame();
        let mut inspector = session.inspector(ui);
        let _ = harness.draw(&mut inspector);
        rect_center(harness.last_item_min, harness.last_item_max)
    };
    drop(ctx.render_legacy());

    // Warm-up frame (see `imgui_small_button_click_can_be_simulated`).
    queue_mouse_left(&mut ctx, remove_center, false);
    {
        let ui = ctx.frame();
        let mut inspector = session.inspector(ui);
        let _ = harness.draw(&mut inspector);
    }
    drop(ctx.render_legacy());

    // Frame 2: press the "-" button (no action yet).
    queue_mouse_left(&mut ctx, remove_center, true);
    {
        let ui = ctx.frame();
        let mut inspector = session.inspector(ui);
        let changed = inspector.input("Root", &mut harness);
        let resp = inspector.into_response();
        assert!(!changed);
        assert!(resp.is_empty());
        assert_eq!(harness.items.len(), 1);
    }
    drop(ctx.render_legacy());

    // Frame 3: release the button (triggers removal and response event).
    queue_mouse_left(&mut ctx, remove_center, false);
    {
        let ui = ctx.frame();
        let mut inspector = session.inspector(ui);
        let changed = inspector.input("Root", &mut harness);
        let resp = inspector.into_response();
        assert!(
            changed || !resp.is_empty(),
            "expected click to change value; hovered={}, active={}, clicked={}, rect={:?}-{:?}, mouse={:?}",
            harness.last_item_hovered,
            harness.last_item_active,
            harness.last_item_clicked,
            harness.last_item_min,
            harness.last_item_max,
            ctx.io().mouse_pos()
        );
        assert!(harness.items.is_empty());

        let events = resp.events();
        assert_eq!(events.len(), 1);
        match &events[0] {
            reflect::ReflectEvent::VecRemoved { path, index } => {
                assert_eq!(path.as_deref(), Some("items"));
                assert_eq!(*index, 0);
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }
    drop(ctx.render_legacy());
}

#[test]
fn bool_buttons_use_field_labels_and_keep_their_id_after_toggling() {
    #[derive(ImGuiReflect)]
    struct Explicit {
        #[imgui(
            name = "Power###custom",
            bool_style = "button",
            true_text = "Enabled",
            false_text = "Disabled"
        )]
        switch: bool,
    }

    #[derive(ImGuiReflect)]
    struct Configured {
        #[imgui(name = "Lights")]
        switch: bool,
    }

    fn assert_toggle(
        ctx: &mut Context,
        session: &reflect::ReflectSession,
        value: &mut impl ImGuiReflect,
        labels: [&str; 2],
    ) {
        let padding = ctx.style().frame_padding()[0] * 2.0;
        let mut draw = |ctx: &mut Context| {
            interaction_frame(ctx, session, |inspector| {
                let ui = inspector.ui();
                ui.set_next_item_open(true);
                let changed = inspector.input("Toggles", value);
                let width = ui.item_rect_max()[0] - ui.item_rect_min()[0];
                let label_widths = labels.map(|label| ui.calc_text_size(label)[0] + padding);
                (changed, ui.item_id(), item_center(ui), width, label_widths)
            })
        };
        let (_, original_id, center, width, expected) = draw(ctx);
        assert!((width - expected[0]).abs() < 0.1);
        assert!(click_item(ctx, center, &mut draw).0);
        let (changed, toggled_id, _, width, expected) = draw(ctx);
        assert!(!changed);
        assert_eq!(original_id, toggled_id);
        assert!((width - expected[1]).abs() < 0.1);
    }

    let _guard = test_guard();
    let mut ctx = new_test_ctx();
    let mut session = reflect::ReflectSession::new();
    session.settings_mut().bools_mut().style = reflect::BoolStyle::Button;
    let mut explicit = Explicit { switch: false };
    assert_toggle(
        &mut ctx,
        &session,
        &mut explicit,
        ["Power: Disabled", "Power: Enabled"],
    );
    assert!(explicit.switch);

    let mut configured = Configured { switch: false };
    assert_toggle(
        &mut ctx,
        &session,
        &mut configured,
        ["Lights: Off", "Lights: On"],
    );
    assert!(configured.switch);
}

#[test]
fn bool_radio_choices_with_matching_labels_edit_only_their_field() {
    #[derive(Default, ImGuiReflect)]
    struct Radios {
        #[imgui(
            name = "Mode",
            bool_style = "radio",
            true_text = "True",
            false_text = "True"
        )]
        explicit: bool,
        #[imgui(name = "Mode")]
        configured: bool,
    }

    let _guard = test_guard();
    let mut ctx = new_test_ctx();
    let mut session = reflect::ReflectSession::new();
    session.settings_mut().bools_mut().style = reflect::BoolStyle::Radio;
    let mut radios = Radios::default();
    let mut draw = |ctx: &mut Context| {
        interaction_frame(ctx, &session, |inspector| {
            let ui = inspector.ui();
            ui.set_next_item_open(true);
            let changed = inspector.input("Radios", &mut radios);
            let row_step = ui.frame_height_with_spacing();
            let x = ui.item_rect_min()[0] + 5.0;
            let y = item_center(ui)[1];
            let explicit_true = [x, y - 3.0 * row_step - ui.text_line_height_with_spacing()];
            (changed, explicit_true, [x, y - row_step])
        })
    };
    let (_, explicit_true, configured_true) = draw(&mut ctx);
    assert!(click_item(&mut ctx, explicit_true, &mut draw).0);
    assert!(!click_item(&mut ctx, explicit_true, &mut draw).0);
    assert!(click_item(&mut ctx, configured_true, &mut draw).0);
    assert!(radios.explicit && radios.configured);
}

#[test]
fn radio_enum_reselection_keeps_payload_and_duplicate_captions_are_independent() {
    #[derive(ImGuiReflect)]
    #[imgui(enum_style = "radio")]
    enum Mode {
        #[imgui(name = "Same caption")]
        Empty,
        #[imgui(name = "Same caption")]
        Payload(NoWidget, #[imgui(skip)] usize),
    }

    let _guard = test_guard();
    let mut ctx = new_test_ctx();
    let session = reflect::ReflectSession::new();
    let modes = RefCell::new([Mode::Payload(NoWidget, 37), Mode::Empty]);
    let mut draw = |ctx: &mut Context| {
        interaction_frame(ctx, &session, |inspector| {
            let ui = inspector.ui();
            let mut modes = modes.borrow_mut();
            ["Left", "Right"].map(|label| {
                let mode = if label == "Left" { 0 } else { 1 };
                let changed = inspector.input(label, &mut modes[mode]);
                (changed, item_center(ui), ui.item_id())
            })
        })
    };
    let [(_, left, left_id), (_, right, right_id)] = draw(&mut ctx);
    assert_ne!(left_id, right_id);
    assert!(!click_item(&mut ctx, left, &mut draw)[0].0);
    assert!(matches!(modes.borrow()[0], Mode::Payload(_, 37)));
    let updates = click_item(&mut ctx, right, &mut draw);
    assert!(!updates[0].0 && updates[1].0);
    assert!(matches!(modes.borrow()[1], Mode::Payload(_, 0)));
    assert!(matches!(modes.borrow()[0], Mode::Payload(_, 37)));
}

#[derive(Default)]
struct DragRow {
    number: usize,
    handle: [f32; 2],
    editor: [f32; 2],
}

impl ImGuiValue for DragRow {
    fn imgui_value(
        inspector: &mut reflect::Inspector<'_, '_>,
        label: &str,
        row: &mut Self,
    ) -> bool {
        let ui = inspector.ui();
        row.handle = item_center(ui);
        ui.small_button(label);
        row.editor = item_center(ui);
        false
    }
}

#[test]
fn vec_drop_moves_to_the_target_index_only_in_its_own_container() {
    let _guard = test_guard();
    let mut ctx = new_test_ctx();
    let session = reflect::ReflectSession::new();
    let settings = reflect::VecSettings::reorder_only();
    let mut vectors = [[10, 20], [40, 50]].map(|numbers| {
        numbers
            .map(|number| DragRow {
                number,
                ..DragRow::default()
            })
            .into_iter()
            .collect::<Vec<_>>()
    });
    let draw = |ctx: &mut Context, vectors: &mut [Vec<DragRow>; 2], open: bool| {
        interaction_frame(ctx, &session, |inspector| {
            let ui = inspector.ui();
            let mut changed = false;
            for (name, vector) in ["left", "right"].into_iter().zip(vectors) {
                let _scope = ui.push_id(name);
                let _path = inspector.push_path_static(name);
                if open {
                    ui.set_next_item_open(true);
                }
                changed |= reflect::imgui_vec_with_settings(inspector, "Items", vector, &settings);
            }
            (
                changed,
                std::mem::take(inspector.response_mut()),
                ui.drag_drop_payload().is_some(),
            )
        })
    };
    draw(&mut ctx, &mut vectors, true);
    for (vector, number) in vectors.iter_mut().zip([30, 60]) {
        vector.push(DragRow {
            number,
            ..DragRow::default()
        });
    }
    draw(&mut ctx, &mut vectors, false);
    assert!(vectors.iter().all(|vector| vector[2].handle[1] > 0.0));

    let drag =
        |ctx: &mut Context, vectors: &mut [Vec<DragRow>; 2], from: [f32; 2], to: [f32; 2]| {
            for (pos, down) in [(from, false), (from, true), (to, true)] {
                queue_mouse_left(ctx, pos, down);
                assert!(!draw(ctx, vectors, false).0);
            }
            assert!(draw(ctx, vectors, false).2, "drag source did not start");
            queue_mouse_left(ctx, to, false);
            draw(ctx, vectors, false)
        };
    let source = vectors[0][0].handle;
    let other_target = vectors[1][1].editor;
    let (changed, events, _) = drag(&mut ctx, &mut vectors, source, other_target);
    assert!(!changed && events.is_empty());

    let next_handle = vectors[0][1].handle;
    let (changed, events, _) = drag(&mut ctx, &mut vectors, source, next_handle);
    assert!(changed);
    assert!(matches!(
        events.events(),
        [ReflectEvent::VecReordered { path, from: 0, to: 1 }] if path.as_deref() == Some("left")
    ));
    assert_eq!(vectors[0][1].number, 10);

    draw(&mut ctx, &mut vectors, false);
    let last_handle = vectors[0][2].handle;
    let first_editor = vectors[0][0].editor;
    let (changed, events, _) = drag(&mut ctx, &mut vectors, last_handle, first_editor);
    assert!(changed);
    assert!(matches!(
        events.events(),
        [ReflectEvent::VecReordered { path, from: 2, to: 0 }] if path.as_deref() == Some("left")
    ));
    let numbers =
        vectors.map(|vector| vector.into_iter().map(|row| row.number).collect::<Vec<_>>());
    assert_eq!(numbers, [vec![30, 20, 10], vec![40, 50, 60]]);
}

#[derive(Clone, Default)]
struct MapRow {
    number: usize,
    key_center: [f32; 2],
    key_id: u32,
}

impl ImGuiValue for MapRow {
    fn imgui_value(
        inspector: &mut reflect::Inspector<'_, '_>,
        _label: &str,
        row: &mut Self,
    ) -> bool {
        let ui = inspector.ui();
        row.key_center = item_center(ui);
        row.key_id = ui.item_id().raw();
        ui.text(row.number.to_string());
        false
    }
}

type MapFrame = (bool, ReflectResponse, [f32; 2]);

fn unchanged_map(frame: MapFrame) -> [f32; 2] {
    let (changed, events, outside) = frame;
    assert!(!changed && events.is_empty());
    outside
}

fn type_map_key(
    ctx: &mut Context,
    center: [f32; 2],
    replacement: &str,
    draw: &mut impl FnMut(&mut Context) -> MapFrame,
) {
    unchanged_map(click_item(ctx, center, &mut *draw));
    let io = ctx.io_mut();
    io.set_config_macosx_behaviors(false);
    io.add_key_event(Key::ModCtrl, true);
    io.add_key_event(Key::A, true);
    unchanged_map(draw(ctx));

    let io = ctx.io_mut();
    io.add_key_event(Key::A, false);
    io.add_key_event(Key::ModCtrl, false);
    if replacement.is_empty() {
        io.add_key_event(Key::Backspace, true);
    } else {
        io.add_input_characters_utf8(replacement);
    }
    unchanged_map(draw(ctx));
    ctx.io_mut().add_key_event(Key::Backspace, false);
}

fn map_edit_key(
    ctx: &mut Context,
    key: Key,
    draw: &mut impl FnMut(&mut Context) -> MapFrame,
) -> MapFrame {
    ctx.io_mut().add_key_event(key, true);
    let frame = draw(ctx);
    ctx.io_mut().add_key_event(key, false);
    frame
}

fn assert_rename(frame: MapFrame, old_key: &str, new_key: &str) {
    assert!(frame.0);
    assert!(matches!(
        frame.1.events(),
        [ReflectEvent::MapRenamed { path, from, to }]
            if path.as_deref() == Some("map") && from == old_key && to == new_key
    ));
}

#[test]
fn table_map_key_drafts_commit_on_enter_or_blur_and_cancel_without_events() {
    let _guard = test_guard();
    let mut ctx = new_test_ctx();
    let session = reflect::ReflectSession::new();
    let settings = reflect::MapSettings {
        use_table: true,
        ..reflect::MapSettings::editable()
    };
    let map = RefCell::new(BTreeMap::from([
        (
            "source".to_owned(),
            MapRow {
                number: 37,
                ..MapRow::default()
            },
        ),
        ("target".to_owned(), MapRow::default()),
    ]));
    let open = Cell::new(true);
    let visible = Cell::new(true);
    let mut draw = |ctx: &mut Context| {
        interaction_frame(ctx, &session, |inspector| {
            let ui = inspector.ui();
            let _path = inspector.push_path_static("map");
            let changed = if visible.get() {
                if open.replace(false) {
                    ui.set_next_item_open(true);
                }
                reflect::imgui_btree_map_with_settings(
                    inspector,
                    "Entries",
                    &mut map.borrow_mut(),
                    &settings,
                )
            } else {
                false
            };
            ui.button("Outside editor");
            (
                changed,
                std::mem::take(inspector.response_mut()),
                item_center(ui),
            )
        })
    };
    unchanged_map(draw(&mut ctx));
    let source = map.borrow()["source"].clone();
    assert_ne!(source.key_id, 0);
    type_map_key(&mut ctx, source.key_center, "entered", &mut draw);
    assert!(!map.borrow().contains_key("entered"));

    map.borrow_mut()
        .insert("earlier".to_owned(), MapRow::default());
    unchanged_map(draw(&mut ctx));
    assert_eq!(map.borrow()["source"].key_id, source.key_id);
    assert_rename(
        map_edit_key(&mut ctx, Key::Enter, &mut draw),
        "source",
        "entered",
    );
    let outside = unchanged_map(draw(&mut ctx));
    assert_eq!(map.borrow()["entered"].number, 37);

    for (candidate, action) in [
        ("target", Key::Enter),
        ("", Key::Enter),
        ("canceled", Key::Escape),
    ] {
        let center = map.borrow()["entered"].key_center;
        type_map_key(&mut ctx, center, candidate, &mut draw);
        unchanged_map(map_edit_key(&mut ctx, action, &mut draw));
        unchanged_map(click_item(&mut ctx, outside, &mut draw));
        assert_eq!(map.borrow()["entered"].number, 37);
        assert_eq!(map.borrow().len(), 3);
    }

    let center = map.borrow()["entered"].key_center;
    type_map_key(&mut ctx, center, "interrupted", &mut draw);
    visible.set(false);
    unchanged_map(draw(&mut ctx));
    visible.set(true);
    unchanged_map(draw(&mut ctx));
    assert!(!map.borrow().contains_key("interrupted"));

    let center = map.borrow()["entered"].key_center;
    type_map_key(&mut ctx, center, "blurred", &mut draw);
    queue_mouse_left(&mut ctx, outside, true);
    assert_rename(draw(&mut ctx), "entered", "blurred");
    assert_eq!(map.borrow()["blurred"].number, 37);
}

#[test]
fn inline_hash_map_distinguishes_keys_with_imgui_delimiters_and_nul() {
    let _guard = test_guard();
    let mut ctx = new_test_ctx();
    let session = reflect::ReflectSession::new();
    let settings = reflect::MapSettings {
        dropdown: false,
        ..reflect::MapSettings::editable()
    };
    let keys = ["foo###bar", "bar", "nul\0suffix", "nul", "foo%23%23%23bar"];
    let map = RefCell::new(HashMap::from(
        keys.map(|key| (key.to_owned(), MapRow::default())),
    ));
    let mut draw = |ctx: &mut Context| {
        interaction_frame(ctx, &session, |inspector| {
            let _path = inspector.push_path_static("map");
            let changed = reflect::imgui_hash_map_with_settings(
                inspector,
                "Map",
                &mut map.borrow_mut(),
                &settings,
            );
            (changed, std::mem::take(inspector.response_mut()), [0.0; 2])
        })
    };
    unchanged_map(draw(&mut ctx));
    let mut ids = keys.map(|key| map.borrow()[key].key_id);
    ids.sort_unstable();
    assert_ne!(ids[0], 0);
    assert!(ids.windows(2).all(|pair| pair[0] != pair[1]));

    let center = map.borrow()["foo###bar"].key_center;
    type_map_key(&mut ctx, center, "renamed", &mut draw);
    assert_rename(
        map_edit_key(&mut ctx, Key::Enter, &mut draw),
        "foo###bar",
        "renamed",
    );
    assert!(map.borrow().contains_key("renamed") && map.borrow().contains_key("bar"));
}
