use dear_imgui_reflect as reflect;
use dear_imgui_reflect::imgui::{Condition, Context, MouseButton, WindowFlags};
use reflect::{ImGuiReflect, ImGuiValue};

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
    let _ = ctx.font_atlas_mut().build();
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
    ctx.render();

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
    ctx.render();

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
        assert!(ui.io().mouse_down_button(MouseButton::Left));
    }
    ctx.render();

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
        assert!(!ui.io().mouse_down_button(MouseButton::Left));
        assert!(pressed, "expected click on mouse-up frame");
    }
    ctx.render();
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

    fn draw(&mut self, ui: &reflect::imgui::Ui) -> bool
    where
        T: ImGuiValue + Default,
    {
        let mut changed = false;
        ui.window("VecHarness")
            .flags(WindowFlags::NO_MOVE | WindowFlags::NO_RESIZE | WindowFlags::NO_COLLAPSE)
            .position([0.0, 0.0], Condition::Always)
            .size([400.0, 300.0], Condition::Always)
            .focused(true)
            .build(|| {
                changed = reflect::with_field_path("items", || {
                    reflect::imgui_vec_with_settings(ui, "items", &mut self.items, &self.settings)
                });
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
    fn imgui_reflect(&mut self, ui: &reflect::imgui::Ui, _label: &str) -> bool {
        self.draw(ui)
    }
}

#[derive(Default)]
struct NoWidget;

impl ImGuiValue for NoWidget {
    fn imgui_value(_ui: &reflect::imgui::Ui, _label: &str, _value: &mut Self) -> bool {
        false
    }
}

#[test]
fn vec_add_button_emits_event_on_click() {
    let _guard = test_guard();
    let mut ctx = new_test_ctx();

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
        let _ = harness.draw(&ui);
        rect_center(harness.last_item_min, harness.last_item_max)
    };
    ctx.render();

    // Warm-up frame (see `imgui_small_button_click_can_be_simulated`).
    queue_mouse_left(&mut ctx, add_center, false);
    {
        let ui = ctx.frame();
        let _ = harness.draw(&ui);
    }
    ctx.render();

    // Frame 2: press the "+" button (no action yet).
    queue_mouse_left(&mut ctx, add_center, true);
    {
        let ui = ctx.frame();
        let mut resp = reflect::ReflectResponse::default();
        let changed = reflect::input_with_response(&ui, "Root", &mut harness, &mut resp);
        assert!(!changed);
        assert!(resp.is_empty());
        assert!(harness.items.is_empty());
    }
    ctx.render();

    // Frame 3: release the button (triggers insertion and response event).
    queue_mouse_left(&mut ctx, add_center, false);
    {
        let ui = ctx.frame();
        let mut resp = reflect::ReflectResponse::default();
        let changed = reflect::input_with_response(&ui, "Root", &mut harness, &mut resp);
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
    ctx.render();
}

#[test]
fn vec_remove_button_emits_event_on_click() {
    let _guard = test_guard();
    let mut ctx = new_test_ctx();

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
        let _ = harness.draw(&ui);
        rect_center(harness.last_item_min, harness.last_item_max)
    };
    ctx.render();

    // Warm-up frame (see `imgui_small_button_click_can_be_simulated`).
    queue_mouse_left(&mut ctx, remove_center, false);
    {
        let ui = ctx.frame();
        let _ = harness.draw(&ui);
    }
    ctx.render();

    // Frame 2: press the "-" button (no action yet).
    queue_mouse_left(&mut ctx, remove_center, true);
    {
        let ui = ctx.frame();
        let mut resp = reflect::ReflectResponse::default();
        let changed = reflect::input_with_response(&ui, "Root", &mut harness, &mut resp);
        assert!(!changed);
        assert!(resp.is_empty());
        assert_eq!(harness.items.len(), 1);
    }
    ctx.render();

    // Frame 3: release the button (triggers removal and response event).
    queue_mouse_left(&mut ctx, remove_center, false);
    {
        let ui = ctx.frame();
        let mut resp = reflect::ReflectResponse::default();
        let changed = reflect::input_with_response(&ui, "Root", &mut harness, &mut resp);
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
    ctx.render();
}
